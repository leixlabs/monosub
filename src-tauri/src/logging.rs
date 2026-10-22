use std::path::{Path, PathBuf};
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

/// Stem and extension of the rolling log file name.
///
/// The daily appender is configured with a prefix and a suffix, which makes the
/// files `monosub.<YYYY-MM-DD>.log`. That is deliberately *not* the appender's
/// one-argument default (`monosub.log.<YYYY-MM-DD>`): there the date is the
/// extension, so the file has no `.log` extension at all — the Finder cannot
/// preview it and a log viewer does not recognise it. The suffix also keeps the
/// date in the middle, which is what makes `monosub.2025-01-31.log` sort by day.
pub(crate) const LOG_FILE_STEM: &str = "monosub";
pub(crate) const LOG_FILE_EXTENSION: &str = "log";

/// How many daily files are kept. A week of history is enough to report a
/// problem, and every file is small.
pub(crate) const KEEP_LOG_FILES: usize = 7;

/// Initialize the logging system: one rolling file layer plus stdout.
///
/// The file is the only place a user can still reach after a failure, because
/// the WebView console is invisible in a release build. Nothing is filtered
/// more aggressively than `info` (see the filter below) — error records are
/// meant to be *findable* in the same file, not isolated into a second one.
///
/// Logs are stored in:
/// - macOS: ~/Library/Logs/MonoSub/
/// - Windows: %APPDATA%/MonoSub/logs/
/// - Linux: ~/.local/share/MonoSub/logs/
///
/// Rotation policy:
/// - Daily rotation (`<stem>.<YYYY-MM-DD>.<extension>`), set by the appender
/// - Keep the newest [`KEEP_LOG_FILES`] files; older ones are deleted on start
pub fn init_logging() -> Result<(), Box<dyn std::error::Error>> {
    let log_dir = log_directory()?;

    // Create log directory if it doesn't exist
    std::fs::create_dir_all(&log_dir)?;

    // Daily rolling file whose name keeps the date in the middle:
    // `monosub.<YYYY-MM-DD>.log`. The builder (rather than
    // `rolling::daily`) is what allows a suffix, and the suffix is the whole
    // point — without it the date becomes the extension.
    let file_appender = tracing_appender::rolling::RollingFileAppender::builder()
        .rotation(tracing_appender::rolling::Rotation::DAILY)
        .filename_prefix(LOG_FILE_STEM)
        .filename_suffix(LOG_FILE_EXTENSION)
        .build(&log_dir)?;

    // Prune stale files, counting today's (already created by the appender) as
    // one of the files to keep.
    cleanup_old_logs(&log_dir, KEEP_LOG_FILES)?;

    // Create file layer
    let file_layer = fmt::layer()
        .with_writer(file_appender)
        .with_ansi(false)
        .with_target(true)
        .with_thread_ids(true)
        .with_line_number(true);

    // Create stdout layer (only for development/debug)
    let stdout_layer = fmt::layer().with_target(false).with_thread_ids(false);

    // Configure filter: INFO level by default, can be overridden with RUST_LOG env var.
    // Deliberately not `error`: the file must keep the context around a failure
    // (which model ran, which stage failed) for the error line to be useful.
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    // Initialize tracing subscriber
    tracing_subscriber::registry()
        .with(filter)
        .with(file_layer)
        .with(stdout_layer)
        .init();

    tracing::info!("Logging initialized. Log directory: {:?}", log_dir);

    Ok(())
}

/// Get the log directory path based on the platform
///
/// Shared with `commands::logs`, so the path the UI shows and opens is exactly
/// the one the file layer writes into.
pub(crate) fn log_directory() -> Result<PathBuf, Box<dyn std::error::Error>> {
    let log_dir = if cfg!(target_os = "macos") {
        // macOS: ~/Library/Logs/MonoSub
        let home = std::env::var("HOME")?;
        PathBuf::from(home)
            .join("Library")
            .join("Logs")
            .join("MonoSub")
    } else if cfg!(target_os = "windows") {
        // Windows: %APPDATA%/MonoSub/logs
        let appdata = std::env::var("APPDATA")?;
        PathBuf::from(appdata).join("MonoSub").join("logs")
    } else {
        // Linux: ~/.local/share/MonoSub/logs
        let home = std::env::var("HOME")?;
        PathBuf::from(home)
            .join(".local")
            .join("share")
            .join("MonoSub")
            .join("logs")
    };

    Ok(log_dir)
}

/// The date in a rolling log file name, or `None` when `name` is not one.
///
/// The shape is `<stem>.<YYYY-MM-DD>.<extension>` — the only name the configured
/// [`tracing_appender::rolling::Rotation::DAILY`] appender produces.
fn rolling_log_date(name: &str) -> Option<&str> {
    let rest = name
        .strip_prefix(LOG_FILE_STEM)?
        .strip_prefix('.')?
        .strip_suffix(LOG_FILE_EXTENSION)?
        .strip_suffix('.')?;
    let bytes = rest.as_bytes();
    let is_date = bytes.len() == 10
        && bytes[4] == b'-'
        && bytes[7] == b'-'
        && bytes
            .iter()
            .enumerate()
            .all(|(index, byte)| index == 4 || index == 7 || byte.is_ascii_digit());
    is_date.then_some(rest)
}

/// Clean up old log files, keeping only the most recent `keep_count` files.
///
/// Files are ordered by the date in their *name*, not by mtime: the name is the
/// rotation date, so it is the real log order and does not depend on the
/// filesystem's timestamp granularity. Anything else in the directory is left
/// untouched — this is a shared platform location (`~/Library/Logs`), so a file
/// that merely looks similar is not ours to delete.
fn cleanup_old_logs(
    log_dir: &Path,
    keep_count: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut log_files: Vec<(String, PathBuf)> = std::fs::read_dir(log_dir)?
        .filter_map(|entry| entry.ok())
        .filter_map(|entry| {
            let name = entry.file_name();
            let name = name.to_str()?;
            Some((rolling_log_date(name)?.to_string(), entry.path()))
        })
        .collect();

    // Newest first. A date can only appear once, the name is the whole identity.
    log_files.sort_by(|left, right| right.0.cmp(&left.0));

    for (_, path) in log_files.iter().skip(keep_count) {
        if let Err(error) = std::fs::remove_file(path) {
            // Runs before the tracing subscriber is installed, so this cannot be
            // a `tracing::warn!`.
            eprintln!("Failed to remove old log file {:?}: {}", path, error);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "monosub-logging-{}-{}",
            tag,
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create scratch dir");
        dir
    }

    fn file_names(dir: &Path) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(dir)
            .expect("read scratch dir")
            .filter_map(|entry| entry.ok())
            .filter_map(|entry| entry.file_name().to_str().map(str::to_string))
            .collect();
        names.sort();
        names
    }

    #[test]
    fn rolling_log_date_accepts_the_daily_name_and_rejects_lookalikes() {
        assert_eq!(
            rolling_log_date("monosub.2025-01-31.log"),
            Some("2025-01-31")
        );

        for name in [
            "monosub.log",                  // no date at all
            "monosub.log.2025-01-31",       // the retired shape: date is the extension
            "monosub.2025-01-31",           // no extension
            "monosub.2025-01-31.log.gz",    // rotated archive
            "monosub.2025-1-31.log",        // unpadded
            "monosub2.2025-01-31.log",      // different stem
            "other.2025-01-31.log",         // another application
            "monosub.2025-01-31.LOG",       // wrong case
        ] {
            assert_eq!(rolling_log_date(name), None, "{name}");
        }
    }

    #[test]
    fn cleanup_keeps_the_newest_logs_and_deletes_the_older_ones() {
        let dir = scratch("cleanup");
        for day in 1..=10 {
            let name = format!("monosub.2025-01-{day:02}.log");
            std::fs::write(dir.join(name), b"line\n").unwrap();
        }

        cleanup_old_logs(&dir, 3).unwrap();

        assert_eq!(
            file_names(&dir),
            vec![
                "monosub.2025-01-08.log".to_string(),
                "monosub.2025-01-09.log".to_string(),
                "monosub.2025-01-10.log".to_string(),
            ]
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn cleanup_leaves_files_that_are_not_this_app_s_daily_logs() {
        let dir = scratch("foreign");
        for name in [
            "monosub.2025-01-01.log",
            "monosub.2025-01-02.log",
            "monosub.2025-01-03.log",
            "notes.log",
            "other.2020-01-01.log",
            "monosub.log",
            "monosub.log.2025-01-31",
            "monosub.notes",
        ] {
            std::fs::write(dir.join(name), b"line\n").unwrap();
        }

        cleanup_old_logs(&dir, 1).unwrap();

        // Only our own rolling files are pruned, and only the newest one stays.
        // The retired `monosub.log.<date>` shape is *not* ours any more, so it is
        // left for the user to delete instead of being removed behind their back.
        assert_eq!(
            file_names(&dir),
            vec![
                "monosub.2025-01-03.log".to_string(),
                "monosub.log".to_string(),
                "monosub.log.2025-01-31".to_string(),
                "monosub.notes".to_string(),
                "notes.log".to_string(),
                "other.2020-01-01.log".to_string(),
            ]
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn cleanup_keeps_everything_when_under_the_limit() {
        let dir = scratch("under-limit");
        std::fs::write(dir.join("monosub.2025-01-01.log"), b"line\n").unwrap();

        cleanup_old_logs(&dir, KEEP_LOG_FILES).unwrap();

        assert_eq!(
            file_names(&dir),
            vec!["monosub.2025-01-01.log".to_string()]
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
