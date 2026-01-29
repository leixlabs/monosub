use serde::{Deserialize, Serialize};
use std::process::Command;
use std::path::Path;
use tauri::AppHandle;

// Audio file extensions
const AUDIO_EXTENSIONS: &[&str] = &["mp3", "wav", "flac", "aac", "m4a", "ogg", "wma"];

/// Get ffmpeg binary path
fn get_ffmpeg_path(app: Option<&AppHandle>) -> String {
    // 在打包后，externalBin 文件位于 Contents/MacOS/ 目录
    // 与主可执行文件在同一目录
    if let Some(_app) = app {
        // 尝试在当前可执行文件目录查找
        if let Ok(exe_path) = std::env::current_exe() {
            if let Some(exe_dir) = exe_path.parent() {
                let ffmpeg_bin = exe_dir.join("ffmpeg");
                if ffmpeg_bin.exists() {
                    return ffmpeg_bin.to_string_lossy().to_string();
                }
            }
        }
    }

    // 回退到系统 ffmpeg
    "ffmpeg".to_string()
}

/// Get target architecture suffix for binary files
fn get_target_suffix() -> String {
    #[cfg(all(target_arch = "aarch64", target_os = "macos"))]
    return "aarch64-apple-darwin".to_string();

    #[cfg(all(target_arch = "x86_64", target_os = "macos"))]
    return "x86_64-apple-darwin".to_string();

    #[cfg(all(target_arch = "x86_64", target_os = "linux"))]
    return "x86_64-unknown-linux-gnu".to_string();

    #[cfg(all(target_arch = "aarch64", target_os = "linux"))]
    return "aarch64-unknown-linux-gnu".to_string();

    #[cfg(all(target_arch = "x86_64", target_os = "windows"))]
    return "x86_64-pc-windows-msvc".to_string();

    // Fallback for other architectures
    #[cfg(not(any(
        all(target_arch = "aarch64", target_os = "macos"),
        all(target_arch = "x86_64", target_os = "macos"),
        all(target_arch = "x86_64", target_os = "linux"),
        all(target_arch = "aarch64", target_os = "linux"),
        all(target_arch = "x86_64", target_os = "windows")
    )))]
    return String::new();
}

/// Get ffmpeg binary path for Python subprocess (checks bundled and system locations)
pub fn get_ffmpeg_path_for_python(app: Option<&AppHandle>) -> String {
    let target_suffix = get_target_suffix();

    // 1. 尝试开发环境的资源目录 (src-tauri/resources/bin/ffmpeg-{arch})
    if let Ok(current_dir) = std::env::current_dir() {
        let dev_ffmpeg = current_dir
            .join("src-tauri")
            .join("resources")
            .join("bin")
            .join(format!("ffmpeg-{}", target_suffix));

        if dev_ffmpeg.exists() {
            tracing::debug!("Found dev ffmpeg: {:?}", dev_ffmpeg);
            return dev_ffmpeg.to_string_lossy().to_string();
        }
    }

    // 2. 尝试打包后的二进制文件（在可执行文件同目录）
    if let Some(_app) = app {
        if let Ok(exe_path) = std::env::current_exe() {
            if let Some(exe_dir) = exe_path.parent() {
                // Tauri externalBin 会将 ffmpeg-{arch} 复制到可执行文件目录
                let ffmpeg_with_suffix = exe_dir.join(format!("ffmpeg-{}", target_suffix));
                if ffmpeg_with_suffix.exists() {
                    tracing::debug!("Found bundled ffmpeg: {:?}", ffmpeg_with_suffix);
                    return ffmpeg_with_suffix.to_string_lossy().to_string();
                }

                // 也尝试没有后缀的版本（以防配置不同）
                let ffmpeg_plain = exe_dir.join("ffmpeg");
                if ffmpeg_plain.exists() {
                    tracing::debug!("Found bundled ffmpeg (plain): {:?}", ffmpeg_plain);
                    return ffmpeg_plain.to_string_lossy().to_string();
                }
            }
        }
    }

    // 3. 尝试常见的系统安装路径
    let common_paths = [
        "/opt/homebrew/bin/ffmpeg",  // Homebrew on Apple Silicon
        "/usr/local/bin/ffmpeg",     // Homebrew on Intel Mac / Linux
        "/usr/bin/ffmpeg",           // System package managers
    ];

    for path in &common_paths {
        if std::path::Path::new(path).exists() {
            tracing::debug!("Found system ffmpeg: {}", path);
            return path.to_string();
        }
    }

    // 4. 最后回退到 PATH 中的 ffmpeg
    tracing::warn!("Falling back to 'ffmpeg' in PATH");
    "ffmpeg".to_string()
}

/// Get ffprobe binary path
fn get_ffprobe_path(app: Option<&AppHandle>) -> String {
    // 在打包后，externalBin 文件位于 Contents/MacOS/ 目录
    // 与主可执行文件在同一目录
    if let Some(_app) = app {
        // 尝试在当前可执行文件目录查找
        if let Ok(exe_path) = std::env::current_exe() {
            if let Some(exe_dir) = exe_path.parent() {
                let ffprobe_bin = exe_dir.join("ffprobe");
                if ffprobe_bin.exists() {
                    return ffprobe_bin.to_string_lossy().to_string();
                }
            }
        }
    }

    // 回退到系统 ffprobe
    "ffprobe".to_string()
}

/// Get ffprobe binary path for Python subprocess (checks bundled and system locations)
pub fn get_ffprobe_path_for_python(app: Option<&AppHandle>) -> String {
    let target_suffix = get_target_suffix();

    // 1. 尝试开发环境的资源目录 (src-tauri/resources/bin/ffprobe-{arch})
    if let Ok(current_dir) = std::env::current_dir() {
        let dev_ffprobe = current_dir
            .join("src-tauri")
            .join("resources")
            .join("bin")
            .join(format!("ffprobe-{}", target_suffix));

        if dev_ffprobe.exists() {
            tracing::debug!("Found dev ffprobe: {:?}", dev_ffprobe);
            return dev_ffprobe.to_string_lossy().to_string();
        }
    }

    // 2. 尝试打包后的二进制文件（在可执行文件同目录）
    if let Some(_app) = app {
        if let Ok(exe_path) = std::env::current_exe() {
            if let Some(exe_dir) = exe_path.parent() {
                // Tauri externalBin 会将 ffprobe-{arch} 复制到可执行文件目录
                let ffprobe_with_suffix = exe_dir.join(format!("ffprobe-{}", target_suffix));
                if ffprobe_with_suffix.exists() {
                    tracing::debug!("Found bundled ffprobe: {:?}", ffprobe_with_suffix);
                    return ffprobe_with_suffix.to_string_lossy().to_string();
                }

                // 也尝试没有后缀的版本（以防配置不同）
                let ffprobe_plain = exe_dir.join("ffprobe");
                if ffprobe_plain.exists() {
                    tracing::debug!("Found bundled ffprobe (plain): {:?}", ffprobe_plain);
                    return ffprobe_plain.to_string_lossy().to_string();
                }
            }
        }
    }

    // 3. 尝试常见的系统安装路径
    let common_paths = [
        "/opt/homebrew/bin/ffprobe",  // Homebrew on Apple Silicon
        "/usr/local/bin/ffprobe",     // Homebrew on Intel Mac / Linux
        "/usr/bin/ffprobe",           // System package managers
    ];

    for path in &common_paths {
        if std::path::Path::new(path).exists() {
            tracing::debug!("Found system ffprobe: {}", path);
            return path.to_string();
        }
    }

    // 4. 最后回退到 PATH 中的 ffprobe
    tracing::warn!("Falling back to 'ffprobe' in PATH");
    "ffprobe".to_string()
}

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
pub async fn get_video_metadata(app: AppHandle, path: String) -> Result<VideoMetadata, String> {
    tracing::info!(path = %path, "Getting video metadata");
    let file_path = Path::new(&path);

    if !file_path.exists() {
        tracing::error!(path = %path, "File does not exist");
        return Err("File does not exist".to_string());
    }

    let is_audio = is_audio_file(file_path);
    tracing::debug!(path = %path, is_audio = is_audio, "Detected file type");

    // Get file size
    let metadata = std::fs::metadata(&path)
        .map_err(|e| {
            tracing::error!(path = %path, error = %e, "Failed to get file metadata");
            format!("Failed to get file metadata: {}", e)
        })?;
    let size = metadata.len();

    // Get media duration using ffprobe
    let ffprobe_cmd = get_ffprobe_path(Some(&app));
    tracing::debug!(path = %path, ffprobe_cmd = %ffprobe_cmd, "Using ffprobe command");

    let output = Command::new(&ffprobe_cmd)
        .args(&[
            "-v", "error",
            "-show_entries", "format=duration",
            "-show_entries", "stream=width,height",
            "-of", "json",
            &path,
        ])
        .output()
        .map_err(|e| {
            tracing::error!(path = %path, error = %e, "Failed to execute ffprobe");
            format!("Failed to execute ffprobe: {}", e)
        })?;

    if !output.status.success() {
        tracing::error!(path = %path, "ffprobe command failed");
        return Err("ffprobe command failed".to_string());
    }

    let json_str = String::from_utf8(output.stdout)
        .map_err(|e| {
            tracing::error!(path = %path, error = %e, "Failed to parse ffprobe output");
            format!("Failed to parse ffprobe output: {}", e)
        })?;

    // Parse JSON manually for simplicity
    let duration = extract_duration(&json_str)
        .ok_or_else(|| {
            tracing::error!(path = %path, "Failed to extract duration");
            "Failed to extract duration".to_string()
        })?;

    // For audio files, width and height will be None
    let (width, height) = if is_audio {
        (None, None)
    } else {
        let (w, h) = extract_resolution(&json_str)
            .ok_or_else(|| {
                tracing::error!(path = %path, "Failed to extract resolution");
                "Failed to extract resolution".to_string()
            })?;
        (Some(w), Some(h))
    };

    tracing::info!(
        path = %path,
        duration = duration,
        size = size,
        width = ?width,
        height = ?height,
        is_audio = is_audio,
        "Video metadata extracted successfully"
    );

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
pub async fn generate_thumbnail(app: AppHandle, path: String) -> Result<ThumbnailResult, String> {
    tracing::info!(path = %path, "Generating thumbnail");
    let file_path = Path::new(&path);

    // For audio files, return a default audio icon
    if is_audio_file(file_path) {
        tracing::debug!(path = %path, "Generating default audio icon");
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

    tracing::debug!(
        path = %path,
        thumbnail_path = ?thumbnail_path,
        "Generating video thumbnail"
    );

    // Generate thumbnail at 3 seconds
    let ffmpeg_cmd = get_ffmpeg_path(Some(&app));
    tracing::debug!(path = %path, ffmpeg_cmd = %ffmpeg_cmd, "Using ffmpeg command");

    let output = Command::new(&ffmpeg_cmd)
        .args(&[
            "-i", &path,
            "-ss", "00:00:03",
            "-vframes", "1",
            "-vf", "scale=240:-1",
            "-y",
            thumbnail_path.to_str().unwrap(),
        ])
        .output()
        .map_err(|e| {
            tracing::error!(path = %path, error = %e, "Failed to execute ffmpeg");
            format!("Failed to execute ffmpeg: {}", e)
        })?;

    if !output.status.success() {
        tracing::error!(path = %path, "ffmpeg command failed");
        return Err("ffmpeg command failed".to_string());
    }

    // Read thumbnail and encode as base64
    let image_data = std::fs::read(&thumbnail_path)
        .map_err(|e| {
            tracing::error!(path = %path, thumbnail_path = ?thumbnail_path, error = %e, "Failed to read thumbnail");
            format!("Failed to read thumbnail: {}", e)
        })?;

    let base64_image = base64_encode(&image_data);

    // Clean up temp file
    if let Err(e) = std::fs::remove_file(&thumbnail_path) {
        tracing::warn!(thumbnail_path = ?thumbnail_path, error = %e, "Failed to remove temp thumbnail file");
    }

    tracing::info!(path = %path, "Thumbnail generated successfully");

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
    tracing::info!(path = %path, "Opening file in file manager");
    let file_path = Path::new(&path);

    if !file_path.exists() {
        tracing::error!(path = %path, "File does not exist");
        return Err("File does not exist".to_string());
    }

    #[cfg(target_os = "macos")]
    {
        Command::new("open")
            .arg("-R")
            .arg(&path)
            .spawn()
            .map_err(|e| {
                tracing::error!(path = %path, error = %e, "Failed to open Finder");
                format!("Failed to open Finder: {}", e)
            })?;
        tracing::debug!(path = %path, "Opened in Finder");
    }

    #[cfg(target_os = "windows")]
    {
        Command::new("explorer")
            .arg("/select,")
            .arg(&path)
            .spawn()
            .map_err(|e| {
                tracing::error!(path = %path, error = %e, "Failed to open Explorer");
                format!("Failed to open Explorer: {}", e)
            })?;
        tracing::debug!(path = %path, "Opened in Explorer");
    }

    #[cfg(target_os = "linux")]
    {
        let parent_dir = file_path.parent()
            .ok_or_else(|| {
                tracing::error!(path = %path, "Failed to get parent directory");
                "Failed to get parent directory".to_string()
            })?;
        Command::new("xdg-open")
            .arg(parent_dir)
            .spawn()
            .map_err(|e| {
                tracing::error!(path = %path, error = %e, "Failed to open file manager");
                format!("Failed to open file manager: {}", e)
            })?;
        tracing::debug!(path = %path, "Opened in file manager");
    }

    Ok(())
}
