use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use tauri_plugin_shell::ShellExt;
use tokio::sync::Mutex;
use std::collections::HashMap;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TranscribeProgress {
    pub task_id: String,
    pub progress: f64,
    pub status: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TranscribeRequest {
    pub id: String,
    pub path: String,
    pub output_path: String,
    pub format: Option<String>,
    pub language: Option<String>,
}

// Global task manager
type TaskMap = Arc<Mutex<HashMap<String, tokio::task::JoinHandle<()>>>>;

lazy_static::lazy_static! {
    static ref TASKS: TaskMap = Arc::new(Mutex::new(HashMap::new()));
}

/// Check if running in development mode
fn is_dev_mode() -> bool {
    // 检查是否存在 core-engine/.venv 目录
    let current_dir = std::env::current_dir().ok();
    if let Some(dir) = current_dir {
        if let Some(parent) = dir.parent() {
            return parent.join("core-engine/.venv").exists();
        }
    }
    false
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

    tracing::info!(
        task_id = %task_id,
        video_path = %video_path,
        output_path = %output_path,
        format = %format,
        language = %language,
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
        let _ = app.emit("transcribe_progress", TranscribeProgress {
            task_id: task_id.clone(),
            progress: 0.0,
            status: "Starting...".to_string(),
        });

        // 开发模式：使用 Python 直接运行
        // 生产模式：使用 sidecar 运行打包的可执行文件
        let is_dev = is_dev_mode();
        tracing::info!(task_id = %task_id, is_dev_mode = is_dev, "Running transcription");

        let result = if is_dev {
            run_dev_mode(&app, &task_id, &video_path, &output_path, &format, &language).await
        } else {
            run_sidecar(&app, &task_id, &video_path, &output_path, &format, &language).await
        };

        if let Err(e) = result {
            tracing::error!(task_id = %task_id, error = %e, "Transcription failed");
            let _ = app.emit("transcribe_progress", TranscribeProgress {
                task_id: task_id.clone(),
                progress: 0.0,
                status: format!("Error: {}", e),
            });
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
) -> Result<(), String> {
    tracing::info!(task_id = %task_id, "Starting sidecar process");
    let shell = app.shell();

    // 使用 sidecar 运行 monosub-engine
    let sidecar_command = shell
        .sidecar("monosub-engine")
        .map_err(|e| {
            tracing::error!(task_id = %task_id, error = %e, "Failed to create sidecar command");
            format!("Failed to create sidecar command: {}", e)
        })?;

    let (mut rx, _child) = sidecar_command
        .args(&[
            video_path,
            output_path,
            "--format",
            format,
            "--language",
            language,
        ])
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
                let _ = app.emit("transcribe_progress", TranscribeProgress {
                    task_id: task_id.to_string(),
                    progress: 0.0,
                    status: format!("STDERR: {}", line_str),
                });
            }
            tauri_plugin_shell::process::CommandEvent::Error(err) => {
                tracing::error!(task_id = %task_id, error = %err, "Sidecar process error");
                return Err(format!("Process error: {}", err));
            }
            tauri_plugin_shell::process::CommandEvent::Terminated(payload) => {
                tracing::info!(task_id = %task_id, exit_code = ?payload.code, "Sidecar process terminated");
                if payload.code != Some(0) {
                    return Err(format!("Process exited with code: {:?}", payload.code));
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
        "{} {} {} {} --format {} --language {}",
        venv_python.display(),
        main_py.display(),
        video_path,
        output_path,
        format,
        language
    );

    tracing::info!(task_id = %task_id, command = %cmd_str, "Executing Python command");

    let _ = app.emit("transcribe_progress", TranscribeProgress {
        task_id: task_id.to_string(),
        progress: 0.0,
        status: format!("CMD: {}", cmd_str),
    });

    let mut child = Command::new(&venv_python)
        .arg(&main_py)
        .arg(video_path)
        .arg(output_path)
        .arg("--format")
        .arg(format)
        .arg("--language")
        .arg(language)
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
            let _ = app_clone.emit("transcribe_progress", TranscribeProgress {
                task_id: task_id_clone.clone(),
                progress: 0.0,
                status: format!("STDERR: {}", line),
            });
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
        return Err(format!("Process exited with code: {:?}", status.code()));
    }

    Ok(())
}

/// Process output line from Python script
fn process_output_line(app: &AppHandle, task_id: &str, line: &str) {
    if line.starts_with("PROGRESS:") {
        if let Ok(progress) = line[9..].trim().parse::<f64>() {
            tracing::debug!(task_id = %task_id, progress = progress, "Transcription progress");
            let _ = app.emit("transcribe_progress", TranscribeProgress {
                task_id: task_id.to_string(),
                progress,
                status: "Processing...".to_string(),
            });
        }
    } else if line.starts_with("MSG:") {
        let message = &line[4..];
        tracing::info!(task_id = %task_id, message = %message, "Transcription message");
        let _ = app.emit("transcribe_progress", TranscribeProgress {
            task_id: task_id.to_string(),
            progress: 0.0,
            status: message.to_string(),
        });
    } else if line.starts_with("SUCCESS:") {
        let output_file = &line[8..];
        tracing::info!(task_id = %task_id, output_file = %output_file, "Transcription completed");
        let _ = app.emit("transcribe_complete", TranscribeProgress {
            task_id: task_id.to_string(),
            progress: 100.0,
            status: format!("Completed: {}", output_file),
        });
    } else if line.starts_with("ERROR:") {
        let error_msg = &line[6..];
        tracing::error!(task_id = %task_id, error = %error_msg, "Transcription error");
        let _ = app.emit("transcribe_progress", TranscribeProgress {
            task_id: task_id.to_string(),
            progress: 0.0,
            status: format!("Error: {}", error_msg),
        });
    }
}

/// Pause transcription task
#[tauri::command]
pub async fn pause_transcription(_task_id: String) -> Result<(), String> {
    Err("Pause not implemented yet".to_string())
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
