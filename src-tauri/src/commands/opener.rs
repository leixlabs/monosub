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
//! folder named `模型 目录` needs no escaping and cannot be split.

use std::path::Path;
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

#[cfg(target_os = "macos")]
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

#[cfg(target_os = "windows")]
fn platform_argv(action: OpenerAction, target: OpenerTarget, path: &str) -> (String, Vec<String>) {
    let program = "explorer".to_string();
    match action {
        // `start "" <path>`: `start` treats a quoted first argument as the
        // window title, so the empty title keeps the path from being eaten.
        (OpenerAction::Open, OpenerTarget::File) => {
            ("cmd".to_string(), vec![
                "/C".to_string(),
                "start".to_string(),
                String::new(),
                path.to_string(),
            ])
        }
        (OpenerAction::Open, OpenerTarget::Directory) => (program, vec![path.to_string()]),
        (OpenerAction::Reveal, OpenerTarget::Directory) => (program, vec![path.to_string()]),
        // `/select,<file>` is one token, not two: explorer parses the path out
        // of the same argument.
        (OpenerAction::Reveal, OpenerTarget::File) => {
            (program, vec![format!("/select,{path}")])
        }
    }
}

/// The program and arguments for one open, without running anything.
fn opener_argv(action: OpenerAction, target: OpenerTarget, path: &Path) -> (String, Vec<String>) {
    // Paths reach this module from IPC (already UTF-8) or from Tauri's path
    // resolvers, so the lossy conversion has nothing to lose.
    platform_argv(action, target, &path.to_string_lossy())
}

/// `exit 1`, or the signal description when a code is unavailable.
fn describe_status(status: &std::process::ExitStatus) -> String {
    match status.code() {
        Some(code) => format!("exit {code}"),
        None => status.to_string(),
    }
}

/// Run one open and turn a failure into something a user can read.
async fn run(action: OpenerAction, target: OpenerTarget, path: &Path) -> Result<(), String> {
    let (program, args) = opener_argv(action, target, path);

    let output = Command::new(&program)
        .args(&args)
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
        let (_, args) = opener_argv(OpenerAction::Open, OpenerTarget::Directory, &dir);

        // Revealing a directory selects it inside its parent — measured: Finder
        // lands on the parent with the folder selected. Opening it makes the
        // folder the window's target. So `-R` must not appear.
        assert!(
            !args.iter().any(|arg| arg == "-R"),
            "a directory must be opened, not revealed: {args:?}"
        );
        assert_eq!(args, vec![dir.to_string_lossy().to_string()]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_file_is_revealed_inside_its_parent_directory() {
        let file = temp_entry("existing-file.txt", false);
        let file_str = file.to_string_lossy().to_string();
        let (_, args) = opener_argv(OpenerAction::Reveal, OpenerTarget::File, &file);

        #[cfg(target_os = "macos")]
        assert_eq!(args, vec!["-R".to_string(), file_str]);

        #[cfg(target_os = "windows")]
        assert_eq!(args, vec![format!("/select,{file_str}")]);

        #[cfg(target_os = "linux")]
        assert_eq!(
            args,
            vec![file.parent().unwrap().to_string_lossy().to_string()]
        );

        let _ = std::fs::remove_file(&file);
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
        assert_eq!(args, vec![expected.clone()]);

        // "Reveal it": the path still rides inside exactly one argument.
        let (_, args) = opener_argv(OpenerAction::Reveal, OpenerTarget::File, &path);

        #[cfg(target_os = "macos")]
        assert_eq!(args, vec!["-R".to_string(), expected]);

        #[cfg(target_os = "windows")]
        assert_eq!(args, vec![format!("/select,{expected}")]);

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

    #[cfg(not(target_os = "windows"))]
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
