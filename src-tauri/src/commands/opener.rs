//! Opening a path in the system file manager — one implementation for every
//! command that needs it.
//!
//! There used to be two, and they disagreed. `commands::video::open_in_finder`
//! passed `open -R` for everything and ignored the child's exit status, so
//! "open the models folder" (Settings ▸ 模型 ▸ 打开模型目录) revealed the *parent*
//! folder with the target selected instead of opening the target, and a failure
//! produced no error at all. `commands::logs::open_directory` did it correctly
//! but was private to the log panel. This module is that correct shape, shared.
//!
//! Two entry points, because they are two different user intents:
//!
//! * [`open_directory`] — "show me what is in here". The directory itself is the
//!   target, so macOS gets `open <dir>`.
//! * [`reveal_in_file_manager`] — "show me this thing". A file is revealed
//!   (`open -R <file>`); a directory is opened, because revealing a directory
//!   only selects it inside its parent, which is not what "show me this folder"
//!   means.
//! * [`open_file`] — "open this file with its default app". A third intent, not
//!   a variant of reveal: the user asked to *read* the subtitle, and revealing it
//!   in Finder leaves them one double-click away from that. macOS gets
//!   `open <file>`, Windows `start`, Linux `xdg-open`.
//!
//! The three defects this fixes, all of them measured:
//!
//! 1. `-R` with a directory: `open -R <dir>` puts Finder in the directory's
//!    *parent* with `<dir>` selected, while `open <dir>` makes `<dir>` the
//!    window's target. `man open`: "-R Reveals the file(s) in the Finder instead
//!    of opening them." Revealing a directory is therefore *by definition* not
//!    opening it.
//! 2. `.spawn()` without waiting: `open` exits 1 and prints
//!    `The file … does not exist.` on stderr, and nobody looked. Now the child is
//!    awaited with `.output()` and a non-zero exit becomes an `Err`.
//! 3. The frontend swallowed that `Err`; `SettingsModelsDir.tsx` now shows it.
//!
//! Every path goes in as its own `argv` element — never a shell string — so a
//! folder named `模型 目录` needs no escaping and cannot be split. Windows
//! `explorer` is the one exception: it parses its switches out of the raw command
//! line, so there the path is quoted by [`quote_for_explorer`] and the fragment is
//! appended verbatim. See that function for the bug this fixes.

use std::path::Path;
#[cfg(not(target_os = "macos"))]
use tokio::process::Command;

/// What the caller wants opened: the directory itself, or the entry located
/// inside its parent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OpenerTarget {
    Directory,
    File,
}

/// The three user intents, which map to different argv on every platform.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OpenerAction {
    /// Open the target itself (a directory, or a file with its default app).
    Open,
    /// Locate a file inside its parent window.
    Reveal,
}

/// Whether a non-zero exit status means the open failed.
///
/// `explorer.exe` exits 1 even when it did open the folder, so on Windows the
/// status cannot decide success and only a launch failure is reported. (Same
/// finding as the log panel's original implementation.) A false "could not
/// open" on a working button would be worse than a silent failure we cannot
/// detect anyway.
#[cfg(any(not(target_os = "macos"), test))]
fn status_decides_success() -> bool {
    cfg!(not(target_os = "windows"))
}

/// Decide how `path` should be presented.
///
/// A path that does not exist is classified as a file, so it goes down the
/// reveal branch and the tool itself reports `The file … does not exist.` — a
/// message that names the path, instead of this module guessing a reason. On
/// macOS/Linux that failure reaches the caller through the exit-status check.
fn classify(path: &Path) -> OpenerTarget {
    if path.is_dir() {
        OpenerTarget::Directory
    } else {
        OpenerTarget::File
    }
}

/// macOS 已经改走 `NSWorkspace`，这里保留的是"如果还 spawn `open` 会怎么传参"的
/// 规格，只被测试用来钉住行为（`open <dir>` / `open -R <file>`）。
#[cfg(target_os = "macos")]
#[cfg_attr(not(test), allow(dead_code))]
fn platform_argv(action: OpenerAction, target: OpenerTarget, path: &str) -> (String, Vec<String>) {
    let program = "open".to_string();
    match (action, target) {
        // `open <dir>`: the window's target becomes the directory.
        // `open <file>`: the file's default app opens it.
        (OpenerAction::Open, _) => (program, vec![path.to_string()]),
        // `open -R <file>`: the parent window opens with the file selected.
        // A directory is still opened, not revealed (see `classify`).
        (OpenerAction::Reveal, OpenerTarget::File) => {
            (program, vec!["-R".to_string(), path.to_string()])
        }
        (OpenerAction::Reveal, OpenerTarget::Directory) => (program, vec![path.to_string()]),
    }
}

#[cfg(target_os = "linux")]
fn platform_argv(action: OpenerAction, target: OpenerTarget, path: &str) -> (String, Vec<String>) {
    let program = "xdg-open".to_string();
    match (action, target) {
        // `xdg-open <dir>` and `xdg-open <file>` both do the obvious thing: a
        // directory opens a file manager, a file opens its default app.
        (OpenerAction::Open, _) => (program, vec![path.to_string()]),
        (OpenerAction::Reveal, OpenerTarget::Directory) => (program, vec![path.to_string()]),
        // No file manager takes "select this" portably, so open the containing
        // directory. A relative bare name has no parent to open; the file
        // itself is the only target left.
        (OpenerAction::Reveal, OpenerTarget::File) => {
            let parent = Path::new(path)
                .parent()
                .filter(|parent| !parent.as_os_str().is_empty())
                .map(|parent| parent.to_string_lossy().to_string())
                .unwrap_or_else(|| path.to_string());
            (program, vec![parent])
        }
    }
}

/// How `explorer` must be quoted on Windows.
///
/// `explorer` does not use `CommandLineToArgvW` rules for its switches: it parses
/// `/select,` out of the raw command line and expects the *path* to carry the
/// quotes. Handing std one argv element `/select,C:\a b.mp4` makes std quote the
/// **whole** token, producing
///
/// ```text
/// explorer "/select,C:\a b.mp4"
/// ```
///
/// where the leading quote now sits before the slash — so the switch is no longer
/// a switch. Measured on Windows 11: explorer ignores it and falls back to its
/// default folder, which is why "打开" on a file whose path contains a space
/// opened 文档 (Documents) instead of the video's directory. The same argv without
/// a space works, which is what made this look intermittent.
///
/// The fix is to build explorer's command line verbatim: the switch stays outside
/// the quotes, only the path is quoted. [`run`] appends these fragments with
/// `raw_arg`, so nothing re-quotes them.
#[cfg(target_os = "windows")]
fn quote_for_explorer(path: &str) -> String {
    // A drive root (`D:\`) cannot contain a space, and `"D:\"` would let the
    // trailing backslash escape the closing quote — so it is passed bare. This
    // is the only path where dropping the backslash would name something else.
    if path.len() == 3 && path.ends_with(":\\") {
        return path.to_string();
    }
    // Any other trailing backslash is redundant for explorer and would escape
    // the closing quote, so it goes.
    let trimmed = path.trim_end_matches('\\');
    let trimmed = if trimmed.is_empty() { path } else { trimmed };
    format!("\"{trimmed}\"")
}

#[cfg(target_os = "windows")]
fn platform_argv(action: OpenerAction, target: OpenerTarget, path: &str) -> (String, Vec<String>) {
    let program = "explorer".to_string();
    match (action, target) {
        // `start "" <path>`: `start` treats a quoted first argument as the
        // window title, so the empty title keeps the path from being eaten. This
        // one goes through std's own quoting (`cmd` does follow its rules), so
        // the elements here are plain paths, not fragments.
        (OpenerAction::Open, OpenerTarget::File) => {
            ("cmd".to_string(), vec![
                "/C".to_string(),
                "start".to_string(),
                String::new(),
                path.to_string(),
            ])
        }
        // Every `explorer` branch below is a **command-line fragment**, already
        // quoted as explorer wants it — see `quote_for_explorer`.
        (OpenerAction::Open, OpenerTarget::Directory) => {
            (program, vec![quote_for_explorer(path)])
        }
        (OpenerAction::Reveal, OpenerTarget::Directory) => {
            (program, vec![quote_for_explorer(path)])
        }
        // `/select,<file>` is one token, not two: explorer parses the path out of
        // the same argument, so the comma must not be separated from the switch
        // and the path must be the only quoted part.
        (OpenerAction::Reveal, OpenerTarget::File) => {
            (program, vec![format!("/select,{}", quote_for_explorer(path))])
        }
    }
}

/// The program and arguments for one open, without running anything.
#[cfg(any(not(target_os = "macos"), test))]
fn opener_argv(action: OpenerAction, target: OpenerTarget, path: &Path) -> (String, Vec<String>) {
    // Paths reach this module from IPC (already UTF-8) or from Tauri's path
    // resolvers, so the lossy conversion has nothing to lose.
    platform_argv(action, target, &path.to_string_lossy())
}

/// `exit 1`, or the signal description when a code is unavailable.
///
/// macOS no longer spawns a child process, so in a release build this only serves
/// the tests that pin the old exit-status wording.
#[cfg(any(not(target_os = "macos"), test))]
#[cfg_attr(not(test), allow(dead_code))]
fn describe_status(status: &std::process::ExitStatus) -> String {
    match status.code() {
        Some(code) => format!("exit {code}"),
        None => status.to_string(),
    }
}

/// Whether `program`'s arguments are raw command-line fragments that must not be
/// re-quoted by std.
///
/// Only `explorer`, and only on Windows — see [`quote_for_explorer`]. `cmd` and
/// the macOS/Linux tools all follow standard argv quoting, so their arguments
/// stay plain.
#[cfg(target_os = "windows")]
fn args_are_raw(program: &str) -> bool {
    program == "explorer"
}

/// macOS: hand the path to the Finder through `NSWorkspace`.
///
/// Under the App Sandbox `/usr/bin/open` cannot reach LaunchServices for a path
/// it did not itself open, and reports `The file couldn't be opened.` — measured
/// on a MAS build, "打开模型目录" was dead. `NSWorkspace.openURL` goes through the
/// app's own connection to the Finder, so the container's folders open normally.
#[cfg(target_os = "macos")]
fn open_with_workspace(action: OpenerAction, target: OpenerTarget, path: &Path) -> Result<(), String> {
    use objc2_app_kit::NSWorkspace;
    use objc2_foundation::{NSArray, NSString, NSURL};

    let text = path
        .to_str()
        .ok_or_else(|| format!("{} is not valid UTF-8", path.display()))?;
    let url = NSURL::fileURLWithPath(&NSString::from_str(text));
    let workspace = NSWorkspace::sharedWorkspace();

    let opened = match (action, target) {
        // "show me this thing": select it inside its parent window.
        (OpenerAction::Reveal, OpenerTarget::File) => {
            let urls = NSArray::from_retained_slice(&[url]);
            // No return value to check: Finder either selects it or does not.
            workspace.activateFileViewerSelectingURLs(&urls);
            true
        }
        // "show me what is in here" / "open this file": openURL covers both.
        _ => workspace.openURL(&url),
    };

    if opened {
        Ok(())
    } else {
        Err(format!("the system refused to open {}", path.display()))
    }
}

/// Run one open and turn a failure into something a user can read.
async fn run(action: OpenerAction, target: OpenerTarget, path: &Path) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    return open_with_workspace(action, target, path);

    #[cfg(not(target_os = "macos"))]
    {
    let (program, args) = opener_argv(action, target, path);

    let mut command = Command::new(&program);
    #[cfg(target_os = "windows")]
    {
        if program == "cmd" {
            command.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
        }
        if args_are_raw(&program) {
            // One verbatim line: `explorer` re-parses its switches from the raw
            // command line, so std's quoting would move the opening quote in
            // front of `/select,` and the switch would stop being a switch.
            command.raw_arg(args.join(" "));
        } else {
            command.args(&args);
        }
    }
    #[cfg(not(target_os = "windows"))]
    command.args(&args);

    let output = command
        .output()
        .await
        .map_err(|error| format!("could not launch {program}: {error}"))?;

    if !status_decides_success() || output.status.success() {
        return Ok(());
    }

    // Display only — never re-parsed, so no quoting is needed here.
    let invocation = format!("{program} {}", args.join(" "));
    let stderr = String::from_utf8_lossy(&output.stderr);
    let detail = stderr.trim();
    let status = describe_status(&output.status);

    Err(if detail.is_empty() {
        format!("{invocation} failed ({status}): no message")
    } else {
        format!("{invocation} failed ({status}): {detail}")
    })
    }
}

/// Open `dir` itself in the system file manager.
///
/// Deliberately not `classify`d: the caller asked for a directory, so a path
/// that somehow is not one is still opened rather than silently revealed in
/// some other folder.
pub(crate) async fn open_directory(dir: &Path) -> Result<(), String> {
    run(OpenerAction::Open, OpenerTarget::Directory, dir).await
}

/// Locate `path` in the system file manager: a file is revealed, a directory is
/// opened.
pub(crate) async fn reveal_in_file_manager(path: &Path) -> Result<(), String> {
    run(OpenerAction::Reveal, classify(path), path).await
}

/// Open a file with its default application.
///
/// A file is the only sensible target here — the caller has a concrete path it
/// wants *read*, not located — so this does not `classify`: a directory passed
/// by mistake opens as a directory, which is still the closest thing to what
/// was asked for.
pub(crate) async fn open_file(path: &Path) -> Result<(), String> {
    run(OpenerAction::Open, OpenerTarget::File, path).await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_entry(name: &str, directory: bool) -> std::path::PathBuf {
        // Unique per test and per process, so a parallel run cannot collide.
        let path =
            std::env::temp_dir().join(format!("monosub-opener-{}-{name}", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_dir_all(&path);
        if directory {
            std::fs::create_dir_all(&path).unwrap();
        } else {
            std::fs::write(&path, b"x").unwrap();
        }
        path
    }

    #[test]
    fn an_existing_directory_is_opened_rather_than_revealed() {
        let dir = temp_entry("existing-dir", true);
        let dir_str = dir.to_string_lossy().to_string();
        let (_, args) = opener_argv(OpenerAction::Open, OpenerTarget::Directory, &dir);

        // Revealing a directory selects it inside its parent — measured: Finder
        // lands on the parent with the folder selected. Opening it makes the
        // folder the window's target. So `-R` must not appear.
        assert!(
            !args.iter().any(|arg| arg == "-R"),
            "a directory must be opened, not revealed: {args:?}"
        );
        // Windows: one raw command-line fragment, quoted around the path only.
        #[cfg(target_os = "windows")]
        assert_eq!(args, vec![format!("\"{dir_str}\"")]);
        #[cfg(not(target_os = "windows"))]
        assert_eq!(args, vec![dir_str]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_file_is_revealed_inside_its_parent_directory() {
        let file = temp_entry("existing-file.txt", false);
        let file_str = file.to_string_lossy().to_string();
        let (_, args) = opener_argv(OpenerAction::Reveal, OpenerTarget::File, &file);

        #[cfg(target_os = "macos")]
        assert_eq!(args, vec!["-R".to_string(), file_str]);

        // `/select,` outside the quotes, the path inside them.
        #[cfg(target_os = "windows")]
        assert_eq!(args, vec![format!("/select,\"{file_str}\"")]);

        #[cfg(target_os = "linux")]
        assert_eq!(
            args,
            vec![file.parent().unwrap().to_string_lossy().to_string()]
        );

        let _ = std::fs::remove_file(&file);
    }

    /// The Windows quoting bug, pinned.
    ///
    /// Measured on Windows 11: `explorer "/select,C:\a b.mp4"` — what std
    /// produces for one argv element containing a space — is not a `/select`
    /// invocation at all. Explorer drops the unrecognised argument and opens its
    /// default folder, so "打开" on `D:\monosub yt-dlp feature.mp4` opened 文档
    /// instead of `D:\`. The switch must therefore sit outside the opening quote.
    #[cfg(target_os = "windows")]
    #[test]
    fn a_path_with_a_space_keeps_the_switch_outside_the_quotes() {
        let path = Path::new("D:\\monosub yt-dlp feature.mp4");
        let (program, args) = opener_argv(OpenerAction::Reveal, OpenerTarget::File, path);

        assert_eq!(program, "explorer");
        assert_eq!(args, vec![r#"/select,"D:\monosub yt-dlp feature.mp4""#]);
        // The failure mode this replaces, stated as an assertion: no fragment may
        // begin with a quote, or explorer stops parsing the switch.
        assert!(
            !args[0].starts_with('"'),
            "the switch must not be swallowed by std's quoting: {args:?}"
        );
    }

    /// A trailing backslash must not escape the closing quote.
    #[cfg(target_os = "windows")]
    #[test]
    fn explorer_quoting_survives_a_trailing_backslash_and_the_drive_root() {
        // `"D:\folder\"` is one unterminated string as far as explorer is
        // concerned, so the backslash has to go.
        assert_eq!(quote_for_explorer(r"D:\my folder\"), r#""D:\my folder""#);
        // The root is the one path where removing the backslash is not cosmetic
        // (and it cannot contain a space), so it stays bare.
        assert_eq!(quote_for_explorer(r"D:\"), r"D:\");
        assert_eq!(quote_for_explorer(r"C:\plain"), r#""C:\plain""#);
    }

    #[test]
    fn classify_reads_the_filesystem_and_treats_a_missing_path_as_a_file() {
        let dir = temp_entry("classify-dir", true);
        let file = temp_entry("classify-file.txt", false);
        let missing = std::env::temp_dir().join(format!(
            "monosub-opener-{}-classify-missing",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&missing);
        let _ = std::fs::remove_dir_all(&missing);

        assert_eq!(classify(&dir), OpenerTarget::Directory);
        assert_eq!(classify(&file), OpenerTarget::File);
        // Missing ⇒ File ⇒ the reveal branch, so the tool's own
        // "The file … does not exist." becomes the error we report. Guessing
        // "Directory" here would open the nearest existing parent instead.
        assert_eq!(classify(&missing), OpenerTarget::File);

        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::remove_file(&file);
    }

    #[test]
    fn a_path_with_spaces_and_cjk_stays_exactly_one_argument() {
        // No shell is involved, so spaces need no escaping — but the path must
        // still arrive as one element rather than being split on the space.
        let path = std::env::temp_dir().join("模型 目录 with spaces");
        let expected = path.to_string_lossy().to_string();

        // "Open it": every platform passes the path itself, verbatim, once.
        let (_, args) = opener_argv(OpenerAction::Open, OpenerTarget::Directory, &path);
        #[cfg(target_os = "windows")]
        assert_eq!(args, vec![format!("\"{expected}\"")]);
        #[cfg(not(target_os = "windows"))]
        assert_eq!(args, vec![expected.clone()]);

        // "Reveal it": the path still rides inside exactly one argument.
        let (_, args) = opener_argv(OpenerAction::Reveal, OpenerTarget::File, &path);

        #[cfg(target_os = "macos")]
        assert_eq!(args, vec!["-R".to_string(), expected]);

        #[cfg(target_os = "windows")]
        assert_eq!(args, vec![format!("/select,\"{expected}\"")]);

        #[cfg(target_os = "linux")]
        assert_eq!(
            args,
            vec![path.parent().unwrap().to_string_lossy().to_string()]
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn a_bare_file_name_on_linux_falls_back_to_opening_the_file() {
        // No parent directory to open, so the file itself is the only target.
        let (_, args) = opener_argv(OpenerAction::Reveal, OpenerTarget::File, Path::new("clip.mp3"));
        assert_eq!(args, vec!["clip.mp3".to_string()]);
    }

    #[test]
    fn a_file_is_opened_with_its_default_app() {
        // The third intent: "open this subtitle" is not "reveal it in Finder".
        // Revealing leaves the user one double-click away from reading the file
        // they just asked to read.
        let file = temp_entry("open-with-app.txt", false);
        let file_str = file.to_string_lossy().to_string();
        let (program, args) = opener_argv(OpenerAction::Open, OpenerTarget::File, &file);

        #[cfg(target_os = "macos")]
        {
            assert_eq!(program, "open");
            assert_eq!(args, vec![file_str]);
        }

        #[cfg(target_os = "windows")]
        {
            // `start` eats its first quoted argument as a window title, so the
            // empty title must be present and must precede the path.
            assert_eq!(program, "cmd");
            assert_eq!(args, vec!["/C", "start", "", file_str.as_str()]);
        }

        #[cfg(target_os = "linux")]
        {
            assert_eq!(program, "xdg-open");
            assert_eq!(args, vec![file_str]);
        }

        // The reveal branch must stay distinct — same path, different argv.
        let (_, reveal_args) = opener_argv(OpenerAction::Reveal, OpenerTarget::File, &file);
        assert_ne!(reveal_args, args);

        let _ = std::fs::remove_file(&file);
    }

    #[test]
    fn the_exit_status_policy_is_pinned_per_platform() {
        // Windows cannot tell a successful open from a failed one by exit code,
        // so a flip here would either silence a real failure on macOS/Linux or
        // raise a false one on Windows.
        assert_eq!(status_decides_success(), !cfg!(target_os = "windows"));
    }

    /// Only `explorer` gets verbatim fragments; `cmd` and the POSIX tools keep
    /// plain argv elements, so their existing quoting must not change.
    #[cfg(target_os = "windows")]
    #[test]
    fn only_explorer_takes_raw_command_line_fragments() {
        assert!(args_are_raw("explorer"));
        assert!(!args_are_raw("cmd"));
    }

    /// Manual smoke test: actually launches explorer.
    ///
    /// `MONOSUB_OPENER_PROBE=<path> cargo test --lib -- --ignored --nocapture
    /// opener_probe`, then look at which folder Windows opened.
    #[cfg(target_os = "windows")]
    #[tokio::test]
    #[ignore = "launches the real file manager; run by hand"]
    async fn opener_probe() {
        let path = std::env::var("MONOSUB_OPENER_PROBE").expect("MONOSUB_OPENER_PROBE");
        let path = Path::new(&path);
        println!("argv = {:?}", opener_argv(OpenerAction::Reveal, classify(path), path));
        println!("result = {:?}", reveal_in_file_manager(path).await);
    }

    /// macOS 版不再启动 `open`，而是问 NSWorkspace，所以这里只要求"失败要说出路径"。
    #[cfg(target_os = "macos")]
    #[tokio::test]
    async fn opening_a_missing_directory_reports_the_failure_instead_of_swallowing_it() {
        let missing = std::env::temp_dir().join(format!(
            "monosub-opener-{}-open-missing",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&missing);

        let error = open_directory(&missing)
            .await
            .expect_err("a missing directory must not report success");
        assert!(
            error.contains(&missing.to_string_lossy().to_string()),
            "the error must name the path that failed: {error}"
        );
    }

    #[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
    #[tokio::test]
    async fn opening_a_missing_directory_reports_the_failure_instead_of_swallowing_it() {
        let missing = std::env::temp_dir().join(format!(
            "monosub-opener-{}-open-missing",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&missing);

        let error = open_directory(&missing)
            .await
            .expect_err("a missing directory must not report success");
        let (program, _) = opener_argv(OpenerAction::Open, OpenerTarget::Directory, &missing);
        assert!(
            error.starts_with(&program),
            "the error must name the tool that failed: {error}"
        );
        assert!(
            error.contains("failed (exit") || error.starts_with("could not launch"),
            "the error must say how it failed: {error}"
        );
    }
}
