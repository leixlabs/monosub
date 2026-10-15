//! Transcription command: WAV extraction → audio.cpp inference → subtitle files.
//!
//! Tasks: P1-15 (rewrite) and P1-16 (cancel). See
//! docs/tech-spec/refactor-audiocpp.md §3, §5.3, §5.7, §5.8.
//!
//! The event contract is frozen (zero frontend change): the three events
//! `transcribe_progress` / `transcribe_complete` / `transcribe_error` carry
//! [`TranscribeProgress`] and the four [`StepInfo`] steps. What used to be the
//! Python stdout protocol (`STEP:`/`ELAPSED:`/`MSG:`/`SUCCESS:`/`ERROR:`,
//! parsed by `process_output_line`) is gone; the values are measured here and
//! the inference itself is delegated to `engine::transcribe`.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Instant, SystemTime};
use tauri::{AppHandle, Emitter};
use tokio::sync::Mutex;

use crate::commands::license;
use crate::commands::models::MODEL_NOT_INSTALLED;
use crate::engine::{self, Backend, EngineError, TranscribeOpts};
use crate::subtitle::{self, Format, PostProcessConfig};

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

/// Step names are part of the frozen contract: `FileItem.tsx` derives the i18n
/// key as `steps.<lowercase, spaces → _>`, so the wording must not change.
const STEP_NAMES: [&str; 4] = [
    "Extracting audio",
    "Loading model",
    "Transcribing",
    "Writing subtitle",
];
const TOTAL_STEPS: u32 = 4;

/// ffmpeg filter chain of the previous engine, reproduced verbatim.
///
/// The old Python extractor always ran with `enable_preprocessing=True`,
/// `normalize_volume=True`, `noise_reduction=True` — callers never passed the
/// flags — appending in this order: `highpass=f=80`, `lowpass=f=8000`,
/// `afftdn=nf=-25:nr=12:nt=w`, `loudnorm=I=-16:TP=-1.5:LRA=11`. Keeping it
/// keeps the recognition input identical (spec §3; PoC §7 / P0-9).
const AUDIO_FILTER_CHAIN: &str =
    "highpass=f=80,lowpass=f=8000,afftdn=nf=-25:nr=12:nt=w,loudnorm=I=-16:TP=-1.5:LRA=11";

/// One in-flight task: its abort handle plus everything needed to clean up
/// after a cancel (P1-16).
struct TaskEntry {
    handle: tokio::task::JoinHandle<()>,
    /// Every file this run will write, derived from `output_path` + formats.
    outputs: Vec<PathBuf>,
    started: SystemTime,
    /// Set once the run emitted `transcribe_complete`, so a cancel that races
    /// the final write cannot delete a finished result.
    completed: Arc<std::sync::atomic::AtomicBool>,
}

// Global task manager
type TaskMap = Arc<Mutex<HashMap<String, TaskEntry>>>;

lazy_static::lazy_static! {
    static ref TASKS: TaskMap = Arc::new(Mutex::new(HashMap::new()));
    static ref AUDIO_SEMAPHORE: Arc<tokio::sync::Semaphore> = Arc::new(tokio::sync::Semaphore::new(2));
    static ref TRANSCRIBE_SEMAPHORE: Arc<tokio::sync::Semaphore> = Arc::new(tokio::sync::Semaphore::new(1));
}

/// Everything one queued file needs, so the spawned task owns no borrows.
struct TaskParams {
    task_id: String,
    input: PathBuf,
    output: PathBuf,
    formats: Vec<Format>,
    language: String,
    model_id: String,
    backend: Backend,
    completed: Arc<std::sync::atomic::AtomicBool>,
}

/// A failure that has already been turned into user-showable text.
///
/// `status` is what `transcribe_error` carries (short, rendered by the UI);
/// `detail` is the full chain, logged with `tracing::error`.
struct TaskFailure {
    context: &'static str,
    status: String,
    detail: String,
}

impl TaskFailure {
    fn new(context: &'static str, status: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            context,
            status: status.into(),
            detail: detail.into(),
        }
    }
}

/// Start a transcription task.
///
/// Request-level preconditions are checked *before* anything is spawned, so a
/// rejected request cannot leave a half-registered task behind.
#[tauri::command]
pub async fn start_transcription(
    app: AppHandle,
    request: TranscribeRequest,
) -> Result<String, String> {
    let TranscribeRequest {
        id: task_id,
        path,
        output_path,
        format,
        language,
        device,
        enable_diarization,
    } = request;

    let format = format.unwrap_or_else(|| "srt".to_string());
    let language = language.unwrap_or_else(|| "zh".to_string());
    let backend = resolve_backend(device.as_deref());
    let enable_diarization = enable_diarization.unwrap_or(false);

    tracing::info!(
        task_id = %task_id,
        path = %path,
        output_path = %output_path,
        format = %format,
        language = %language,
        backend = ?backend,
        enable_diarization = %enable_diarization,
        "Starting transcription task"
    );

    let formats = match parse_formats(&format) {
        Ok(formats) => formats,
        Err(status) => {
            tracing::warn!(task_id = %task_id, format = %format, "rejected request");
            emit_error(&app, &task_id, &status);
            return Ok(task_id);
        }
    };

    if enable_diarization {
        // Speaker diarization is offline for v2.0 (spec §5.7); the licence gate
        // is kept so the check itself cannot regress.
        let status = if license::is_pro_licensed() {
            "Multi-speaker recognition is not available yet."
        } else {
            "Multi-speaker recognition requires a MonoSub Pro license."
        };
        tracing::warn!(task_id = %task_id, "rejected diarization request");
        emit_error(&app, &task_id, status);
        return Ok(task_id);
    }

    // Model gate (spec §5.2): the frontend recognises this code and opens the
    // model download dialog, so fail the invoke instead of emitting an event.
    let model_id = engine::catalog::default_model_id().to_string();
    if !engine::catalog::is_installed(&app, &model_id) {
        tracing::warn!(task_id = %task_id, model = %model_id, "model is not installed");
        return Err(MODEL_NOT_INSTALLED.to_string());
    }

    let input = PathBuf::from(path);
    let output = PathBuf::from(output_path);
    let outputs = derive_output_paths(&output, &formats);

    {
        let mut tasks = TASKS.lock().await;
        if let Some(existing) = tasks.get(&task_id) {
            if !existing.handle.is_finished() {
                tracing::warn!(task_id = %task_id, "Task already running");
                return Err("Task already running".to_string());
            }
            // A task that finished before the parent registered its handle
            // would otherwise wedge this id forever.
            tasks.remove(&task_id);
        }
    }

    let params = TaskParams {
        task_id: task_id.clone(),
        input,
        output,
        formats,
        language,
        model_id,
        backend,
        completed: Arc::new(std::sync::atomic::AtomicBool::new(false)),
    };
    let app_for_task = app.clone();
    let completed = Arc::clone(&params.completed);
    let handle = tokio::spawn(async move { run_transcription(app_for_task, params).await });

    TASKS.lock().await.insert(
        task_id.clone(),
        TaskEntry {
            handle,
            outputs,
            started: SystemTime::now(),
            completed,
        },
    );

    Ok(task_id)
}

/// Cancel a running task.
///
/// `engine::cli::spawn` marks the CLI child `kill_on_drop(true)` and
/// `engine::transcribe` owns a `Drop` guard for its temp directory, so aborting
/// the tokio task is sufficient to kill the child process and remove the temp
/// directory — the future is dropped, which drops both guards.
#[tauri::command]
pub async fn cancel_transcription(task_id: String) -> Result<(), String> {
    tracing::info!(task_id = %task_id, "Cancelling transcription task");

    let entry = TASKS.lock().await.remove(&task_id);
    let Some(entry) = entry else {
        tracing::warn!(task_id = %task_id, "Cannot cancel: task not found");
        return Err("Task not found".to_string());
    };

    entry.handle.abort();
    // Awaiting the aborted handle makes the cleanup below deterministic: the
    // future has been dropped, so the CLI child is dead and no writer can still
    // be mid-write.
    let _ = entry.handle.await;

    if entry.completed.load(std::sync::atomic::Ordering::SeqCst) {
        // The run finished while the cancel was in flight; its output is whole.
        tracing::info!(task_id = %task_id, "Task completed before the cancel arrived");
    } else {
        remove_partial_outputs(&entry.outputs, entry.started);
    }

    tracing::info!(task_id = %task_id, "Transcription task cancelled");
    Ok(())
}

/// Task body: run one file end to end, then report a single outcome.
async fn run_transcription(app: AppHandle, params: TaskParams) {
    if let Err(failure) = transcribe_file(&app, &params).await {
        tracing::error!(
            task_id = %params.task_id,
            detail = %failure.detail,
            "{}",
            failure.context
        );
        emit_error(&app, &params.task_id, &failure.status);
    }

    // Only unregister ourselves: a cancel followed by an immediate restart of
    // the same id leaves a newer entry in the map, which must survive.
    let mut tasks = TASKS.lock().await;
    let ours = tasks
        .get(&params.task_id)
        .is_some_and(|entry| Arc::ptr_eq(&entry.completed, &params.completed));
    if ours {
        tasks.remove(&params.task_id);
    }
}

async fn transcribe_file(app: &AppHandle, params: &TaskParams) -> Result<(), TaskFailure> {
    let mut steps = Steps::new();
    emit_queued(app, &params.task_id);

    // Step 1: 16 kHz mono s16le WAV, cached next to the media file. Audio
    // inputs are converted too — audio.cpp only accepts WAV (spec §3).
    steps.begin(app, &params.task_id, 1);
    let audio = {
        let _permit = AUDIO_SEMAPHORE.acquire().await;
        extract_wav(app, &params.input)
            .await
            .map_err(engine_failure)?
    };

    // Steps 2/3: the CLI loads the model inside the ASR session, so "Loading
    // model" is a transition only; the work happens under "Transcribing".
    let chunks = {
        let _permit = TRANSCRIBE_SEMAPHORE.acquire().await;
        steps.begin(app, &params.task_id, 2);
        steps.begin(app, &params.task_id, 3);

        let opts = TranscribeOpts {
            language: params.language.clone(),
            model_id: Some(params.model_id.clone()),
            backend: params.backend,
        };
        let on_progress = |done: usize, total: usize, message: &str| {
            let percent = if total == 0 {
                0.0
            } else {
                done as f64 * 100.0 / total as f64
            };
            steps.progress(app, &params.task_id, percent, message);
        };

        engine::transcribe(app, &params.task_id, audio.path(), &opts, &on_progress)
            .await
            .map_err(engine_failure)?
    };

    // Step 4: post-process and write every requested format.
    steps.begin(app, &params.task_id, 4);
    if let Some(parent) = params.output.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).map_err(|e| {
                TaskFailure::new(
                    "could not create the output directory",
                    "Could not create the output folder",
                    format!("create {}: {}", parent.display(), e),
                )
            })?;
        }
    }

    // An empty chunk list means "no speech": still write valid empty files.
    let segments = subtitle::postprocess(&chunks, &PostProcessConfig::default());
    for (index, format) in params.formats.iter().enumerate() {
        let path = output_path_for(&params.output, index, *format);
        subtitle::write(&segments, &path, *format).map_err(write_failure)?;
        tracing::info!(
            task_id = %params.task_id,
            path = %path.display(),
            format = %format.extension(),
            "Subtitle written"
        );
    }

    // Exactly one completion event, after every format landed.
    params
        .completed
        .store(true, std::sync::atomic::Ordering::SeqCst);
    let _ = app.emit(
        "transcribe_complete",
        TranscribeProgress {
            task_id: params.task_id.clone(),
            progress: 100.0,
            status: format!("Completed: {}", params.output.display()),
            step: None,
            total_time: Some(steps.total_seconds()),
        },
    );
    Ok(())
}

/// Emits the exported step transitions with real elapsed seconds.
///
/// The previous implementation fed `step_elapsed`/`total_elapsed` from the
/// Python `STEP_ELAPSED:`/`ELAPSED:` protocol; they are measured here directly.
struct Steps {
    start: Instant,
    step_start: Instant,
    current: u32,
}

impl Steps {
    fn new() -> Self {
        let now = Instant::now();
        Self {
            start: now,
            step_start: now,
            current: 0,
        }
    }

    /// Enter `step` (1..=4) and emit its transition event.
    fn begin(&mut self, app: &AppHandle, task_id: &str, step: u32) {
        self.current = step;
        self.step_start = Instant::now();
        self.emit(app, task_id, 0.0, step_name(step));
    }

    /// In-step update; `status` carries the engine's progress message.
    fn progress(&self, app: &AppHandle, task_id: &str, progress: f64, status: &str) {
        self.emit(app, task_id, progress, status);
    }

    fn emit(&self, app: &AppHandle, task_id: &str, progress: f64, status: &str) {
        let _ = app.emit(
            "transcribe_progress",
            TranscribeProgress {
                task_id: task_id.to_string(),
                progress,
                status: status.to_string(),
                step: Some(StepInfo {
                    current_step: self.current,
                    total_steps: TOTAL_STEPS,
                    // Never taken from `status`: the i18n keys depend on it.
                    step_name: step_name(self.current).to_string(),
                    step_elapsed: self.step_start.elapsed().as_secs_f64(),
                    total_elapsed: self.start.elapsed().as_secs_f64(),
                }),
                total_time: None,
            },
        );
    }

    fn total_seconds(&self) -> f64 {
        self.start.elapsed().as_secs_f64()
    }
}

fn step_name(step: u32) -> &'static str {
    STEP_NAMES
        .get(step.saturating_sub(1) as usize)
        .copied()
        .unwrap_or("Processing")
}

fn emit_queued(app: &AppHandle, task_id: &str) {
    let _ = app.emit(
        "transcribe_progress",
        TranscribeProgress {
            task_id: task_id.to_string(),
            progress: 0.0,
            status: "Pending in queue...".to_string(),
            step: None,
            total_time: None,
        },
    );
}

fn emit_error(app: &AppHandle, task_id: &str, status: &str) {
    let _ = app.emit(
        "transcribe_error",
        TranscribeProgress {
            task_id: task_id.to_string(),
            progress: 0.0,
            status: status.to_string(),
            step: None,
            total_time: None,
        },
    );
}

/// Short status for a failed engine call (spec §5.3).
///
/// These are i18n **keys**, not sentences: the frontend resolves
/// `errors.<status>` when such a key exists and otherwise shows the raw string,
/// so a technical writer message still comes through verbatim. The full detail
/// (including `EngineError::Failed::stderr_tail`) always goes to the log.
fn engine_failure(err: EngineError) -> TaskFailure {
    let status = match &err {
        EngineError::Missing(_) => "engineMissing",
        EngineError::ModelNotInstalled(_) => "modelNotInstalled",
        EngineError::AudioExtractFailed(_) => "audioExtractFailed",
        EngineError::Failed { .. }
        | EngineError::Io(_)
        | EngineError::InvalidAudio(_)
        | EngineError::Protocol(_) => "engineFailed",
    };
    TaskFailure::new("transcription failed", status, engine_error_detail(&err))
}

/// Full error text, including the CLI's stderr tail.
fn engine_error_detail(err: &EngineError) -> String {
    match err {
        EngineError::Failed {
            message,
            stderr_tail,
        } => format!("{} — stderr: {}", message, stderr_tail),
        other => other.to_string(),
    }
}

/// A writer failure is usually user-facing (e.g. `json` without word-level
/// data), so surface the writer's own message; pure I/O failures only carry a
/// path, which stays in the log.
fn write_failure(err: EngineError) -> TaskFailure {
    let status = match &err {
        EngineError::Failed { message, .. } | EngineError::Protocol(message) => message.clone(),
        _ => "Could not write the subtitle file".to_string(),
    };
    TaskFailure::new("writing the subtitle failed", status, engine_error_detail(&err))
}

// --- Step 1: ffmpeg ---------------------------------------------------------

/// A WAV produced for one run.
///
/// The cached copy lives next to the media file and is kept (like the previous
/// engine); when the input has no usable location it goes to the temp dir and
/// is removed on drop, including when the task is cancelled.
struct ExtractedAudio {
    path: PathBuf,
    temporary: bool,
}

impl ExtractedAudio {
    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for ExtractedAudio {
    fn drop(&mut self) {
        if self.temporary {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

fn get_audio_cache_path(media_path: &Path) -> Option<PathBuf> {
    let parent = media_path.parent()?;
    let stem = media_path.file_stem()?.to_str()?;
    Some(parent.join(format!("{}_audio.wav", stem)))
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

async fn extract_wav(app: &AppHandle, input: &Path) -> Result<ExtractedAudio, EngineError> {
    let (cache, temporary) = match get_audio_cache_path(input) {
        Some(path) => (path, false),
        None => (
            std::env::temp_dir().join(format!("monosub-extract-{}.wav", uuid::Uuid::new_v4())),
            true,
        ),
    };

    if is_cache_fresh(&cache, input) {
        tracing::info!(
            source = %input.display(),
            wav = %cache.display(),
            "reusing cached extracted audio"
        );
        return Ok(ExtractedAudio {
            path: cache,
            temporary,
        });
    }

    let ffmpeg = crate::commands::video::get_ffmpeg_path(Some(app));
    match run_ffmpeg(&ffmpeg, input, &cache, Some(AUDIO_FILTER_CHAIN)).await {
        Ok(()) => {
            tracing::info!(
                source = %input.display(),
                wav = %cache.display(),
                "audio extracted with the filter chain"
            );
            Ok(ExtractedAudio {
                path: cache,
                temporary,
            })
        }
        Err(filtered) => {
            // Mirrors the old engine's `_extract_basic` fallback: an input
            // ffmpeg cannot filter may still decode without `-af`. Only a
            // genuine ffmpeg failure reaches here.
            tracing::warn!(
                source = %input.display(),
                error = %filtered,
                "filtered extraction failed; retrying without filters"
            );
            let _ = std::fs::remove_file(&cache);
            run_ffmpeg(&ffmpeg, input, &cache, None).await?;
            Ok(ExtractedAudio {
                path: cache,
                temporary,
            })
        }
    }
}

/// The exact ffmpeg argv, shared by the extraction and its no-filter fallback.
fn ffmpeg_args(input: &Path, output: &Path, filters: Option<&str>) -> Vec<std::ffi::OsString> {
    let mut args: Vec<std::ffi::OsString> = vec![std::ffi::OsString::from("-i")];
    args.push(input.into());
    args.push("-vn".into());
    if let Some(filters) = filters {
        args.push("-af".into());
        args.push(filters.into());
    }
    args.extend(
        ["-acodec", "pcm_s16le", "-ar", "16000", "-ac", "1", "-y"]
            .map(std::ffi::OsString::from),
    );
    args.push(output.into());
    args
}

async fn run_ffmpeg(
    ffmpeg: &str,
    input: &Path,
    output: &Path,
    filters: Option<&str>,
) -> Result<(), EngineError> {
    let result = tokio::process::Command::new(ffmpeg)
        .args(ffmpeg_args(input, output, filters))
        .output()
        .await
        .map_err(|e| {
            EngineError::AudioExtractFailed(format!("failed to run ffmpeg: {}", e))
        })?;

    if !result.status.success() {
        return Err(EngineError::AudioExtractFailed(format!(
            "ffmpeg exited with {:?}: {}",
            result.status.code(),
            stderr_tail(&result.stderr)
        )));
    }
    if !output.is_file() {
        return Err(EngineError::AudioExtractFailed(format!(
            "ffmpeg reported success but {} was not created",
            output.display()
        )));
    }
    Ok(())
}

/// Last few stderr lines: enough to identify a failure, without the banner.
fn stderr_tail(stderr: &[u8]) -> String {
    let text = String::from_utf8_lossy(stderr);
    let lines: Vec<&str> = text.lines().filter(|line| !line.trim().is_empty()).collect();
    let start = lines.len().saturating_sub(3);
    lines[start..].join(" | ")
}

// --- Format parsing and output paths (spec §3) ------------------------------

/// Parse the comma-separated `format` field: trim, lowercase, drop empties,
/// reject unknown values, deduplicate preserving order.
fn parse_formats(value: &str) -> Result<Vec<Format>, String> {
    let mut formats: Vec<Format> = Vec::new();
    for raw in value.split(',') {
        let token = raw.trim().to_lowercase();
        if token.is_empty() {
            continue;
        }
        let Some(format) = Format::parse(&token) else {
            return Err(format!("Unsupported subtitle format: {}", token));
        };
        if !formats.contains(&format) {
            formats.push(format);
        }
    }

    if formats.is_empty() {
        return Err("No valid subtitle format specified".to_string());
    }
    Ok(formats)
}

/// The first format writes to `output_path` verbatim, each later one replaces
/// only the extension (`os.path.splitext` semantics).
fn output_path_for(output_path: &Path, index: usize, format: Format) -> PathBuf {
    if index == 0 {
        return output_path.to_path_buf();
    }
    let mut path = output_path.to_path_buf();
    path.set_extension(format.extension());
    path
}

fn derive_output_paths(output_path: &Path, formats: &[Format]) -> Vec<PathBuf> {
    formats
        .iter()
        .enumerate()
        .map(|(index, format)| output_path_for(output_path, index, *format))
        .collect()
}

/// The frontend does not send `device` today; `auto`/absent means "the engine
/// picks", which on macOS is Metal (engine retries CPU itself). Other platforms
/// stay on CPU until the Phase 4 platform work.
fn resolve_backend(device: Option<&str>) -> Backend {
    match device.map(|value| value.trim().to_lowercase()) {
        Some(value) if value == "cpu" => Backend::Cpu,
        _ if cfg!(target_os = "macos") => Backend::Metal,
        _ => Backend::Cpu,
    }
}

/// Remove the subtitle files a cancelled run may have left behind.
///
/// A pre-existing file this run never opened (the cancel arrived before the
/// write step) predates `started` and is left alone.
fn remove_partial_outputs(outputs: &[PathBuf], started: SystemTime) {
    for path in outputs {
        let touched_by_this_run = std::fs::metadata(path)
            .and_then(|meta| meta.modified())
            .map(|modified| modified >= started)
            .unwrap_or(false);
        if !touched_by_this_run {
            continue;
        }
        match std::fs::remove_file(path) {
            Ok(()) => tracing::info!(
                path = %path.display(),
                "removed partially written subtitle after cancel"
            ),
            Err(e) => tracing::warn!(
                path = %path.display(),
                error = %e,
                "could not remove partial subtitle"
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    /// Creates a unique scratch directory (the crate has no `tempfile`).
    fn scratch_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("monosub-transcribe-{}-{}", tag, uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn touch(path: &Path, offset_secs: i64) {
        let now = SystemTime::now();
        let time = if offset_secs >= 0 {
            now + Duration::from_secs(offset_secs as u64)
        } else {
            now - Duration::from_secs(offset_secs.unsigned_abs())
        };
        std::fs::File::options()
            .write(true)
            .open(path)
            .unwrap()
            .set_modified(time)
            .unwrap();
    }

    #[test]
    fn parses_a_comma_separated_format_list_in_order() {
        assert_eq!(
            parse_formats("srt,vtt,txt").unwrap(),
            vec![Format::Srt, Format::Vtt, Format::Txt]
        );
        assert_eq!(
            parse_formats("srt,vtt,txt,md,json").unwrap(),
            vec![
                Format::Srt,
                Format::Vtt,
                Format::Txt,
                Format::Md,
                Format::Json
            ]
        );
        assert_eq!(parse_formats("srt").unwrap(), vec![Format::Srt]);
    }

    #[test]
    fn format_list_is_trimmed_lowercased_and_deduplicated() {
        assert_eq!(parse_formats("SRT, srt").unwrap(), vec![Format::Srt]);
        assert_eq!(parse_formats(" srt , txt ").unwrap(), vec![Format::Srt, Format::Txt]);
        assert_eq!(
            parse_formats("vtt,srt,vtt,txt,Srt").unwrap(),
            vec![Format::Vtt, Format::Srt, Format::Txt]
        );
        // Empty entries between separators are dropped, not rejected.
        assert_eq!(parse_formats("srt,,vtt,").unwrap(), vec![Format::Srt, Format::Vtt]);
    }

    #[test]
    fn rejects_unknown_and_empty_format_lists() {
        assert!(parse_formats("srt,bogus").is_err());
        assert_eq!(
            parse_formats("srt,bogus").unwrap_err(),
            "Unsupported subtitle format: bogus"
        );
        assert!(parse_formats("").is_err());
        assert!(parse_formats("   ").is_err());
        assert!(parse_formats(",").is_err());
        assert!(parse_formats(" , ,").is_err());
    }

    #[test]
    fn first_format_uses_the_requested_output_path_verbatim() {
        let output = Path::new("/media/clip.srt");
        assert_eq!(output_path_for(output, 0, Format::Srt), PathBuf::from("/media/clip.srt"));
        // Even when the extension disagrees with the first format.
        assert_eq!(output_path_for(output, 0, Format::Vtt), PathBuf::from("/media/clip.srt"));
    }

    #[test]
    fn later_formats_replace_only_the_extension() {
        let output = Path::new("/media/clip.srt");
        assert_eq!(output_path_for(output, 1, Format::Vtt), PathBuf::from("/media/clip.vtt"));
        assert_eq!(output_path_for(output, 2, Format::Txt), PathBuf::from("/media/clip.txt"));
        assert_eq!(output_path_for(output, 3, Format::Md), PathBuf::from("/media/clip.md"));
        assert_eq!(output_path_for(output, 4, Format::Json), PathBuf::from("/media/clip.json"));

        // Multiple dots: only the last extension is replaced.
        let dotted = Path::new("/media/a.b.c.mp4");
        assert_eq!(
            output_path_for(dotted, 1, Format::Srt),
            PathBuf::from("/media/a.b.c.srt")
        );
        assert_eq!(output_path_for(dotted, 0, Format::Srt), dotted.to_path_buf());

        // No extension at all.
        let bare = Path::new("/media/clip");
        assert_eq!(output_path_for(bare, 0, Format::Srt), bare.to_path_buf());
        assert_eq!(output_path_for(bare, 1, Format::Srt), PathBuf::from("/media/clip.srt"));

        // A custom output directory, and a relative path.
        assert_eq!(
            output_path_for(Path::new("/custom/out/clip.srt"), 1, Format::Vtt),
            PathBuf::from("/custom/out/clip.vtt")
        );
        assert_eq!(
            output_path_for(Path::new("clip.srt"), 1, Format::Vtt),
            PathBuf::from("clip.vtt")
        );
    }

    #[test]
    fn derives_one_output_path_per_requested_format() {
        let formats = vec![Format::Srt, Format::Vtt, Format::Txt];
        assert_eq!(
            derive_output_paths(Path::new("/media/clip.srt"), &formats),
            vec![
                PathBuf::from("/media/clip.srt"),
                PathBuf::from("/media/clip.vtt"),
                PathBuf::from("/media/clip.txt"),
            ]
        );
    }

    #[test]
    fn audio_cache_lives_next_to_the_media_file() {
        assert_eq!(
            get_audio_cache_path(Path::new("/media/clip.mov")).unwrap(),
            PathBuf::from("/media/clip_audio.wav")
        );
        // An audio input is converted now, and uses the same rule.
        assert_eq!(
            get_audio_cache_path(Path::new("/media/song.mp3")).unwrap(),
            PathBuf::from("/media/song_audio.wav")
        );
        assert_eq!(
            get_audio_cache_path(Path::new("/media/a.b.c.mp4")).unwrap(),
            PathBuf::from("/media/a.b.c_audio.wav")
        );
        assert!(get_audio_cache_path(Path::new("/")).is_none());
    }

    #[test]
    fn audio_cache_is_fresh_when_not_older_than_the_source() {
        let dir = scratch_dir("cache");
        let source = dir.join("clip.mov");
        let cache = dir.join("clip_audio.wav");
        std::fs::write(&source, b"source").unwrap();
        std::fs::write(&cache, b"cache").unwrap();

        // Equal mtimes (the Python `>=` rule).
        touch(&source, -100);
        touch(&cache, -100);
        assert!(is_cache_fresh(&cache, &source));

        // Newer cache.
        touch(&source, -200);
        touch(&cache, -100);
        assert!(is_cache_fresh(&cache, &source));

        // Stale cache.
        touch(&source, -100);
        touch(&cache, -200);
        assert!(!is_cache_fresh(&cache, &source));

        // Missing cache / missing source.
        assert!(!is_cache_fresh(&dir.join("absent_audio.wav"), &source));
        assert!(!is_cache_fresh(&cache, &dir.join("absent.mov")));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn extracts_wav_with_the_previous_engines_filter_chain() {
        // PoC §7 / P0-9: dropping the filters would change the recognition
        // input, so the exact chain is part of the ported behaviour.
        let args = ffmpeg_args(
            Path::new("/media/clip.mov"),
            Path::new("/media/clip_audio.wav"),
            Some(AUDIO_FILTER_CHAIN),
        );
        let args: Vec<String> = args
            .iter()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect();

        assert_eq!(
            args,
            vec![
                "-i",
                "/media/clip.mov",
                "-vn",
                "-af",
                "highpass=f=80,lowpass=f=8000,afftdn=nf=-25:nr=12:nt=w,loudnorm=I=-16:TP=-1.5:LRA=11",
                "-acodec",
                "pcm_s16le",
                "-ar",
                "16000",
                "-ac",
                "1",
                "-y",
                "/media/clip_audio.wav",
            ]
        );
    }

    #[test]
    fn basic_fallback_extraction_has_no_filter_argument() {
        let args = ffmpeg_args(
            Path::new("/media/clip.mp3"),
            Path::new("/media/clip_audio.wav"),
            None,
        );
        assert!(!args.iter().any(|arg| arg == "-af"));
        // Everything else is unchanged, so the fallback output is still 16 kHz mono.
        let args: Vec<String> = args
            .iter()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect();
        assert!(args.windows(2).any(|pair| pair == ["-ar", "16000"]));
        assert!(args.windows(2).any(|pair| pair == ["-ac", "1"]));
    }

    #[test]
    fn backend_defaults_to_metal_on_macos_and_honours_cpu() {
        assert_eq!(resolve_backend(Some("cpu")), Backend::Cpu);
        assert_eq!(resolve_backend(Some("CPU")), Backend::Cpu);
        let expected = if cfg!(target_os = "macos") {
            Backend::Metal
        } else {
            Backend::Cpu
        };
        assert_eq!(resolve_backend(None), expected);
        assert_eq!(resolve_backend(Some("auto")), expected);
        assert_eq!(resolve_backend(Some("cuda")), expected);
    }

    #[test]
    fn all_silence_still_writes_one_empty_file_per_format() {
        // `engine::transcribe` returns no chunks for an all-silent input (PoC
        // §4.3); that must still produce valid, empty subtitle files.
        let dir = scratch_dir("silence");
        let output = dir.join("clip.srt");
        let formats = vec![
            Format::Srt,
            Format::Vtt,
            Format::Txt,
            Format::Md,
            Format::Json,
        ];

        let segments = subtitle::postprocess(&[], &PostProcessConfig::default());
        assert!(segments.is_empty());

        for (index, format) in formats.iter().enumerate() {
            let path = output_path_for(&output, index, *format);
            subtitle::write(&segments, &path, *format).expect("empty subtitle must still write");
            assert!(path.is_file(), "{} must exist", path.display());
            assert_eq!(path.extension().unwrap(), format.extension());
        }

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The engine-failure statuses are i18n keys, not sentences: the frontend
    /// resolves `errors.<status>` and falls back to the raw string. Pin the keys
    /// so a rename cannot silently make every locale dead again (review P2-2).
    #[test]
    fn engine_failures_emit_i18n_keys_that_exist_in_the_locales() {
        let cases = [
            (
                EngineError::Missing("no binary".into()),
                "engineMissing",
            ),
            (
                EngineError::AudioExtractFailed("ffmpeg".into()),
                "audioExtractFailed",
            ),
            (
                EngineError::Failed {
                    message: "exit 1".into(),
                    stderr_tail: "boom".into(),
                },
                "engineFailed",
            ),
            (EngineError::Io("disk".into()), "engineFailed"),
            (EngineError::InvalidAudio("layout".into()), "engineFailed"),
            (EngineError::Protocol("bad json".into()), "engineFailed"),
        ];

        let locales = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("src/i18n/locales");

        for (err, expected) in cases {
            let failure = engine_failure(err);
            assert_eq!(failure.status, expected);

            // Every locale must define the key, or the fallback hides it forever.
            for entry in std::fs::read_dir(&locales).expect("read locales") {
                let path = entry.expect("entry").path();
                if path.extension().and_then(|e| e.to_str()) != Some("json") {
                    continue;
                }
                let body = std::fs::read_to_string(&path).expect("read locale");
                let json: serde_json::Value = serde_json::from_str(&body)
                    .unwrap_or_else(|e| panic!("{}: {}", path.display(), e));
                assert!(
                    json.get("errors").and_then(|e| e.get(expected)).is_some(),
                    "{} is missing errors.{}",
                    path.display(),
                    expected
                );
            }
        }
    }

    #[test]
    fn step_names_are_the_frozen_contract() {
        for (index, name) in STEP_NAMES.iter().enumerate() {
            assert_eq!(step_name(index as u32 + 1), *name);
            // FileItem.tsx maps these to `steps.<snake_case>` i18n keys.
            let key = name.to_lowercase().replace(' ', "_");
            assert_eq!(
                key,
                ["extracting_audio", "loading_model", "transcribing", "writing_subtitle"][index]
            );
        }
        assert_eq!(TOTAL_STEPS, STEP_NAMES.len() as u32);
    }

    #[test]
    fn cancelled_runs_only_remove_files_they_touched() {
        let dir = scratch_dir("cancel");
        let started = SystemTime::now();

        let untouched = dir.join("old.srt");
        std::fs::write(&untouched, b"previous run").unwrap();
        touch(&untouched, -600);

        // Simulate a file this run wrote after it started.
        let partial = dir.join("new.vtt");
        std::fs::write(&partial, b"half").unwrap();
        touch(&partial, 1);
        let missing = dir.join("never.vtt");

        remove_partial_outputs(&[untouched.clone(), partial.clone(), missing.clone()], started);

        assert!(untouched.is_file(), "a pre-existing output must survive");
        assert!(!partial.exists(), "a file written by this run must be removed");
        assert!(!missing.exists());

        let _ = std::fs::remove_dir_all(&dir);
    }
}
