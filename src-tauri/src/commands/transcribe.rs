use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use tauri_plugin_shell::ShellExt;
use tokio::sync::Mutex;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct StepInfo {
    pub current_step: u32,
    pub total_steps: u32,
    pub step_name: String,
    pub step_elapsed: f64,
    pub total_elapsed: f64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TranscribeProgress {
    pub task_id: String,
    pub progress: f64,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub step: Option<StepInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_time: Option<f64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TranscribeRequest {
    pub id: String,
    pub path: String,
    pub output_path: String,
    pub format: Option<String>,
    pub language: Option<String>,
    pub device: Option<String>,
    pub enable_diarization: Option<bool>,
}

// Global task manager
type TaskMap = Arc<Mutex<HashMap<String, tokio::task::JoinHandle<()>>>>;

lazy_static::lazy_static! {
    static ref TASKS: TaskMap = Arc::new(Mutex::new(HashMap::new()));
    static ref AUDIO_SEMAPHORE: Arc<tokio::sync::Semaphore> = Arc::new(tokio::sync::Semaphore::new(2));
    static ref TRANSCRIBE_SEMAPHORE: Arc<tokio::sync::Semaphore> = Arc::new(tokio::sync::Semaphore::new(1));
}

/// Check if running in development mode
fn is_dev_mode() -> bool {
    // 检查是否存在 core-engine/.venv 目录
    let current_dir = std::env::current_dir().ok();
    if let Some(dir) = current_dir {
        // Dev: monosub/src-tauri -> monosub/core-engine/.venv
        if dir.join("../core-engine/.venv").exists() {
            return true;
        }
    }
    false
}

/// Get the core-engine directory
fn get_core_engine_dir(app: &AppHandle) -> Result<std::path::PathBuf, String> {
    use tauri::Manager;
    if is_dev_mode() {
        let current_dir = std::env::current_dir().map_err(|e| e.to_string())?;
        Ok(current_dir.join("../core-engine"))
    } else {
        let resource_dir = app.path().resource_dir().map_err(|e| e.to_string())?;
        Ok(resource_dir.join("resources").join("core-engine"))
    }
}

fn is_audio_input(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| {
            ["mp3", "wav", "flac", "aac", "m4a", "ogg", "wma"]
                .contains(&ext.to_lowercase().as_str())
        })
        .unwrap_or(false)
}

fn get_audio_cache_path(video_path: &Path) -> Option<PathBuf> {
    let parent = video_path.parent()?;
    let stem = video_path.file_stem()?.to_str()?;
    Some(parent.join(format!(".{}_audio.wav", stem)))
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

/// Start transcription task using Tauri sidecar
#[tauri::command]
pub async fn start_transcription(
    app: AppHandle,
    request: TranscribeRequest,
) -> Result<String, String> {
    let task_id = request.id.clone();
    let video_path = request.path.clone();
    let output_path = request.output_path.clone();
    let format = request.format.unwrap_or_else(|| "srt".to_string());
    let language = request.language.unwrap_or_else(|| "zh".to_string());
    let requested_device = request.device.unwrap_or_else(|| "auto".to_string());
    let requested_device = requested_device.trim().to_lowercase();
    let device = match requested_device.as_str() {
        "cpu" | "cuda" | "mps" | "auto" => requested_device,
        _ => "auto".to_string(),
    };
    let enable_diarization = request.enable_diarization.unwrap_or(false);

    tracing::info!(
        task_id = %task_id,
        video_path = %video_path,
        output_path = %output_path,
        format = %format,
        language = %language,
        device = %device,
        enable_diarization = %enable_diarization,
        "Starting transcription task"
    );

    // Check if task already exists
    let tasks = TASKS.lock().await;
    if tasks.contains_key(&task_id) {
        tracing::warn!(task_id = %task_id, "Task already running");
        return Err("Task already running".to_string());
    }
    drop(tasks);

    let task_id_for_map = task_id.clone();
    let task_id_for_spawn = task_id.clone();

    // Spawn async task for transcription
    let handle = tokio::spawn(async move {
        let task_id = task_id_for_spawn;

        // Emit initial progress
        let _ = app.emit(
            "transcribe_progress",
            TranscribeProgress {
                task_id: task_id.clone(),
                progress: 0.0,
                status: "Pending in queue...".to_string(),
                step: None,
                total_time: None,
            },
        );

        let input_path = Path::new(&video_path);
        let is_audio = is_audio_input(input_path);

        let (final_audio_path, keep_audio_cache) = if !is_audio {
            // Wait for audio extraction permit
            let _permit = AUDIO_SEMAPHORE.acquire().await;

            let _ = app.emit(
                "transcribe_progress",
                TranscribeProgress {
                    task_id: task_id.clone(),
                    progress: 0.0,
                    status: "Extracting audio...".to_string(),
                    step: Some(StepInfo {
                        current_step: 1,
                        total_steps: 4,
                        step_name: "Extracting audio".to_string(),
                        step_elapsed: 0.0,
                        total_elapsed: 0.0,
                    }),
                    total_time: None,
                },
            );

            let cache_path = get_audio_cache_path(input_path).unwrap_or_else(|| {
                std::env::temp_dir().join(format!("extracted_{}.wav", uuid::Uuid::new_v4()))
            });
            let keep_cache = cache_path
                .parent()
                .map(|parent| parent != std::env::temp_dir())
                .unwrap_or(false);

            if is_cache_fresh(&cache_path, input_path) {
                let cache_path_str = cache_path.to_string_lossy().to_string();
                tracing::info!(
                    task_id = %task_id,
                    cached_audio = %cache_path_str,
                    "Using cached extracted audio"
                );
                (cache_path_str, keep_cache)
            } else {
                let audio_path_str = cache_path.to_string_lossy().to_string();
                let ffmpeg_path = crate::commands::video::get_ffmpeg_path_for_python(Some(&app));
                let output = tokio::process::Command::new(&ffmpeg_path)
                    .args(&[
                        "-i",
                        &video_path,
                        "-vn",
                        "-acodec",
                        "pcm_s16le",
                        "-ar",
                        "16000",
                        "-ac",
                        "1",
                        "-y",
                        &audio_path_str,
                    ])
                    .output()
                    .await;

                match output {
                    Ok(out) if out.status.success() => {
                        tracing::info!(
                            task_id = %task_id,
                            cached_audio = %audio_path_str,
                            "Audio extracted successfully"
                        );
                        (audio_path_str, keep_cache)
                    }
                    _ => {
                        let _ = std::fs::remove_file(&cache_path);
                        tracing::error!(task_id = %task_id, "Failed to extract audio natively, falling back to python extraction");
                        (video_path.clone(), false)
                    }
                }
            }
        } else {
            (video_path.clone(), false)
        };

        // Wait for transcription permit
        let _permit = TRANSCRIBE_SEMAPHORE.acquire().await;

        let _ = app.emit(
            "transcribe_progress",
            TranscribeProgress {
                task_id: task_id.clone(),
                progress: 0.0,
                status: "Starting transcription...".to_string(),
                step: Some(StepInfo {
                    current_step: 2,
                    total_steps: 4,
                    step_name: "Loading model".to_string(),
                    step_elapsed: 0.0,
                    total_elapsed: 0.0,
                }),
                total_time: None,
            },
        );

        // 开发模式：使用 Python 直接运行
        // 生产模式：使用 sidecar 运行打包的可执行文件
        let is_dev = is_dev_mode();
        tracing::info!(task_id = %task_id, is_dev_mode = is_dev, "Running transcription");

        let result = if is_dev {
            run_dev_mode(
                &app,
                &task_id,
                &final_audio_path,
                &output_path,
                &format,
                &language,
                &device,
                enable_diarization,
            )
            .await
        } else {
            run_sidecar(
                &app,
                &task_id,
                &final_audio_path,
                &output_path,
                &format,
                &language,
                &device,
                enable_diarization,
            )
            .await
        };

        // Cleanup temporary audio file if we created one (keep media-directory cache)
        if final_audio_path != video_path && !keep_audio_cache {
            let _ = std::fs::remove_file(&final_audio_path);
        }

        if let Err(e) = result {
            tracing::error!(task_id = %task_id, error = %e, "Transcription failed");
            // 发送错误事件（不是 progress 事件）
            let _ = app.emit(
                "transcribe_error",
                TranscribeProgress {
                    task_id: task_id.clone(),
                    progress: 0.0,
                    status: format!("Error: {}", e),
                    step: None,
                    total_time: None,
                },
            );
        } else {
            tracing::info!(task_id = %task_id, "Transcription completed successfully");
        }

        // Remove task from map
        TASKS.lock().await.remove(&task_id);
    });

    // Store task handle
    TASKS.lock().await.insert(task_id_for_map, handle);

    Ok(task_id)
}

/// Run in production mode using sidecar
async fn run_sidecar(
    app: &AppHandle,
    task_id: &str,
    video_path: &str,
    output_path: &str,
    format: &str,
    language: &str,
    device: &str,
    enable_diarization: bool,
) -> Result<(), String> {
    tracing::info!(task_id = %task_id, "Starting sidecar process");
    let shell = app.shell();

    // Get ffmpeg path from video.rs helper
    let ffmpeg_path = crate::commands::video::get_ffmpeg_path_for_python(Some(app));
    let ffprobe_path = crate::commands::video::get_ffprobe_path_for_python(Some(app));

    tracing::debug!(
        task_id = %task_id,
        ffmpeg_path = %ffmpeg_path,
        ffprobe_path = %ffprobe_path,
        "Setting ffmpeg paths for sidecar subprocess"
    );

    // 使用 sidecar 运行 monosub-engine
    let sidecar_command = shell.sidecar("monosub-engine").map_err(|e| {
        tracing::error!(task_id = %task_id, error = %e, "Failed to create sidecar command");
        format!("Failed to create sidecar command: {}", e)
    })?;

    // 构建命令参数
    let mut args = vec![
        video_path.to_string(),
        output_path.to_string(),
        "--format".to_string(),
        format.to_string(),
        "--language".to_string(),
        language.to_string(),
        "--device".to_string(),
        device.to_string(),
    ];

    // 如果启用了说话人分离，添加参数
    if enable_diarization {
        args.push("--diarization".to_string());
        tracing::info!(task_id = %task_id, "Speaker diarization enabled");
    }

    let (mut rx, _child) = sidecar_command
        .args(&args)
        .env("MONOSUB_FFMPEG_PATH", ffmpeg_path)
        .env("MONOSUB_FFPROBE_PATH", ffprobe_path)
        .env("MONOSUB_ASR_DEVICE", device)
        .spawn()
        .map_err(|e| {
            tracing::error!(task_id = %task_id, error = %e, "Failed to spawn sidecar");
            format!("Failed to spawn sidecar: {}", e)
        })?;

    tracing::debug!(task_id = %task_id, "Sidecar process spawned successfully");

    // Read output
    while let Some(event) = rx.recv().await {
        match event {
            tauri_plugin_shell::process::CommandEvent::Stdout(line) => {
                let line_str = String::from_utf8_lossy(&line);
                process_output_line(app, task_id, &line_str);
            }
            tauri_plugin_shell::process::CommandEvent::Stderr(line) => {
                let line_str = String::from_utf8_lossy(&line);
                tracing::warn!(task_id = %task_id, stderr = %line_str, "Sidecar stderr output");

                // 如果是 Python 错误（ModuleNotFoundError, ImportError 等），发送错误事件
                if line_str.contains("Error") || line_str.contains("Traceback") {
                    let _ = app.emit(
                        "transcribe_error",
                        TranscribeProgress {
                            task_id: task_id.to_string(),
                            progress: 0.0,
                            status: format!("Python Error: {}", line_str.trim()),
                            step: None,
                            total_time: None,
                        },
                    );
                }
            }
            tauri_plugin_shell::process::CommandEvent::Error(err) => {
                tracing::error!(task_id = %task_id, error = %err, "Sidecar process error");
                return Err(format!("Process error: {}", err));
            }
            tauri_plugin_shell::process::CommandEvent::Terminated(payload) => {
                tracing::info!(task_id = %task_id, exit_code = ?payload.code, "Sidecar process terminated");
                if payload.code != Some(0) {
                    let error_msg = format!("Process exited with code: {:?}", payload.code);
                    let _ = app.emit(
                        "transcribe_error",
                        TranscribeProgress {
                            task_id: task_id.to_string(),
                            progress: 0.0,
                            status: error_msg.clone(),
                            step: None,
                            total_time: None,
                        },
                    );
                    return Err(error_msg);
                }
            }
            _ => {}
        }
    }

    Ok(())
}

/// Run in development mode using Python directly
async fn run_dev_mode(
    app: &AppHandle,
    task_id: &str,
    video_path: &str,
    output_path: &str,
    format: &str,
    language: &str,
    device: &str,
    enable_diarization: bool,
) -> Result<(), String> {
    use tokio::io::{AsyncBufReadExt, BufReader};
    use tokio::process::Command;

    tracing::info!(task_id = %task_id, "Starting development mode with Python venv");

    // 获取 Python 路径
    let current_dir = std::env::current_dir().map_err(|e| e.to_string())?;
    let parent = current_dir.parent().ok_or("No parent directory")?;
    let venv_python = parent.join("core-engine/.venv/bin/python");
    let main_py = parent.join("core-engine/main.py");

    if !venv_python.exists() || !main_py.exists() {
        tracing::error!(
            task_id = %task_id,
            venv_python = ?venv_python,
            main_py = ?main_py,
            "Development environment not found"
        );
        return Err("Development environment not found".to_string());
    }

    tracing::debug!(
        task_id = %task_id,
        venv_python = ?venv_python,
        main_py = ?main_py,
        "Using Python venv"
    );

    let cmd_str = format!(
        "{} {} {} {} --format {} --language {} --device {}",
        venv_python.display(),
        main_py.display(),
        video_path,
        output_path,
        format,
        language,
        device
    );

    tracing::info!(task_id = %task_id, command = %cmd_str, "Executing Python command");

    let _ = app.emit(
        "transcribe_progress",
        TranscribeProgress {
            task_id: task_id.to_string(),
            progress: 0.0,
            status: format!("CMD: {}", cmd_str),
            step: None,
            total_time: None,
        },
    );

    // Get ffmpeg path from video.rs helper
    let ffmpeg_path = crate::commands::video::get_ffmpeg_path_for_python(Some(app));
    let ffprobe_path = crate::commands::video::get_ffprobe_path_for_python(Some(app));

    tracing::debug!(
        task_id = %task_id,
        ffmpeg_path = %ffmpeg_path,
        ffprobe_path = %ffprobe_path,
        "Setting ffmpeg paths for Python subprocess"
    );

    // 构建命令
    let mut command = Command::new(&venv_python);
    command
        .arg(&main_py)
        .arg(video_path)
        .arg(output_path)
        .arg("--format")
        .arg(format)
        .arg("--language")
        .arg(language)
        .arg("--device")
        .arg(device);

    // 如果启用了说话人分离，添加参数
    if enable_diarization {
        command.arg("--diarization");
        tracing::info!(task_id = %task_id, "Speaker diarization enabled (dev mode)");
    }

    let mut child = command
        .env("MONOSUB_FFMPEG_PATH", ffmpeg_path)
        .env("MONOSUB_FFPROBE_PATH", ffprobe_path)
        .env("MONOSUB_ASR_DEVICE", device)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| {
            tracing::error!(task_id = %task_id, error = %e, "Failed to spawn Python process");
            format!("Failed to spawn process: {}", e)
        })?;

    tracing::debug!(task_id = %task_id, "Python process spawned successfully");

    let stdout = child.stdout.take().ok_or("Failed to capture stdout")?;
    let stderr = child.stderr.take().ok_or("Failed to capture stderr")?;

    let mut stdout_reader = BufReader::new(stdout).lines();
    let mut stderr_reader = BufReader::new(stderr).lines();

    // Read stderr in background
    let app_clone = app.clone();
    let task_id_clone = task_id.to_string();
    tokio::spawn(async move {
        while let Ok(Some(line)) = stderr_reader.next_line().await {
            tracing::warn!(task_id = %task_id_clone, stderr = %line, "Python stderr output");

            // 如果是 Python 错误，发送错误事件
            if line.contains("Error") || line.contains("Traceback") {
                let _ = app_clone.emit(
                    "transcribe_error",
                    TranscribeProgress {
                        task_id: task_id_clone.clone(),
                        progress: 0.0,
                        status: format!("Python Error: {}", line.trim()),
                        step: None,
                        total_time: None,
                    },
                );
            }
        }
    });

    // Read stdout
    while let Ok(Some(line)) = stdout_reader.next_line().await {
        tracing::debug!(task_id = %task_id, stdout = %line, "Python stdout output");
        process_output_line(app, task_id, &line);
    }

    // Wait for process to complete
    let status = child.wait().await.map_err(|e| {
        tracing::error!(task_id = %task_id, error = %e, "Error waiting for Python process");
        format!("Wait error: {}", e)
    })?;

    tracing::info!(task_id = %task_id, exit_code = ?status.code(), "Python process exited");

    if !status.success() {
        tracing::error!(task_id = %task_id, exit_code = ?status.code(), "Python process failed");
        let error_msg = format!("Process exited with code: {:?}", status.code());
        let _ = app.emit(
            "transcribe_error",
            TranscribeProgress {
                task_id: task_id.to_string(),
                progress: 0.0,
                status: error_msg.clone(),
                step: None,
                total_time: None,
            },
        );
        return Err(error_msg);
    }

    Ok(())
}

// Thread-local storage for current step info
thread_local! {
    static CURRENT_STEP: std::cell::RefCell<Option<StepInfo>> = std::cell::RefCell::new(None);
    static TOTAL_TIME: std::cell::RefCell<Option<f64>> = std::cell::RefCell::new(None);
}

/// Process output line from Python script
fn process_output_line(app: &AppHandle, task_id: &str, line: &str) {
    if line.starts_with("STEP:") {
        // Format: STEP:1/4:step_name
        let parts: Vec<&str> = line[5..].splitn(3, ':').collect();
        if parts.len() >= 2 {
            let step_parts: Vec<&str> = parts[0].split('/').collect();
            if step_parts.len() == 2 {
                if let (Ok(current), Ok(total)) =
                    (step_parts[0].parse::<u32>(), step_parts[1].parse::<u32>())
                {
                    let step_name = if parts.len() >= 2 {
                        parts[1].to_string()
                    } else {
                        "Processing".to_string()
                    };

                    let step_info = StepInfo {
                        current_step: current,
                        total_steps: total,
                        step_name: step_name.clone(),
                        step_elapsed: 0.0,
                        total_elapsed: 0.0,
                    };

                    CURRENT_STEP.with(|s| {
                        *s.borrow_mut() = Some(step_info.clone());
                    });

                    tracing::info!(task_id = %task_id, step = current, total = total, name = %step_name, "Step started");

                    let _ = app.emit(
                        "transcribe_progress",
                        TranscribeProgress {
                            task_id: task_id.to_string(),
                            progress: 0.0,
                            status: step_name,
                            step: Some(step_info),
                            total_time: None,
                        },
                    );
                }
            }
        }
    } else if line.starts_with("ELAPSED:") {
        // Format: ELAPSED:12.5 (total elapsed seconds)
        if let Ok(elapsed) = line[8..].trim().parse::<f64>() {
            CURRENT_STEP.with(|s| {
                if let Some(ref mut step) = *s.borrow_mut() {
                    step.total_elapsed = elapsed;

                    let _ = app.emit(
                        "transcribe_progress",
                        TranscribeProgress {
                            task_id: task_id.to_string(),
                            progress: 0.0,
                            status: step.step_name.clone(),
                            step: Some(step.clone()),
                            total_time: None,
                        },
                    );
                }
            });
        }
    } else if line.starts_with("STEP_ELAPSED:") {
        // Format: STEP_ELAPSED:3.2 (step elapsed seconds)
        if let Ok(step_elapsed) = line[13..].trim().parse::<f64>() {
            CURRENT_STEP.with(|s| {
                if let Some(ref mut step) = *s.borrow_mut() {
                    step.step_elapsed = step_elapsed;
                }
            });
        }
    } else if line.starts_with("TOTAL_TIME:") {
        // Format: TOTAL_TIME:45.3
        if let Ok(total) = line[11..].trim().parse::<f64>() {
            TOTAL_TIME.with(|t| {
                *t.borrow_mut() = Some(total);
            });
            tracing::info!(task_id = %task_id, total_time = total, "Total processing time");
        }
    } else if line.starts_with("PROGRESS:") {
        // Legacy support - ignore PROGRESS messages now
        tracing::debug!(task_id = %task_id, line = %line, "Ignoring legacy PROGRESS message");
    } else if line.starts_with("MSG:") {
        let message = &line[4..];
        tracing::info!(task_id = %task_id, message = %message, "Transcription message");

        let current_step = CURRENT_STEP.with(|s| s.borrow().clone());

        let _ = app.emit(
            "transcribe_progress",
            TranscribeProgress {
                task_id: task_id.to_string(),
                progress: 0.0,
                status: message.to_string(),
                step: current_step,
                total_time: None,
            },
        );
    } else if line.starts_with("SUCCESS:") {
        let output_file = &line[8..];
        tracing::info!(task_id = %task_id, output_file = %output_file, "Transcription completed");

        let total_time = TOTAL_TIME.with(|t| *t.borrow());

        let _ = app.emit(
            "transcribe_complete",
            TranscribeProgress {
                task_id: task_id.to_string(),
                progress: 100.0,
                status: format!("Completed: {}", output_file),
                step: None,
                total_time,
            },
        );

        // Clear thread-local state
        CURRENT_STEP.with(|s| *s.borrow_mut() = None);
        TOTAL_TIME.with(|t| *t.borrow_mut() = None);
    } else if line.starts_with("ERROR:") {
        let error_msg = &line[6..];
        tracing::error!(task_id = %task_id, error = %error_msg, "Transcription error");
        // Python 脚本中的 ERROR: 消息，发送到 transcribe_error 事件
        let _ = app.emit(
            "transcribe_error",
            TranscribeProgress {
                task_id: task_id.to_string(),
                progress: 0.0,
                status: format!("Error: {}", error_msg),
                step: None,
                total_time: None,
            },
        );

        // Clear thread-local state
        CURRENT_STEP.with(|s| *s.borrow_mut() = None);
        TOTAL_TIME.with(|t| *t.borrow_mut() = None);
    }
}

/// Cancel transcription task
#[tauri::command]
pub async fn cancel_transcription(task_id: String) -> Result<(), String> {
    tracing::info!(task_id = %task_id, "Cancelling transcription task");
    let mut tasks = TASKS.lock().await;

    if let Some(handle) = tasks.remove(&task_id) {
        handle.abort();
        tracing::info!(task_id = %task_id, "Transcription task cancelled successfully");
        Ok(())
    } else {
        tracing::warn!(task_id = %task_id, "Cannot cancel: task not found");
        Err("Task not found".to_string())
    }
}

/// Check if AI dependencies are correctly installed
#[tauri::command]
pub async fn check_dependencies(app: AppHandle) -> Result<bool, String> {
    tracing::info!("Checking AI dependencies...");
    let is_dev = is_dev_mode();

    if is_dev {
        let core_dir = get_core_engine_dir(&app)?;
        let venv_python = core_dir.join(".venv/bin/python");
        if !venv_python.exists() {
            return Ok(false);
        }

        let output = tokio::process::Command::new(&venv_python)
            .arg("-c")
            .arg("import funasr; print('OK')")
            .output()
            .await
            .map_err(|e| e.to_string())?;

        return Ok(output.status.success());
    } else {
        // sidecar mode
        use tauri_plugin_shell::ShellExt;
        let shell = app.shell();
        let sidecar_command = shell
            .sidecar("monosub-engine")
            .map_err(|e| format!("Sidecar error: {}", e))?;

        // We run with --help to check if it can start and import everything
        let output = sidecar_command
            .arg("--help")
            .output()
            .await
            .map_err(|e| format!("Failed to run sidecar: {}", e))?;

        // If it exits with 0, it means imports were successful
        Ok(output.status.success())
    }
}

/// Repair AI dependencies by deleting venv and re-installing
#[tauri::command]
pub async fn install_dependencies(app: AppHandle) -> Result<(), String> {
    tracing::info!("Repairing AI dependencies...");

    let core_dir = get_core_engine_dir(&app)?;
    let venv_dir = core_dir.join(".venv");

    if venv_dir.exists() {
        tracing::info!("Removing existing venv: {:?}", venv_dir);
        std::fs::remove_dir_all(&venv_dir).map_err(|e| format!("Failed to remove venv: {}", e))?;
    }

    // Emit initial progress
    let _ = app.emit(
        "install_progress",
        TranscribeProgress {
            task_id: "install".to_string(),
            progress: 0.0,
            status: "Starting installation...".to_string(),
            step: None,
            total_time: None,
        },
    );

    // Run sidecar with a dummy task to trigger installation
    // Actually, we can just run sidecar with --help, it will trigger the launcher's install logic
    let is_dev = is_dev_mode();
    if is_dev {
        // In dev mode, we might need to manual run uv sync if we want to show it
        // But for now, let's just use the sidecar approach if possible
        // Actually dev mode sidecar is just running python.
        return Err("Manual repair not supported in dev mode. Please run 'uv sync' in core-engine directory.".to_string());
    }

    use tauri_plugin_shell::ShellExt;
    let shell = app.shell();
    let sidecar_command = shell
        .sidecar("monosub-engine")
        .map_err(|e| format!("Sidecar error: {}", e))?;

    let (mut rx, _child) = sidecar_command
        .arg("--help")
        .spawn()
        .map_err(|e| format!("Failed to spawn sidecar: {}", e))?;

    while let Some(event) = rx.recv().await {
        match event {
            tauri_plugin_shell::process::CommandEvent::Stdout(line) => {
                let line_str = String::from_utf8_lossy(&line);
                process_install_output_line(&app, &line_str);
            }
            tauri_plugin_shell::process::CommandEvent::Stderr(line) => {
                let line_str = String::from_utf8_lossy(&line);
                process_install_output_line(&app, &line_str);
            }
            tauri_plugin_shell::process::CommandEvent::Terminated(payload) => {
                if payload.code == Some(0) {
                    let _ = app.emit("install_complete", ());
                    return Ok(());
                } else {
                    return Err(format!("Installation failed with code {:?}", payload.code));
                }
            }
            _ => {}
        }
    }

    Ok(())
}

fn process_install_output_line(app: &AppHandle, line: &str) {
    if line.contains("MSG:") {
        let msg = line.split("MSG:").last().unwrap_or("").trim();
        let _ = app.emit(
            "install_progress",
            TranscribeProgress {
                task_id: "install".to_string(),
                progress: 0.0,
                status: msg.to_string(),
                step: None,
                total_time: None,
            },
        );
    } else if line.contains("PROGRESS:") {
        if let Ok(p) = line
            .split("PROGRESS:")
            .last()
            .unwrap_or("0")
            .trim()
            .parse::<f64>()
        {
            let _ = app.emit(
                "install_progress",
                TranscribeProgress {
                    task_id: "install".to_string(),
                    progress: p,
                    status: "Installing...".to_string(),
                    step: None,
                    total_time: None,
                },
            );
        }
    }
}
