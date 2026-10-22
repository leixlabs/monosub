//! `yt-dlp` process management: binary resolution, constant argv, spawn, and
//! line-by-line stdout reading with a stderr ring buffer.
//!
//! Tasks: U1-4 (path + argv) and U1-5 (spawn). See
//! docs/tech-spec/url-import-design.md §6 (packaging and invocation rules).
//!
//! The two hard rules from §6.2 are pinned by tests rather than by comments:
//!
//! 1. Every invocation carries the constant flags. Without `--ignore-config` a
//!    user's `~/.config/yt-dlp/config` can change the output template, the
//!    format or the subtitle options, and every judgement this app makes is
//!    based on files on disk — so a foreign config silently invalidates them.
//!    `--newline` is what makes progress readable line by line, and
//!    `--ffmpeg-location` stops a stray system ffmpeg from being used for a
//!    merge.
//! 2. `--no-warnings` must never be added: 412, "needs login" and
//!    region-restriction notices all arrive as warnings, and they are exactly
//!    what `site::error` classifies.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::process::Stdio;

use tokio::io::{AsyncBufReadExt, BufReader};

use crate::site::error::ImportError;

/// Base name of the bundled executable; the file on disk carries a
/// `-<target-triple>` suffix like every other bundled binary.
const BINARY: &str = "yt-dlp";
/// Env override, mirroring `MONOSUB_AUDIOCPP_PATH` / `MONOSUB_LLAMACPP_PATH`.
pub const PATH_ENV: &str = "MONOSUB_YTDLP_PATH";
/// stderr kept for error reporting; same bound as `engine::cli`.
const STDERR_TAIL_LINES: usize = 50;

/// Flags every invocation carries (design §6.2), split into the always-on set and
/// the one flag the audio stage must override.
///
/// `--no-progress` is NOT in [`ALWAYS_ARGS`]: measured with the bundled 2026.08.19
/// binary, `--no-progress` together with `--progress-template` emits **zero**
/// `PROG` lines. The flag does not merely suppress the default progress display,
/// it suppresses the template too, which left the audio-download bar permanently
/// indeterminate. Resolve and FetchSubtitles do not want progress, so they add it.
pub const ALWAYS_ARGS: [&str; 3] = ["--ignore-config", "--no-playlist", "--newline"];

/// Added only by the stages that want no progress output at all.
pub const NO_PROGRESS_ARG: &str = "--no-progress";

/// The full constant set for a quiet stage: [`ALWAYS_ARGS`] plus
/// [`NO_PROGRESS_ARG`]. Kept as one name so a test can assert the whole list
/// without rebuilding it.
#[cfg(test)]
pub const QUIET_ARGS: [&str; 4] = ["--ignore-config", "--no-playlist", "--newline", "--no-progress"];

/// The progress template for the audio-download stage. `download:` is the
/// prefix yt-dlp itself prints, `PROG` is ours (see `site::progress`).
pub const PROGRESS_TEMPLATE: &str = "download:PROG %(progress.downloaded_bytes)s %(progress.total_bytes)s %(progress.total_bytes_estimate)s %(progress.status)s";

/// Subtitle container preference: SRT natively, best available otherwise.
/// `--convert-subs` is deliberately NOT used — Bilibili serves SRT already, and
/// converting would add an ffmpeg dependency to the subtitle path (design §2).
pub const SUB_FORMAT: &str = "srt/best";

/// Audio format preference for the fallback path (design §5.3).
pub const AUDIO_FORMAT: &str = "ba[ext=m4a]/ba/b";

/// Which yt-dlp stage an invocation is for. The argv differs per stage, so the
/// difference is a type rather than a comment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Stage {
    /// One call that returns the info dict (`--list-subs --print-json`).
    Resolve,
    /// Download exactly one subtitle track from a saved info dict.
    FetchSubtitles {
        /// Directory holding `info.json`; the output template points here.
        work_dir: PathBuf,
        tag: String,
    },
    /// Download the audio for the transcription fallback.
    ///
    /// The finished file is the user's: it goes to `home_dir` (the import output
    /// directory) under the same stem as the subtitle, so the two sit side by
    /// side. `temp_dir` only receives the in-flight `.part`, which yt-dlp itself
    /// moves into `home_dir` when the transfer completes — so the app cache never
    /// keeps a second copy of the media.
    DownloadAudio {
        home_dir: PathBuf,
        temp_dir: PathBuf,
        /// The `<sanitized title> [<id>]` stem (see `site::naming`). A literal:
        /// yt-dlp cannot reproduce the sanitised title from `%(title)s`.
        file_stem: String,
    },
}

/// Everything an invocation needs beyond the stage.
#[derive(Debug, Clone)]
pub struct Invocation {
    pub stage: Stage,
    /// The URL, passed as ONE argv element (never through a shell).
    pub url: String,
    /// `--cookies-from-browser <browser>`; `None` for "not configured".
    pub cookies_browser: Option<String>,
    /// `--ffmpeg-location <bundled>`; only used when a merge is needed.
    pub ffmpeg: Option<PathBuf>,
}

/// `regex::escape` equivalent for the `--sub-langs` value.
///
/// yt-dlp treats `--sub-langs` as a regular expression, so a tag like `zh.` would
/// match more than intended. Only `.` is a realistic hazard in these tags, but
/// escaping the whole set keeps one rule instead of a special case (design §4.2
/// rule 5).
pub fn escape_sub_lang(tag: &str) -> String {
    let mut escaped = String::with_capacity(tag.len() * 2);
    for ch in tag.chars() {
        if !ch.is_ascii_alphanumeric() && ch != '-' && ch != '_' {
            escaped.push('\\');
        }
        escaped.push(ch);
    }
    escaped
}

/// The complete argv for one invocation, executable name excluded.
///
/// Pure so every stage's command line can be pinned by a test.
pub fn build_argv(invocation: &Invocation) -> Vec<String> {
    // The audio stage asks for progress, so it must NOT carry `--no-progress`:
    // the two flags together suppress the template as well (see ALWAYS_ARGS).
    let wants_progress = matches!(invocation.stage, Stage::DownloadAudio { .. });
    let mut argv: Vec<String> = ALWAYS_ARGS.iter().map(|arg| (*arg).to_string()).collect();
    if !wants_progress {
        argv.push(NO_PROGRESS_ARG.to_string());
    }

    if let Some(browser) = invocation
        .cookies_browser
        .as_deref()
        .map(str::trim)
        .filter(|browser| !browser.is_empty())
    {
        argv.push("--cookies-from-browser".to_string());
        argv.push(browser.to_string());
    }

    if let Some(ffmpeg) = &invocation.ffmpeg {
        argv.push("--ffmpeg-location".to_string());
        argv.push(ffmpeg.to_string_lossy().into_owned());
    }

    match &invocation.stage {
        Stage::Resolve => {
            // `--list-subs` is required, not cosmetic: yt-dlp extracts subtitles
            // lazily and only does it when `writesubtitles` or `listsubtitles`
            // is on. With `--print-json` alone Bilibili reports `subtitles: {}`
            // (design §4.1, measured in §9.4).
            argv.push("--skip-download".to_string());
            argv.push("--list-subs".to_string());
            argv.push("--print-json".to_string());
        }
        Stage::FetchSubtitles { work_dir, tag } => {
            argv.push("--load-info-json".to_string());
            argv.push(work_dir.join("info.json").to_string_lossy().into_owned());
            argv.push("--skip-download".to_string());
            argv.push("--write-subs".to_string());
            argv.push("--sub-langs".to_string());
            argv.push(escape_sub_lang(tag));
            argv.push("--sub-format".to_string());
            argv.push(SUB_FORMAT.to_string());
            argv.push("-o".to_string());
            argv.push(output_template(work_dir));
        }
        Stage::DownloadAudio {
            home_dir,
            temp_dir,
            file_stem,
        } => {
            // No `--no-progress` here: it would silence the template below.
            argv.push("-f".to_string());
            argv.push(AUDIO_FORMAT.to_string());
            argv.push("--progress-template".to_string());
            argv.push(PROGRESS_TEMPLATE.to_string());
            // `--paths` splits the two locations. Both are explicit so the argv
            // stays a constant shape: `home` is where the finished file belongs,
            // `temp` receives the `.part` (measured: yt-dlp downloads into `temp`
            // and moves the finished file into `home` itself).
            argv.push("--paths".to_string());
            argv.push(format!("home:{}", home_dir.to_string_lossy()));
            argv.push("--paths".to_string());
            argv.push(format!("temp:{}", temp_dir.to_string_lossy()));
            // Relative on purpose: an absolute `-o` makes yt-dlp ignore `--paths`.
            argv.push("-o".to_string());
            argv.push(media_output_template(file_stem));
        }
    }

    argv.push(invocation.url.clone());
    argv
}

/// The `-o` template for the audio stage: the exact stem the subtitle carries,
/// plus yt-dlp's extension placeholder.
///
/// `%` is yt-dlp's template escape, so it is doubled. Without that a title
/// containing `%(` would be read as a field and substituted (or fail), which is
/// how a downloaded file would end up under a name that is not the subtitle's.
pub fn media_output_template(file_stem: &str) -> String {
    format!("{}.%(ext)s", file_stem.replace('%', "%%"))
}

/// yt-dlp's output template for files this app manages:
/// `<work_dir>/%(id)s/%(id)s.%(ext)s`, which produces
/// `<work_dir>/<id>/<id>.srt`.
fn output_template(work_dir: &Path) -> String {
    work_dir
        .join("%(id)s")
        .join("%(id)s.%(ext)s")
        .to_string_lossy()
        .into_owned()
}

/// Resolve the bundled `yt-dlp-<target-triple>`.
///
/// Reuses `engine::cli::resolve_bundled_binary`, which owns the whole chain
/// (env override → dev checkout → executable dir → app bundle → PATH). Copying
/// those rules here is how the two engines would drift.
pub fn resolve_path(app: Option<&tauri::AppHandle>) -> PathBuf {
    match crate::engine::cli::resolve_bundled_binary(BINARY, PATH_ENV, app) {
        Ok(path) => path,
        // Resolution only fails if it cannot even build a candidate; the bare
        // name is the same fallback the resolver itself uses.
        Err(error) => {
            tracing::warn!(error = %error, "could not resolve yt-dlp; using the bare name");
            PathBuf::from(BINARY)
        }
    }
}

/// One finished invocation.
#[derive(Debug, Clone)]
pub struct RunOutput {
    /// Every stdout line, in order.
    pub stdout: String,
    /// The last [`STDERR_TAIL_LINES`] stderr lines, joined with `\n`.
    pub stderr: String,
    /// Exit status code, `None` when killed by a signal.
    pub code: Option<i32>,
}

impl RunOutput {
    /// True when the process exited 0. Callers must NOT use this as the "did we
    /// get a subtitle?" test — an unmatched `--sub-langs` exits 0 and writes
    /// nothing (design §4.3, measured in §9.5).
    pub fn success(&self) -> bool {
        self.code == Some(0)
    }
}

/// Spawn yt-dlp and call `on_line` for each stdout line as it arrives.
///
/// stderr is drained concurrently into a bounded ring buffer: the child writes to
/// both pipes, and a full stderr pipe would deadlock it while we read stdout.
/// The child is `kill_on_drop(true)`, so aborting the owning task (a cancel)
/// kills it, and `stdin` is closed.
pub async fn run(
    app: Option<&tauri::AppHandle>,
    invocation: &Invocation,
    mut on_line: impl FnMut(&str) + Send,
) -> Result<RunOutput, ImportError> {
    let program = resolve_path(app);
    let argv = build_argv(invocation);

    tracing::debug!(
        program = %program.display(),
        argv = ?argv,
        "Spawning yt-dlp"
    );

    let mut command = tokio::process::Command::new(&program);
    command
        .args(&argv)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);

    let mut child = command.spawn().map_err(|error| {
        tracing::error!(program = %program.display(), error = %error, "Failed to spawn yt-dlp");
        ImportError::BinaryMissing(format!("{}: {}", program.display(), error))
    })?;

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| ImportError::Other("failed to capture yt-dlp stdout".to_string()))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| ImportError::Other("failed to capture yt-dlp stderr".to_string()))?;

    let stderr_task = tokio::spawn(async move {
        let mut reader = BufReader::new(stderr).lines();
        let mut tail: VecDeque<String> = VecDeque::with_capacity(STDERR_TAIL_LINES);
        while let Ok(Some(line)) = reader.next_line().await {
            if tail.len() == STDERR_TAIL_LINES {
                tail.pop_front();
            }
            tail.push_back(line);
        }
        tail.into_iter().collect::<Vec<_>>().join("\n")
    });

    let mut collected = String::new();
    let mut reader = BufReader::new(stdout).lines();
    while let Some(line) = reader.next_line().await.map_err(|error| {
        tracing::error!(error = %error, "Failed to read yt-dlp stdout");
        ImportError::Other(format!("failed to read yt-dlp stdout: {}", error))
    })? {
        on_line(&line);
        collected.push_str(&line);
        collected.push('\n');
    }

    let status = child.wait().await.map_err(|error| {
        tracing::error!(error = %error, "Failed to wait for yt-dlp");
        ImportError::Other(format!("failed to wait for yt-dlp: {}", error))
    })?;

    let stderr = stderr_task
        .await
        .map_err(|error| ImportError::Other(format!("yt-dlp stderr reader failed: {}", error)))?;

    let output = RunOutput {
        stdout: collected,
        stderr,
        code: status.code(),
    };

    tracing::debug!(
        code = ?output.code,
        stderr_tail = %output.stderr,
        "yt-dlp finished"
    );

    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn resolve_invocation(cookies: Option<&str>, ffmpeg: Option<&str>) -> Invocation {
        Invocation {
            stage: Stage::Resolve,
            url: "https://www.bilibili.com/video/BV1GJ411x7h7".to_string(),
            cookies_browser: cookies.map(str::to_string),
            ffmpeg: ffmpeg.map(PathBuf::from),
        }
    }

    /// Every stage must carry the always-on constant flags — this is the one rule
    /// a per-stage argv could quietly break.
    #[test]
    fn every_stage_carries_the_constant_flags() {
        let dir = PathBuf::from("/tmp/import");
        let invocations = [
            resolve_invocation(None, None),
            Invocation {
                stage: Stage::FetchSubtitles {
                    work_dir: dir.clone(),
                    tag: "zh-Hans".to_string(),
                },
                url: "u".to_string(),
                cookies_browser: None,
                ffmpeg: None,
            },
            Invocation {
                stage: Stage::DownloadAudio {
                    home_dir: dir.clone(),
                    temp_dir: PathBuf::from("/tmp/cache"),
                    file_stem: "Title [BV1]".to_string(),
                },
                url: "u".to_string(),
                cookies_browser: None,
                ffmpeg: None,
            },
        ];

        for invocation in invocations {
            let argv = build_argv(&invocation);
            for flag in ALWAYS_ARGS {
                assert!(argv.contains(&flag.to_string()), "{:?} is missing {flag}", invocation.stage);
            }
            // `--no-progress` is per-stage, so only the quiet stages carry it.
            let quiet = !matches!(invocation.stage, Stage::DownloadAudio { .. });
            assert_eq!(
                argv.contains(&NO_PROGRESS_ARG.to_string()),
                quiet,
                "{:?} got the wrong --no-progress handling",
                invocation.stage
            );
        }
    }

    /// Regression for a measured silent failure: `--no-progress` together with
    /// `--progress-template` makes yt-dlp emit **zero** `PROG` lines, so asking
    /// for both produced a permanently indeterminate download bar. The audio
    /// stage must therefore not carry `--no-progress` at all.
    #[test]
    fn the_audio_stage_never_passes_no_progress() {
        let argv = build_argv(&Invocation {
            stage: Stage::DownloadAudio {
                home_dir: PathBuf::from("/tmp/out"),
                temp_dir: PathBuf::from("/tmp/cache"),
                file_stem: "Title [BV1]".to_string(),
            },
            url: "u".to_string(),
            cookies_browser: None,
            ffmpeg: None,
        });

        assert!(!argv.iter().any(|arg| arg == NO_PROGRESS_ARG), "{argv:?}");
        // Guard against a vacuous pass: it really is the audio stage.
        assert!(argv.iter().any(|arg| arg == "--progress-template"), "{argv:?}");
        assert!(argv.iter().any(|arg| arg == "-f"), "{argv:?}");
    }

    /// ...while the stages that do not want progress still suppress it.
    #[test]
    fn the_quiet_stages_still_suppress_progress() {
        let resolve = build_argv(&resolve_invocation(None, None));
        assert!(resolve.iter().any(|arg| arg == NO_PROGRESS_ARG), "{resolve:?}");

        let fetch = build_argv(&Invocation {
            stage: Stage::FetchSubtitles {
                work_dir: PathBuf::from("/tmp"),
                tag: "zh-CN".to_string(),
            },
            url: "u".to_string(),
            cookies_browser: None,
            ffmpeg: None,
        });
        assert!(fetch.iter().any(|arg| arg == NO_PROGRESS_ARG), "{fetch:?}");
        // A quiet stage has no template to silence.
        assert!(!fetch.iter().any(|arg| arg == "--progress-template"), "{fetch:?}");
    }

    /// The split between the always-on set and `--no-progress` must add up to the
    /// documented quiet-stage list, so a future edit cannot silently drop a flag.
    #[test]
    fn quiet_stage_flags_are_exactly_the_documented_set() {
        let mut rebuilt: Vec<&str> = ALWAYS_ARGS.to_vec();
        rebuilt.push(NO_PROGRESS_ARG);
        assert_eq!(rebuilt, QUIET_ARGS.to_vec());
    }

    /// Regression: `--no-warnings` would hide 412, "needs login" and
    /// region-restriction notices — the very lines the error classifier reads.
    #[test]
    fn never_passes_no_warnings() {
        let argv = build_argv(&resolve_invocation(None, None));
        assert!(!argv.iter().any(|arg| arg == "--no-warnings"), "{argv:?}");
    }

    /// `--convert-subs` would need ffmpeg and is unnecessary: Bilibili serves
    /// SRT (design §2).
    #[test]
    fn never_passes_convert_subs() {
        let argv = build_argv(&Invocation {
            stage: Stage::FetchSubtitles {
                work_dir: PathBuf::from("/tmp"),
                tag: "zh-CN".to_string(),
            },
            url: "u".to_string(),
            cookies_browser: None,
            ffmpeg: None,
        });
        assert!(!argv.iter().any(|arg| arg == "--convert-subs"), "{argv:?}");
        assert!(argv.iter().any(|arg| arg == "--sub-format"), "{argv:?}");
    }

    /// The URL is the last element and is never split or quoted: argv is not a
    /// shell string.
    #[test]
    fn the_url_is_the_last_single_argv_element() {
        let url = "https://www.bilibili.com/video/BV1GJ411x7h7?p=2&spm_id_from=x";
        let mut invocation = resolve_invocation(Some("chrome"), Some("/ffmpeg"));
        invocation.url = url.to_string();
        let argv = build_argv(&invocation);

        assert_eq!(argv.last().map(String::as_str), Some(url));
        assert_eq!(
            argv.iter().filter(|arg| *arg == url).count(),
            1,
            "the URL must appear exactly once, whole"
        );
    }

    #[test]
    fn cookies_flag_appears_only_when_configured() {
        let with = build_argv(&resolve_invocation(Some("firefox"), None));
        let index = with
            .iter()
            .position(|arg| arg == "--cookies-from-browser")
            .expect("flag present");
        assert_eq!(with[index + 1], "firefox");

        for absent in [None, Some(""), Some("   ")] {
            let argv = build_argv(&resolve_invocation(absent, None));
            assert!(
                !argv.iter().any(|arg| arg == "--cookies-from-browser"),
                "{absent:?} must not produce the flag: {argv:?}"
            );
        }
    }

    #[test]
    fn resolve_stage_asks_for_the_subtitle_list_and_the_info_dict() {
        let argv = build_argv(&resolve_invocation(None, None));
        // `--list-subs` is what un-lazies subtitle extraction (design §4.1).
        assert!(argv.iter().any(|arg| arg == "--list-subs"), "{argv:?}");
        assert!(argv.iter().any(|arg| arg == "--print-json"), "{argv:?}");
        assert!(argv.iter().any(|arg| arg == "--skip-download"), "{argv:?}");
    }

    #[test]
    fn fetch_stage_loads_the_info_dict_and_escapes_the_tag() {
        let invocation = Invocation {
            stage: Stage::FetchSubtitles {
                work_dir: PathBuf::from("/tmp/import"),
                tag: "zh.1".to_string(),
            },
            url: "u".to_string(),
            cookies_browser: None,
            ffmpeg: None,
        };
        let argv = build_argv(&invocation);

        assert_eq!(
            argv.iter().position(|arg| arg == "--load-info-json").map(|index| argv[index + 1].as_str()),
            Some("/tmp/import/info.json")
        );
        // `--sub-langs` is a regex, so `.` is escaped.
        assert_eq!(
            argv.iter().position(|arg| arg == "--sub-langs").map(|index| argv[index + 1].as_str()),
            Some("zh\\.1")
        );
        assert!(argv.iter().any(|arg| arg == "--write-subs"), "{argv:?}");
        assert!(argv.iter().any(|arg| arg == "--skip-download"), "{argv:?}");
        // The output template keeps the id as both directory and file name.
        let template = argv
            .iter()
            .position(|arg| arg == "-o")
            .map(|index| argv[index + 1].clone())
            .expect("-o template");
        assert!(template.contains("%(id)s"), "{template}");
        assert!(template.starts_with("/tmp/import/"), "{template}");
    }

    #[test]
    fn escape_sub_lang_keeps_ordinary_tags_untouched() {
        for tag in ["zh-Hans", "zh-Hant", "en-US", "ai-zh", "es-419", "pt_BR"] {
            assert_eq!(escape_sub_lang(tag), tag, "{tag} needs no escaping");
        }
        assert_eq!(escape_sub_lang("zh."), "zh\\.");
        assert_eq!(escape_sub_lang("a+b"), "a\\+b");
        assert_eq!(escape_sub_lang("(zh)"), "\\(zh\\)");
    }

    #[test]
    fn audio_stage_requests_m4a_with_a_progress_template() {
        let argv = build_argv(&Invocation {
            stage: Stage::DownloadAudio {
                home_dir: PathBuf::from("/tmp/out"),
                temp_dir: PathBuf::from("/tmp/cache"),
                file_stem: "Title [BV1]".to_string(),
            },
            url: "u".to_string(),
            cookies_browser: None,
            ffmpeg: Some(PathBuf::from("/bin/ffmpeg")),
        });

        assert_eq!(
            argv.iter().position(|arg| arg == "-f").map(|index| argv[index + 1].as_str()),
            Some(AUDIO_FORMAT)
        );
        assert_eq!(AUDIO_FORMAT, "ba[ext=m4a]/ba/b");
        assert_eq!(
            argv.iter()
                .position(|arg| arg == "--progress-template")
                .map(|index| argv[index + 1].as_str()),
            Some(PROGRESS_TEMPLATE)
        );
        // The template is what `site::progress` parses; pin the exact shape.
        assert_eq!(
            PROGRESS_TEMPLATE,
            "download:PROG %(progress.downloaded_bytes)s %(progress.total_bytes)s %(progress.total_bytes_estimate)s %(progress.status)s"
        );
        // ...and it survives the parser.
        let sample = "PROG 1024 5408198 NA downloading";
        assert!(crate::site::progress::parse_line(sample).is_some());

        // `--ffmpeg-location` is only there for a merge fallback.
        assert_eq!(
            argv.iter()
                .position(|arg| arg == "--ffmpeg-location")
                .map(|index| argv[index + 1].as_str()),
            Some("/bin/ffmpeg")
        );
    }

    /// The audio stage writes the finished media into the output directory under
    /// the same stem as the subtitle, and only the `.part` into the cache.
    ///
    /// Two things are load-bearing and both were measured: `-P` takes literal
    /// paths (a `%(…)s` in one is not substituted), and an **absolute** `-o` makes
    /// yt-dlp ignore `--paths` entirely — so `-o` must stay relative.
    #[test]
    fn the_audio_stage_separates_the_output_dir_from_the_partial_dir() {
        let argv = build_argv(&Invocation {
            stage: Stage::DownloadAudio {
                home_dir: PathBuf::from("/Users/me/Downloads/MonoSub"),
                temp_dir: PathBuf::from("/Users/me/Library/Caches/monosub/url-import/BV1"),
                file_stem: "Title [BV1]".to_string(),
            },
            url: "u".to_string(),
            cookies_browser: None,
            ffmpeg: None,
        });

        // Both paths are explicit, each as its own `--paths` argument.
        let paths: Vec<&str> = argv
            .iter()
            .enumerate()
            .filter(|(_, arg)| *arg == "--paths")
            .map(|(index, _)| argv[index + 1].as_str())
            .collect();
        assert_eq!(
            paths,
            vec![
                "home:/Users/me/Downloads/MonoSub",
                "temp:/Users/me/Library/Caches/monosub/url-import/BV1",
            ]
        );

        // The output template is the shared stem, relative, and single-element.
        let template = argv
            .iter()
            .position(|arg| arg == "-o")
            .map(|index| argv[index + 1].clone())
            .expect("-o template");
        assert_eq!(template, "Title [BV1].%(ext)s");
        assert!(
            !template.starts_with('/'),
            "an absolute -o makes yt-dlp ignore --paths: {template}"
        );
    }

    /// A `%` in the title is yt-dlp's template escape, so it must be doubled or
    /// the downloaded file lands under a name the subtitle does not share.
    #[test]
    fn a_percent_in_the_title_is_escaped_for_yt_dlp() {
        assert_eq!(media_output_template("50% off [BV1]"), "50%% off [BV1].%(ext)s");
        assert_eq!(media_output_template("Title [BV1]"), "Title [BV1].%(ext)s");
        // A template-looking title must not become a field reference.
        assert_eq!(
            media_output_template("literal %(id)s"),
            "literal %%(id)s.%(ext)s"
        );
    }

    /// `--skip-download` and the audio stage are mutually exclusive: asking for
    /// no download while asking for a download would download nothing.
    #[test]
    fn only_the_audio_stage_omits_skip_download() {
        let audio = build_argv(&Invocation {
            stage: Stage::DownloadAudio {
                home_dir: PathBuf::from("/tmp/out"),
                temp_dir: PathBuf::from("/tmp/cache"),
                file_stem: "Title [BV1]".to_string(),
            },
            url: "u".to_string(),
            cookies_browser: None,
            ffmpeg: None,
        });
        assert!(!audio.iter().any(|arg| arg == "--skip-download"), "{audio:?}");
    }

    #[test]
    fn resolve_path_honours_the_env_override() {
        // The shared resolver's env branch is covered in `engine::cli`; this only
        // pins that the URL-import path uses the documented variable name.
        assert_eq!(PATH_ENV, "MONOSUB_YTDLP_PATH");
    }

    /// The bundled binary, when fetched, is discoverable under the exact name the
    /// resolver looks for. Skipped on a fresh clone (`bin/` is gitignored).
    #[test]
    fn dev_checkout_resolves_the_bundled_ytdlp() {
        let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("crate lives in <repo>/src-tauri");
        let candidates =
            crate::engine::cli::binary_candidates_for_tests(BINARY)
                .into_iter()
                .map(|name| repo_root.join("src-tauri").join("resources").join("bin").join(name))
                .collect::<Vec<_>>();

        let Some(found) = candidates.iter().find(|path| path.is_file()) else {
            eprintln!("skip: bundled yt-dlp not present (run ./scripts/build/fetch-ytdlp.sh)");
            return;
        };
        assert!(found.is_file(), "{}", found.display());
        // Executable bit matters: `install -m 755` sets it in the fetch script.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(found).unwrap().permissions().mode();
            assert!(mode & 0o111 != 0, "{} is not executable", found.display());
        }
    }
}
