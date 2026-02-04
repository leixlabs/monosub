mod commands;
mod logging;
mod tray;

use commands::*;
use tauri::Manager;
use window_vibrancy::{apply_vibrancy, NSVisualEffectMaterial};

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

            // 为 macOS 应用活力效果 (vibrancy)
            #[cfg(target_os = "macos")]
            {
                let _ = apply_vibrancy(
                    &window,
                    NSVisualEffectMaterial::UnderWindowBackground,
                    None,
                    None,
                );
                let _ = window.set_title_bar_style(tauri::TitleBarStyle::Transparent);
            }

            // 也尝试设置透明背景 (如果尚未设置)
            #[cfg(target_os = "macos")]
            {
                // 注意：在 Tauri 2 中，transparent 属性通常在 tauri.conf.json 中设置
                // 这里我们确保它是激活的
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
            check_dependencies,
            install_dependencies,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
