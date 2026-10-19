//! The seams the import pipeline runs against.
//!
//! Tasks: U3-5 (make the decision path testable). See
//! docs/tech-spec/url-import-design.md §3.
//!
//! `pipeline::run` decides *what* to do — select a track, fetch it, write it,
//! translate it, or fall back to transcription — and does so through this trait.
//! The production implementation shells out to yt-dlp; the tests supply a fake,
//! which is the only way to cover the branching (hit / fall back / skip /
//! translate) without a network and a real video.

use std::path::{Path, PathBuf};

#[cfg(test)]
use tokio::sync::Mutex;

use crate::site::error::ImportError;
use crate::site::resolve::Resolved;
use crate::site::{JobParams, Stage};

/// The translation stage as the pipeline needs it.
pub type WebTranslation = crate::translate::stage::TranslationPlan;

/// A boxed future. The crate has no `async-trait` dependency, and every `Deps`
/// method is called through a generic (`run<D: Deps>`), so a plain `Future` is
/// all the seam needs — but two of the methods must be `dyn`-free futures
/// because they are called from an `async fn` that also borrows `params`.
pub type BoxFuture<'a, T> = std::pin::Pin<Box<dyn std::future::Future<Output = T> + Send + 'a>>;

/// One import's outside world.
///
/// Deliberately **not** an `async fn` trait: without a proc-macro dependency
/// those are not dyn-compatible, and the point of this seam is that the
/// production implementation and the test fake are interchangeable. Two methods
/// (`resolve`, `fetch_subtitle`, `download_audio`, `translate`) are the network
/// calls; everything else is bookkeeping.
pub trait Deps: Send + Sync {
    /// Emit a progress event for this stage. Implementations override this: the
    /// production one emits a Tauri event, the test fake records the stage.
    fn progress(&self, stage: Stage, progress: Option<f64>);

    /// One Resolve call → the media, its subtitle list and its stderr.
    fn resolve<'a>(&'a self, params: &'a JobParams) -> BoxFuture<'a, Result<Resolved, ImportError>>;

    /// Fetch one subtitle track and hand back its SRT text.
    fn fetch_subtitle<'a>(
        &'a self,
        params: &'a JobParams,
        media: &'a crate::site::SiteMedia,
        tag: &'a str,
    ) -> BoxFuture<'a, Result<String, ImportError>>;

    /// Download the audio, reporting progress, and return the media file.
    fn download_audio<'a>(
        &'a self,
        params: &'a JobParams,
        media: &'a crate::site::SiteMedia,
    ) -> BoxFuture<'a, Result<PathBuf, ImportError>>;

    /// A previously downloaded audio file for this video, if still on disk.
    fn cached_audio(&self, media: &crate::site::SiteMedia) -> Option<PathBuf>;

    /// Translate the written cues, returning the translated paths.
    fn translate<'a>(
        &'a self,
        primary: &'a Path,
        formats: &'a [crate::subtitle::Format],
        segments: &'a [crate::subtitle::Segment],
        translation: &'a WebTranslation,
        source_lang: &'a str,
    ) -> BoxFuture<'a, Vec<PathBuf>>;
}

/// The production implementation: everything goes through yt-dlp on disk.
pub(crate) struct RealDeps<'a> {
    pub app: &'a tauri::AppHandle,
    task_id: String,
}

impl RealDeps<'_> {
    pub fn new<'a>(app: &'a tauri::AppHandle, task_id: &str) -> RealDeps<'a> {
        RealDeps {
            app,
            task_id: task_id.to_string(),
        }
    }
}

impl Deps for RealDeps<'_> {
    fn progress(&self, stage: Stage, progress: Option<f64>) {
        crate::site::emit_stage(self.app, &self.task_id, stage, progress);
    }

    fn resolve<'a>(&'a self, params: &'a JobParams) -> BoxFuture<'a, Result<Resolved, ImportError>> {
        Box::pin(async move { crate::site::resolve::run(self.app, &params.request).await })
    }

    fn fetch_subtitle<'a>(
        &'a self,
        params: &'a JobParams,
        media: &'a crate::site::SiteMedia,
        tag: &'a str,
    ) -> BoxFuture<'a, Result<String, ImportError>> {
        Box::pin(
            async move { crate::site::fetch_subtitle_text(self.app, params, media, tag).await },
        )
    }

    fn download_audio<'a>(
        &'a self,
        params: &'a JobParams,
        media: &'a crate::site::SiteMedia,
    ) -> BoxFuture<'a, Result<PathBuf, ImportError>> {
        Box::pin(async move { crate::site::download_audio(self.app, params, media).await })
    }

    fn cached_audio(&self, media: &crate::site::SiteMedia) -> Option<PathBuf> {
        crate::site::cached_media_file(self.app, &media.id)
    }

    fn translate<'a>(
        &'a self,
        primary: &'a Path,
        formats: &'a [crate::subtitle::Format],
        segments: &'a [crate::subtitle::Segment],
        translation: &'a WebTranslation,
        source_lang: &'a str,
    ) -> BoxFuture<'a, Vec<PathBuf>> {
        let app = self.app;
        let task_id = self.task_id.clone();
        Box::pin(async move {
            let outcome = crate::translate::stage::translate_and_write(
                primary,
                formats,
                translation,
                segments,
                Some(source_lang),
                Some(app),
                move |done, total| {
                    let percent = if total == 0 {
                        None
                    } else {
                        Some(done as f64 * 100.0 / total as f64)
                    };
                    crate::site::emit_stage(app, &task_id, Stage::Translating, percent);
                },
            )
            .await;
            outcome.written
        })
    }
}

#[cfg(test)]
#[derive(Default)]
pub struct FakeDeps {
    pub events: Mutex<Vec<(Stage, Option<f64>)>>,
    pub resolved: Mutex<Option<Resolved>>,
    pub resolve_error: Mutex<Option<ImportError>>,
    /// Inline text per tag, as `fetch_subtitle` would return it.
    pub fetch: Mutex<std::collections::HashMap<String, Result<String, ImportError>>>,
    /// What `download_audio` hands back; `None` means "already cached".
    pub downloaded: Mutex<Option<Result<PathBuf, ImportError>>>,
    pub cached: Mutex<Option<PathBuf>>,
    /// Records the `(tag)` of every fetch so a test can assert it was never made.
    pub fetched_tags: Mutex<Vec<String>>,
    pub translated_texts: Mutex<Vec<String>>,
}

#[cfg(test)]
impl FakeDeps {
    pub async fn events(&self) -> Vec<(Stage, Option<f64>)> {
        self.events.lock().await.clone()
    }

    pub async fn fetched_tags(&self) -> Vec<String> {
        self.fetched_tags.lock().await.clone()
    }
}

#[cfg(test)]
impl Deps for FakeDeps {
    fn progress(&self, stage: Stage, progress: Option<f64>) {
        // `try_lock` keeps this sync method usable: the test awaits the recorded
        // events after the run, and a contended lock here would deadlock the
        // single-threaded runtime.
        if let Ok(mut events) = self.events.try_lock() {
            events.push((stage, progress));
        }
    }

    fn resolve<'a>(&'a self, _params: &'a JobParams) -> BoxFuture<'a, Result<Resolved, ImportError>> {
        Box::pin(async move {
            if let Some(error) = self.resolve_error.lock().await.take() {
                return Err(error);
            }
            self.resolved
                .lock()
                .await
                .clone()
                .ok_or_else(|| ImportError::Other("fake: no resolve result".to_string()))
        })
    }

    fn fetch_subtitle<'a>(
        &'a self,
        _params: &'a JobParams,
        _media: &'a crate::site::SiteMedia,
        tag: &'a str,
    ) -> BoxFuture<'a, Result<String, ImportError>> {
        Box::pin(async move {
            self.fetched_tags.lock().await.push(tag.to_string());
            self.fetch
                .lock()
                .await
                .get(tag)
                .cloned()
                .unwrap_or_else(|| Err(ImportError::SubtitleUnusable(format!("fake: no {tag}"))))
        })
    }

    fn download_audio<'a>(
        &'a self,
        _params: &'a JobParams,
        _media: &'a crate::site::SiteMedia,
    ) -> BoxFuture<'a, Result<PathBuf, ImportError>> {
        Box::pin(async move {
            self.downloaded
                .lock()
                .await
                .clone()
                .unwrap_or_else(|| Err(ImportError::Other("fake: no audio".to_string())))
        })
    }

    fn cached_audio(&self, _media: &crate::site::SiteMedia) -> Option<PathBuf> {
        self.cached.try_lock().ok().and_then(|cached| cached.clone())
    }

    fn translate<'a>(
        &'a self,
        primary: &'a Path,
        formats: &'a [crate::subtitle::Format],
        segments: &'a [crate::subtitle::Segment],
        _translation: &'a WebTranslation,
        source_lang: &'a str,
    ) -> BoxFuture<'a, Vec<PathBuf>> {
        Box::pin(async move {
            self.translated_texts.lock().await.push(source_lang.to_string());
            crate::translate::stage::translated_paths(primary, formats, "en")
                .into_iter()
                .filter(|_| !segments.is_empty())
                .collect()
        })
    }
}
