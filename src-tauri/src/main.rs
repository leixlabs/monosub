// The modules live in `lib.rs` so a second binary (`src/bin/monosub-cli.rs`) can
// share them: a `[[bin]]` is its own crate root and cannot `use crate::engine`.
// `monosub::` is the crate name from Cargo.toml.
use monosub::commands::*;
use monosub::{logging, tray};
use tauri::Manager;

fn main() {
    // Initialize logging system
    if let Err(e) = logging::init_logging() {
        eprintln!("Failed to initialize logging: {}", e);
    }

    tracing::info!("MonoSub application starting...");

    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_notification::init())
        // Clipboard watch: reads only what the user already copied, and only to
        // offer a confirm button (see `site::clipboard`).
        .plugin(tauri_plugin_clipboard_manager::init());

    // Debug-only UI automation bridge (tauri-pilot). Compiled out of release builds.
    #[cfg(debug_assertions)]
    let builder = builder.plugin(tauri_plugin_pilot::init());

    builder
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
            cancel_transcription,
            open_in_finder,
            find_existing_path,
            resolve_paths,
            open_external_url,
            check_github_release_update,
            monosub::site::parse_url_input,
            monosub::site::start_url_import,
            monosub::site::cancel_url_import,
            monosub::site::default_url_import_dir,
            monosub::site::read_clipboard_video,
            list_models,
            resolve_model_plan,
            save_translation_settings,
            log_directory,
            open_logs_dir,
            log_frontend_error,
            config_file_path,
            reveal_config_file,
            download_model,
            cancel_model_download,
            remove_model,
            open_models_dir,
            get_models_dir,
            set_models_dir,
            update_tray_menu,
            update_tray_progress,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app_handle, event| {
            // 处理 macOS Dock 图标点击，将窗口唤到前台
            if let tauri::RunEvent::Reopen { .. } = event {
                if let Some(window) = app_handle.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                    let _ = window.unminimize();
                }
            }
        });
}
