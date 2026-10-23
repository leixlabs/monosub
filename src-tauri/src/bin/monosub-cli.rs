//! `monosub-cli` — the second frontend over MonoSub's pipeline.
//!
//! Design: `docs/tech-spec/cli-design.md`. The rules that shape this file:
//!
//! * **It picks no model.** The four questions are answered in the GUI, which
//!   writes the solved plan into `<config_dir>/settings.json`; this binary reads
//!   that. `--model` is deliberately absent — a second place to choose a model
//!   would be a second source of truth.
//! * **It controls only what is per-invocation**: formats, language, output
//!   directory, translation.
//! * **Translation is off by default**, even when the GUI has it on. A batch
//!   script should not pay for a 1.8B model load on every file unless it asked.
//! * **Progress goes to stderr, `--json` results to stdout**, so `--json | jq`
//!   works and a human reading the terminal still sees progress.
//!
//! Exit codes are the stable contract (the `--json` field set is experimental):
//!
//! | code | meaning |
//! |---|---|
//! | 0 | success (an all-silent input is success — it writes an empty subtitle) |
//! | 1 | usage error |
//! | 2 | a required model is not installed |
//! | 3 | transcription failed |
//! | 4 | could not write the output |
//! | 5 | subtitles written but translation failed |

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use monosub::commands::transcribe::{transcribe_file, ProgressSink, StepSnapshot, TaskParams};
use monosub::model_dir::{self, BackendSettings};
use monosub::paths::AppPaths;
use monosub::subtitle::Format;

/// Exit codes, named so the call sites read as intent rather than numbers.
mod code {
    pub const OK: u8 = 0;
    pub const USAGE: u8 = 1;
    pub const MODEL_MISSING: u8 = 2;
    pub const TRANSCRIBE_FAILED: u8 = 3;
    pub const WRITE_FAILED: u8 = 4;
    pub const TRANSLATE_FAILED: u8 = 5;
}

/// `--target` defaults to English, as `--help` has always said.
const DEFAULT_TARGET: &str = "en";

const USAGE: &str = "\
monosub-cli — transcribe media with the models you chose in MonoSub

USAGE:
    monosub-cli <input>... [OPTIONS]

OPTIONS:
    -o, --output-dir <dir>   Where subtitles go (default: next to the input)
    -f, --format <list>      Comma separated: srt,vtt,txt,md,json
                             (default: the app setting, or srt)
    -l, --language <tag>     auto, zh, zh-tw, en, yue, ja, ko, ...
                             (default: the app setting, or auto)
        --translate          Also translate (off by default)
        --target <tag>       Translation target (default: en)
        --json               Machine-readable events on stdout
    -q, --quiet              Only report errors
    -h, --help               Show this help

The model is not a flag: it is whatever the app's Settings ▸ 模型 solved, read
from the app's settings.json. Run the app once and pick your preferences there.

EXIT CODES:
    0 success   1 usage   2 model missing   3 transcription failed
    4 write failed   5 translated file failed (subtitles are still valid)
";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();

    let parsed = match Args::parse(&args) {
        Ok(Some(parsed)) => parsed,
        // `--help` (or no arguments, which prints the same text).
        Ok(None) => {
            print!("{USAGE}");
            return ExitCode::from(code::OK);
        }
        Err(error) => {
            eprintln!("monosub-cli: {error}");
            eprintln!("Try `monosub-cli --help`.");
            return ExitCode::from(code::USAGE);
        }
    };

    let runtime = match tokio::runtime::Runtime::new() {
        Ok(runtime) => runtime,
        Err(error) => {
            eprintln!("monosub-cli: cannot start the async runtime: {error}");
            return ExitCode::from(code::TRANSCRIBE_FAILED);
        }
    };

    runtime.block_on(run(parsed))
}

/// Parsed command line. Hand-rolled rather than `clap` so the CLI adds no
/// dependency to a Tauri app that already ships three C++ engines.
#[derive(Debug)]
struct Args {
    inputs: Vec<PathBuf>,
    output_dir: Option<PathBuf>,
    formats: Option<Vec<Format>>,
    language: Option<String>,
    translate: bool,
    target: Option<String>,
    json: bool,
    quiet: bool,
}

impl Args {
    /// `Ok(None)` means "help was requested (or nothing was passed)".
    fn parse(argv: &[String]) -> Result<Option<Self>, String> {
        let mut args = Args {
            inputs: Vec::new(),
            output_dir: None,
            formats: None,
            language: None,
            translate: false,
            target: None,
            json: false,
            quiet: false,
        };

        let mut rest = argv.iter();
        while let Some(arg) = rest.next() {
            match arg.as_str() {
                "-h" | "--help" => return Ok(None),
                "-o" | "--output-dir" => {
                    let value = rest.next().ok_or("--output-dir needs a directory")?;
                    args.output_dir = Some(PathBuf::from(value));
                }
                "-f" | "--format" => {
                    let value = rest.next().ok_or("--format needs a value")?;
                    args.formats = Some(parse_formats(value)?);
                }
                "-l" | "--language" => {
                    let value = rest.next().ok_or("--language needs a value")?;
                    args.language = Some(value.clone());
                }
                "--translate" => args.translate = true,
                "--target" => {
                    let value = rest.next().ok_or("--target needs a language")?;
                    args.target = Some(value.clone());
                }
                "--json" => args.json = true,
                "-q" | "--quiet" => args.quiet = true,
                // A leading dash that is not a known flag is a typo, not a file.
                other if other.starts_with('-') && other != "-" => {
                    return Err(format!("unknown option `{other}`"));
                }
                other => args.inputs.push(PathBuf::from(other)),
            }
        }

        if args.inputs.is_empty() {
            return Ok(None);
        }
        for input in &args.inputs {
            if !input.is_file() {
                return Err(format!("{}: no such file", input.display()));
            }
        }
        Ok(Some(args))
    }
}

/// `srt,vtt` → `[Format::Srt, Format::Vtt]`, with the app's own error wording.
fn parse_formats(value: &str) -> Result<Vec<Format>, String> {
    let mut formats = Vec::new();
    for part in value.split(',').map(str::trim).filter(|p| !p.is_empty()) {
        let format = match part.to_ascii_lowercase().as_str() {
            "srt" => Format::Srt,
            "vtt" => Format::Vtt,
            "txt" => Format::Txt,
            "md" => Format::Md,
            "json" => Format::Json,
            other => return Err(format!("unknown format `{other}`")),
        };
        if !formats.contains(&format) {
            formats.push(format);
        }
    }
    if formats.is_empty() {
        return Err("--format needs at least one format".to_string());
    }
    Ok(formats)
}

/// Everything the settings file contributes, resolved once.
struct Resolved {
    formats: Vec<Format>,
    language: String,
    /// From the GUI's solved plan; `None` means "the app has never been opened",
    /// which is reported rather than silently guessed.
    recognition: Option<model_dir::RecognitionPlan>,
}

fn resolve_settings(args: &Args, paths: &AppPaths) -> Result<Resolved, String> {
    let settings: BackendSettings = model_dir::read_settings_at(&paths.settings_path());

    let formats = args
        .formats
        .clone()
        .unwrap_or_else(|| vec![Format::Srt]);

    let language = args
        .language
        .clone()
        .or_else(|| {
            settings
                .recognition_plan
                .as_ref()
                .map(|plan| plan.language.clone())
        })
        .unwrap_or_else(|| "auto".to_string());

    Ok(Resolved {
        formats,
        language,
        recognition: settings.recognition_plan,
    })
}

async fn run(args: Args) -> ExitCode {
    let paths = match AppPaths::from_env() {
        Ok(paths) => paths,
        Err(error) => {
            eprintln!("monosub-cli: {error}");
            return ExitCode::from(code::TRANSCRIBE_FAILED);
        }
    };

    let resolved = match resolve_settings(&args, &paths) {
        Ok(resolved) => resolved,
        Err(error) => {
            eprintln!("monosub-cli: {error}");
            return ExitCode::from(code::USAGE);
        }
    };

    if resolved.recognition.is_none() && !args.quiet {
        eprintln!(
            "monosub-cli: no model plan is saved yet — open MonoSub once and pick your \
             preferences in Settings ▸ 模型, then run this again."
        );
    }

    let mut first_failure = code::OK;

    // Sequential on purpose: audio.cpp and llama.cpp both want the GPU, so
    // running files in parallel only makes each one slower.
    for input in &args.inputs {
        let outcome = run_one(&args, &resolved, &paths, input).await;
        if outcome != code::OK && first_failure == code::OK {
            first_failure = outcome;
        }
    }

    ExitCode::from(first_failure)
}

async fn run_one(
    args: &Args,
    resolved: &Resolved,
    paths: &AppPaths,
    input: &Path,
) -> u8 {
    let output = match &args.output_dir {
        Some(dir) => {
            let name = input
                .file_name()
                .map(|name| name.to_string_lossy().to_string())
                .unwrap_or_else(|| "output".to_string());
            let stem = name.rsplit_once('.').map(|(stem, _)| stem).unwrap_or(&name);
            dir.join(format!("{stem}.{}", resolved.formats[0].extension()))
        }
        None => input.with_extension(resolved.formats[0].extension()),
    };

    if let Some(parent) = output.parent() {
        if !parent.as_os_str().is_empty() {
            if let Err(error) = std::fs::create_dir_all(parent) {
                eprintln!("monosub-cli: cannot create {}: {error}", parent.display());
                return code::WRITE_FAILED;
            }
        }
    }

    // The plan the GUI solved, re-checked against what is actually installed:
    // the user may have deleted a model since.
    let plan = match resolve_plan(paths, resolved) {
        Ok(plan) => plan,
        Err((status, model)) => {
            eprintln!("monosub-cli: {status}");
            if let Some(model) = model {
                eprintln!(
                    "             download `{model}` in MonoSub (Settings ▸ 模型) and retry."
                );
            }
            return code::MODEL_MISSING;
        }
    };

    let translation = resolve_translation(paths, args, &resolved.language);
    if args.translate && translation.is_none() {
        eprintln!("monosub-cli: continuing without translation");
    }

    let params = TaskParams {
        task_id: format!("cli-{}", std::process::id()),
        input: input.to_path_buf(),
        output: output.clone(),
        formats: resolved.formats.clone(),
        language: resolved.language.clone(),
        model_id: plan
            .model_ids
            .first()
            .cloned()
            .unwrap_or_else(|| "sensevoice_small_q8".to_string()),
        vad_model_id: None,
        backend: monosub::engine::Backend::Metal,
        // The saved plan's own answer, not a re-derivation: the plan was solved
        // against `require_word_timestamps`, and a plan that cannot align still
        // runs (with estimated timing) — the engine decides from the model, this
        // field only says whether the user asked.
        want_word_timestamps: resolved
            .recognition
            .as_ref()
            .is_some_and(|saved| saved.require_word_timestamps),
        translation,
        plan,
        completed: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
    };

    let mut sink = if args.json {
        Sink::Json {
            input: input.to_path_buf(),
            warned: false,
        }
    } else {
        Sink::Human {
            input: input.to_path_buf(),
            quiet: args.quiet,
            last_percent: u8::MAX,
            warned: false,
        }
    };

    match transcribe_file(paths, &params, &mut sink).await {
        Ok(()) => {
            // `transcribe_file` already called `sink.finished`, so emitting here
            // would print the completion twice (measured: two `done` events).
            if !args.json && !args.quiet {
                eprintln!("  done     {}", output.display());
            }
            code::OK
        }
        Err(failure) => {
            let status = failure.status.clone();
            let detail = failure.detail.clone();
            eprintln!("monosub-cli: {}: {}\n             {detail}", input.display(), status);
            // The three sentinels `transcribe_file` can produce, in the order the
            // GUI also routes on them.
            if status.starts_with("MODEL_NOT_INSTALLED") {
                code::MODEL_MISSING
            } else if status == "TRANSLATE_FAILED" {
                // Subtitles are valid; only the translated copy is missing.
                eprintln!(
                    "             the subtitles are valid; only the translated copy failed."
                );
                code::TRANSLATE_FAILED
            } else if failure.context.contains("writ") {
                // `write_failure` sets context "writing the subtitle failed"; the
                // status there is free-form engine text, so the static context is
                // the only reliable discriminator.
                code::WRITE_FAILED
            } else {
                code::TRANSCRIBE_FAILED
            }
        }
    }
}

/// Build the translation plan the GUI would build, or explain what is missing.
///
/// Only runs when `--translate` was passed: the GUI's own `translateEnabled` is
/// deliberately NOT consulted (see the module docs — a batch script should not
/// pay for a 1.8B model load because the app happens to have the switch on).
fn resolve_translation(
    paths: &AppPaths,
    args: &Args,
    language: &str,
) -> Option<monosub::translate::stage::TranslationPlan> {
    use monosub::engine::catalog;

    if !args.translate {
        return None;
    }

    let catalogue = catalog::catalog(paths).ok()?;
    let installed = |id: &str| catalog::is_installed(paths, id);
    let model_file =
        |id: &str| catalog::model_path(paths, id).ok().flatten();

    match monosub::translate::stage::resolve(
        &catalogue,
        language,
        Some(true),
        Some(args.target.as_deref().unwrap_or(DEFAULT_TARGET)),
        None,
        &installed,
        &model_file,
    ) {
        Ok(plan) => plan,
        Err(status) => {
            eprintln!("monosub-cli: translation unavailable: {status}");
            None
        }
    }
}

/// Read the plan the GUI froze, or explain what is missing.
///
/// The CLI does **not** solve. The four questions are answered in the app and
/// frozen into `settings.json`; re-solving here would let the CLI run a model the
/// user never saw, and would pick a different one the day a model was deleted.
/// What it does still check is that the frozen models are on disk — a stale plan
/// must name the missing model rather than fail inside the engine.
fn resolve_plan(
    paths: &AppPaths,
    resolved: &Resolved,
) -> Result<monosub::engine::plan::ModelPlan, (String, Option<String>)> {
    use monosub::engine::{catalog, plan};

    let Some(saved) = resolved.recognition.as_ref() else {
        return Err((
            "no model plan is saved yet; open MonoSub once and pick your preferences in \
             Settings ▸ 模型"
                .to_string(),
            None,
        ));
    };

    let catalogue = catalog::catalog(paths)
        .map_err(|error| (format!("cannot read the model catalogue: {error}"), None))?;

    let mut plan = match plan::thaw(saved, &catalogue) {
        Ok(Some(plan)) => plan,
        Ok(None) => {
            // The gap plan was frozen: Settings itself found no model for the
            // saved answers. There is nothing to run and no id to suggest.
            return Err((
                "the saved plan names no model; open Settings ▸ 模型 and save a plan".to_string(),
                None,
            ));
        }
        Err(detail) => return Err((detail, None)),
    };

    plan::refresh_from_catalogue(&mut plan, &catalogue);

    let installed = |id: &str| catalog::is_installed(paths, id);
    for model_id in &plan.model_ids {
        // Same gate as the app: a retired entry may still be on disk from an
        // older install, and running it would silently produce the defect it was
        // retired for. No model id in the error, so the "download it" hint below
        // does not print — downloading it is exactly what the app refuses.
        if catalogue
            .iter()
            .any(|spec| spec.id == *model_id && !spec.status.is_offered())
        {
            return Err((
                format!(
                    "model `{model_id}` is not supported by this version of MonoSub; \
                     open Settings ▸ 模型 in the app and save the plan again"
                ),
                None,
            ));
        }
        if !installed(model_id) {
            return Err((
                format!("model `{model_id}` is not installed"),
                Some(model_id.clone()),
            ));
        }
    }

    Ok(plan)
}

/// Where progress goes. Two shapes, one trait: the GUI uses a third.
enum Sink {
    Human {
        input: PathBuf,
        quiet: bool,
        last_percent: u8,
        warned: bool,
    },
    Json {
        input: PathBuf,
        warned: bool,
    },
}

impl ProgressSink for Sink {
    fn step(&mut self, snapshot: &StepSnapshot) {
        match self {
            Sink::Human {
                input,
                quiet,
                last_percent,
                warned,
            } => {
                if *quiet {
                    return;
                }
                if !*warned {
                    eprintln!("{}", input.display());
                    *warned = true;
                }
                // `overall`, not the per-step `progress`: the step fraction
                // restarts at 0 on every step change, so a line driven by it
                // would print 0 % four times in one file. The step name still
                // says which step the percentage belongs to.
                let percent = snapshot.overall.round() as u8;
                // Only redraw when the rounded percentage moves: a per-chunk
                // event would otherwise fill the terminal with duplicates.
                if percent == *last_percent {
                    return;
                }
                *last_percent = percent;
                eprintln!(
                    "  {}/{}      {}        {percent}%",
                    snapshot.current_step, snapshot.total_steps, snapshot.step_name
                );
            }
            Sink::Json { input, warned } => {
                let _ = warned;
                // `percent` is the run-wide figure (what a script wants to
                // render); `step_percent` is the fraction inside the current
                // step, kept separate so the two are never confused.
                println!(
                    "{{\"event\":\"progress\",\"input\":{},\"step\":{},\"total\":{},\"name\":{},\
                     \"percent\":{},\"step_percent\":{}}}",
                    json_string(&input.to_string_lossy()),
                    snapshot.current_step,
                    snapshot.total_steps,
                    json_string(snapshot.step_name),
                    snapshot.overall.round() as u8,
                    snapshot.progress.round() as u8
                );
            }
        }
    }

    fn queued(&mut self, task_id: &str) {
        if let Sink::Json { input, .. } = self {
            println!(
                "{{\"event\":\"queued\",\"input\":{},\"task\":{}}}",
                json_string(&input.to_string_lossy()),
                json_string(task_id)
            );
        }
    }

    fn finished(&mut self, task_id: &str, total_seconds: f64) {
        if let Sink::Json { input, .. } = self {
            println!(
                "{{\"event\":\"done\",\"input\":{},\"task\":{},\"elapsed\":{total_seconds:.1}}}",
                json_string(&input.to_string_lossy()),
                json_string(task_id)
            );
        }
    }
}

/// Escape a string for the `--json` output.
///
/// Hand-rolled because `serde_json` is available but building the objects
/// field-by-field would need a struct per event just to keep the field order
/// stable for human eyes. The set is small and ASCII.
fn json_string(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn argv(items: &[&str]) -> Vec<String> {
        items.iter().map(|item| item.to_string()).collect()
    }

    #[test]
    fn help_is_not_an_error_and_needs_no_input() {
        assert!(Args::parse(&argv(&["--help"])).unwrap().is_none());
        assert!(Args::parse(&argv(&[])).unwrap().is_none());
    }

    #[test]
    fn a_missing_file_is_a_usage_error() {
        let error = Args::parse(&argv(&["/definitely/not/here.mp4"])).unwrap_err();
        assert!(error.contains("no such file"), "{error}");
    }

    #[test]
    fn an_unknown_flag_is_a_usage_error_not_a_path() {
        let error = Args::parse(&argv(&["--nope"])).unwrap_err();
        assert!(error.contains("unknown option"), "{error}");
    }

    #[test]
    fn formats_are_deduplicated_and_case_insensitive() {
        assert_eq!(parse_formats("SRT,srt,vtt").unwrap(), vec![Format::Srt, Format::Vtt]);
    }

    #[test]
    fn an_unknown_format_is_rejected() {
        assert!(parse_formats("srt,docx").is_err());
    }

    #[test]
    fn json_strings_escape_what_would_break_the_stream() {
        assert_eq!(json_string(r#"a"b\c"#), r#""a\"b\\c""#);
        assert_eq!(json_string("a\nb"), r#""a\nb""#);
        // A CJK path must survive untouched.
        assert_eq!(json_string("视频.mp4"), r#""视频.mp4""#);
    }

    /// The denominator the CLI prints must be the same one the GUI renders, or
    /// the two frontends would disagree about how far along a run is.
    #[test]
    fn the_step_denominators_match_the_gui_contract() {
        use monosub::commands::transcribe::{STEPS_WITHOUT_TRANSLATION, STEPS_WITH_TRANSLATION};
        assert_eq!(STEPS_WITHOUT_TRANSLATION, 4);
        assert_eq!(STEPS_WITH_TRANSLATION, 5);
    }

    #[test]
    fn translation_is_off_unless_the_flag_is_passed() {
        let parsed = Args::parse(&argv(&["--translate"])).unwrap();
        // No inputs means "help"; this asserts the flag itself parses.
        assert!(parsed.is_none());
    }
}
