#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

// The modules live in `lib.rs` so a second binary (`src/bin/monosub-cli.rs`) can
// share them: a `[[bin]]` is its own crate root and cannot `use crate::engine`.
// `monosub::` is the crate name from Cargo.toml.
use monosub::commands::*;
use monosub::{logging, tray};
// `Manager` supplies `get_webview_window`. It used to be macOS-only (title bar
// overlay, Dock reopen), but the single-instance callback below also calls it on
// every platform, so the import is now unconditional.
use tauri::Manager;

fn main() {
    // Initialize logging system
    if let Err(e) = logging::init_logging() {
        eprintln!("Failed to initialize logging: {}", e);
    }

    tracing::info!("MonoSub application starting...");

    // The concurrency limit must exist before the first task acquires it; the
    // frontend re-sends the user's setting on mount, which resizes this one.
    monosub::commands::transcribe::ensure_transcription_limit();

    let builder = tauri::Builder::default()
        // MUST be the first plugin registered. Plugin setup runs before the
        // app's own `setup` callback, so the named mutex is claimed before a
        // second instance can build a window or a tray icon — the two things
        // that made "launch it twice" look like two running apps. (Logging is
        // initialised above this line, so a rejected second launch does leave
        // one extra "application starting" line in the log; that is harmless,
        // and the alternative is duplicating the plugin's platform logic.)
        //
        // The callback runs in the *first* instance, so "open the app again"
        // means "bring the window I already have to the front" — the same
        // thing the tray's Show item does.
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            // A second launch reached us instead of starting a rival process.
            // Logged because the user-visible effect ("my window came back") is
            // otherwise indistinguishable from having clicked the tray icon.
            tracing::info!("single_instance: second launch detected, restoring main window");
            if let Some(window) = app.get_webview_window("main") {
                // `unminimize` first: a minimised window shown without it stays
                // in the taskbar as a strip rather than becoming usable.
                let _ = window.unminimize();
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
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
            // macOS 标题栏使用 Overlay，避免透明标题色异常
            #[cfg(target_os = "macos")]
            {
                // 获取主窗口
                let window = app.get_webview_window("main").unwrap();
                let _ = window.set_title_bar_style(tauri::TitleBarStyle::Overlay);
            }

            // 恢复用户此前授权的目录。沙盒的目录授权只活一个进程，没有这一步
            // 用户每次重启都要重新选一次保存位置。
            monosub::commands::grants::activate_grants(app.handle());

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
            set_transcription_concurrency,
            open_in_finder,
            open_file,
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
            cli_info,
            cuda_status,
            download_model,
            cancel_model_download,
            remove_model,
            open_models_dir,
            get_models_dir,
            set_models_dir,
            authorize_dir,
            update_tray_menu,
            update_tray_progress,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app_handle, event| {
            // 处理 macOS Dock 图标点击，将窗口唤到前台。`RunEvent::Reopen` only
            // exists on macOS — on Windows the tray menu serves the same need, so
            // the arm is compiled out rather than emulated.
            #[cfg(target_os = "macos")]
            if let tauri::RunEvent::Reopen { .. } = event {
                if let Some(window) = app_handle.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                    let _ = window.unminimize();
                }
            }

            // Silence the unused-binding warning on platforms without the arm above.
            #[cfg(not(target_os = "macos"))]
            let _ = (app_handle, event);
        });
}
