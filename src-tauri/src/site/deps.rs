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

    /// A previously downloaded media file for this video, if still on disk. The
    /// lookup happens in the import output directory, because that is where a
    /// finished download now lives.
    fn cached_audio(&self, params: &JobParams, media: &crate::site::SiteMedia) -> Option<PathBuf>;

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

    fn cached_audio(&self, params: &JobParams, media: &crate::site::SiteMedia) -> Option<PathBuf> {
        crate::site::cached_media_file(params, media)
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
    /// What `download_audio` hands back; `None` means "nothing was produced".
    pub downloaded: Mutex<Option<Result<PathBuf, ImportError>>>,
    /// An explicit `cached_audio` answer; when unset the fake falls back to the
    /// production rule (look in the output directory), so a reuse test exercises
    /// the same lookup the app runs.
    pub cached: Mutex<Option<PathBuf>>,
    /// The `output_dir` of every `download_audio` call, so a test can assert the
    /// media was asked for in the user's directory rather than in the cache.
    pub download_dirs: Mutex<Vec<PathBuf>>,
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

    pub async fn download_dirs(&self) -> Vec<PathBuf> {
        self.download_dirs.lock().await.clone()
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

    /// Download the audio, reporting progress, and return the media file.
    ///
    /// With no explicit answer set, the fake mirrors the production contract
    /// instead of failing: the media file is created in the request's
    /// `output_dir` under the subtitle's stem. A test can therefore assert where
    /// the media lands without restating the rule.
    fn download_audio<'a>(
        &'a self,
        params: &'a JobParams,
        media: &'a crate::site::SiteMedia,
    ) -> BoxFuture<'a, Result<PathBuf, ImportError>> {
        Box::pin(async move {
            let output_dir = PathBuf::from(&params.request.output_dir);
            self.download_dirs.lock().await.push(output_dir.clone());
            if let Some(explicit) = self.downloaded.lock().await.clone() {
                return explicit;
            }
            let path = output_dir.join(format!(
                "{}.m4a",
                crate::site::naming::file_stem(&media.title, &media.id)
            ));
            std::fs::create_dir_all(&output_dir).map_err(|error| {
                ImportError::Other(format!("fake: cannot create {}: {error}", output_dir.display()))
            })?;
            std::fs::write(&path, b"audio")
                .map_err(|error| ImportError::Other(format!("fake: cannot write media: {error}")))?;
            Ok(path)
        })
    }

    fn cached_audio(&self, params: &JobParams, media: &crate::site::SiteMedia) -> Option<PathBuf> {
        if let Some(explicit) = self.cached.try_lock().ok().and_then(|cached| cached.clone()) {
            return Some(explicit);
        }
        // No explicit answer: run the production rule, so the reuse tests cover
        // the lookup the app actually performs.
        crate::site::cached_media_file(params, media)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::site::resolve::{Resolved, SiteMedia};
    use crate::site::{JobParams, Outcome, UrlImportRequest};

    fn scratch_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir()
            .join(format!("monosub-deps-{}-{}", tag, uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// A job whose output (and therefore media) directory is `output_dir`.
    fn params(output_dir: &Path) -> JobParams {
        JobParams {
            request: UrlImportRequest {
                task_id: "t1".to_string(),
                url: "https://www.bilibili.com/video/BV1GJ411x7h7".to_string(),
                language: "zh".to_string(),
                formats: "srt".to_string(),
                output_dir: output_dir.to_string_lossy().into_owned(),
                skip_existing: false,
                cookies_browser: None,
                translate_enabled: None,
                target_language: None,
                translation_model: None,
                prefer_local_recognition: None,
            },
            formats: crate::subtitle::parse_formats("srt").unwrap(),
            work_dir: output_dir.to_path_buf(),
        }
    }

    /// No usable track, so the run has to take the download path.
    fn media_without_subtitles() -> SiteMedia {
        SiteMedia {
            id: "BV1GJ411x7h7".to_string(),
            title: "Title".to_string(),
            duration: Some(212.4),
            subs: Default::default(),
        }
    }

    async fn deps_for(media: SiteMedia) -> FakeDeps {
        let deps = FakeDeps::default();
        *deps.resolved.lock().await = Some(Resolved {
            media,
            stderr: String::new(),
        });
        deps
    }

    /// The media the fallback downloaded is the user's, so it lands in the output
    /// directory — not in the app cache, which is where the old design put it.
    #[tokio::test]
    async fn the_downloaded_media_lands_in_the_configured_directory() {
        let dir = scratch_dir("media-out");
        let params = params(&dir);
        let deps = deps_for(media_without_subtitles()).await;

        let outcome = crate::site::pipeline::run(&deps, &params, None).await.unwrap();
        let Outcome::Queued { path, .. } = outcome else {
            panic!("expected Queued, got {outcome:?}");
        };

        let expected = dir.join("Title [BV1GJ411x7h7].m4a");
        assert_eq!(path, expected.to_string_lossy());
        assert!(expected.is_file(), "the media must exist in the output directory");
        // ...and the download stage was asked for exactly that directory.
        assert_eq!(deps.download_dirs().await, vec![dir.clone()]);

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A platform-subtitle hit writes the subtitle into the same directory the
    /// media would have gone to, under the same stem: one folder, one video.
    #[tokio::test]
    async fn a_subtitle_hit_shares_the_directory_and_stem_with_the_media() {
        let dir = scratch_dir("same-dir");
        let mut media = media_without_subtitles();
        media.subs.insert(
            "zh-Hans".to_string(),
            crate::site::resolve::SubTrack {
                ext: "srt".to_string(),
                data: Some("inline".to_string()),
            },
        );
        let deps = deps_for(media).await;
        *deps.fetch.lock().await = std::collections::HashMap::from([(
            "zh-Hans".to_string(),
            Ok("1\n00:00:01,000 --> 00:00:02,000\n你好\n".to_string()),
        )]);

        let params = params(&dir);
        let outcome = crate::site::pipeline::run(&deps, &params, None).await.unwrap();
        let Outcome::Subtitle { paths, .. } = outcome else {
            panic!("expected a subtitle outcome, got {outcome:?}");
        };

        let subtitle = PathBuf::from(&paths[0]);
        assert_eq!(subtitle, dir.join("Title [BV1GJ411x7h7].srt"));
        assert_eq!(
            subtitle.with_extension("m4a"),
            dir.join("Title [BV1GJ411x7h7].m4a"),
            "the media name is the subtitle name with a different extension"
        );
        assert!(
            deps.download_dirs().await.is_empty(),
            "a hit must not download anything"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Re-importing a URL whose media is already in the output directory skips the
    /// download completely, so a retry costs no network.
    #[tokio::test]
    async fn a_media_file_already_in_the_output_directory_is_reused() {
        let dir = scratch_dir("reuse");
        let existing = dir.join("Title [BV1GJ411x7h7].m4a");
        std::fs::write(&existing, b"audio").unwrap();

        let params = params(&dir);
        let deps = deps_for(media_without_subtitles()).await;

        let outcome = crate::site::pipeline::run(&deps, &params, None).await.unwrap();
        let Outcome::Queued { path, name, .. } = outcome else {
            panic!("expected Queued, got {outcome:?}");
        };

        assert_eq!(path, existing.to_string_lossy());
        assert_eq!(name, "Title [BV1GJ411x7h7].m4a");
        assert!(
            deps.download_dirs().await.is_empty(),
            "the download stage must not run when the media is already there"
        );
        assert!(
            deps.events().await.is_empty(),
            "a reuse must not emit a downloading_audio stage"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A cancel sweeps the scratch and temp directories only. A finished media file
    /// lives in the output directory, so no cancel can delete it — the same rule
    /// the project relies on for a pre-existing subtitle.
    #[test]
    fn a_cancel_sweep_never_reaches_the_output_directory() {
        let output = scratch_dir("cancel-output");
        let work = scratch_dir("cancel-work");
        let temp = scratch_dir("cancel-temp");

        let finished = output.join("Title [BV1GJ411x7h7].m4a");
        std::fs::write(&finished, b"audio").unwrap();
        std::fs::write(temp.join("Title [BV1GJ411x7h7].m4a.part"), b"half").unwrap();
        std::fs::write(work.join("info.json"), b"{}").unwrap();

        // The same two calls `cancel_url_import` makes when the dirs differ.
        crate::site::remove_partial_downloads(&work);
        let _ = std::fs::remove_dir_all(&work);
        if temp != work && temp.is_dir() {
            crate::site::remove_partial_downloads(&temp);
        }

        assert!(!work.exists(), "the scratch directory must be gone");
        assert!(
            !temp.join("Title [BV1GJ411x7h7].m4a.part").exists(),
            "the in-flight .part must be cleaned"
        );
        assert!(finished.is_file(), "the finished media must survive a cancel");

        let _ = std::fs::remove_dir_all(&output);
        let _ = std::fs::remove_dir_all(&temp);
    }
}
