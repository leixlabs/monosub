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

fn resolve_binary_path(
    binary_name: &str,
    env_var: &str,
    paths: Option<&crate::paths::AppPaths>,
) -> String {
    // 0. 显式环境变量覆盖
    if let Ok(override_path) = std::env::var(env_var) {
        if !override_path.trim().is_empty() {
            tracing::debug!(binary = binary_name, env_var = env_var, override_path = %override_path, "Using binary path from environment override");
            return override_path;
        }
    }

    let target_suffix = get_target_suffix();
    let candidates = get_binary_candidates(binary_name, &target_suffix);

    // 1. The resolved resources root's `bin/`. `AppPaths` already probed the dev
    //    checkout, the executable's own directory and the app bundle, so this is
    //    the single place that knows where a bundled binary lives — repeating
    //    those probes here is how the two would drift.
    if let Some(paths) = paths {
        let bin_dir = paths.resources.join("bin");
        for candidate in &candidates {
            let bundled = bin_dir.join(candidate);
            if bundled.exists() {
                tracing::debug!(binary = binary_name, path = ?bundled, "Found bundled binary");
                return bundled.to_string_lossy().to_string();
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
pub fn get_ffmpeg_path(paths: &crate::paths::AppPaths) -> String {
    resolve_binary_path("ffmpeg", "MONOSUB_FFMPEG_PATH", Some(paths))
}

/// Get ffprobe binary path (bundled or system)
pub fn get_ffprobe_path(paths: &crate::paths::AppPaths) -> String {
    resolve_binary_path("ffprobe", "MONOSUB_FFPROBE_PATH", Some(paths))
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
    let ffprobe_cmd = get_ffprobe_path(&crate::paths::AppPaths::from_app(&app).map_err(|e| e.to_string())?);
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
    let ffmpeg_cmd = get_ffmpeg_path(&crate::paths::AppPaths::from_app(&app).map_err(|e| e.to_string())?);
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

/// Whether any of `paths` already exists on disk.
///
/// This answers the `skipExistingSubtitle` question, and it is a Rust command on
/// purpose: the Tauri fs plugin's `exists` is gated by the plugin's path scope,
/// which only covers the app's own directories plus whatever the user picked in a
/// *dialog*. A file added by drag-and-drop, by a URL import's cache directory, or
/// simply a subtitle next to a video in `~/Movies` is therefore outside the scope
/// and the plugin rejects the query — measured:
///
/// ```text
/// warn Failed to check if output file exists forbidden path: /Users/…/x.srt,
///      maybe it is not allowed on the scope for `allow-exists` permission
/// ```
///
/// and the failure was only logged, so the file was transcribed anyway. The
/// backend already reads and writes these exact paths with `std::fs`, so asking
/// the OS directly is both correct and cheaper than widening the plugin scope to
/// "everywhere".
///
/// Returning the first hit (rather than a bool) keeps the log line useful: it
/// names the file that caused the skip.
#[tauri::command]
pub async fn find_existing_path(paths: Vec<String>) -> Result<Option<String>, String> {
    Ok(first_existing_file(&paths).map(|path| path.to_string_lossy().to_string()))
}

/// The body of [`find_existing_path`], split out so it is testable without a
/// Tauri runtime.
///
/// A directory is not a hit: `output_paths_for` can name a `.srt` that happens
/// to be an existing *directory* of that name, and "skip because there is a
/// folder called video.srt" would leave the user with no subtitle and no error.
fn first_existing_file<P: AsRef<Path>>(paths: &[P]) -> Option<&Path> {
    paths
        .iter()
        .map(|path| path.as_ref())
        .find(|path| path.is_file())
}

/// Locate `path` in the system file manager: a file is revealed, a directory is
/// opened.
///
/// The command name and signature are unchanged (the frontend still calls
/// `invoke('open_in_finder', { path })`); the platform argv and the exit-status
/// check live in [`super::opener`], which shared them with
/// `open_models_dir` and the log panel.
#[tauri::command]
pub async fn open_in_finder(path: String) -> Result<(), String> {
    tracing::info!(path = %path, "Opening file in file manager");
    let file_path = Path::new(&path);

    // Kept here as well as in the shared module: on Windows the child's exit
    // status cannot decide success, so this is the only place a missing path
    // is still reported there.
    if !file_path.exists() {
        tracing::error!(path = %path, "File does not exist");
        return Err("File does not exist".to_string());
    }

    super::opener::reveal_in_file_manager(file_path)
        .await
        .inspect_err(|error| {
            tracing::error!(path = %path, error = %error, "Failed to open file manager");
        })
        .inspect(|()| tracing::debug!(path = %path, "Opened in file manager"))
}

#[cfg(test)]
mod tests {
    use super::first_existing_file;

    #[test]
    fn missing_paths_are_not_a_hit() {
        let paths = vec!["/definitely/not/here.srt".to_string()];
        assert!(first_existing_file(&paths).is_none());
    }

    #[test]
    fn a_directory_is_not_an_existing_subtitle() {
        let dir = std::env::temp_dir().join(format!("monosub-skip-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let as_subtitle = dir.with_extension("srt");
        std::fs::create_dir_all(&as_subtitle).unwrap();

        assert!(first_existing_file(&[as_subtitle.to_string_lossy().to_string()]).is_none());

        let _ = std::fs::remove_dir_all(&as_subtitle);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_first_hit_is_returned_so_the_row_can_name_it() {
        let dir = std::env::temp_dir().join(format!("monosub-skip-order-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let missing = dir.join("missing.srt");
        let present = dir.join("present.vtt");
        std::fs::write(&present, "WEBVTT\n").unwrap();

        // The primary format is checked first; a later format that exists is
        // still a hit, which is what makes `srt,vtt` with only a `.vtt` skip.
        let paths = vec![
            missing.to_string_lossy().to_string(),
            present.to_string_lossy().to_string(),
        ];
        assert_eq!(first_existing_file(&paths), Some(present.as_path()));

        let _ = std::fs::remove_dir_all(&dir);
    }
}
