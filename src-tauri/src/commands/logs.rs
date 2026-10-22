//! Making the log file reachable, and feeding frontend failures into it.
//!
//! The Rust side writes its own log through `crate::logging`; a failure that is
//! only reported to the WebView (`console.error`) is invisible in a release
//! build, because there is no console to open. Two things follow:
//!
//! * [`log_directory`] / [`open_logs_dir`] let the user find the one file the
//!   app writes, from Settings ▸ System.
//! * [`log_frontend_error`] writes a frontend failure into that same file, so a
//!   user report and its cause end up in one place instead of two.

use std::path::PathBuf;

use super::opener::open_directory;

/// Longest message/context this command will write.
///
/// The caller is a browser stack trace or an error string; a pasted 5 MB string
/// would fill the log and rotate away the evidence before it.
const MAX_MESSAGE_CHARS: usize = 4000;
const MAX_CONTEXT_CHARS: usize = 2000;

/// Create `dir` if needed and hand it back, so a path the UI offers to open is
/// always a real directory.
fn ensure_dir(dir: PathBuf) -> Result<PathBuf, String> {
    std::fs::create_dir_all(&dir)
        .map_err(|error| format!("could not create {}: {}", dir.display(), error))?;
    Ok(dir)
}

/// Resolve the log directory and make sure it exists. It is the same directory
/// `crate::logging` writes into, so the path the UI shows is the one that
/// actually contains the file.
fn log_dir() -> Result<PathBuf, String> {
    let dir = crate::logging::log_directory()
        .map_err(|error| format!("could not resolve the log directory: {error}"))?;
    ensure_dir(dir)
}

/// Absolute path of the directory holding the rolling log files.
#[tauri::command]
pub async fn log_directory() -> Result<String, String> {
    let dir = log_dir()?;
    Ok(dir.to_string_lossy().to_string())
}

/// Open the log directory in the system file manager.
#[tauri::command]
pub async fn open_logs_dir() -> Result<(), String> {
    let dir = log_dir()?;
    tracing::info!(dir = %dir.display(), "Opening log directory");
    open_directory(&dir).await.inspect_err(|error| {
        tracing::error!(dir = %dir.display(), error = %error, "Failed to open log directory");
    })
}

/// Truncate on a character boundary — `&s[..n]` would panic on a CJK message.
fn truncate_chars(value: &str, max_chars: usize) -> String {
    if value.chars().count() <= max_chars {
        return value.to_string();
    }
    let mut truncated: String = value.chars().take(max_chars).collect();
    truncated.push('…');
    truncated
}

/// Map the frontend's level word onto a `tracing` level.
///
/// The file layer keeps `info` and above, so an unrecognised word must not fall
/// to `debug` — it would be dropped and the report would vanish. `warn` is the
/// fallback: still visible, and not overstated as an error.
fn frontend_level(level: &str) -> tracing::Level {
    match level.trim().to_ascii_lowercase().as_str() {
        "error" => tracing::Level::ERROR,
        "info" => tracing::Level::INFO,
        _ => tracing::Level::WARN,
    }
}

/// Write one frontend error into the application log.
///
/// `level` is restricted to `error`/`warn`/`info`: the file layer keeps `info`
/// and above, so an unknown level falls back to `warn` rather than being
/// rejected (a frontend that spells the level differently still gets its line)
/// or silently landing below the filter. The log stays at `info` overall —
/// error lines are meant to be searchable in it, not moved to a second file.
///
/// Message and context are truncated, never rejected: losing the tail of a stack
/// trace is better than losing the report.
#[tauri::command]
pub async fn log_frontend_error(
    level: String,
    message: String,
    context: Option<String>,
) -> Result<(), String> {
    let message = truncate_chars(&message, MAX_MESSAGE_CHARS);
    let context = context.map(|value| truncate_chars(&value, MAX_CONTEXT_CHARS));

    match frontend_level(&level) {
        tracing::Level::ERROR => {
            tracing::error!(source = "frontend", context = ?context, "{message}")
        }
        tracing::Level::INFO => tracing::info!(source = "frontend", context = ?context, "{message}"),
        _ => tracing::warn!(source = "frontend", context = ?context, "{message}"),
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncate_chars_keeps_short_values_and_marks_long_ones() {
        assert_eq!(truncate_chars("boom", 10), "boom");
        assert_eq!(truncate_chars("0123456789", 10), "0123456789");
        assert_eq!(truncate_chars("0123456789a", 10), "0123456789…");
    }

    #[test]
    fn truncate_chars_never_splits_a_multibyte_character() {
        // 8 CJK chars are 24 bytes; a byte slice at 4 would panic.
        assert_eq!(truncate_chars("识别失败了啊", 4), "识别失败…");
    }

    #[test]
    fn an_unknown_level_falls_back_to_warn_not_below_the_filter() {
        assert_eq!(frontend_level("error"), tracing::Level::ERROR);
        assert_eq!(frontend_level("ERROR"), tracing::Level::ERROR);
        assert_eq!(frontend_level(" info "), tracing::Level::INFO);
        // `debug` is below the file layer's filter, so it must not be honoured.
        assert_eq!(frontend_level("debug"), tracing::Level::WARN);
        assert_eq!(frontend_level(""), tracing::Level::WARN);
    }

    #[test]
    fn ensure_dir_creates_a_missing_directory_and_is_idempotent() {
        let target = std::env::temp_dir()
            .join(format!("monosub-logs-ensure-{}", std::process::id()))
            .join("nested");
        let _ = std::fs::remove_dir_all(target.parent().unwrap());

        assert_eq!(ensure_dir(target.clone()).unwrap(), target);
        assert!(target.is_dir(), "{target:?}");
        // Opening the panel twice must not fail the second time.
        assert!(ensure_dir(target.clone()).is_ok());
        let _ = std::fs::remove_dir_all(target.parent().unwrap());
    }

    #[test]
    fn the_reported_log_directory_is_absolute() {
        // The platform location is asserted by `crate::logging`; here only that
        // the command resolves to an absolute path the UI can show and open. A
        // host without `HOME`/`APPDATA` cannot resolve one — a skip, not a fail.
        match log_dir() {
            Ok(dir) => assert!(dir.is_absolute(), "{dir:?}"),
            Err(error) => eprintln!("skip: no home directory to resolve the log dir from ({error})"),
        }
    }

    #[cfg(not(target_os = "windows"))]
    #[tokio::test]
    async fn opening_a_missing_directory_is_an_error_not_a_silent_success() {
        let missing = std::env::temp_dir().join(format!(
            "monosub-logs-open-missing-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&missing);
        // No `create_dir_all`: the point is that a bad path is reported.
        // The argv and exit-status check are `commands::opener`'s (it has its
        // own test); this one pins that the log panel still surfaces the error.
        assert!(open_directory(&missing).await.is_err());
    }
}
