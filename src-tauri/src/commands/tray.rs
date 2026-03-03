use tauri::{menu::{Menu, MenuItem}, AppHandle};

#[tauri::command]
pub async fn update_tray_menu(
    app: AppHandle,
    show: String,
    hide: String,
    quit: String,
) -> Result<(), String> {
    let show_item = MenuItem::with_id(&app, "show", &show, true, None::<&str>)
        .map_err(|e| e.to_string())?;
    let hide_item = MenuItem::with_id(&app, "hide", &hide, true, None::<&str>)
        .map_err(|e| e.to_string())?;
    let quit_item = MenuItem::with_id(&app, "quit", &quit, true, None::<&str>)
        .map_err(|e| e.to_string())?;

    let menu = Menu::with_items(&app, &[&show_item, &hide_item, &quit_item])
        .map_err(|e| e.to_string())?;

    if let Some(tray) = app.tray_by_id("main-tray") {
        tray.set_menu(Some(menu)).map_err(|e| e.to_string())?;
    }

    Ok(())
}
