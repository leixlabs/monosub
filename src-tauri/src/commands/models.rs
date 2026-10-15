//! Model management commands: list / download / cancel / remove / open dir.
//!
//! Task: P1-14. See docs/tech-spec/refactor-audiocpp.md §5.2.
//!
//! Downloads stream into `<app_data_dir>/models/<target_directory>/<file>.part`,
//! verify size (and sha256 when the manifest pins one) and are then renamed onto
//! the final name, so a partial transfer never appears under the real name.
//! Progress is reported through `model_download_progress` (throttled to one
//! event per [`PROGRESS_INTERVAL`]); completion and failure use
//! `model_download_complete` / `model_download_error`.

use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use futures_util::StreamExt;
use serde::Serialize;
use sha2::{Digest, Sha256};
use tauri::{AppHandle, Emitter, Manager};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::commands::video::open_in_finder;
use crate::engine::catalog::{self, ModelFile, ModelSpec};

/// Sentinel error string used by the transcription layer to signal a missing
/// model (spec §5.2/§5.3). `start_transcription` (P1-15) returns exactly this
/// value and the frontend matches on it (P1-19).
pub const MODEL_NOT_INSTALLED: &str = "MODEL_NOT_INSTALLED";

/// At most one `model_download_progress` event per interval (spec §5.2).
const PROGRESS_INTERVAL: Duration = Duration::from_millis(200);

/// Connection / per-read timeouts. A 254 MB download must not have a total
/// deadline, so only connect and stall detection are bounded.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(15);
const READ_TIMEOUT: Duration = Duration::from_secs(30);

/// How long `cancel_model_download` waits for the transfer task to delete its
/// `.part` file before returning.
const CANCEL_JOIN_TIMEOUT: Duration = Duration::from_secs(5);

/// How often an in-flight transfer re-checks the cancel flag while waiting for
/// the next chunk, so cancelling a stalled connection still stops promptly.
const CANCEL_POLL_INTERVAL: Duration = Duration::from_millis(100);

/// Read buffer for the streaming sha256 pass.
const HASH_BUFFER: usize = 64 * 1024;

/// One manifest file of a model, as `list_models` reports it.
///
/// Mirrors `catalog::ModelFile`, which does not derive `Serialize` and so
/// cannot cross the IPC boundary (see the P1-14 report).
#[derive(Debug, Clone, Serialize)]
pub struct ModelFileStatus {
    pub path: String,
    /// Primary URL followed by fallback mirrors, tried in order.
    pub urls: Vec<String>,
    pub size: u64,
    pub sha256: Option<String>,
}

/// `list_models` payload: the manifest entry plus its install state, flat, so
/// the frontend can render model metadata and download size without another
/// round trip.
#[derive(Debug, Clone, Serialize)]
pub struct ModelStatus {
    pub id: String,
    /// audio.cpp `family`, e.g. `sense_asr`.
    pub family: String,
    pub display_name: String,
    /// Directory under `<app_data_dir>/models/` that `remove_model` deletes.
    pub target_directory: String,
    pub model_file: String,
    pub files: Vec<ModelFileStatus>,
    pub languages: Vec<String>,
    pub license: String,
    pub attribution: Option<String>,
    /// Every manifest file present with a matching size.
    pub installed: bool,
    /// Bytes currently on disk (`0` when not installed).
    pub size_on_disk: u64,
}

impl ModelStatus {
    fn from_catalog(status: crate::engine::catalog::ModelStatus) -> Self {
        let spec = status.spec;
        Self {
            id: spec.id,
            family: spec.family,
            display_name: spec.display_name,
            target_directory: spec.target_directory,
            model_file: spec.model_file,
            files: spec
                .files
                .into_iter()
                .map(|file| ModelFileStatus {
                    path: file.path,
                    urls: file.urls,
                    size: file.size,
                    sha256: file.sha256,
                })
                .collect(),
            languages: spec.languages,
            license: spec.license,
            attribution: spec.attribution,
            installed: status.installed,
            size_on_disk: status.size_on_disk,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
struct DownloadProgressPayload {
    id: String,
    downloaded: u64,
    total: u64,
}

#[derive(Debug, Clone, Serialize)]
struct DownloadCompletePayload {
    id: String,
}

#[derive(Debug, Clone, Serialize)]
struct DownloadErrorPayload {
    id: String,
    message: String,
}

/// A running download, kept so `cancel_model_download` can stop it and know
/// that its `.part` file is gone before returning.
struct DownloadEntry {
    cancel: Arc<AtomicBool>,
    task: tokio::task::JoinHandle<()>,
}

type DownloadRegistry = Arc<tokio::sync::Mutex<HashMap<String, DownloadEntry>>>;

lazy_static::lazy_static! {
    static ref DOWNLOADS: DownloadRegistry = Arc::new(tokio::sync::Mutex::new(HashMap::new()));
}

/// One step per model: the state file transfer can end in.
#[derive(Debug)]
enum DownloadError {
    /// The user cancelled; no `.part` file is left behind and no error event is
    /// emitted (the cancel command itself reports the outcome).
    Cancelled,
    /// Transport or HTTP failure for one URL; the caller may try the next one.
    Network(String),
    /// Size or sha256 mismatch against the manifest; terminal, no fallback.
    Verify(String),
    /// Local filesystem failure.
    Io(String),
    /// The manifest entry is unusable (no url, escaping path, …).
    InvalidManifest(String),
}

impl std::fmt::Display for DownloadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DownloadError::Cancelled => write!(f, "cancelled"),
            DownloadError::Network(m) => write!(f, "download failed: {}", m),
            DownloadError::Verify(m) => write!(f, "verification failed: {}", m),
            DownloadError::Io(m) => write!(f, "io error: {}", m),
            DownloadError::InvalidManifest(m) => write!(f, "invalid manifest: {}", m),
        }
    }
}

/// Catalogue plus install state for every model in `models.json`.
#[tauri::command]
pub async fn list_models(app: AppHandle) -> Result<Vec<ModelStatus>, String> {
    let statuses = catalog::list(Some(&app)).map_err(|e| e.to_string())?;
    Ok(statuses.into_iter().map(ModelStatus::from_catalog).collect())
}

/// Start downloading `id` in the background and return immediately.
///
/// Progress/completion arrive as `model_download_progress` /
/// `model_download_complete` / `model_download_error`. Starting a download that
/// is already in flight is an idempotent no-op.
#[tauri::command]
pub async fn download_model(app: AppHandle, id: String) -> Result<(), String> {
    let spec = find_spec(&app, &id)?;

    let mut downloads = DOWNLOADS.lock().await;
    // A task that finished before it could deregister itself is not in flight.
    if downloads
        .get(&id)
        .is_some_and(|entry| !entry.task.is_finished())
    {
        tracing::info!(model = %id, "Download already in progress, ignoring duplicate start");
        return Ok(());
    }
    downloads.remove(&id);

    let cancel = Arc::new(AtomicBool::new(false));
    let task_cancel = Arc::clone(&cancel);
    let task_app = app.clone();
    let task = tokio::spawn(async move { run_download(task_app, spec, task_cancel).await });
    downloads.insert(id, DownloadEntry { cancel, task });
    Ok(())
}

/// Stop the in-flight download for `id` and remove its `.part` file.
///
/// Returns once the transfer task has deleted its `.part` (or after
/// [`CANCEL_JOIN_TIMEOUT`]). Cancelling an unknown or already finished id is a
/// no-op that returns `Ok(())`, so the command is idempotent.
#[tauri::command]
pub async fn cancel_model_download(id: String) -> Result<(), String> {
    let was_in_flight = DOWNLOADS
        .lock()
        .await
        .get(&id)
        .is_some_and(|entry| !entry.task.is_finished());
    cancel_in_flight(&id).await;
    if was_in_flight {
        tracing::info!(model = %id, "Model download cancelled");
    } else {
        tracing::debug!(model = %id, "cancel_model_download: nothing in flight");
    }
    Ok(())
}

/// Delete `<app_data_dir>/models/<target_directory>` recursively.
///
/// A model that is not installed is a no-op. An in-flight download for the same
/// id is cancelled first, otherwise it would recreate the directory it is
/// writing into. The target directory is resolved and checked to live inside the
/// models root before anything is deleted.
#[tauri::command]
pub async fn remove_model(app: AppHandle, id: String) -> Result<(), String> {
    cancel_in_flight(&id).await;

    let dir = catalog::model_dir(&app, &id).map_err(|e| e.to_string())?;
    if !dir.exists() {
        tracing::info!(model = %id, path = %dir.display(), "remove_model: not installed");
        return Ok(());
    }

    let root = models_root(&app)?;
    remove_model_dir(&root, &dir)?;
    tracing::info!(model = %id, path = %dir.display(), "Removed model files");
    Ok(())
}

/// Create (if needed) and reveal `<app_data_dir>/models` in the file manager.
#[tauri::command]
pub async fn open_models_dir(app: AppHandle) -> Result<(), String> {
    let root = models_root(&app)?;
    std::fs::create_dir_all(&root)
        .map_err(|e| format!("failed to create {}: {}", root.display(), e))?;
    open_in_finder(root.to_string_lossy().to_string()).await
}

/// Stop the transfer task registered for `id`, if any, and wait for it to clean
/// up its `.part` file.
async fn cancel_in_flight(id: &str) {
    let entry = DOWNLOADS.lock().await.remove(id);
    let Some(entry) = entry else { return };
    entry.cancel.store(true, Ordering::SeqCst);
    match tokio::time::timeout(CANCEL_JOIN_TIMEOUT, entry.task).await {
        Ok(Ok(())) => tracing::debug!(model = %id, "Stopped in-flight download"),
        Ok(Err(join_error)) => {
            tracing::warn!(model = %id, error = %join_error, "Download task ended abnormally")
        }
        Err(_) => tracing::warn!(
            model = %id,
            "Timed out waiting for the download task to stop; it will clean up on its own"
        ),
    }
}

fn find_spec(app: &AppHandle, id: &str) -> Result<ModelSpec, String> {
    catalog::catalog(Some(app))
        .map_err(|e| e.to_string())?
        .into_iter()
        .find(|spec| spec.id == id)
        .ok_or_else(|| format!("unknown model id: {}", id))
}

fn models_root(app: &AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_data_dir()
        .map(|dir| dir.join("models"))
        .map_err(|e| format!("failed to resolve app data dir: {}", e))
}

/// Download every file of `spec` in manifest order, emitting throttled progress.
async fn run_download(app: AppHandle, spec: ModelSpec, cancel: Arc<AtomicBool>) {
    let id = spec.id.clone();
    let result = download_model_files(&app, &spec, &cancel).await;

    DOWNLOADS.lock().await.remove(&id);

    match result {
        Ok(()) => {
            tracing::info!(model = %id, "Model download complete");
            let _ = app.emit("model_download_complete", DownloadCompletePayload { id });
        }
        Err(DownloadError::Cancelled) => {
            // The `.part` file is already gone; `cancel_model_download` (or the
            // user's action) owns the outcome, so no error event is emitted.
            tracing::info!(model = %id, "Model download cancelled");
        }
        Err(err) => {
            let message = err.to_string();
            tracing::error!(model = %id, error = %message, "Model download failed");
            let _ = app.emit(
                "model_download_error",
                DownloadErrorPayload { id, message },
            );
        }
    }
}

async fn download_model_files(
    app: &AppHandle,
    spec: &ModelSpec,
    cancel: &AtomicBool,
) -> Result<(), DownloadError> {
    if spec.files.is_empty() {
        return Err(DownloadError::InvalidManifest(format!(
            "model '{}' lists no files",
            spec.id
        )));
    }

    let dest_dir =
        catalog::model_dir(app, &spec.id).map_err(|e| DownloadError::Io(e.to_string()))?;
    tokio::fs::create_dir_all(&dest_dir).await.map_err(io_error)?;

    let total: u64 = spec.files.iter().map(|file| file.size).sum();
    let client = http_client()?;
    let mut throttle = ProgressThrottle::new(PROGRESS_INTERVAL);
    let model_id = spec.id.clone();
    let mut on_progress = |downloaded: u64, total: u64| {
        if throttle.should_emit(Instant::now()) {
            let _ = app.emit(
                "model_download_progress",
                DownloadProgressPayload {
                    id: model_id.clone(),
                    downloaded,
                    total,
                },
            );
        }
    };

    download_files(
        &client,
        &spec.files,
        &dest_dir,
        total,
        cancel,
        &mut on_progress,
    )
    .await
}

/// Download `files` in manifest order into `dest_dir`, reporting progress as
/// `(downloaded, total)` where `total` is the sum of the manifest sizes.
///
/// Split from [`download_model_files`] so the ordering, fallback and running
/// total can be tested without a Tauri `AppHandle`.
async fn download_files(
    client: &reqwest::Client,
    files: &[ModelFile],
    dest_dir: &Path,
    total: u64,
    cancel: &AtomicBool,
    on_progress: &mut impl FnMut(u64, u64),
) -> Result<(), DownloadError> {
    let mut completed: u64 = 0;

    for file in files {
        let base = completed;
        {
            let mut on_bytes = |written: u64| on_progress(base + written, total);
            download_file(client, file, dest_dir, cancel, &mut on_bytes).await?;
        }
        completed += file.size;
    }

    Ok(())
}
/// Download one manifest file: try each URL in order, verify, then rename.
///
/// Returns the verified byte count. A verification failure is terminal (the
/// spec reports it instead of silently retrying another mirror).
async fn download_file(
    client: &reqwest::Client,
    file: &ModelFile,
    dest_dir: &Path,
    cancel: &AtomicBool,
    on_bytes: &mut impl FnMut(u64),
) -> Result<u64, DownloadError> {
    if file.urls.is_empty() {
        return Err(DownloadError::InvalidManifest(format!(
            "no download url for '{}'",
            file.path
        )));
    }

    let dest = safe_join(dest_dir, &file.path)?;
    if let Some(parent) = dest.parent() {
        tokio::fs::create_dir_all(parent).await.map_err(io_error)?;
    }
    let part = part_path(&dest);

    let mut last_error: Option<DownloadError> = None;
    for url in &file.urls {
        match transfer(client, url, &part, cancel, on_bytes).await {
            Ok(()) => {
                if let Err(err) = verify_file(&part, file.size, file.sha256.as_deref()).await {
                    let _ = tokio::fs::remove_file(&part).await;
                    return Err(err);
                }
                tokio::fs::rename(&part, &dest).await.map_err(io_error)?;
                tracing::info!(
                    path = %dest.display(),
                    bytes = file.size,
                    "Model file downloaded and verified"
                );
                return Ok(file.size);
            }
            Err(DownloadError::Cancelled) => {
                let _ = tokio::fs::remove_file(&part).await;
                return Err(DownloadError::Cancelled);
            }
            Err(err) => {
                tracing::warn!(url = %url, error = %err, "Download attempt failed, trying next url");
                let _ = tokio::fs::remove_file(&part).await;
                last_error = Some(err);
            }
        }
    }

    Err(last_error.unwrap_or_else(|| {
        DownloadError::InvalidManifest(format!("no usable download url for '{}'", file.path))
    }))
}

/// Stream one URL into `part`, calling `on_bytes` with the running byte count.
async fn transfer(
    client: &reqwest::Client,
    url: &str,
    part: &Path,
    cancel: &AtomicBool,
    on_bytes: &mut impl FnMut(u64),
) -> Result<(), DownloadError> {
    let mut written: u64 = 0;
    let mut stream = {
        if cancel.load(Ordering::SeqCst) {
            return Err(DownloadError::Cancelled);
        }
        let response = client
            .get(url)
            .send()
            .await
            .map_err(|e| DownloadError::Network(format!("GET {}: {}", url, e)))?;
        if !response.status().is_success() {
            return Err(DownloadError::Network(format!(
                "GET {}: HTTP {}",
                url,
                response.status()
            )));
        }
        response.bytes_stream()
    };

    // Opened with truncate, so a stale partial is never resumed by accident and
    // an empty body still lands a 0-byte `.part` for the size check to reject.
    let mut file = tokio::fs::File::create(part).await.map_err(io_error)?;

    // `cancel` is also polled while the stream is idle, so cancelling a stalled
    // connection does not wait for the next chunk. The first tick is immediate
    // and discarded, otherwise it would race the first chunk.
    let mut cancel_poll = tokio::time::interval(CANCEL_POLL_INTERVAL);
    cancel_poll.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    cancel_poll.tick().await;

    loop {
        if cancel.load(Ordering::SeqCst) {
            return Err(DownloadError::Cancelled);
        }
        let next = tokio::select! {
            biased;
            _ = cancel_poll.tick() => continue,
            chunk = stream.next() => chunk,
        };
        let Some(chunk) = next else {
            break;
        };
        let chunk = chunk.map_err(|e| DownloadError::Network(format!("GET {}: {}", url, e)))?;
        file.write_all(&chunk).await.map_err(io_error)?;
        written += chunk.len() as u64;
        on_bytes(written);
    }

    file.flush().await.map_err(io_error)?;
    Ok(())
}

/// Check the downloaded file against the manifest: byte count, and the sha256
/// when one is pinned. The hash is computed streaming, never buffered whole.
async fn verify_file(
    path: &Path,
    expected_size: u64,
    expected_sha256: Option<&str>,
) -> Result<(), DownloadError> {
    let metadata = tokio::fs::metadata(path).await.map_err(io_error)?;
    if metadata.len() != expected_size {
        return Err(DownloadError::Verify(format!(
            "'{}': expected {} bytes, got {}",
            path.display(),
            expected_size,
            metadata.len()
        )));
    }

    if let Some(expected) = expected_sha256 {
        let actual = sha256_file(path).await?;
        if !actual.eq_ignore_ascii_case(expected.trim()) {
            return Err(DownloadError::Verify(format!(
                "'{}': sha256 mismatch (expected {}, got {})",
                path.display(),
                expected.trim(),
                actual
            )));
        }
    }

    Ok(())
}

/// Streaming sha256 of a file, lowercase hex.
async fn sha256_file(path: &Path) -> Result<String, DownloadError> {
    let mut file = tokio::fs::File::open(path).await.map_err(io_error)?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; HASH_BUFFER];
    loop {
        let read = file.read(&mut buffer).await.map_err(io_error)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hex_lower(&hasher.finalize()))
}

fn hex_lower(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

fn http_client() -> Result<reqwest::Client, DownloadError> {
    reqwest::Client::builder()
        .connect_timeout(CONNECT_TIMEOUT)
        .read_timeout(READ_TIMEOUT)
        .user_agent(concat!("MonoSub/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|e| DownloadError::Network(format!("failed to initialize http client: {}", e)))
}

/// `<dest>.part` — the only name a partial transfer ever uses.
fn part_path(dest: &Path) -> PathBuf {
    let mut os = dest.as_os_str().to_os_string();
    os.push(".part");
    PathBuf::from(os)
}

/// Join a manifest-relative path onto the model directory, refusing anything
/// that would escape it (absolute paths, `..`, prefixes).
fn safe_join(base: &Path, relative: &str) -> Result<PathBuf, DownloadError> {
    let rel = Path::new(relative);
    let escapes = rel.is_absolute()
        || rel.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        });
    if escapes {
        return Err(DownloadError::InvalidManifest(format!(
            "manifest path escapes the model directory: {}",
            relative
        )));
    }
    Ok(base.join(rel))
}

fn io_error(error: std::io::Error) -> DownloadError {
    DownloadError::Io(error.to_string())
}

/// Remove `dir` recursively, refusing to touch the models root itself or
/// anything outside it. Returns `false` when `dir` does not exist.
fn remove_model_dir(models_root: &Path, dir: &Path) -> Result<bool, String> {
    if !dir.exists() {
        return Ok(false);
    }

    let root = std::fs::canonicalize(models_root)
        .map_err(|e| format!("failed to resolve {}: {}", models_root.display(), e))?;
    let target = std::fs::canonicalize(dir)
        .map_err(|e| format!("failed to resolve {}: {}", dir.display(), e))?;

    if target == root || !target.starts_with(&root) {
        return Err(format!(
            "refusing to delete {}: outside of {}",
            target.display(),
            root.display()
        ));
    }

    std::fs::remove_dir_all(&target)
        .map_err(|e| format!("failed to remove {}: {}", target.display(), e))?;
    Ok(true)
}

/// Emits progress at most once per interval, so a chunked transfer does not
/// flood the IPC channel.
#[derive(Debug)]
struct ProgressThrottle {
    interval: Duration,
    last_emit: Option<Instant>,
}

impl ProgressThrottle {
    fn new(interval: Duration) -> Self {
        Self {
            interval,
            last_emit: None,
        }
    }

    /// True when at least `interval` elapsed since the previous emission.
    fn should_emit(&mut self, now: Instant) -> bool {
        match self.last_emit {
            Some(last) if now.duration_since(last) < self.interval => false,
            _ => {
                self.last_emit = Some(now);
                true
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, BufReader, Write};
    use std::net::{TcpListener, TcpStream};

    /// A throwaway directory under the system temp dir, removed on drop. Tests
    /// never touch the real application data directory.
    struct TempDir(PathBuf);

    impl TempDir {
        fn new(tag: &str) -> Self {
            let path = std::env::temp_dir().join(format!(
                "monosub-models-test-{}-{}",
                tag,
                uuid::Uuid::new_v4()
            ));
            std::fs::create_dir_all(&path).expect("create temp dir");
            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    const HELLO_SHA256: &str = "5891b5b522d5df086d0ff0b110fbd9d21bb4fc7163af34d08286a2e846f6be03";

    fn sha256_of(bytes: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(bytes);
        hex_lower(&hasher.finalize())
    }

    fn model_file(path: &str, urls: Vec<String>, body: &[u8], sha256: Option<String>) -> ModelFile {
        ModelFile {
            path: path.to_string(),
            urls,
            size: body.len() as u64,
            sha256,
        }
    }

    fn test_client() -> reqwest::Client {
        reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(10))
            .build()
            .expect("test http client")
    }

    #[tokio::test]
    async fn sha256_file_matches_a_known_digest() {
        let dir = TempDir::new("sha-ok");
        let file = dir.path().join("sample.txt");
        tokio::fs::write(&file, b"hello\n").await.expect("write");

        assert_eq!(sha256_file(&file).await.unwrap(), HELLO_SHA256);
    }

    #[tokio::test]
    async fn sha256_file_streams_across_multiple_buffer_reads() {
        // Larger than HASH_BUFFER, so the streaming loop must hash every block.
        let body: Vec<u8> = (0..(HASH_BUFFER * 3 + 7)).map(|i| (i % 251) as u8).collect();
        let dir = TempDir::new("sha-large");
        let file = dir.path().join("large.bin");
        tokio::fs::write(&file, &body).await.expect("write");

        assert_eq!(sha256_file(&file).await.unwrap(), sha256_of(&body));
        assert!(verify_file(&file, body.len() as u64, Some(&sha256_of(&body)))
            .await
            .is_ok());
    }

    #[tokio::test]
    async fn verify_file_accepts_the_pinned_sha256() {
        let dir = TempDir::new("sha-accept");
        let file = dir.path().join("sample.txt");
        tokio::fs::write(&file, b"hello\n").await.expect("write");

        assert!(verify_file(&file, 6, Some(HELLO_SHA256)).await.is_ok());
        // Uppercase digests in a manifest are still accepted.
        assert!(verify_file(&file, 6, Some(&HELLO_SHA256.to_uppercase()))
            .await
            .is_ok());
        // No sha256 pinned: the size check alone decides.
        assert!(verify_file(&file, 6, None).await.is_ok());
    }

    #[tokio::test]
    async fn verify_file_rejects_a_wrong_sha256() {
        let dir = TempDir::new("sha-bad");
        let file = dir.path().join("sample.txt");
        tokio::fs::write(&file, b"hello\n").await.expect("write");

        let wrong = "0".repeat(64);
        let err = verify_file(&file, 6, Some(&wrong)).await.unwrap_err();
        assert!(matches!(err, DownloadError::Verify(_)), "got {:?}", err);
    }

    #[tokio::test]
    async fn verify_file_rejects_a_size_mismatch() {
        let dir = TempDir::new("size-bad");
        let file = dir.path().join("sample.txt");
        tokio::fs::write(&file, b"hello\n").await.expect("write");

        let err = verify_file(&file, 7, Some(HELLO_SHA256)).await.unwrap_err();
        match err {
            DownloadError::Verify(message) => assert!(message.contains("expected 7 bytes")),
            other => panic!("expected a verify error, got {:?}", other),
        }
    }

    #[test]
    fn part_path_appends_the_part_suffix() {
        assert_eq!(
            part_path(Path::new("/models/x/model.gguf")),
            PathBuf::from("/models/x/model.gguf.part")
        );
    }

    #[test]
    fn event_payloads_keep_the_snake_case_field_names_the_frontend_reads() {
        let progress = serde_json::to_value(DownloadProgressPayload {
            id: "sensevoice_small_q8".to_string(),
            downloaded: 10,
            total: 20,
        })
        .expect("serialize progress");
        assert_eq!(progress["id"], "sensevoice_small_q8");
        assert_eq!(progress["downloaded"], 10);
        assert_eq!(progress["total"], 20);

        let complete = serde_json::to_value(DownloadCompletePayload {
            id: "sensevoice_small_q8".to_string(),
        })
        .expect("serialize complete");
        assert_eq!(complete["id"], "sensevoice_small_q8");

        let error = serde_json::to_value(DownloadErrorPayload {
            id: "sensevoice_small_q8".to_string(),
            message: "boom".to_string(),
        })
        .expect("serialize error");
        assert_eq!(error["id"], "sensevoice_small_q8");
        assert_eq!(error["message"], "boom");
    }

    #[test]
    fn model_not_installed_sentinel_is_the_documented_literal() {
        assert_eq!(MODEL_NOT_INSTALLED, "MODEL_NOT_INSTALLED");
    }

    #[test]
    fn safe_join_rejects_paths_that_escape_the_model_dir() {
        let base = Path::new("/models/SenseVoice-Small-GGUF");
        assert_eq!(
            safe_join(base, "sensevoice.gguf").unwrap(),
            base.join("sensevoice.gguf")
        );
        assert!(safe_join(base, "../other/model.gguf").is_err());
        assert!(safe_join(base, "/tmp/steal.gguf").is_err());
    }

    #[test]
    fn progress_throttle_allows_one_event_per_interval() {
        let start = Instant::now();
        let mut throttle = ProgressThrottle::new(Duration::from_millis(200));

        assert!(throttle.should_emit(start));
        assert!(!throttle.should_emit(start + Duration::from_millis(199)));
        assert!(throttle.should_emit(start + Duration::from_millis(200)));
        assert!(!throttle.should_emit(start + Duration::from_millis(399)));
        assert!(throttle.should_emit(start + Duration::from_millis(400)));
    }

    #[test]
    fn remove_model_dir_deletes_the_model_directory_including_part_files() {
        let workspace = TempDir::new("remove-ok");
        let models_root = workspace.path().join("models");
        let dir = models_root.join("SenseVoice-Small-GGUF");
        std::fs::create_dir_all(&dir).expect("create model dir");
        std::fs::write(dir.join("model.gguf"), b"weights").expect("write model");
        std::fs::write(dir.join("model.gguf.part"), b"partial").expect("write part");

        assert!(remove_model_dir(&models_root, &dir).unwrap());
        assert!(!dir.exists());
        assert!(models_root.exists());
    }

    #[test]
    fn remove_model_dir_refuses_paths_outside_the_models_root() {
        let workspace = TempDir::new("remove-guard");
        let models_root = workspace.path().join("models");
        let outside = workspace.path().join("elsewhere");
        std::fs::create_dir_all(&models_root).expect("create models root");
        std::fs::create_dir_all(&outside).expect("create outside dir");
        std::fs::write(outside.join("keep.txt"), b"keep").expect("write");

        assert!(remove_model_dir(&models_root, &outside).is_err());
        assert!(outside.join("keep.txt").exists());

        // The models root itself is never a deletion target.
        assert!(remove_model_dir(&models_root, &models_root).is_err());
        assert!(models_root.exists());
    }

    #[test]
    fn remove_model_dir_is_a_noop_when_the_model_is_absent() {
        let workspace = TempDir::new("remove-missing");
        let models_root = workspace.path().join("models");
        std::fs::create_dir_all(&models_root).expect("create models root");

        assert!(!remove_model_dir(&models_root, &models_root.join("Missing")).unwrap());
    }

    /// A route the local test server answers.
    struct Route {
        status: u16,
        body: Vec<u8>,
    }

    /// Minimal blocking HTTP/1.1 server bound to `127.0.0.1:0`. It never leaves
    /// the loopback interface, so these tests need no network access.
    struct TestServer {
        base_url: String,
    }

    impl TestServer {
        fn start(routes: Vec<(&'static str, Route)>, chunk_delay: Option<Duration>) -> Self {
            let listener = TcpListener::bind("127.0.0.1:0").expect("bind test server");
            let addr = listener.local_addr().expect("server addr");
            std::thread::spawn(move || {
                for stream in listener.incoming() {
                    let Ok(mut stream) = stream else { continue };
                    let Some(path) = read_request_path(&stream) else {
                        continue;
                    };
                    let route = routes.iter().find(|(route, _)| *route == path);
                    let (status, body) = match route {
                        Some((_, route)) => (route.status, route.body.as_slice()),
                        None => (404, &[][..]),
                    };
                    let _ = write_response(&mut stream, status, body, chunk_delay);
                }
            });
            Self {
                base_url: format!("http://{}", addr),
            }
        }

        fn url(&self, path: &str) -> String {
            format!("{}{}", self.base_url, path)
        }
    }

    impl Route {
        fn ok(body: &[u8]) -> Self {
            Self {
                status: 200,
                body: body.to_vec(),
            }
        }

        fn status(status: u16) -> Self {
            Self {
                status,
                body: Vec::new(),
            }
        }
    }

    fn read_request_path(stream: &TcpStream) -> Option<String> {
        let mut reader = BufReader::new(stream.try_clone().ok()?);
        let mut request_line = String::new();
        reader.read_line(&mut request_line).ok()?;
        let path = request_line.split(' ').nth(1)?.to_string();
        loop {
            let mut header = String::new();
            match reader.read_line(&mut header) {
                Ok(0) => break,
                Ok(_) if header == "\r\n" || header == "\n" => break,
                Ok(_) => {}
                Err(_) => break,
            }
        }
        Some(path)
    }

    fn write_response(
        stream: &mut TcpStream,
        status: u16,
        body: &[u8],
        chunk_delay: Option<Duration>,
    ) -> std::io::Result<()> {
        let reason = if status == 200 { "OK" } else { "Error" };
        write!(
            stream,
            "HTTP/1.1 {} {}\r\nContent-Length: {}\r\nContent-Type: application/octet-stream\r\nConnection: close\r\n\r\n",
            status,
            reason,
            body.len()
        )?;
        stream.flush()?;
        for piece in body.chunks(256) {
            stream.write_all(piece)?;
            stream.flush()?;
            if let Some(delay) = chunk_delay {
                std::thread::sleep(delay);
            }
        }
        Ok(())
    }

    #[tokio::test]
    async fn download_files_follows_manifest_order_and_accumulates_progress() {
        let first = vec![1u8; 1024];
        let second = vec![2u8; 512];
        let server = TestServer::start(
            vec![
                ("/first.bin", Route::ok(&first)),
                ("/second.bin", Route::ok(&second)),
            ],
            None,
        );
        let dir = TempDir::new("download-order");
        let files = vec![
            model_file("first.bin", vec![server.url("/first.bin")], &first, None),
            model_file("second.bin", vec![server.url("/second.bin")], &second, None),
        ];
        let total = (first.len() + second.len()) as u64;

        let cancel = AtomicBool::new(false);
        let mut samples: Vec<(u64, u64)> = Vec::new();
        {
            let mut on_progress = |downloaded: u64, total: u64| samples.push((downloaded, total));
            download_files(
                &test_client(),
                &files,
                dir.path(),
                total,
                &cancel,
                &mut on_progress,
            )
            .await
            .expect("download");
        }

        assert_eq!(std::fs::read(dir.path().join("first.bin")).unwrap(), first);
        assert_eq!(std::fs::read(dir.path().join("second.bin")).unwrap(), second);
        assert!(samples.iter().all(|(_, reported)| *reported == total));
        // The second file's offset starts where the first one ended.
        assert!(samples.iter().any(|(downloaded, _)| *downloaded == first.len() as u64));
        assert!(samples.iter().any(|(downloaded, _)| *downloaded == total));
        assert!(samples.windows(2).all(|pair| pair[0].0 <= pair[1].0));
    }

    #[tokio::test]
    async fn download_files_can_be_restarted_without_a_stale_part_file() {
        // A `.part` left over from an earlier attempt must not be reused.
        let body = vec![4u8; 256];
        let server = TestServer::start(vec![("/model.gguf", Route::ok(&body))], None);
        let dir = TempDir::new("download-restart");
        std::fs::write(dir.path().join("model.gguf.part"), vec![0u8; 4096])
            .expect("seed stale part");
        let file = model_file(
            "model.gguf",
            vec![server.url("/model.gguf")],
            &body,
            Some(sha256_of(&body)),
        );

        let cancel = AtomicBool::new(false);
        let mut on_bytes = |_written: u64| {};
        download_file(&test_client(), &file, dir.path(), &cancel, &mut on_bytes)
            .await
            .expect("download");

        assert_eq!(std::fs::read(dir.path().join("model.gguf")).unwrap(), body);
        assert!(!dir.path().join("model.gguf.part").exists());
    }

    #[tokio::test]
    async fn download_writes_verifies_and_renames_the_model_file() {
        let body = vec![7u8; 4096];
        let server = TestServer::start(
            vec![("/model.gguf", Route::ok(&body))],
            None,
        );
        let dir = TempDir::new("download-ok");
        let file = model_file(
            "model.gguf",
            vec![server.url("/model.gguf")],
            &body,
            Some(sha256_of(&body)),
        );

        let cancel = AtomicBool::new(false);
        let mut samples: Vec<u64> = Vec::new();
        {
            let mut on_bytes = |written: u64| samples.push(written);
            download_file(&test_client(), &file, dir.path(), &cancel, &mut on_bytes)
                .await
                .expect("download");
        }

        assert_eq!(std::fs::read(dir.path().join("model.gguf")).unwrap(), body);
        assert!(!dir.path().join("model.gguf.part").exists());
        assert!(!samples.is_empty());
        assert!(samples.windows(2).all(|pair| pair[0] <= pair[1]));
        assert_eq!(*samples.last().unwrap(), body.len() as u64);
    }

    #[tokio::test]
    async fn download_falls_back_to_the_next_url() {
        let body = vec![3u8; 2048];
        let server = TestServer::start(
            vec![
                ("/primary.gguf", Route::status(500)),
                ("/mirror.gguf", Route::ok(&body)),
            ],
            None,
        );
        let dir = TempDir::new("download-fallback");
        let file = model_file(
            "model.gguf",
            vec![server.url("/primary.gguf"), server.url("/mirror.gguf")],
            &body,
            None,
        );

        let cancel = AtomicBool::new(false);
        let mut on_bytes = |_written: u64| {};
        download_file(&test_client(), &file, dir.path(), &cancel, &mut on_bytes)
            .await
            .expect("mirror should succeed");

        assert_eq!(std::fs::read(dir.path().join("model.gguf")).unwrap(), body);
        assert!(!dir.path().join("model.gguf.part").exists());
    }

    #[tokio::test]
    async fn download_fails_only_after_every_url_is_exhausted() {
        let server = TestServer::start(
            vec![
                ("/primary.gguf", Route::status(500)),
                ("/mirror.gguf", Route::status(404)),
            ],
            None,
        );
        let dir = TempDir::new("download-exhausted");
        let file = model_file(
            "model.gguf",
            vec![server.url("/primary.gguf"), server.url("/mirror.gguf")],
            &[1u8; 16],
            None,
        );

        let cancel = AtomicBool::new(false);
        let mut on_bytes = |_written: u64| {};
        let err = download_file(&test_client(), &file, dir.path(), &cancel, &mut on_bytes)
            .await
            .unwrap_err();

        assert!(matches!(err, DownloadError::Network(_)), "got {:?}", err);
        assert!(!dir.path().join("model.gguf").exists());
        assert!(!dir.path().join("model.gguf.part").exists());
    }

    #[tokio::test]
    async fn download_rejects_a_size_mismatch_and_removes_the_part_file() {
        let body = vec![1u8; 128];
        let server = TestServer::start(vec![("/model.gguf", Route::ok(&body))], None);
        let dir = TempDir::new("download-size");
        let mut file = model_file("model.gguf", vec![server.url("/model.gguf")], &body, None);
        file.size = body.len() as u64 + 1;

        let cancel = AtomicBool::new(false);
        let mut on_bytes = |_written: u64| {};
        let err = download_file(&test_client(), &file, dir.path(), &cancel, &mut on_bytes)
            .await
            .unwrap_err();

        assert!(matches!(err, DownloadError::Verify(_)), "got {:?}", err);
        assert!(!dir.path().join("model.gguf").exists());
        assert!(!dir.path().join("model.gguf.part").exists());
    }

    #[tokio::test]
    async fn download_rejects_a_sha256_mismatch_and_removes_the_part_file() {
        let body = vec![9u8; 128];
        let server = TestServer::start(vec![("/model.gguf", Route::ok(&body))], None);
        let dir = TempDir::new("download-sha");
        let file = model_file(
            "model.gguf",
            vec![server.url("/model.gguf")],
            &body,
            Some("f".repeat(64)),
        );

        let cancel = AtomicBool::new(false);
        let mut on_bytes = |_written: u64| {};
        let err = download_file(&test_client(), &file, dir.path(), &cancel, &mut on_bytes)
            .await
            .unwrap_err();

        assert!(matches!(err, DownloadError::Verify(_)), "got {:?}", err);
        assert!(!dir.path().join("model.gguf").exists());
        assert!(!dir.path().join("model.gguf.part").exists());
    }

    #[tokio::test]
    async fn cancelling_an_unknown_id_is_a_noop() {
        // No entry registered under this id: the helper must return immediately
        // without touching any file.
        cancel_in_flight("monosub-test-never-registered").await;
        assert!(!DOWNLOADS
            .lock()
            .await
            .contains_key("monosub-test-never-registered"));
    }

    #[tokio::test]
    async fn cancel_in_flight_flags_the_task_and_removes_it_from_the_registry() {
        let id = format!("monosub-test-cancel-{}", uuid::Uuid::new_v4());
        let cancel = Arc::new(AtomicBool::new(false));
        let observed_cancel = Arc::clone(&cancel);
        let finished = Arc::new(AtomicBool::new(false));
        let observed_finished = Arc::clone(&finished);

        let task = tokio::spawn(async move {
            while !observed_cancel.load(Ordering::SeqCst) {
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
            observed_finished.store(true, Ordering::SeqCst);
        });
        DOWNLOADS.lock().await.insert(
            id.clone(),
            DownloadEntry {
                cancel: Arc::clone(&cancel),
                task,
            },
        );

        cancel_in_flight(&id).await;

        assert!(cancel.load(Ordering::SeqCst));
        assert!(finished.load(Ordering::SeqCst), "waited for the task to stop");
        assert!(!DOWNLOADS.lock().await.contains_key(&id));
    }

    #[tokio::test]
    async fn cancelling_an_idle_transfer_stops_without_waiting_for_chunks() {
        // The server dribbles the body out; a cancel arriving while the stream
        // is idle must still stop the transfer long before the body finishes.
        let body = vec![5u8; 4096];
        let server = TestServer::start(
            vec![("/model.gguf", Route::ok(&body))],
            Some(Duration::from_millis(500)),
        );
        let dir = TempDir::new("download-idle-cancel");
        let file = model_file("model.gguf", vec![server.url("/model.gguf")], &body, None);

        let cancel = Arc::new(AtomicBool::new(false));
        let cancel_task = Arc::clone(&cancel);
        let dir_path = dir.path().to_path_buf();
        let started = Instant::now();
        let handle = tokio::spawn(async move {
            let mut on_bytes = |_written: u64| {};
            download_file(&test_client(), &file, &dir_path, &cancel_task, &mut on_bytes).await
        });

        tokio::time::sleep(Duration::from_millis(50)).await;
        cancel.store(true, Ordering::SeqCst);
        let err = handle.await.expect("join download").unwrap_err();

        assert!(matches!(err, DownloadError::Cancelled), "got {:?}", err);
        assert!(
            started.elapsed() < Duration::from_millis(400),
            "cancel took {:?}",
            started.elapsed()
        );
        assert!(!dir.path().join("model.gguf").exists());
        assert!(!dir.path().join("model.gguf.part").exists());
    }

    #[tokio::test]
    async fn cancelling_a_download_leaves_no_part_file() {
        let body = vec![5u8; 4096];
        let server = TestServer::start(
            vec![("/model.gguf", Route::ok(&body))],
            Some(Duration::from_millis(5)),
        );
        let dir = TempDir::new("download-cancel");
        let file = model_file("model.gguf", vec![server.url("/model.gguf")], &body, None);

        let cancel = Arc::new(AtomicBool::new(false));
        let cancel_from_callback = Arc::clone(&cancel);
        let mut on_bytes = |_written: u64| cancel_from_callback.store(true, Ordering::SeqCst);

        let err = download_file(&test_client(), &file, dir.path(), &cancel, &mut on_bytes)
            .await
            .unwrap_err();

        assert!(matches!(err, DownloadError::Cancelled), "got {:?}", err);
        assert!(!dir.path().join("model.gguf").exists());
        assert!(!dir.path().join("model.gguf.part").exists());
    }

    #[tokio::test]
    async fn a_download_cancelled_before_it_starts_creates_nothing() {
        let dir = TempDir::new("download-precancel");
        let file = model_file(
            "model.gguf",
            vec!["http://127.0.0.1:1/never".to_string()],
            &[1u8; 8],
            None,
        );

        let cancel = AtomicBool::new(true);
        let mut on_bytes = |_written: u64| {};
        let err = download_file(&test_client(), &file, dir.path(), &cancel, &mut on_bytes)
            .await
            .unwrap_err();

        assert!(matches!(err, DownloadError::Cancelled), "got {:?}", err);
        assert!(!dir.path().join("model.gguf").exists());
        assert!(!dir.path().join("model.gguf.part").exists());
    }
}
