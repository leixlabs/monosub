//! Directory grants: remembering that the user allowed a folder.
//!
//! The open panel in the frontend hands the app access to a folder **for this
//! process only**. On macOS that grant can be captured as a security-scoped
//! bookmark and replayed at every launch ([`activate_grants`]), which is what
//! makes "choose a save folder once" actually mean once instead of "every time
//! you restart".
//!
//! Platforms without the App Sandbox need none of it: a path is already
//! permission, so [`authorize_dir`] answers `Ok(())` and stores nothing.

use std::path::PathBuf;
use tauri::AppHandle;

/// Remember access to `path`, which the user just picked in an open panel.
///
/// Called by the frontend immediately after every directory picker — the grant
/// exists only while the panel's result is being handled, so this is the one
/// moment the bookmark can be created.
#[tauri::command]
pub async fn authorize_dir(app: AppHandle, path: String) -> Result<(), String> {
    let dir = PathBuf::from(path.trim());
    if !dir.is_dir() {
        return Err(format!("not a directory: {}", dir.display()));
    }

    // 未沙盒（官网 DMG 版）：路径本身就是权限，没有任何东西需要记住。
    if !crate::paths::is_sandboxed() {
        return Ok(());
    }

    let Some(bookmark) = crate::bookmark::create(&dir)? else {
        return Ok(());
    };

    let mut settings = crate::model_dir::load_settings(&app);
    settings.remember_grant(&dir, bookmark);
    crate::model_dir::save_settings(&app, &settings)?;
    tracing::info!(dir = %dir.display(), "remembered access to a user-granted directory");
    Ok(())
}

/// Restore every remembered grant at launch.
///
/// A sandbox grant does not survive the process, so this runs once during setup:
/// without it every custom folder would need the panel again after a restart.
/// A failure is logged, not fatal — the write path still checks writability and
/// reports the folder by name, which is what lets the UI ask again for *that*
/// folder instead of failing silently.
pub fn activate_grants(app: &AppHandle) {
    let settings = crate::model_dir::load_settings(app);
    if settings.dir_grants.is_empty() {
        return;
    }

    for (dir, outcome) in crate::bookmark::activate(&settings.dir_grants) {
        if let Err(error) = outcome {
            tracing::warn!(
                dir = %dir,
                error = %error,
                "a remembered folder grant is no longer usable; the folder will have to be re-authorized"
            );
        }
    }
}
