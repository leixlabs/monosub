//! URL import: fetch a Bilibili video's platform subtitle, or fall back to
//! downloading its audio and queueing it for recognition.
//!
//! Tasks: U2-2 (job skeleton), U2-3 (fetch), U2-4 (materialise), U2-5
//! (translation), U3-1..U3-5 (fallback), U3-6 (cancel), U3-7 (prefer local).
//! See docs/tech-spec/url-import-design.md §3 (the flow), §5 (the stages) and
//! §5.9 (the IPC contract).
//!
//! Two properties shape this whole module:
//!
//! * **A subtitle hit never enters `start_transcription`.** That command takes an
//!   audio path, needs a model plan, and reports `transcribe_*` events; a
//!   platform subtitle has none of those. It writes through `subtitle::write`
//!   directly and reports `url_import_*`.
//! * **The audio fallback is fully downloaded before the job finishes.** The
//!   `Queued` outcome carries a path that already exists, so the existing
//!   `fileImport` → metadata/thumbnail → `start_transcription` path needs no
//!   change at all (design §5.7).

pub mod deps;
pub mod error;
pub mod lang;
pub mod pipeline;
pub mod naming;
pub mod progress;
pub mod resolve;
pub mod srt;
pub mod url;
pub mod ytdlp;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::{Mutex, Semaphore};

use crate::commands::models::MODEL_NOT_INSTALLED;
use crate::engine::catalog::ModelSpec;
use crate::site::deps::RealDeps;
use crate::site::lang::SubKind;
use crate::subtitle::{self, Format};
use crate::translate::stage::{self, TranslationPlan};

pub use error::ImportError;
/// Re-exported for the IPC documentation and for tests in sibling modules:
/// nothing in the crate needs them through this path, but they are the names a
/// reader of the design doc will look for.
#[allow(unused_imports)]
pub use error::NEEDS_COOKIES_STATUS;
#[allow(unused_imports)]
pub use resolve::{find_info_dict, from_info_dict};
#[allow(unused_imports)]
pub use url::UrlError;
#[allow(unused_imports)]
pub use lang::select;
pub use resolve::SiteMedia;
pub use url::{parse_input, ParsedUrl};

/// Concurrent imports. Independent of the transcription semaphores: an import is
/// network-bound, while recognition is GPU-bound (design §5.8).
const MAX_CONCURRENT_IMPORTS: usize = 3;

// --- Events ---------------------------------------------------------------

/// Stage names carried by `url_import_progress`. A frozen set: the frontend maps
/// each one to a `urlImport.stage.*` i18n key, so adding a name is a new key
/// while renaming one blanks that stage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Stage {
    Resolving,
    FetchingSubtitles,
    DownloadingAudio,
    Writing,
    Translating,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProgressPayload {
    pub task_id: String,
    pub stage: Stage,
    /// `0.0..=100.0`, or `None` for indeterminate progress (unknown size, or a
    /// stage with no measurable work).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub progress: Option<f64>,
}

/// What one URL resolved to.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum Outcome {
    /// A platform subtitle was written.
    Subtitle {
        paths: Vec<String>,
        translated_paths: Vec<String>,
        /// The platform tag that was used (`zh-Hans`, `ai-zh`, …).
        tag: String,
        kind: SubKind,
        /// The language of the subtitle text, handed to the frontend so it can
        /// label the row.
        source_lang: String,
        /// True when `skipExistingSubtitle` skipped every file (nothing written).
        skipped: bool,
    },
    /// No usable subtitle: the audio is on disk and the frontend should queue it.
    Queued {
        path: String,
        title: String,
        /// The name the frontend should show, `<title> [<id>].<ext>` (design §5.5).
        name: String,
        duration: Option<f64>,
        /// True when the platform would have had a subtitle behind a login.
        login_hint: bool,
    },
}

#[derive(Debug, Clone, Serialize)]
pub struct CompletePayload {
    pub task_id: String,
    pub outcome: Outcome,
}

#[derive(Debug, Clone, Serialize)]
pub struct ErrorPayload {
    pub task_id: String,
    /// A sentinel (`URL_IMPORT_NEEDS_COOKIES`, `MODEL_NOT_INSTALLED:<id>`), an
    /// i18n key (`urlImport.multiPart`) or a message, optionally prefixed with a
    /// key and `:` so the UI can render a localised sentence plus the detail.
    pub status: String,
}

// --- Request --------------------------------------------------------------

/// One URL to import (design §5.9).
///
/// Everything the job needs arrives in the request so the spawned task holds no
/// borrows and the frontend stays the only place where "which directory" is
/// decided.
#[derive(Debug, Clone, Deserialize)]
pub struct UrlImportRequest {
    pub task_id: String,
    /// Already normalised by [`parse_input`].
    pub url: String,
    /// `settings.outputLanguage`.
    pub language: String,
    /// `settings.outputFormat`, e.g. `srt,txt`.
    pub formats: String,
    /// Computed by the frontend per design §5.4.
    pub output_dir: String,
    pub skip_existing: bool,
    /// `None` = cookies not configured; `chrome`/`edge`/`firefox`/`safari`.
    pub cookies_browser: Option<String>,
    /// Same three fields as `TranscribeRequest`, same meaning.
    pub translate_enabled: Option<bool>,
    pub target_language: Option<String>,
    pub translation_model: Option<String>,
    /// Skip the subtitle lookup and recognise locally instead (design §7-2).
    pub prefer_local_recognition: Option<bool>,
}

// --- Task registry --------------------------------------------------------

struct ImportEntry {
    handle: tokio::task::JoinHandle<()>,
    /// The job's scratch directory; removed when the `Drop` guard inside the task
    /// runs, which an abort triggers.
    work_dir: PathBuf,
    /// The per-video cache directory partial downloads land in.
    ///
    /// Separate from `work_dir` because a partial `<id>.m4a.part` is written
    /// under `<app_cache_dir>/url-import/<id>/`, not in the scratch directory: a
    /// cancel has to sweep both or it leaves the `.part` behind, which is exactly
    /// what design §5.8 promises not to do.
    cache_dir: PathBuf,
}

type ImportMap = Arc<Mutex<HashMap<String, ImportEntry>>>;

lazy_static::lazy_static! {
    static ref IMPORT_TASKS: ImportMap = Arc::new(Mutex::new(HashMap::new()));
    static ref IMPORT_SEMAPHORE: Arc<Semaphore> = Arc::new(Semaphore::new(MAX_CONCURRENT_IMPORTS));
}

// --- Commands -------------------------------------------------------------

/// `~/Downloads/MonoSub` — the default output directory for imported subtitles.
///
/// Resolved here rather than in the frontend: the home directory is a platform
/// fact Tauri already knows, and the frontend must not hard-code `/Users/<name>`.
/// The directory is created on demand by the write stage, not here, so merely
/// opening the dialog does not litter the user's home directory.
#[tauri::command]
pub fn default_url_import_dir(app: AppHandle) -> Result<String, String> {
    let home = app
        .path()
        .home_dir()
        .map_err(|error| format!("could not resolve the home directory: {error}"))?;
    Ok(home.join("Downloads").join("MonoSub").to_string_lossy().into_owned())
}

/// Split, normalise and whitelist the dialog's text.
#[tauri::command]
pub fn parse_url_input(text: String) -> Vec<ParsedUrl> {
    parse_input(&text)
}

/// Start one URL import. Returns as soon as the task is registered; the result
/// arrives as `url_import_complete` / `url_import_error`.
#[tauri::command]
pub async fn start_url_import(
    app: AppHandle,
    request: UrlImportRequest,
) -> Result<(), String> {
    let task_id = request.task_id.clone();

    tracing::info!(
        task_id = %task_id,
        url = %request.url,
        language = %request.language,
        formats = %request.formats,
        "starting URL import"
    );

    // Reject the request before registering, so a typo cannot leave a job that
    // never reports anything.
    let formats = match subtitle::parse_formats(&request.formats) {
        Ok(formats) => formats,
        Err(status) => {
            emit_error(&app, &task_id, &status);
            return Ok(());
        }
    };

    let work_dir = import_work_dir(&app, &task_id);
    if let Err(error) = std::fs::create_dir_all(&work_dir) {
        let status = format!("urlImport.outputDirFailed:{error}");
        emit_error(&app, &task_id, &status);
        return Ok(());
    }

    // The cache root is where yt-dlp writes media and partial files. It is
    // recorded now (not when the download starts) so a cancel during resolve or
    // the subtitle fetch can still sweep a leftover from an earlier attempt.
    let cache_dir = import_root(&app)
        .map(|root| root.join(video_id_hint(&request.url)))
        .unwrap_or_else(|_| work_dir.clone());

    let params = JobParams {
        request,
        formats,
        work_dir: work_dir.clone(),
    };

    let app_for_task = app.clone();
    let handle = tokio::spawn(async move {
        run_job(app_for_task, params).await;
    });

    let mut tasks = IMPORT_TASKS.lock().await;
    if tasks.contains_key(&task_id) {
        // A stale finished entry for the same id must not wedge the new one.
        tasks.remove(&task_id);
    }
    tasks.insert(
        task_id,
        ImportEntry {
            handle,
            work_dir,
            cache_dir,
        },
    );

    Ok(())
}

/// Cancel an import.
///
/// Only the import stage: once a `Queued` outcome has been emitted the job is
/// finished and cancelling is `cancel_transcription` or removing the queue row
/// (design §5.8). The downloaded audio is deliberately kept for reuse; only the
/// scratch directory and any `.part` file go.
#[tauri::command]
pub async fn cancel_url_import(task_id: String) -> Result<(), String> {
    tracing::info!(task_id = %task_id, "cancelling URL import");

    let entry = IMPORT_TASKS.lock().await.remove(&task_id);
    let Some(entry) = entry else {
        tracing::warn!(task_id = %task_id, "cannot cancel: import not found");
        return Err("Import not found".to_string());
    };

    entry.handle.abort();
    // Awaiting the aborted handle makes cleanup deterministic: the future (and
    // with it the yt-dlp child and the temp-dir guard) has been dropped.
    let _ = entry.handle.await;
    let _ = tokio::task::yield_now().await;

    // Sweep BOTH locations. The scratch directory holds the info dict and any
    // subtitle file; the cache directory holds the media and its `.part`. Only
    // partial files are removed from the cache: a *finished* download is kept so
    // the same URL can be retried without downloading again (design §5.8).
    remove_partial_downloads(&entry.work_dir);
    let _ = std::fs::remove_dir_all(&entry.work_dir);
    if entry.cache_dir != entry.work_dir && entry.cache_dir.is_dir() {
        remove_partial_downloads(&entry.cache_dir);
    }

    Ok(())
}

// --- Job ------------------------------------------------------------------

/// One job's immutable inputs. `pub(crate)` because `site::pipeline` and
/// `site::deps` take it by reference across their seams.
pub(crate) struct JobParams {
    pub request: UrlImportRequest,
    pub formats: Vec<Format>,
    pub work_dir: PathBuf,
}

/// Removes the scratch directory when the job ends, including on abort.
struct WorkDirGuard {
    path: PathBuf,
}

impl Drop for WorkDirGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

async fn run_job(app: AppHandle, params: JobParams) {
    let task_id = params.request.task_id.clone();
    let _guard = WorkDirGuard {
        path: params.work_dir.clone(),
    };

    let permit = match IMPORT_SEMAPHORE.acquire().await {
        Ok(permit) => permit,
        Err(error) => {
            tracing::error!(task_id = %task_id, error = %error, "import semaphore closed");
            emit_error(&app, &task_id, "urlImport.failed");
            unregister(&task_id).await;
            return;
        }
    };

    let outcome = execute(&app, &params).await;
    drop(permit);

    match outcome {
        Ok(outcome) => {
            tracing::info!(task_id = %task_id, "URL import finished");
            let _ = app.emit(
                "url_import_complete",
                CompletePayload {
                    task_id: task_id.clone(),
                    outcome,
                },
            );
        }
        Err(error) => {
            tracing::warn!(
                task_id = %task_id,
                error = ?error,
                status = %error.status(),
                "URL import failed"
            );
            emit_error(&app, &task_id, &error.status());
        }
    }

    unregister(&task_id).await;
}

/// Remove our own registry entry, but never a newer job with the same id.
async fn unregister(task_id: &str) {
    IMPORT_TASKS.lock().await.remove(task_id);
}

/// The whole import for one URL.
///
/// The decision path itself lives in [`pipeline`], written against
/// [`deps::Deps`] so the branching is testable without yt-dlp, a network or a
/// Tauri handle. This function supplies the real implementations and the
/// translation plan (which needs the catalogue, hence the `AppHandle`).
async fn execute(app: &AppHandle, params: &JobParams) -> Result<Outcome, ImportError> {
    let request = &params.request;
    let catalogue = crate::engine::catalog::catalog(Some(app)).ok();
    let translation = resolve_translation(app, request, catalogue.as_deref())
        .map_err(ImportError::Other)?;

    let deps = RealDeps::new(app, &request.task_id);
    pipeline::run(&deps, params, translation).await
}

/// Resolve the translation stage for this request, mapping the sentinel into an
/// `ImportError` the frontend already knows how to route.
fn resolve_translation(
    app: &AppHandle,
    request: &UrlImportRequest,
    catalogue: Option<&[ModelSpec]>,
) -> Result<Option<TranslationPlan>, String> {
    let enabled = request.translate_enabled;
    let target = request.target_language.as_deref();
    if !stage::will_translate(&request.language, enabled, target) {
        return Ok(None);
    }
    let Some(catalogue) = catalogue else {
        // Without a catalogue there is no translation model to plan; treat it as
        // "not installed" so the user is pointed at the download list.
        return Err(MODEL_NOT_INSTALLED.to_string());
    };
    stage::resolve(
        catalogue,
        &request.language,
        enabled,
        target,
        request.translation_model.as_deref(),
        &|id| crate::engine::catalog::is_installed(app, id),
        &|id: &str| crate::engine::catalog::model_path(app, id).ok().flatten(),
    )
}

// --- Subtitle fetch -------------------------------------------------------

/// The SRT text for one track.
///
/// Inline `data` is preferred: Bilibili's extractor already converted the track
/// and put the SRT text in the info dict, so using it saves a whole network round
/// trip (design §2, measured in §9.11 / U0-9). Otherwise the track is downloaded
/// from a saved info dict — and the verdict comes from the file on disk, never
/// from the exit code, because an unmatched `--sub-langs` exits 0 (design §4.3).
pub(crate) async fn fetch_subtitle_text(
    app: &AppHandle,
    params: &JobParams,
    media: &SiteMedia,
    tag: &str,
) -> Result<String, ImportError> {
    if let Some(track) = media.subs.get(tag) {
        if let Some(data) = track.data.as_deref().filter(|data| !data.trim().is_empty()) {
            tracing::info!(tag = %tag, bytes = data.len(), "using inline subtitle data");
            return Ok(data.to_string());
        }
    }

    // The fetch stage reads an info dict from disk. Resolve has just parsed one,
    // so re-running it would be a wasted round trip: the dict is re-serialised
    // from the parsed value instead.
    write_info_json(params, media)?;

    let invocation = ytdlp::Invocation {
        stage: ytdlp::Stage::FetchSubtitles {
            work_dir: params.work_dir.clone(),
            tag: tag.to_string(),
        },
        url: params.request.url.clone(),
        cookies_browser: cookies(&params.request),
        ffmpeg: Some(ffmpeg_path(app)),
    };

    with_rate_limit_retry(|| ytdlp::run(Some(app), &invocation, |_| {}))
        .await
        .and_then(|output| {
            // Exit code is not the verdict (design §4.3); the file on disk is.
            match read_subtitle_file(params, media, tag) {
                Some(text) => Ok(text),
                None => Err(if output.success() {
                    ImportError::SubtitleUnusable(format!("no subtitle file for {}", tag))
                } else {
                    error::classify(&output.stderr)
                }),
            }
        })
}

/// Write the info dict the fetch stage reads.
///
/// The dict is re-serialised from the parsed `SiteMedia` rather than copied from
/// stdout: the fields that matter (`id`, `subtitles[].data`) survive, and the file
/// cannot carry a stray table line.
fn write_info_json(params: &JobParams, media: &SiteMedia) -> Result<(), ImportError> {
    let subs: serde_json::Map<String, serde_json::Value> = media
        .subs
        .iter()
        .map(|(tag, track)| {
            let (ext, data) = (&track.ext, &track.data);
            let entry = match data {
                Some(data) => serde_json::json!([{ "ext": ext, "data": data }]),
                None => serde_json::json!([{ "ext": ext }]),
            };
            (tag.clone(), entry)
        })
        .collect();

    let info = serde_json::json!({
        "id": media.id,
        "title": media.title,
        "duration": media.duration,
        "subtitles": subs,
    });

    let body = serde_json::to_string(&info)
        .map_err(|error| ImportError::Other(format!("urlImport.infoJsonFailed:{error}")))?;
    let path = params.work_dir.join("info.json");
    std::fs::write(&path, body).map_err(|error| {
        ImportError::Other(format!("urlImport.infoJsonFailed:{}: {error}", path.display()))
    })?;

    tracing::debug!(path = %path.display(), "wrote the info dict for the subtitle fetch");
    Ok(())
}

/// `--write-subs` prefixes the language, so the expected name is
/// `<work_dir>/<id>/<id>.<tag>.srt`. The verdict requires non-empty content.
fn read_subtitle_file(params: &JobParams, media: &SiteMedia, tag: &str) -> Option<String> {
    let path = params
        .work_dir
        .join(&media.id)
        .join(format!("{}.{}.srt", media.id, tag));
    let text = std::fs::read_to_string(&path).ok()?;
    if text.trim().is_empty() {
        tracing::warn!(path = %path.display(), "subtitle file exists but is empty");
        return None;
    }
    Some(text)
}

// --- Fallback: audio download ---------------------------------------------

/// Download the audio for a video, reporting progress, and return the media file.
///
/// The file lands in `<app_cache_dir>/url-import/<id>/` — it is an intermediate
/// artefact the recognition pipeline reads, never user output (design §5.3).
pub(crate) async fn download_audio(
    app: &AppHandle,
    params: &JobParams,
    media: &SiteMedia,
) -> Result<PathBuf, ImportError> {
    let request = &params.request;
    let download_dir = download_dir(app, &media.id)?;

    let invocation = ytdlp::Invocation {
        stage: ytdlp::Stage::DownloadAudio {
            work_dir: download_dir.clone(),
        },
        url: request.url.clone(),
        cookies_browser: cookies(request),
        ffmpeg: Some(ffmpeg_path(app)),
    };

    let app_for_lines = app.clone();
    let task_id_for_lines = request.task_id.clone();
    let output = with_rate_limit_retry(|| {
        let app = app_for_lines.clone();
        let task_id = task_id_for_lines.clone();
        let invocation = invocation.clone();
        async move {
            ytdlp::run(Some(&app), &invocation, |line| {
                if let Some(progress) = progress::parse_line(line) {
                    // A finished transfer reports 100% even when the last line
                    // carries an estimate rather than a total, so the bar cannot
                    // stop one update short of full.
                    let percent = if progress.is_finished() {
                        Some(100.0)
                    } else {
                        progress.fraction().map(|fraction| fraction * 100.0)
                    };
                    emit_progress(&app, &task_id, Stage::DownloadingAudio, percent);
                }
            })
            .await
        }
    })
    .await?;

    if !output.success() {
        return Err(error::classify(&output.stderr));
    }

    downloaded_media_file(&download_dir, &media.id)
        .ok_or_else(|| ImportError::Other("urlImport.downloadFailed:no audio file".to_string()))
}

// --- Download cache -------------------------------------------------------

/// `<app_cache_dir>/url-import` — intermediate media, never user output.
///
/// Subtitles are written to `output_dir`; this directory only ever holds audio
/// that the recognition pipeline reads (design §5.3).
fn import_root(app: &AppHandle) -> Result<PathBuf, ImportError> {
    app.path()
        .app_cache_dir()
        .map(|dir| dir.join("url-import"))
        .map_err(|error| ImportError::Other(format!("urlImport.cacheDirFailed:{error}")))
}

fn import_work_dir(app: &AppHandle, task_id: &str) -> PathBuf {
    match import_root(app) {
        Ok(root) => root.join(format!("task-{}-{}", task_id, unique_suffix())),
        // No cache dir at all is not fatal: a temp dir works for a single job and
        // is removed on drop.
        Err(error) => {
            tracing::warn!(error = %error, "no app cache dir; using the temp dir");
            std::env::temp_dir().join(format!("monosub-url-import-{}-{}", task_id, unique_suffix()))
        }
    }
}

fn download_dir(app: &AppHandle, id: &str) -> Result<PathBuf, ImportError> {
    let dir = import_root(app)?.join(id);
    std::fs::create_dir_all(&dir).map_err(|error| {
        ImportError::Other(format!("urlImport.cacheDirFailed:{}: {error}", dir.display()))
    })?;
    Ok(dir)
}

/// Extensions that are never the downloaded media.
fn is_sidecar_extension(extension: &str) -> bool {
    matches!(
        extension.to_ascii_lowercase().as_str(),
        "part" | "ytdl" | "json" | "srt" | "vtt" | "xml" | "temp"
    )
}

/// The already-downloaded media for `id`, if any (design §5.3 reuse rule).
pub(crate) fn cached_media_file(app: &AppHandle, id: &str) -> Option<PathBuf> {
    let dir = import_root(app).ok()?.join(id);
    media_file_in(&dir)
}

/// The media file in `dir`, ignoring sidecars, `.part` files and subtitles.
fn media_file_in(dir: &Path) -> Option<PathBuf> {
    let mut found: Vec<PathBuf> = std::fs::read_dir(dir)
        .ok()?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| path.is_file())
        .filter(|path| {
            let extension = path
                .extension()
                .and_then(|extension| extension.to_str())
                .unwrap_or_default();
            !is_sidecar_extension(extension)
        })
        .filter(|path| std::fs::metadata(path).map(|meta| meta.len() > 0).unwrap_or(false))
        .collect();
    found.sort();
    found.into_iter().next()
}

/// What the download stage produced: `<download_dir>/<id>/<id>.<ext>`.
fn downloaded_media_file(download_dir: &Path, id: &str) -> Option<PathBuf> {
    let nested = download_dir.join(id);
    media_file_in(&nested).or_else(|| media_file_in(download_dir))
}

fn remove_partial_downloads(dir: &Path) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            remove_partial_downloads(&path);
            continue;
        }
        let is_partial = path
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| {
                matches!(extension.to_ascii_lowercase().as_str(), "part" | "ytdl" | "temp")
            });
        if is_partial {
            let _ = std::fs::remove_file(&path);
        }
    }
}

// --- Helpers --------------------------------------------------------------

/// `--cookies-from-browser` value, or `None` when not configured.
fn cookies(request: &UrlImportRequest) -> Option<String> {
    request
        .cookies_browser
        .as_deref()
        .map(str::trim)
        .filter(|browser| !browser.is_empty())
        .map(str::to_string)
}

pub(crate) fn cookies_of(request: &UrlImportRequest) -> Option<String> {
    cookies(request)
}

fn ffmpeg_path(app: &AppHandle) -> PathBuf {
    PathBuf::from(crate::commands::video::get_ffmpeg_path(Some(app)))
}

pub(crate) fn ffmpeg_of(app: &AppHandle) -> PathBuf {
    ffmpeg_path(app)
}

/// Retry a transient failure once or twice with a growing pause.
///
/// Only `RateLimited` (HTTP 429) and `Network` are retried — the subtitle stage
/// still falls back to transcription when the retries are exhausted, which is what
/// the design asks for (design §5.9).
pub(crate) async fn with_rate_limit_retry<F, Fut, T>(mut attempt: F) -> Result<T, ImportError>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<T, ImportError>>,
{
    const DELAYS_MS: [u64; 2] = [500, 1500];
    let mut last: Result<T, ImportError> = attempt().await;
    for delay in DELAYS_MS {
        match &last {
            Err(error) if error.is_retryable() => {
                tracing::warn!(
                    delay_ms = delay,
                    status = %error.status(),
                    "retrying a transient import failure"
                );
                tokio::time::sleep(std::time::Duration::from_millis(delay)).await;
                last = attempt().await;
            }
            _ => break,
        }
    }
    last
}

/// The video id a URL names, from the path (`/video/BV…` or `/video/av…`).
///
/// Only used to point the cancel sweep at the right cache directory; it never
/// decides where output is written, and a failure is harmless (the sweep then
/// covers the scratch directory only).
///
/// The segment is percent-DECODED by `url`, so `..%2F..%2Fetc` arrives as
/// `../../etc` — a path-traversal attempt smuggled through an encoding. Since
/// this value is joined onto a cache path, it is filtered down to characters a
/// Bilibili id can actually contain; anything else yields `unknown`.
fn video_id_hint(url: &str) -> String {
    let raw = ::url::Url::parse(url).ok().and_then(|parsed| {
        parsed
            .path_segments()
            .and_then(|mut segments| segments.nth(1).map(str::to_string))
    });

    match raw {
        Some(id)
            if !id.is_empty()
                && id
                    .chars()
                    .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-') =>
        {
            id
        }
        _ => "unknown".to_string(),
    }
}

fn unique_suffix() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default()
}

fn emit_progress(app: &AppHandle, task_id: &str, stage: Stage, progress: Option<f64>) {
    let _ = app.emit(
        "url_import_progress",
        ProgressPayload {
            task_id: task_id.to_string(),
            stage,
            progress,
        },
    );
}

/// Public within the crate so `site::pipeline`'s production wiring can emit the
/// same event without duplicating the payload shape.
pub(crate) fn emit_stage(app: &AppHandle, task_id: &str, stage: Stage, progress: Option<f64>) {
    emit_progress(app, task_id, stage, progress);
}

fn emit_error(app: &AppHandle, task_id: &str, status: &str) {
    let _ = app.emit(
        "url_import_error",
        ErrorPayload {
            task_id: task_id.to_string(),
            status: status.to_string(),
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("monosub-import-{}-{}", tag, uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn request(formats: &str) -> UrlImportRequest {
        UrlImportRequest {
            task_id: "t1".to_string(),
            url: "https://www.bilibili.com/video/BV1GJ411x7h7".to_string(),
            language: "zh".to_string(),
            formats: formats.to_string(),
            output_dir: "/tmp/out".to_string(),
            skip_existing: false,
            cookies_browser: None,
            translate_enabled: None,
            target_language: None,
            translation_model: None,
            prefer_local_recognition: None,
        }
    }

    /// The stage set is frozen: the frontend derives its i18n keys from it.
    #[test]
    fn stage_names_are_the_frozen_contract() {
        let names: Vec<String> = [
            Stage::Resolving,
            Stage::FetchingSubtitles,
            Stage::DownloadingAudio,
            Stage::Writing,
            Stage::Translating,
        ]
        .iter()
        .map(|stage| serde_json::to_string(stage).unwrap())
        .collect();
        assert_eq!(
            names,
            vec![
                "\"resolving\"",
                "\"fetching_subtitles\"",
                "\"downloading_audio\"",
                "\"writing\"",
                "\"translating\"",
            ]
        );
    }

    #[test]
    fn outcome_serialises_with_an_outcome_tag() {
        let queued = Outcome::Queued {
            path: "/cache/x.m4a".to_string(),
            title: "T".to_string(),
            name: "T [BV1].m4a".to_string(),
            duration: Some(1.0),
            login_hint: false,
        };
        let json = serde_json::to_value(&queued).unwrap();
        assert_eq!(json["outcome"], "queued");
        assert_eq!(json["path"], "/cache/x.m4a");

        let subtitle = Outcome::Subtitle {
            paths: vec!["/out/a.srt".to_string()],
            translated_paths: vec![],
            tag: "zh-Hans".to_string(),
            kind: SubKind::Human,
            source_lang: "zh".to_string(),
            skipped: false,
        };
        let json = serde_json::to_value(&subtitle).unwrap();
        assert_eq!(json["outcome"], "subtitle");
        assert_eq!(json["tag"], "zh-Hans");
        // `SubKind` is a *field* of the subtitle outcome, so the enum tag
        // (`outcome`) and the track kind (`kind`) coexist without colliding.
        assert_eq!(json["kind"], "human");
        assert_eq!(json["source_lang"], "zh");
        assert_eq!(json["skipped"], false);
    }

    fn touch(path: &Path, body: &str) {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(path, body).unwrap();
    }

    /// Regression for the reuse rule: `.part`, `.ytdl`, `.info.json`, `.json` and
    /// `.srt` leftovers must never be mistaken for a finished download.
    #[test]
    fn cache_lookup_ignores_sidecars_and_partials() {
        let dir = scratch_dir("cache");
        touch(&dir.join("BV1.m4a.part"), "partial");
        touch(&dir.join("BV1.webm.ytdl"), "{}");
        touch(&dir.join("BV1.info.json"), "{}");
        touch(&dir.join("BV1.json"), "{}");
        touch(&dir.join("BV1.zh-CN.srt"), "1\n00:00:00,000 --> 00:00:01,000\nhi\n");
        touch(&dir.join("BV1.xml"), "<xml/>");
        touch(&dir.join("BV1.m4a"), "");
        assert_eq!(media_file_in(&dir), None, "only a non-empty media file counts");

        touch(&dir.join("BV1.m4a"), "audio bytes");
        assert_eq!(media_file_in(&dir), Some(dir.join("BV1.m4a")));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn downloaded_media_is_found_in_its_id_subdirectory() {
        let dir = scratch_dir("downloaded");
        let nested = dir.join("BV1");
        touch(&nested.join("BV1.m4a"), "audio");
        assert_eq!(downloaded_media_file(&dir, "BV1"), Some(nested.join("BV1.m4a")));

        // A flat download (the `-o` template did not nest) is found too.
        let flat = scratch_dir("downloaded-flat");
        touch(&flat.join("BV1.m4a"), "audio");
        assert_eq!(downloaded_media_file(&flat, "BV1"), Some(flat.join("BV1.m4a")));

        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::remove_dir_all(&flat);
    }

    /// A cancel removes the `.part` file but keeps the finished audio.
    #[test]
    fn removing_partials_leaves_finished_files_alone() {
        let dir = scratch_dir("partials");
        touch(&dir.join("BV1/BV1.m4a"), "done");
        touch(&dir.join("BV1/BV1.m4a.part"), "half");
        touch(&dir.join("BV1/BV1.webm.ytdl"), "{}");
        touch(&dir.join("info.json"), "{}");
        remove_partial_downloads(&dir);

        assert!(dir.join("BV1/BV1.m4a").is_file(), "finished audio must survive");
        assert!(!dir.join("BV1/BV1.m4a.part").exists(), ".part must be cleaned");
        assert!(!dir.join("BV1/BV1.webm.ytdl").exists(), ".ytdl must be cleaned");

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Regression for a gap the design promises but the first cut missed: a
    /// partial download lives under the per-video CACHE directory, not in the
    /// scratch directory, so a cancel has to sweep both.
    #[test]
    fn cancelling_sweeps_partials_in_the_cache_directory_too() {
        let work = scratch_dir("cancel-work");
        let cache = scratch_dir("cancel-cache");

        // What a download in progress leaves behind.
        touch(&cache.join("BV1.m4a.part"), "half");
        touch(&cache.join("BV1.webm.ytdl"), "{}");
        // What a finished earlier download leaves behind: must survive.
        touch(&cache.join("BV1.m4a"), "audio");
        // Scratch artefacts.
        touch(&work.join("info.json"), "{}");
        touch(&work.join("BV1/partial.srt.part"), "half");

        // The same two calls `cancel_url_import` makes when the dirs differ.
        remove_partial_downloads(&work);
        let _ = std::fs::remove_dir_all(&work);
        if cache != work && cache.is_dir() {
            remove_partial_downloads(&cache);
        }

        assert!(!work.exists(), "the scratch directory must be gone");
        assert!(!cache.join("BV1.m4a.part").exists(), ".part must be cleaned from the cache");
        assert!(!cache.join("BV1.webm.ytdl").exists(), ".ytdl must be cleaned from the cache");
        assert!(
            cache.join("BV1.m4a").is_file(),
            "a finished download must survive so a retry costs no network"
        );

        let _ = std::fs::remove_dir_all(&cache);
    }

    /// The cache directory the cancel sweep targets is named after the video id in
    /// the URL; an unparseable URL must not produce a path that escapes the root.
    #[test]
    fn the_video_id_hint_only_ever_yields_a_single_path_segment() {
        assert_eq!(
            video_id_hint("https://www.bilibili.com/video/BV1GJ411x7h7"),
            "BV1GJ411x7h7"
        );
        assert_eq!(
            video_id_hint("https://www.bilibili.com/video/BV1GJ411x7h7?p=3"),
            "BV1GJ411x7h7"
        );
        assert_eq!(
            video_id_hint("https://www.bilibili.com/video/av80433022"),
            "av80433022"
        );
        for hostile in [
            "not a url",
            "https://b23.tv/",
            "https://www.bilibili.com/video/..%2F..%2Fetc/passwd",
        ] {
            let hint = video_id_hint(hostile);
            assert!(!hint.contains('/'), "{hostile} -> {hint}");
            assert!(!hint.contains(".."), "{hostile} -> {hint}");
        }
    }

    /// The work-dir guard is what makes a cancel clean: dropping it (which an
    /// abort does) removes the whole scratch directory.
    #[test]
    fn scratch_directory_is_removed_when_the_guard_drops() {
        let dir = scratch_dir("guard");
        {
            let _guard = WorkDirGuard { path: dir.clone() };
            assert!(dir.is_dir());
        }
        assert!(!dir.exists(), "the guard must remove the scratch directory");
    }

    #[test]
    fn skip_and_partial_extensions_are_recognised() {
        for extension in ["part", "ytdl", "json", "srt", "vtt", "xml", "temp", "PART"] {
            assert!(is_sidecar_extension(extension), "{extension}");
        }
        for extension in ["m4a", "webm", "mp4", "mp3", "opus", "flac"] {
            assert!(!is_sidecar_extension(extension), "{extension}");
        }
    }

    #[test]
    fn blank_cookies_are_treated_as_unconfigured() {
        let mut req = request("srt");
        assert_eq!(cookies(&req), None);
        req.cookies_browser = Some("  ".to_string());
        assert_eq!(cookies(&req), None);
        req.cookies_browser = Some(" chrome ".to_string());
        assert_eq!(cookies(&req), Some("chrome".to_string()));
    }

    /// The registry starts empty for a given id and is id-scoped, so two imports
    /// cannot clobber each other.
    #[tokio::test]
    async fn unregister_only_removes_the_named_entry() {
        let mut tasks = IMPORT_TASKS.lock().await;
        tasks.clear();
        for id in ["a", "b"] {
            let handle = tokio::spawn(async {});
            tasks.insert(
                id.to_string(),
                ImportEntry {
                    handle,
                    work_dir: PathBuf::from(format!("/tmp/{}", id)),
                    cache_dir: PathBuf::from(format!("/tmp/cache/{}", id)),
                },
            );
        }
        drop(tasks);

        unregister("a").await;
        let tasks = IMPORT_TASKS.lock().await;
        assert!(!tasks.contains_key("a"));
        assert!(tasks.contains_key("b"));
        for entry in tasks.values() {
            entry.handle.abort();
        }
        drop(tasks);
        IMPORT_TASKS.lock().await.clear();
    }

    #[tokio::test]
    async fn rate_limit_retries_then_gives_up() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        let attempts = Arc::new(AtomicUsize::new(0));
        let counter = Arc::clone(&attempts);

        let result: Result<(), ImportError> = with_rate_limit_retry(|| {
            let counter = Arc::clone(&counter);
            async move {
                counter.fetch_add(1, Ordering::SeqCst);
                Err(ImportError::RateLimited)
            }
        })
        .await;

        assert!(matches!(result, Err(ImportError::RateLimited)));
        // The first try plus two retries.
        assert_eq!(attempts.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn a_non_retryable_failure_is_not_retried() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        let attempts = Arc::new(AtomicUsize::new(0));
        let counter = Arc::clone(&attempts);

        let result: Result<(), ImportError> = with_rate_limit_retry(|| {
            let counter = Arc::clone(&counter);
            async move {
                counter.fetch_add(1, Ordering::SeqCst);
                Err(ImportError::NeedsCookies)
            }
        })
        .await;

        assert_eq!(result.unwrap_err(), ImportError::NeedsCookies);
        assert_eq!(attempts.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn a_success_is_returned_immediately() {
        let result: Result<u8, ImportError> = with_rate_limit_retry(|| async { Ok(7) }).await;
        assert_eq!(result.unwrap(), 7);
    }
}
