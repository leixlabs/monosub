//! Opening URLs in the user's default browser.
//!
//! This used to live in `commands/license.rs` alongside the Polar / multi-speaker
//! licence gate. MonoSub is a personal, non-commercial learning project, so the
//! Pro gate was removed and this helper is all that remains of that module.

use std::process::Command;

/// Open `url` with the platform's default handler.
///
/// Validated only for being non-empty; the frontend passes constants or a URL
/// the user explicitly chose to open, so no scheme allow-list is applied.
#[tauri::command]
pub async fn open_external_url(url: String) -> Result<(), String> {
    let target = url.trim();
    if target.is_empty() {
        return Err("URL is required".to_string());
    }

    tracing::info!(url = %target, "Opening external URL");

    #[cfg(target_os = "macos")]
    {
        Command::new("open")
            .arg(target)
            .spawn()
            .map_err(|e| format!("Failed to open URL: {}", e))?;
    }

    #[cfg(target_os = "windows")]
    {
        Command::new("cmd")
            .args(["/C", "start", "", target])
            .spawn()
            .map_err(|e| format!("Failed to open URL: {}", e))?;
    }

    #[cfg(target_os = "linux")]
    {
        Command::new("xdg-open")
            .arg(target)
            .spawn()
            .map_err(|e| format!("Failed to open URL: {}", e))?;
    }

    Ok(())
}
