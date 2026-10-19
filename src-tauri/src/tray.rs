//! The menu-bar (tray) icon.
//!
//! The menu itself is rendered by [`crate::commands::tray::refresh`] from the
//! state the frontend pushes, so this module only builds the icon once and wires
//! the events.
//!
//! Interaction shape:
//!
//! - **Left click** opens the menu, which leads with the current transcription
//!   progress and ends with Show / Hide / Quit. It does **not** toggle the
//!   window: a left click is how the user asks "how is it going?", and answering
//!   with a window that may already be visible would hide it.
//! - **Right click** shows the same menu (the platform default), so both buttons
//!   behave the same and the progress is always one click away.
//!
//! `show_menu_on_left_click(true)` is the load-bearing part: the default already
//! makes a left click open the menu, but it is stated explicitly because the
//! previous version turned it off and toggled the window instead.

use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    AppHandle, Manager, Runtime,
};

use crate::commands::tray::refresh;

/// Builds the menu-bar icon and shows its menu on either mouse button.
pub fn create_tray<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    // The initial menu is a placeholder: `refresh` re-renders it from the shared
    // state (labels + whatever progress the frontend has pushed) on every update,
    // and calling it right after `build` means the very first render already goes
    // through that single path.
    let show_item = MenuItem::with_id(app, "show", "Show Window", true, None::<&str>)?;
    let hide_item = MenuItem::with_id(app, "hide", "Hide Window", true, None::<&str>)?;
    let quit_item = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show_item, &hide_item, &quit_item])?;

    TrayIconBuilder::with_id("main-tray")
        .menu(&menu)
        .show_menu_on_left_click(true)
        .icon(app.default_window_icon().unwrap().clone())
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show" => show_window(app),
            "hide" => {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.hide();
                }
            }
            "quit" => {
                app.exit(0);
            }
            _ => {}
        })
        // No `on_tray_icon_event` handler: the window is no longer toggled from
        // the icon itself, since a left click now opens the menu instead.
        .build(app)?;

    // Render from the shared state immediately, so a menu built before the
    // frontend's first push still has the right shape.
    let _ = refresh(app);

    Ok(())
}

/// Bring the main window forward, restoring it if it was hidden or minimised.
fn show_window<R: Runtime>(app: &AppHandle<R>) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.set_focus();
        let _ = window.unminimize();
    }
}
