use std::path::PathBuf;
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

/// Initialize the logging system with file rotation and size limits
///
/// Logs are stored in:
/// - macOS: ~/Library/Logs/MonoSub/
/// - Windows: %APPDATA%/MonoSub/logs/
/// - Linux: ~/.local/share/MonoSub/logs/
///
/// Rotation policy:
/// - Daily rotation
/// - Keep last 7 log files
/// - Each file max ~10MB (enforced by daily rotation, not hard limit)
pub fn init_logging() -> Result<(), Box<dyn std::error::Error>> {
    let log_dir = get_log_directory()?;

    // Create log directory if it doesn't exist
    std::fs::create_dir_all(&log_dir)?;

    // Create a rolling file appender (daily rotation)
    let file_appender = tracing_appender::rolling::daily(&log_dir, "monosub.log");

    // Also keep last 7 days of logs by manual cleanup
    cleanup_old_logs(&log_dir, 7)?;

    // Create file layer
    let file_layer = fmt::layer()
        .with_writer(file_appender)
        .with_ansi(false)
        .with_target(true)
        .with_thread_ids(true)
        .with_line_number(true);

    // Create stdout layer (only for development/debug)
    let stdout_layer = fmt::layer().with_target(false).with_thread_ids(false);

    // Configure filter: INFO level by default, can be overridden with RUST_LOG env var
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
fn get_log_directory() -> Result<PathBuf, Box<dyn std::error::Error>> {
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

/// Clean up old log files, keeping only the most recent N files
fn cleanup_old_logs(
    log_dir: &PathBuf,
    keep_count: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut log_files: Vec<_> = std::fs::read_dir(log_dir)?
        .filter_map(|entry| entry.ok())
        .filter(|entry| {
            entry
                .path()
                .extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| ext == "log")
                .unwrap_or(false)
        })
        .collect();

    // Sort by modification time (newest first)
    log_files.sort_by_key(|entry| entry.metadata().and_then(|m| m.modified()).ok());
    log_files.reverse();

    // Delete old files beyond keep_count
    for entry in log_files.iter().skip(keep_count) {
        if let Err(e) = std::fs::remove_file(entry.path()) {
            eprintln!("Failed to remove old log file {:?}: {}", entry.path(), e);
        }
    }

    Ok(())
}
