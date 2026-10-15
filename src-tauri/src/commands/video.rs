use serde::{Deserialize, Serialize};
use std::path::Path;
use std::process::Command;
use tauri::AppHandle;

// Audio file extensions
const AUDIO_EXTENSIONS: &[&str] = &["mp3", "wav", "flac", "aac", "m4a", "ogg", "wma"];

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

fn get_binary_candidates(binary_name: &str, target_suffix: &str) -> Vec<String> {
    let mut candidates = Vec::new();

    candidates.push(binary_name.to_string());
    if !target_suffix.is_empty() {
        candidates.push(format!("{}-{}", binary_name, target_suffix));
    }

    #[cfg(target_os = "windows")]
    {
        if !target_suffix.is_empty() {
            candidates.push(format!("{}-{}.exe", binary_name, target_suffix));
        }
        candidates.push(format!("{}.exe", binary_name));
    }

    candidates
}

fn resolve_binary_path(binary_name: &str, env_var: &str, app: Option<&AppHandle>) -> String {
    // 0. 显式环境变量覆盖
    if let Ok(override_path) = std::env::var(env_var) {
        if !override_path.trim().is_empty() {
            tracing::debug!(binary = binary_name, env_var = env_var, override_path = %override_path, "Using binary path from environment override");
            return override_path;
        }
    }

    let target_suffix = get_target_suffix();
    let candidates = get_binary_candidates(binary_name, &target_suffix);

    // 1. 尝试开发环境资源目录
    if let Ok(current_dir) = std::env::current_dir() {
        let dev_bin_dir = current_dir.join("src-tauri").join("resources").join("bin");

        for candidate in &candidates {
            let dev_binary = dev_bin_dir.join(candidate);
            if dev_binary.exists() {
                tracing::debug!(binary = binary_name, path = ?dev_binary, "Found dev binary");
                return dev_binary.to_string_lossy().to_string();
            }
        }
    }

    // 2. 尝试打包目录（与可执行文件同目录）
    if let Some(_app) = app {
        if let Ok(exe_path) = std::env::current_exe() {
            if let Some(exe_dir) = exe_path.parent() {
                for candidate in &candidates {
                    let bundled_binary = exe_dir.join(candidate);
                    if bundled_binary.exists() {
                        tracing::debug!(binary = binary_name, path = ?bundled_binary, "Found bundled binary");
                        return bundled_binary.to_string_lossy().to_string();
                    }
                }
            }

            // 2.1 尝试 app bundle 的 Resources/resources/bin
            if let Some(contents_dir) = exe_path.parent().and_then(|p| p.parent()) {
                let resources_bin_dir =
                    contents_dir.join("Resources").join("resources").join("bin");
                for candidate in &candidates {
                    let bundled_resource_binary = resources_bin_dir.join(candidate);
                    if bundled_resource_binary.exists() {
                        tracing::debug!(binary = binary_name, path = ?bundled_resource_binary, "Found bundled resource binary");
                        return bundled_resource_binary.to_string_lossy().to_string();
                    }
                }
            }
        }
    }

    // 3. 常见系统路径
    let common_paths = match binary_name {
        "ffmpeg" => vec![
            "/opt/homebrew/bin/ffmpeg",
            "/usr/local/bin/ffmpeg",
            "/usr/bin/ffmpeg",
        ],
        "ffprobe" => vec![
            "/opt/homebrew/bin/ffprobe",
            "/usr/local/bin/ffprobe",
            "/usr/bin/ffprobe",
        ],
        _ => Vec::new(),
    };

    for path in common_paths {
        if std::path::Path::new(path).exists() {
            tracing::debug!(binary = binary_name, path = path, "Found system binary");
            return path.to_string();
        }
    }

    // 4. 最后回退到 PATH
    tracing::warn!(binary = binary_name, "Falling back to binary name in PATH");
    binary_name.to_string()
}

/// Get ffmpeg binary path (bundled or system)
pub fn get_ffmpeg_path(app: Option<&AppHandle>) -> String {
    resolve_binary_path("ffmpeg", "MONOSUB_FFMPEG_PATH", app)
}

/// Get ffprobe binary path (bundled or system)
pub fn get_ffprobe_path(app: Option<&AppHandle>) -> String {
    resolve_binary_path("ffprobe", "MONOSUB_FFPROBE_PATH", app)
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

fn get_thumbnail_cache_path(video_path: &Path) -> Option<std::path::PathBuf> {
    let parent = video_path.parent()?;
    let stem = video_path.file_stem()?.to_str()?;
    let ext = video_path
        .extension()
        .and_then(|e| e.to_str())
        .filter(|e| !e.is_empty())
        .unwrap_or("media");
    Some(parent.join(format!("{}_{}_thumb.jpg", stem, ext)))
}

fn is_cache_fresh(cache_path: &Path, source_path: &Path) -> bool {
    let cache_meta = match std::fs::metadata(cache_path) {
        Ok(meta) => meta,
        Err(_) => return false,
    };
    let source_meta = match std::fs::metadata(source_path) {
        Ok(meta) => meta,
        Err(_) => return false,
    };
    let cache_mtime = match cache_meta.modified() {
        Ok(time) => time,
        Err(_) => return false,
    };
    let source_mtime = match source_meta.modified() {
        Ok(time) => time,
        Err(_) => return false,
    };
    cache_mtime >= source_mtime
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
    let metadata = std::fs::metadata(&path).map_err(|e| {
        tracing::error!(path = %path, error = %e, "Failed to get file metadata");
        format!("Failed to get file metadata: {}", e)
    })?;
    let size = metadata.len();

    // Get media duration using ffprobe
    let ffprobe_cmd = get_ffprobe_path(Some(&app));
    tracing::debug!(path = %path, ffprobe_cmd = %ffprobe_cmd, "Using ffprobe command");

    let output = Command::new(&ffprobe_cmd)
        .args(&[
            "-v",
            "error",
            "-show_entries",
            "format=duration",
            "-show_entries",
            "stream=width,height",
            "-of",
            "json",
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

    let json_str = String::from_utf8(output.stdout).map_err(|e| {
        tracing::error!(path = %path, error = %e, "Failed to parse ffprobe output");
        format!("Failed to parse ffprobe output: {}", e)
    })?;

    // Parse JSON manually for simplicity
    let duration = extract_duration(&json_str).ok_or_else(|| {
        tracing::error!(path = %path, "Failed to extract duration");
        "Failed to extract duration".to_string()
    })?;

    // For audio files, width and height will be None
    let (width, height) = if is_audio {
        (None, None)
    } else {
        let (w, h) = extract_resolution(&json_str).ok_or_else(|| {
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

// Default video icon as SVG (converted to data URI)
fn get_video_icon_svg() -> String {
    format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="240" height="135" viewBox="0 0 240 135">
        <rect fill="#{}" width="240" height="135"/>
        <rect x="72" y="35" rx="6" ry="6" width="96" height="54" fill="#{}"/>
        <polygon points="108,50 108,74 130,62" fill="white"/>
        <text x="120" y="110" text-anchor="middle" fill="#{}" font-size="12">Video</text>
        </svg>"##,
        "0f172a", "334155", "94a3b8"
    )
}

fn fallback_video_thumbnail() -> ThumbnailResult {
    let svg_content = get_video_icon_svg();
    let svg_base64 = base64_encode(svg_content.as_bytes());
    ThumbnailResult {
        thumbnail_base64: format!("data:image/svg+xml;base64,{}", svg_base64),
    }
}

/// Generate thumbnail for media file (video: frame extract, audio: default icon)
#[tauri::command]
pub async fn generate_thumbnail(app: AppHandle, path: String) -> Result<ThumbnailResult, String> {
    tracing::debug!(path = %path, "Loading thumbnail");
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

    if let Some(cache_path) = get_thumbnail_cache_path(file_path) {
        if is_cache_fresh(&cache_path, file_path) {
            match std::fs::read(&cache_path) {
                Ok(image_data) => {
                    tracing::debug!(path = %path, cache_path = ?cache_path, "Using cached thumbnail");
                    return Ok(ThumbnailResult {
                        thumbnail_base64: format!(
                            "data:image/jpeg;base64,{}",
                            base64_encode(&image_data)
                        ),
                    });
                }
                Err(e) => {
                    tracing::warn!(path = %path, cache_path = ?cache_path, error = %e, "Failed to read cached thumbnail, regenerating");
                }
            }
        }
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

    // Try with hardware acceleration first (needed for AV1 etc.), fall back to software decoding
    let thumbnail_out = thumbnail_path.to_str().unwrap();
    let args_hwaccel = vec![
        "-hwaccel",
        "videotoolbox",
        "-i",
        &path,
        "-ss",
        "00:00:03",
        "-frames:v",
        "1",
        "-vf",
        "scale=240:-1",
        "-update",
        "1",
        "-y",
        thumbnail_out,
    ];
    let args_software = vec![
        "-i",
        &path,
        "-ss",
        "00:00:03",
        "-frames:v",
        "1",
        "-vf",
        "scale=240:-1",
        "-update",
        "1",
        "-y",
        thumbnail_out,
    ];

    let output = match Command::new(&ffmpeg_cmd).args(&args_hwaccel).output() {
        Ok(o) if o.status.success() => o,
        _ => {
            // Retry without hardware acceleration
            match Command::new(&ffmpeg_cmd).args(&args_software).output() {
                Ok(output) => output,
                Err(e) => {
                    tracing::error!(path = %path, ffmpeg_cmd = %ffmpeg_cmd, error = %e, "Failed to execute ffmpeg, using fallback thumbnail");
                    return Ok(fallback_video_thumbnail());
                }
            }
        }
    };

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        tracing::error!(
            path = %path,
            ffmpeg_cmd = %ffmpeg_cmd,
            status = ?output.status.code(),
            stderr = %stderr,
            "ffmpeg command failed, using fallback thumbnail"
        );
        return Ok(fallback_video_thumbnail());
    }

    // Read thumbnail and encode as base64
    let image_data = match std::fs::read(&thumbnail_path) {
        Ok(data) => data,
        Err(e) => {
            tracing::error!(path = %path, thumbnail_path = ?thumbnail_path, error = %e, "Failed to read generated thumbnail, using fallback");
            return Ok(fallback_video_thumbnail());
        }
    };

    let base64_image = base64_encode(&image_data);

    if let Some(cache_path) = get_thumbnail_cache_path(file_path) {
        if let Err(e) = std::fs::write(&cache_path, &image_data) {
            tracing::warn!(path = %path, cache_path = ?cache_path, error = %e, "Failed to persist thumbnail cache");
        } else {
            tracing::debug!(path = %path, cache_path = ?cache_path, "Thumbnail cache updated");
        }
    }

    // Clean up temp file
    if let Err(e) = std::fs::remove_file(&thumbnail_path) {
        tracing::warn!(thumbnail_path = ?thumbnail_path, error = %e, "Failed to remove temp thumbnail file");
    }

    tracing::debug!(path = %path, "Thumbnail generated successfully");

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
        result.push(if chunk.len() > 1 {
            CHARSET[b3] as char
        } else {
            '='
        });
        result.push(if chunk.len() > 2 {
            CHARSET[b4] as char
        } else {
            '='
        });
    }

    result
}

/// Supported media extensions for directory scanning
const MEDIA_EXTENSIONS: &[&str] = &[
    "mp4", "mov", "avi", "mkv", "webm", "flv", "m4v", "mp3", "wav", "flac", "aac", "m4a", "ogg",
    "wma",
];

/// Resolve a list of paths, recursively expanding directories into media file paths
#[tauri::command]
pub async fn resolve_paths(paths: Vec<String>) -> Result<Vec<String>, String> {
    let media_set: std::collections::HashSet<&str> = MEDIA_EXTENSIONS.iter().copied().collect();
    let mut result = Vec::new();

    for path_str in &paths {
        let path = Path::new(path_str);
        if path.is_dir() {
            collect_media_files(path, &media_set, &mut result);
        } else if is_media_file(path, &media_set) {
            result.push(path_str.clone());
        }
    }

    result.sort();
    Ok(result)
}

fn is_media_file(path: &Path, media_set: &std::collections::HashSet<&str>) -> bool {
    if let Some(file_name) = path.file_name().and_then(|n| n.to_str()) {
        if file_name.starts_with('.')
            || file_name.ends_with("_audio.wav")
            || file_name.ends_with("_thumb.jpg")
        {
            return false;
        }
    }

    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| media_set.contains(ext.to_lowercase().as_str()))
        .unwrap_or(false)
}

fn collect_media_files(
    dir: &Path,
    media_set: &std::collections::HashSet<&str>,
    result: &mut Vec<String>,
) {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) => {
            tracing::warn!(dir = ?dir, error = %e, "Failed to read directory");
            return;
        }
    };

    for entry in entries.flatten() {
        let path = entry.path();

        // Skip hidden files/directories and cache files
        if let Some(file_name) = path.file_name().and_then(|n| n.to_str()) {
            if file_name.starts_with('.')
                || file_name.ends_with("_audio.wav")
                || file_name.ends_with("_thumb.jpg")
            {
                continue;
            }
        }

        if path.is_dir() {
            collect_media_files(&path, media_set, result);
        } else if is_media_file(&path, media_set) {
            result.push(path.to_string_lossy().to_string());
        }
    }
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
        let parent_dir = file_path.parent().ok_or_else(|| {
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
