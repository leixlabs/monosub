mod commands;
mod logging;
mod tray;

use commands::*;
use tauri::Manager;

fn main() {
    // Initialize logging system
    if let Err(e) = logging::init_logging() {
        eprintln!("Failed to initialize logging: {}", e);
    }

    tracing::info!("MonoSub application starting...");

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_shell::init())
        .setup(|app| {
            // 获取主窗口
            let window = app.get_webview_window("main").unwrap();

            // macOS 标题栏使用 Overlay，避免透明标题色异常
            #[cfg(target_os = "macos")]
            {
                let _ = window.set_title_bar_style(tauri::TitleBarStyle::Overlay);
            }

            // 创建系统托盘
            tray::create_tray(app.handle())?;
            Ok(())
        })
        .on_window_event(|window, event| {
            // 拦截窗口关闭事件，改为隐藏窗口
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                window.hide().unwrap();
                api.prevent_close();
            }
        })
        .invoke_handler(tauri::generate_handler![
            get_video_metadata,
            generate_thumbnail,
            start_transcription,
            pause_transcription,
            cancel_transcription,
            open_in_finder,
            activate_license,
            validate_license,
            deactivate_license,
            open_external_url,
            check_dependencies,
            install_dependencies,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
