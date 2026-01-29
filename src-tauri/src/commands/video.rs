use serde::{Deserialize, Serialize};
use std::process::Command;
use std::path::Path;

// Audio file extensions
const AUDIO_EXTENSIONS: &[&str] = &["mp3", "wav", "flac", "aac", "m4a", "ogg", "wma"];

#[derive(Debug, Serialize, Deserialize)]
pub struct VideoMetadata {
    pub duration: f64,
    pub size: u64,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub is_audio: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ThumbnailResult {
    pub thumbnail_base64: String,
}

/// Check if file is an audio file based on extension
fn is_audio_file(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| AUDIO_EXTENSIONS.contains(&ext.to_lowercase().as_str()))
        .unwrap_or(false)
}

/// Get media metadata using ffprobe (supports both video and audio)
#[tauri::command]
pub async fn get_video_metadata(path: String) -> Result<VideoMetadata, String> {
    let file_path = Path::new(&path);

    if !file_path.exists() {
        return Err("File does not exist".to_string());
    }

    let is_audio = is_audio_file(file_path);

    // Get file size
    let metadata = std::fs::metadata(&path)
        .map_err(|e| format!("Failed to get file metadata: {}", e))?;
    let size = metadata.len();

    // Get media duration using ffprobe
    let output = Command::new("ffprobe")
        .args(&[
            "-v", "error",
            "-show_entries", "format=duration",
            "-show_entries", "stream=width,height",
            "-of", "json",
            &path,
        ])
        .output()
        .map_err(|e| format!("Failed to execute ffprobe: {}", e))?;

    if !output.status.success() {
        return Err("ffprobe command failed".to_string());
    }

    let json_str = String::from_utf8(output.stdout)
        .map_err(|e| format!("Failed to parse ffprobe output: {}", e))?;

    // Parse JSON manually for simplicity
    let duration = extract_duration(&json_str)
        .ok_or_else(|| "Failed to extract duration".to_string())?;

    // For audio files, width and height will be None
    let (width, height) = if is_audio {
        (None, None)
    } else {
        let (w, h) = extract_resolution(&json_str)
            .ok_or_else(|| "Failed to extract resolution".to_string())?;
        (Some(w), Some(h))
    };

    Ok(VideoMetadata {
        duration,
        size,
        width,
        height,
        is_audio,
    })
}

// Default audio icon as SVG (converted to data URI)
fn get_audio_icon_svg() -> String {
    format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="240" height="135" viewBox="0 0 240 135">
        <rect fill="#{}" width="240" height="135"/>
        <circle cx="120" cy="55" r="32" fill="#{}"/>
        <polygon points="110,42 110,68 130,55" fill="white"/>
        <text x="120" y="105" text-anchor="middle" fill="#{}" font-size="12">Audio</text>
        </svg>"##,
        "1a1a2e", "6366f1", "9ca3af"
    )
}

/// Generate thumbnail for media file (video: frame extract, audio: default icon)
#[tauri::command]
pub async fn generate_thumbnail(path: String) -> Result<ThumbnailResult, String> {
    let file_path = Path::new(&path);

    // For audio files, return a default audio icon
    if is_audio_file(file_path) {
        let svg_content = get_audio_icon_svg();
        let svg_base64 = base64_encode(svg_content.as_bytes());
        return Ok(ThumbnailResult {
            thumbnail_base64: format!("data:image/svg+xml;base64,{}", svg_base64),
        });
    }

    // For video files, generate thumbnail using ffmpeg
    let temp_dir = std::env::temp_dir();
    let uuid = uuid::Uuid::new_v4();
    let thumbnail_path = temp_dir.join(format!("thumb_{}.jpg", uuid));

    // Generate thumbnail at 3 seconds
    let output = Command::new("ffmpeg")
        .args(&[
            "-i", &path,
            "-ss", "00:00:03",
            "-vframes", "1",
            "-vf", "scale=240:-1",
            "-y",
            thumbnail_path.to_str().unwrap(),
        ])
        .output()
        .map_err(|e| format!("Failed to execute ffmpeg: {}", e))?;

    if !output.status.success() {
        return Err("ffmpeg command failed".to_string());
    }

    // Read thumbnail and encode as base64
    let image_data = std::fs::read(&thumbnail_path)
        .map_err(|e| format!("Failed to read thumbnail: {}", e))?;

    let base64_image = base64_encode(&image_data);

    // Clean up temp file
    let _ = std::fs::remove_file(&thumbnail_path);

    Ok(ThumbnailResult {
        thumbnail_base64: format!("data:image/jpeg;base64,{}", base64_image),
    })
}

// Helper functions
fn extract_duration(json: &str) -> Option<f64> {
    // Simple JSON parsing for duration
    if let Some(start) = json.find("\"duration\":") {
        let rest = &json[start + 11..];
        if let Some(end) = rest.find(&[',', '}'][..]) {
            let duration_str = rest[..end].trim().trim_matches('"');
            return duration_str.parse::<f64>().ok();
        }
    }
    None
}

fn extract_resolution(json: &str) -> Option<(u32, u32)> {
    let width = if let Some(start) = json.find("\"width\":") {
        let rest = &json[start + 8..];
        if let Some(end) = rest.find(&[',', '}'][..]) {
            rest[..end].trim().parse::<u32>().ok()
        } else {
            None
        }
    } else {
        None
    }?;

    let height = if let Some(start) = json.find("\"height\":") {
        let rest = &json[start + 9..];
        if let Some(end) = rest.find(&[',', '}'][..]) {
            rest[..end].trim().parse::<u32>().ok()
        } else {
            None
        }
    } else {
        None
    }?;

    Some((width, height))
}

fn base64_encode(data: &[u8]) -> String {
    const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut result = String::new();

    for chunk in data.chunks(3) {
        let mut buf = [0u8; 3];
        for (i, &byte) in chunk.iter().enumerate() {
            buf[i] = byte;
        }

        let b1 = (buf[0] >> 2) as usize;
        let b2 = (((buf[0] & 0x03) << 4) | (buf[1] >> 4)) as usize;
        let b3 = (((buf[1] & 0x0f) << 2) | (buf[2] >> 6)) as usize;
        let b4 = (buf[2] & 0x3f) as usize;

        result.push(CHARSET[b1] as char);
        result.push(CHARSET[b2] as char);
        result.push(if chunk.len() > 1 { CHARSET[b3] as char } else { '=' });
        result.push(if chunk.len() > 2 { CHARSET[b4] as char } else { '=' });
    }

    result
}

/// Open file in Finder (macOS) or File Explorer (Windows/Linux)
#[tauri::command]
pub async fn open_in_finder(path: String) -> Result<(), String> {
    let file_path = Path::new(&path);

    if !file_path.exists() {
        return Err("File does not exist".to_string());
    }

    let parent_dir = file_path.parent()
        .ok_or_else(|| "Failed to get parent directory".to_string())?;

    #[cfg(target_os = "macos")]
    {
        Command::new("open")
            .arg("-R")
            .arg(&path)
            .spawn()
            .map_err(|e| format!("Failed to open Finder: {}", e))?;
    }

    #[cfg(target_os = "windows")]
    {
        Command::new("explorer")
            .arg("/select,")
            .arg(&path)
            .spawn()
            .map_err(|e| format!("Failed to open Explorer: {}", e))?;
    }

    #[cfg(target_os = "linux")]
    {
        Command::new("xdg-open")
            .arg(parent_dir)
            .spawn()
            .map_err(|e| format!("Failed to open file manager: {}", e))?;
    }

    Ok(())
}
