mod commands;
mod logging;

use commands::*;

fn main() {
    // Initialize logging system
    if let Err(e) = logging::init_logging() {
        eprintln!("Failed to initialize logging: {}", e);
    }

    tracing::info!("MonoSub application starting...");

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![
            get_video_metadata,
            generate_thumbnail,
            start_transcription,
            pause_transcription,
            cancel_transcription,
            open_in_finder,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
