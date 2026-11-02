//! `monosub-cli` — the second frontend over MonoSub's pipeline.
//!
//! Design: `docs/tech-spec/cli-design.md`. The rules that shape this file:
//!
//! * **It does not solve a plan.** The four questions are answered in the GUI,
//!   which writes the solved plan into `<config_dir>/settings.json`; this binary
//!   reads that. Re-solving here would run a model the user never saw.
//! * **`--model` overrides one slot of that plan, for one run, and writes
//!   nothing back.** It is an override, not a second source of truth: the saved
//!   answers still decide the plan's shape (how many speakers it separates, how
//!   much memory it may take), and only the named model is substituted.
//! * **It controls only what is per-invocation**: formats, language, output
//!   directory, translation, word timing, model pins.
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
//! | 6 | subtitles written but the word-level timing is estimated, not aligned |

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use monosub::commands::transcribe::{
    transcribe_file, ProgressSink, StepSnapshot, TaskOutcome, TaskParams, TranslationReport,
};
use monosub::engine::plan::StepKind;
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
    /// Word timing was asked for and not produced. Same shape as
    /// [`TRANSLATE_FAILED`]: the subtitle is valid, the requested extra is not.
    pub const WORDS_ESTIMATED: u8 = 6;
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
        --word-timestamps    Also write <stem>.words.json: every word with its
                             own start/end, plus the silences between them
        --model <[slot=]id>  Use this model for this run only (default slot:
                             asr; also diar). settings.json is not modified
        --list-models        Print the models this build can run, then exit
        --translate          Also translate (off by default)
        --target <tag>       Translation target (default: en)
        --json               Machine-readable events on stdout
    -q, --quiet              Only report errors
    -h, --help               Show this help

The plan comes from the app: the four questions in Settings ▸ 模型 are solved once
and read from settings.json, so both frontends run the same model. Word-level
timing is one of those answers — it needs an alignable model (`qwen3_asr` + the
forced aligner), so `--word-timestamps` with a plan that cannot align warns and
falls back to estimated timing (exit code 6).

`--model` is the one exception, and it is not sticky: it substitutes one model
for this invocation, keeps the rest of your answers, and writes nothing back.
`--list-models` prints the ids it accepts (installed ones first).

EXAMPLES:
    monosub-cli talk.mp4
        Transcribe with the plan saved in the app; writes talk.srt beside it.

    monosub-cli talk.mp4 --model qwen3_asr_0_6b_q8_0 --word-timestamps -o ~/subs
        The same plan and the same answers, but qwen3_asr transcribes this one
        file — settings.json is not touched, and nothing is saved for next time.
        It writes ~/subs/talk.srt and ~/subs/talk.words.json, the latter holding
        every word with its own start and end plus the silences between them.
        Exits 6 if that model could not align and the timing is only estimated.

    monosub-cli interview.mp4 --model diar=nemotron_3_diar_q8_0
        Keep the transcribing model, separate speakers with that one instead.

    monosub-cli --list-models
        The ids, slots and timestamp support of every model this build can run.

EXIT CODES:
    0 success   1 usage   2 model missing   3 transcription failed
    4 write failed   5 translated file failed (subtitles are still valid)
    6 word timing was requested but only estimated (subtitles are valid)
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

    // Before the async runtime: reading the catalogue is a file read and runs no
    // inference, and `--list-models` takes no input file.
    if parsed.list_models {
        return list_models_and_exit();
    }

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
    /// Ask for word-level timing on this run. ORs with the saved plan's
    /// `requireWordTimestamps`; it never turns the saved answer off, so a GUI
    /// user who always wants it does not have to repeat the flag.
    word_timestamps: bool,
    /// `(slot, model id)` pins for this run, in the order given. `slot` is the
    /// GUI's own vocabulary (`asr` / `diar`), resolved at parse time so a typo
    /// is a usage error rather than a silently ignored override.
    models: Vec<(String, String)>,
    /// Print the catalogue and exit; needs no input file.
    list_models: bool,
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
            word_timestamps: false,
            models: Vec::new(),
            list_models: false,
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
                "--word-timestamps" => args.word_timestamps = true,
                "--model" => {
                    let value = rest.next().ok_or("--model needs a model id")?;
                    let parsed = parse_model_override(value)?;
                    // Last one wins per slot: a script that appends a default
                    // and then a specific pin should get the specific one, not
                    // two overrides fighting in HashMap order.
                    args.models.retain(|(slot, _)| slot != &parsed.0);
                    args.models.push(parsed);
                }
                "--list-models" => args.list_models = true,
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

        if args.inputs.is_empty() && !args.list_models {
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

/// `qwen3_asr_0_6b_q8_0` or `diar=nemotron_3_diar_q8_0` → `(slot, id)`.
///
/// The slot spelling is the GUI's own: `commands::plan`'s `model_overrides` uses
/// the same words, so someone who pinned a model in Settings types the same word
/// here instead of learning a second vocabulary.
fn parse_model_override(value: &str) -> Result<(String, String), String> {
    let (slot, id) = match value.split_once('=') {
        Some((slot, id)) => (slot.trim().to_ascii_lowercase(), id.trim()),
        None => ("asr".to_string(), value.trim()),
    };
    if id.is_empty() {
        return Err(format!("--model `{value}` names no model"));
    }

    match monosub::engine::plan::override_slot_kind(&slot) {
        None => Err(format!(
            "unknown slot `{slot}` in --model `{value}`; use asr (the default) or diar"
        )),
        // Both refusals are about honesty, not capability: a step this binary
        // would not execute is worse than an error, because the run would look
        // like it honoured the flag.
        Some(StepKind::Align) => Err(
            "--model align=… is not supported: this app never runs an `align` step — word \
             timing comes from the ASR session, so use --word-timestamps"
                .to_string(),
        ),
        Some(StepKind::Vad) => Err(
            "--model vad=… is not supported: the CLI always segments with the bundled Silero \
             VAD inside the ASR session"
                .to_string(),
        ),
        Some(_) => Ok((slot, id.to_string())),
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

fn list_models_and_exit() -> ExitCode {
    let paths = match AppPaths::from_env() {
        Ok(paths) => paths,
        Err(error) => {
            eprintln!("monosub-cli: {error}");
            return ExitCode::from(code::TRANSCRIBE_FAILED);
        }
    };
    match list_models(&paths) {
        Ok(()) => ExitCode::from(code::OK),
        // An unreadable catalogue means no model can be named or resolved, which
        // is the same failure the run path reports with this code.
        Err(status) => {
            eprintln!("monosub-cli: {status}");
            ExitCode::from(code::MODEL_MISSING)
        }
    }
}

/// Print the catalogue as a table, installed models first.
///
/// Read-only on purpose — nothing here downloads or removes; that stays in
/// Settings ▸ 模型. It exists because `--model` is unusable without the exact ids,
/// and sending someone back to the App to read them off a dropdown defeats the
/// point of having a CLI.
///
/// The list is filtered the way the solver filters: audio.cpp models that are
/// not translators. Those are exactly the entries `--model` can name, so the
/// table cannot advertise a model the flag would refuse.
fn list_models(paths: &AppPaths) -> Result<(), String> {
    use monosub::engine::catalog::{self, Engine, Role};

    let catalogue = catalog::catalog(paths)
        .map_err(|error| format!("cannot read the model catalogue: {error}"))?;

    let mut rows: Vec<(&catalog::ModelSpec, bool)> = catalogue
        .iter()
        .filter(|spec| spec.engine == Engine::Audiocpp && !spec.has_role(Role::Translate))
        // Installed first: the entries that are not on this machine are reference
        // material, not choices. Within each group, by id, so the order is stable
        // enough to diff.
        .map(|spec| (spec, catalog::is_installed(paths, &spec.id)))
        .collect();
    rows.sort_by(|(a, a_ready), (b, b_ready)| {
        b_ready.cmp(a_ready).then_with(|| a.id.cmp(&b.id))
    });

    let width = rows.iter().map(|(spec, _)| spec.id.len()).max().unwrap_or(2);
    println!(
        "{:<width$}  {:<9}  {:<9}  {:<9}  STATUS",
        "ID", "SLOTS", "TIMING", "INSTALLED"
    );
    for (spec, ready) in &rows {
        println!(
            "{:<width$}  {:<9}  {:<9}  {:<9}  {}",
            spec.id,
            slots(spec),
            spec.capabilities().timestamps.as_str(),
            if *ready { "yes" } else { "no" },
            spec.status.as_str(),
        );
    }
    Ok(())
}

/// Which `--model` slots a catalogue entry can fill, comma separated.
///
/// The four names are [`monosub::engine::plan::override_slot_kind`]'s, so the
/// table and the flag cannot disagree about what a slot is called. `align` is
/// listed even though the CLI refuses it: the entry genuinely can fill the slot,
/// and hiding it would make the refusal look like a missing capability.
fn slots(spec: &monosub::engine::catalog::ModelSpec) -> String {
    use monosub::engine::catalog::Role;

    let mut names = Vec::new();
    for (role, name) in [
        (Role::Asr, "asr"),
        (Role::Vad, "vad"),
        (Role::Diar, "diar"),
        (Role::Align, "align"),
    ] {
        if spec.has_role(role) {
            names.push(name);
        }
    }
    if names.is_empty() {
        "-".to_string()
    } else {
        names.join(",")
    }
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

    // The plan the GUI solved — with this run's `--model` pins applied — then
    // re-checked against what is actually installed: the user may have deleted a
    // model since.
    let plan = match resolve_plan(paths, resolved, &args.models) {
        Ok(plan) => plan,
        Err(PlanError::Usage(status)) => {
            eprintln!("monosub-cli: {status}");
            return code::USAGE;
        }
        Err(PlanError::MissingModel(status, model)) => {
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

    // Which model runs each stage. The CLI has no per-run speaker switch, so the
    // frozen plan executes exactly as saved — which means `diar` here never
    // carries `[disabled]`; it is `none` when the plan separates no speakers.
    // Human output only: `--json`'s field set is experimental, and a script
    // reads the exit code and the output paths.
    if !args.json && !args.quiet {
        let stages = plan.stage_models();
        eprintln!(
            "  plan     vad={}  asr={}  diar={}",
            stages.vad,
            stages.asr_label(),
            stages.diar_label(),
        );
    }

    // The ASR model, asked of the plan **by role** rather than by position.
    // `model_ids` is in step order, so `.first()` was only ever the ASR because no
    // template starts with a VAD step — and `--model` is exactly the kind of
    // change that turns "happens to be" into "requests the wrong model".
    let asr_model_id = plan
        .stage_models()
        .asr
        .unwrap_or_else(|| "sensevoice_small_q8".to_string());

    // Word timing comes from the model, not from the flag. `--word-timestamps`
    // cannot pick one (design §1.3 — a second place to choose a model is a second
    // source of truth), so when the saved plan's ASR cannot align the honest
    // answer is to say so *before* the expensive run rather than after it: on a
    // feature-length file the difference is minutes, and the failure is otherwise
    // only visible in the output an agent consumes.
    let aligns = can_align(paths, &asr_model_id);
    let want_words = args.word_timestamps
        || resolved
            .recognition
            .as_ref()
            .is_some_and(|saved| saved.require_word_timestamps);
    if want_words && !aligns {
        eprintln!(
            "monosub-cli: warning: `{asr_model_id}` cannot produce word-level timing, so this \
             run estimates it\n             and writes no .words.json. To fix: MonoSub ▸ \
             Settings ▸ 模型, turn on 词级时间戳,\n             save the plan (it then picks \
             qwen3_asr + the forced aligner), and run this again."
        );
    }

    let params = TaskParams {
        task_id: format!("cli-{}", std::process::id()),
        input: input.to_path_buf(),
        output: output.clone(),
        formats: resolved.formats.clone(),
        language: resolved.language.clone(),
        model_id: asr_model_id,
        vad_model_id: None,
        // The platform's own backend, not a hard-coded Metal: on Windows the
        // bundled engine is the CUDA build and `metal` does not exist there.
        backend: monosub::engine::Backend::platform_default(),
        // The saved plan's own answer, OR the flag. A flag that could only add
        // is a per-run override; one that could also switch the saved answer off
        // would make the CLI a second place where the plan is decided.
        //
        // It says the user *asked* — the engine decides from the model whether
        // that is possible, and falls back to estimated timing when it is not.
        want_word_timestamps: want_words,
        translation,
        plan,
        completed: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
    };

    let mut sink = if args.json {
        Sink::Json {
            input: input.to_path_buf(),
            closing: None,
        }
    } else {
        Sink::Human {
            input: input.to_path_buf(),
            quiet: args.quiet,
            last_percent: u8::MAX,
            warned: false,
        }
    };

    let result = transcribe_file(paths, &params, &mut sink).await;

    // Both closing reports live here rather than in the sink's `finished`,
    // because the sidecar is only known once the run has returned — and `done`
    // means "nothing else follows for this input", so it must be the last line.
    // A `words` event printed after `done` is an event a streaming consumer
    // never reads.
    if args.json {
        close_json(&mut sink, input, result.as_ref().ok(), want_words);
    } else if let Ok(outcome) = &result {
        if !args.quiet {
            match &outcome.word_sidecar {
                Some(path) => eprintln!("  words    {} ({} spans)", path.display(), outcome.words),
                // Only when something was predicted to work and did not, *and*
                // there was speech to align. The pre-flight warning above already
                // covers the unalignable-plan case (repeating it reads as two
                // problems), and a silent input has nothing to estimate — it is
                // an ordinary empty subtitle, not a degradation.
                None if words_estimated(want_words, outcome) && aligns => eprintln!(
                    "  words    none — the aligner returned no spans, so timing is estimated"
                ),
                None => {}
            }
            eprintln!("  done     {}", output.display());
        }
    }

    match result {
        Ok(outcome) => {
            if words_estimated(want_words, &outcome) {
                // The subtitle is valid — this is the same shape as exit 5, not a
                // reason to throw away a finished transcript. A silent input is
                // *not* this: it has no cues and no words, and 0 is its answer.
                eprintln!(
                    "             the subtitles are valid; only the word-level timing is estimated."
                );
                code::WORDS_ESTIMATED
            } else {
                code::OK
            }
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

/// Why a plan could not be resolved. The two cases need different exit codes:
/// a bad `--model` is something the caller typed wrong, a deleted model is
/// something the caller has to install.
enum PlanError {
    Usage(String),
    /// `(status, model to download)`.
    MissingModel(String, Option<String>),
}

/// Read the plan the GUI froze, apply this run's `--model` pins, or explain what
/// is missing.
///
/// The CLI does **not** solve. The four questions are answered in the app and
/// frozen into `settings.json`; re-solving here would let the CLI run a model the
/// user never saw, and would pick a different one the day a model was deleted.
/// What it does still check is that the frozen models are on disk — a stale plan
/// must name the missing model rather than fail inside the engine.
///
/// `overrides` are `(slot, id)` pairs from `--model`. They replace one step of
/// the *saved* plan rather than re-solving it: the answers to the other four
/// questions are the user's, and swapping a model must not quietly change how
/// many speakers it separates or how much memory it may take.
///
/// The override goes through [`plan::apply_model_overrides`] — the function the
/// GUI's advanced settings use — so a pinned model means the same thing on both
/// frontends, down to adding the turn-attribution step a diarization pin needs.
fn resolve_plan(
    paths: &AppPaths,
    resolved: &Resolved,
    overrides: &[(String, String)],
) -> Result<monosub::engine::plan::ModelPlan, PlanError> {
    use monosub::engine::{catalog, plan};

    let Some(saved) = resolved.recognition.as_ref() else {
        return Err(PlanError::Usage(
            "no model plan is saved yet; open MonoSub once and pick your preferences in \
             Settings ▸ 模型"
                .to_string(),
        ));
    };

    let catalogue = catalog::catalog(paths)
        .map_err(|error| PlanError::Usage(format!("cannot read the model catalogue: {error}")))?;

    let mut plan = match plan::thaw(saved, &catalogue) {
        Ok(Some(plan)) => plan,
        Ok(None) => {
            // The gap plan was frozen: Settings itself found no model for the
            // saved answers. There is nothing to run and no id to suggest.
            return Err(PlanError::Usage(
                "the saved plan names no model; open Settings ▸ 模型 and save a plan".to_string(),
            ));
        }
        Err(detail) => return Err(PlanError::Usage(detail)),
    };

    plan::refresh_from_catalogue(&mut plan, &catalogue);

    if !overrides.is_empty() {
        // Validated here, not left to `apply_model_overrides`: that function
        // *ignores* an unknown id after a `tracing::warn!`, which never reaches
        // stderr in a CLI. Running the saved model while the user asked for
        // another one is exactly the silent substitution this flag must not do.
        let mut by_slot: std::collections::HashMap<String, String> =
            std::collections::HashMap::with_capacity(overrides.len());
        for (slot, id) in overrides {
            match catalogue.iter().find(|spec| spec.id == *id) {
                None => {
                    return Err(PlanError::Usage(format!(
                        "`--model {slot}={id}`: no model in the catalogue has that id \
                         (see `monosub-cli --list-models`)"
                    )))
                }
                Some(spec) if !spec.status.is_offered() => {
                    return Err(PlanError::Usage(format!(
                        "`--model {slot}={id}`: `{id}` is {} in this version of MonoSub",
                        spec.status.as_str()
                    )))
                }
                Some(_) => {}
            }
            by_slot.insert(slot.clone(), id.clone());
        }

        // A one-element slice because the GUI's helper is written for the solver's
        // plan list; the CLI has exactly one plan to patch.
        let mut only = [plan];
        plan::apply_model_overrides(&mut only, &by_slot, &catalogue);
        plan = only.into_iter().next().expect("one plan in, one plan out");
    }

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
            return Err(PlanError::Usage(format!(
                "model `{model_id}` is not supported by this version of MonoSub; \
                 open Settings ▸ 模型 in the app and save the plan again"
            )));
        }
        if !installed(model_id) {
            // A pinned model that is merely *not installed* is still a model the
            // caller named, so it gets the actionable "download it" hint rather
            // than a usage error.
            return Err(PlanError::MissingModel(
                format!("model `{model_id}` is not installed"),
                Some(model_id.clone()),
            ));
        }
    }

    Ok(plan)
}

/// Whether the engine will actually align with this model.
///
/// Asks `engine::aligned::can_align` — the engine's own gate — rather than
/// forming a second opinion: a CLI check that could disagree with the run would
/// be worse than none, because it would promise a `.words.json` the run never
/// writes.
///
/// An id the catalogue does not know answers "no complaint". `resolve_plan` has
/// already rejected an unknown or uninstalled model a few lines above, so the
/// only reachable case is a race (the user deleted it since), and inventing a
/// second diagnostic for one condition would print two contradictory messages.
fn can_align(paths: &AppPaths, model_id: &str) -> bool {
    let Ok(catalogue) = monosub::engine::catalog::catalog(paths) else {
        return true;
    };
    match catalogue.iter().find(|spec| spec.id == model_id) {
        Some(spec) => monosub::engine::aligned::can_align(paths, spec),
        None => true,
    }
}

/// What this run did about word timing, spelled as the `--json` event says it.
///
/// Four answers, not three: `estimated` would be a lie about an all-silent input,
/// where nothing was estimated because there was nothing to time. The field has
/// to be readable without also parsing `count`, or a consumer will treat silence
/// as a degraded run.
fn word_timing(want: bool, outcome: &TaskOutcome) -> &'static str {
    if !want {
        "off"
    } else if outcome.words > 0 {
        "aligned"
    } else if outcome.cues == 0 {
        "none"
    } else {
        "estimated"
    }
}

/// Word timing was asked for, there was speech to align, and none of it was
/// aligned.
///
/// `cues == 0` is excluded deliberately: an all-silent input has neither cues nor
/// words and exit 0 is its answer — the same rule a translation `SkipReason::
/// NoSegments` gets. Without that clause, a silent clip asked for word timing
/// would report exit 6 for being silent.
fn words_estimated(want: bool, outcome: &TaskOutcome) -> bool {
    want && outcome.words == 0 && outcome.cues > 0
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
        /// `(task_id, elapsed)` recorded by [`ProgressSink::finished`] and printed
        /// by [`close_json`]. The sink cannot print `done` itself any more: the
        /// word-sidecar event has to precede it, and only the caller knows that.
        closing: Option<(String, f64)>,
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
            Sink::Json { input, .. } => {
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

    fn waiting(&mut self, _task_id: &str) {
        // The CLI processes inputs in order and never runs two at once, so there
        // is no slot to wait for. Printing a line here would invent a state the
        // CLI cannot be in.
    }

    fn finished(
        &mut self,
        task_id: &str,
        total_seconds: f64,
        _translation: Option<&TranslationReport>,
    ) {
        // The translation report is the GUI's: the CLI's stable contract is the
        // exit code plus the output paths, and a `--json` consumer that wanted
        // the detail would be reading an experimental field set anyway.
        //
        // Recorded, not printed: `done` must be the last line for this input, and
        // the word-sidecar event that precedes it is only known to the caller.
        if let Sink::Json { closing, .. } = self {
            *closing = Some((task_id.to_string(), total_seconds));
        }
    }
}

/// Print the two events that close one input, in an order that means something:
/// the sidecar first, then `done`.
///
/// `outcome` is `None` when the run failed. `done` is still printed if
/// `transcribe_file` reached `finished` — that is what today's output does, and a
/// `TRANSLATE_FAILED` run genuinely did finish writing its subtitles.
fn close_json(sink: &mut Sink, input: &Path, outcome: Option<&TaskOutcome>, want_words: bool) {
    let Sink::Json { closing, .. } = sink else {
        return;
    };
    let Some((task_id, elapsed)) = closing.take() else {
        return;
    };

    if let Some(outcome) = outcome {
        println!(
            "{{\"event\":\"words\",\"input\":{},\"timing\":{},\"count\":{},\"path\":{}}}",
            json_string(&input.to_string_lossy()),
            json_string(word_timing(want_words, outcome)),
            outcome.words,
            match &outcome.word_sidecar {
                Some(path) => json_string(&path.to_string_lossy()),
                None => "null".to_string(),
            }
        );
    }
    println!(
        "{{\"event\":\"done\",\"input\":{},\"task\":{},\"elapsed\":{elapsed:.1}}}",
        json_string(&input.to_string_lossy()),
        json_string(&task_id)
    );
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

    /// Every flag `--help` advertises must actually exist.
    ///
    /// `cli-design.md` §11.8 records an option that was documented, parsed and
    /// then never read: 502 tests were green while `--translate` did nothing.
    /// This is the cheap half of that guard — the flag is at least spelled the
    /// same in both places.
    #[test]
    fn the_help_text_lists_every_flag() {
        for flag in [
            "--word-timestamps",
            "--model",
            "--list-models",
            "--output-dir",
            "--format",
            "--language",
            "--translate",
            "--target",
            "--json",
            "--quiet",
            "--help",
        ] {
            assert!(USAGE.contains(flag), "`{flag}` is missing from --help");
        }
    }

    /// A bare id means the ASR slot, because that is the model that transcribes.
    #[test]
    fn a_bare_model_id_is_the_asr_slot() {
        assert_eq!(
            parse_model_override("qwen3_asr_0_6b_q8_0").unwrap(),
            ("asr".to_string(), "qwen3_asr_0_6b_q8_0".to_string())
        );
        assert_eq!(
            parse_model_override("DIAR=nemotron_3_diar_q8_0").unwrap(),
            ("diar".to_string(), "nemotron_3_diar_q8_0".to_string()),
            "the slot is case-insensitive, the id is not"
        );
    }

    /// A slot this binary would not execute is refused, not ignored: a run that
    /// looks like it honoured the flag but pinned nothing is the failure mode.
    #[test]
    fn slots_the_cli_cannot_honour_are_refused() {
        for value in ["vad=silero_vad", "align=qwen3_forced_aligner_0_6b_q8_0"] {
            let error = parse_model_override(value).expect_err("should be refused");
            assert!(error.contains("--model"), "unhelpful: {error}");
        }
        let error = parse_model_override("speaker=nemotron_3_diar_q8_0").unwrap_err();
        assert!(error.contains("unknown slot"), "unhelpful: {error}");
        assert!(parse_model_override("asr=").is_err(), "an empty id names nothing");
    }

    #[test]
    fn the_last_pin_for_a_slot_wins() {
        let (_, path) = scratch_input("clip.wav");
        let parsed = Args::parse(&argv(&[
            &path,
            "--model",
            "sensevoice_small_q8",
            "--model",
            "qwen3_asr_0_6b_q8_0",
        ]))
        .expect("parses")
        .expect("one input");

        assert_eq!(
            parsed.models,
            vec![("asr".to_string(), "qwen3_asr_0_6b_q8_0".to_string())],
            "a script that appends a default and then a pin must get the pin"
        );
    }

    /// Listing is not "help": it needs no input file, and `--help` needs no
    /// catalogue.
    #[test]
    fn listing_models_needs_no_input_file() {
        let parsed = Args::parse(&argv(&["--list-models"]))
            .expect("parses")
            .expect("listing is not help");
        assert!(parsed.list_models);
        assert!(parsed.inputs.is_empty());
    }

    /// The `--model` example in `--help` must be a command that runs.
    ///
    /// Not a tautology: `--help` is prose, and prose drifts. This parses the
    /// example line *out of* the help text and feeds it to the real parser, so a
    /// renamed flag or a changed slot spelling breaks a test instead of
    /// misleading the next reader (cli-design.md §11.8 is the time it didn't).
    #[test]
    fn the_model_example_in_help_parses() {
        let (path, real) = scratch_input("talk.mp4");

        let line = USAGE
            .lines()
            .find(|line| line.contains(" --model ") && line.trim_start().starts_with("monosub-cli "))
            .expect("--help must show a complete --model example on one line");

        let argv: Vec<String> = line
            .split_whitespace()
            .skip(1) // the program name
            .map(|token| if token == "talk.mp4" { real.clone() } else { token.to_string() })
            .collect();

        let parsed = Args::parse(&argv)
            .expect("the example parses")
            .expect("the example names an input");
        assert_eq!(
            parsed.models,
            vec![("asr".to_string(), "qwen3_asr_0_6b_q8_0".to_string())]
        );
        assert!(parsed.word_timestamps, "the example shows both flags together");
        assert!(parsed.output_dir.is_some());

        let _ = std::fs::remove_file(&path);
    }

    /// A real file, because `parse` rejects inputs that do not exist.
    fn scratch_input(name: &str) -> (PathBuf, String) {
        let dir = std::env::temp_dir().join(format!("monosub-cli-args-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("scratch dir");
        let path = dir.join(name);
        std::fs::write(&path, b"").expect("scratch file");
        let text = path.to_string_lossy().to_string();
        (path, text)
    }

    #[test]
    fn word_timestamps_are_off_unless_asked_for() {
        let (path, text) = scratch_input("clip.wav");

        let plain = Args::parse(&argv(&[&text]))
            .expect("a file parses")
            .expect("one input");
        assert!(!plain.word_timestamps, "default is off");

        let asked = Args::parse(&argv(&[&text, "--word-timestamps"]))
            .expect("a file parses")
            .expect("one input");
        assert!(asked.word_timestamps);

        let _ = std::fs::remove_file(&path);
    }

    /// Word timing is a per-run override that can only add: it must not become a
    /// second place where the saved plan's answer is decided.
    #[test]
    fn the_flag_only_ever_turns_word_timing_on() {
        let outcome = TaskOutcome {
            cues: 12,
            words: 0,
            word_sidecar: None,
        };
        assert_eq!(word_timing(false, &outcome), "off");
        assert_eq!(word_timing(true, &outcome), "estimated");
        assert_eq!(
            word_timing(true, &TaskOutcome::default()),
            "none",
            "silence was not estimated, and saying so would read as a degraded run"
        );

        let aligned = TaskOutcome {
            cues: 12,
            words: 40,
            word_sidecar: Some(PathBuf::from("/tmp/clip.words.json")),
        };
        assert_eq!(word_timing(true, &aligned), "aligned");
    }

    /// Exit 6 means "speech, requested, not aligned". A silent input has no cues
    /// and no words and must stay exit 0 — the same rule a translation
    /// `SkipReason::NoSegments` gets (cli-design §5.3).
    #[test]
    fn a_silent_input_is_not_a_missing_word_sidecar() {
        let speech_without_words = TaskOutcome {
            cues: 12,
            words: 0,
            word_sidecar: None,
        };
        assert!(words_estimated(true, &speech_without_words));

        assert!(
            !words_estimated(true, &TaskOutcome::default()),
            "no cues means silence, which is a successful run"
        );
        assert!(
            !words_estimated(false, &speech_without_words),
            "nobody asked, so nothing is missing"
        );
        assert!(!words_estimated(
            true,
            &TaskOutcome {
                cues: 12,
                words: 40,
                word_sidecar: Some(PathBuf::from("/tmp/clip.words.json")),
            }
        ));
    }
}
