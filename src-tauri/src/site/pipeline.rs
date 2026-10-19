//! The import decision path: select a track, fetch it, write it, translate it,
//! or fall back to transcription.
//!
//! Tasks: U2-3 (`materialize`/fetch), U2-4 (write), U2-5 (translate), U3-3
//! (`Queued`), U3-7 (prefer local). See docs/tech-spec/url-import-design.md §3.
//!
//! Written against [`Deps`] rather than yt-dlp, so every branch — hit, fetch from
//! the info dict, skip, translate, fall back, fail — is exercised by a test with
//! no process and no network. The production wiring is `deps::RealDeps`.

use std::path::PathBuf;

use crate::site::deps::{Deps, WebTranslation};
use crate::site::error::ImportError;
use crate::site::lang::{self, OutputLanguage, Pick};
use crate::site::naming;
use crate::site::{srt, JobParams, Outcome, SiteMedia, Stage};
use crate::subtitle;

/// Run one URL end to end.
///
/// `translation` is decided by the caller before this runs, so a missing
/// translation model reports `MODEL_NOT_INSTALLED:<id>` without a network round
/// trip (design §5.6).
pub async fn run<D: Deps + ?Sized>(
    deps: &D,
    params: &JobParams,
    translation: Option<WebTranslation>,
) -> Result<Outcome, ImportError> {
    let request = &params.request;
    let task_id = &request.task_id;
    let language = OutputLanguage::parse(&request.language);

    let resolved = deps.resolve(params).await?;
    let media = &resolved.media;
    crate::site::resolve::check_multipart(&request.url, media)?;

    // "Prefer local recognition" skips selection entirely (design §7-2): the user
    // wants the app's own ASR and its `words.json` sidecar, not the platform's.
    let pick = if request.prefer_local_recognition.unwrap_or(false) {
        tracing::info!(task_id = %task_id, "prefer_local_recognition: skipping the subtitle lookup");
        None
    } else {
        lang::select(language, media)
    };

    if let Some(pick) = pick {
        deps.progress(Stage::FetchingSubtitles, None);
        match materialize(deps, params, media, &pick, translation.as_ref()).await {
            Ok(outcome) => return Ok(outcome),
            // A subtitle that is absent, empty or unparseable is the documented
            // fallback trigger (design §4.3), not a failure.
            Err(error) if error.allows_fallback() => tracing::warn!(
                task_id = %task_id,
                status = %error.status(),
                "platform subtitle unusable; falling back to transcription"
            ),
            Err(error) => return Err(error),
        }
    }

    fallback(deps, params, media, resolved.needs_login_hint()).await
}

/// Fetch the selected track, write every format and translate when asked.
async fn materialize<D: Deps + ?Sized>(
    deps: &D,
    params: &JobParams,
    media: &SiteMedia,
    pick: &Pick,
    translation: Option<&WebTranslation>,
) -> Result<Outcome, ImportError> {
    let request = &params.request;
    let task_id = &request.task_id;

    let srt_text = deps.fetch_subtitle(params, media, &pick.tag).await?;
    let segments = srt::parse(&srt_text);
    if segments.is_empty() {
        return Err(ImportError::SubtitleUnusable(format!(
            "{} produced no cues",
            pick.tag
        )));
    }

    deps.progress(Stage::Writing, None);

    let output_dir = PathBuf::from(&request.output_dir);
    std::fs::create_dir_all(&output_dir)
        .map_err(|error| ImportError::Other(format!("urlImport.outputDirFailed:{error}")))?;

    // The primary path takes the first format's extension, so every later format
    // only swaps it (`subtitle::output_path_for`) — the same rule the
    // transcription path uses for its own multi-format output.
    let primary = naming::output_path(
        &output_dir,
        &media.title,
        &media.id,
        params.formats[0].extension(),
    );
    let paths = subtitle::output_paths_for(&primary, &params.formats);
    let paths_as_strings = || {
        paths
            .iter()
            .map(|path| path.to_string_lossy().into_owned())
            .collect::<Vec<_>>()
    };

    // `skipExistingSubtitle` is the local-file rule; because the name carries the
    // video id, a "same name" file can only be the same video (design §5.5).
    if request.skip_existing && paths.iter().any(|path| path.is_file()) {
        tracing::info!(
            task_id = %task_id,
            path = %primary.display(),
            "subtitle already exists; skipping per skipExistingSubtitle"
        );
        return Ok(Outcome::Subtitle {
            paths: paths_as_strings(),
            translated_paths: Vec::new(),
            tag: pick.tag.clone(),
            kind: pick.kind,
            source_lang: pick.source_lang.to_string(),
            skipped: true,
        });
    }

    for (index, format) in params.formats.iter().enumerate() {
        let path = subtitle::output_path_for(&primary, index, *format);
        subtitle::write(&segments, &path, *format)
            .map_err(|error| ImportError::Other(format!("urlImport.writeFailed:{}", error)))?;
        tracing::info!(
            task_id = %task_id,
            path = %path.display(),
            format = %format.extension(),
            track_kind = pick.kind.as_str(),
            "platform subtitle written"
        );
    }

    // Translation is the same shared stage the transcription path runs, with a
    // *known* source language (`Pick.source_lang`), so a same-language run is
    // skipped accurately instead of by script guess (design §5.6).
    let mut translated_paths = Vec::new();
    if let Some(translation) = translation {
        deps.progress(Stage::Translating, None);
        translated_paths = deps
            .translate(&primary, &params.formats, &segments, translation, pick.source_lang)
            .await
            .iter()
            .map(|path| path.to_string_lossy().into_owned())
            .collect();
    }

    Ok(Outcome::Subtitle {
        paths: paths_as_strings(),
        translated_paths,
        tag: pick.tag.clone(),
        kind: pick.kind,
        source_lang: pick.source_lang.to_string(),
        skipped: false,
    })
}

/// Download the audio, then hand it to the frontend as a queued file.
async fn fallback<D: Deps + ?Sized>(
    deps: &D,
    params: &JobParams,
    media: &SiteMedia,
    login_hint: bool,
) -> Result<Outcome, ImportError> {
    let audio = match deps.cached_audio(media) {
        Some(path) => {
            tracing::info!(path = %path.display(), "reusing cached audio");
            path
        }
        None => {
            deps.progress(Stage::DownloadingAudio, None);
            deps.download_audio(params, media).await?
        }
    };

    let name = naming::file_stem(&media.title, &media.id)
        + "."
        + audio
            .extension()
            .and_then(|extension| extension.to_str())
            .unwrap_or("m4a");

    Ok(Outcome::Queued {
        path: audio.to_string_lossy().into_owned(),
        title: media.title.clone(),
        name,
        duration: media.duration,
        login_hint,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::site::deps::FakeDeps;
    use crate::site::lang::SubKind;
    use crate::site::resolve::{Resolved, SiteMedia, SubTrack};
    use std::collections::BTreeMap;

    fn scratch_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("monosub-pipeline-{}-{}", tag, uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn cue_srt(text: &str) -> String {
        format!("1\n00:00:01,000 --> 00:00:02,000\n{text}\n")
    }

    fn media(tracks: &[(&str, &str, Option<&str>)]) -> SiteMedia {
        let mut subs = BTreeMap::new();
        for (tag, ext, data) in tracks {
            subs.insert(
                (*tag).to_string(),
                SubTrack {
                    ext: (*ext).to_string(),
                    data: data.map(str::to_string),
                },
            );
        }
        SiteMedia {
            id: "BV1GJ411x7h7".to_string(),
            title: "Title".to_string(),
            duration: Some(212.4),
            subs,
        }
    }

    fn params(output_dir: &std::path::Path, formats: &str) -> JobParams {
        JobParams {
            request: crate::site::UrlImportRequest {
                task_id: "t1".to_string(),
                url: "https://www.bilibili.com/video/BV1GJ411x7h7".to_string(),
                language: "zh".to_string(),
                formats: formats.to_string(),
                output_dir: output_dir.to_string_lossy().into_owned(),
                skip_existing: false,
                cookies_browser: None,
                translate_enabled: None,
                target_language: None,
                translation_model: None,
                prefer_local_recognition: None,
            },
            formats: crate::subtitle::parse_formats(formats).unwrap(),
            work_dir: output_dir.to_path_buf(),
        }
    }

    async fn fake(media: SiteMedia, tags: &[(&str, &str)]) -> FakeDeps {
        let deps = FakeDeps::default();
        *deps.resolved.lock().await = Some(Resolved {
            media,
            stderr: String::new(),
        });
        {
            let mut fetch = deps.fetch.lock().await;
            for (tag, text) in tags {
                fetch.insert((*tag).to_string(), Ok((*text).to_string()));
            }
        }
        deps
    }

    /// The hit path: two formats, both written, named `<title> [<id>].<ext>`.
    #[tokio::test]
    async fn a_hit_writes_every_requested_format() {
        let dir = scratch_dir("hit");
        let deps = fake(media(&[("zh-CN", "srt", Some("inline"))]), &[("zh-CN", &cue_srt("你好"))]).await;
        let params = params(&dir, "srt,txt");

        let outcome = run(&deps, &params, None).await.expect("a hit must succeed");
        match outcome {
            Outcome::Subtitle { paths, tag, kind, translated_paths, skipped, .. } => {
                assert_eq!(tag, "zh-CN");
                assert_eq!(kind, SubKind::Human);
                assert!(!skipped);
                assert!(translated_paths.is_empty());
                assert_eq!(
                    paths,
                    vec![
                        dir.join("Title [BV1GJ411x7h7].srt").to_string_lossy().into_owned(),
                        dir.join("Title [BV1GJ411x7h7].txt").to_string_lossy().into_owned(),
                    ]
                );
            }
            other => panic!("expected a subtitle outcome, got {other:?}"),
        }

        let srt = std::fs::read_to_string(dir.join("Title [BV1GJ411x7h7].srt")).unwrap();
        assert!(srt.contains("你好"), "{srt}");
        let txt = std::fs::read_to_string(dir.join("Title [BV1GJ411x7h7].txt")).unwrap();
        assert!(txt.contains("你好"), "{txt}");

        // The stage sequence is the frozen one.
        let stages: Vec<Stage> = deps.events().await.into_iter().map(|(stage, _)| stage).collect();
        assert_eq!(stages, vec![Stage::FetchingSubtitles, Stage::Writing]);

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// No matching track → fall back, and the `Queued` name keeps the id.
    #[tokio::test]
    async fn no_matching_track_falls_back_to_transcription() {
        let dir = scratch_dir("fallback");
        let audio = dir.join("BV1GJ411x7h7.m4a");
        std::fs::write(&audio, b"audio").unwrap();

        let deps = fake(media(&[("en-US", "srt", Some("inline"))]), &[]).await;
        *deps.downloaded.lock().await = Some(Ok(audio.clone()));
        let params = params(&dir, "srt");

        let outcome = run(&deps, &params, None).await.expect("fallback is not an error");
        match outcome {
            Outcome::Queued { path, name, title, duration, login_hint } => {
                assert_eq!(path, audio.to_string_lossy());
                assert_eq!(name, "Title [BV1GJ411x7h7].m4a");
                assert_eq!(title, "Title");
                assert_eq!(duration, Some(212.4));
                assert!(!login_hint);
            }
            other => panic!("expected Queued, got {other:?}"),
        }

        // Nothing was fetched: no track matched, so no fetch call happened.
        assert!(deps.fetched_tags().await.is_empty());
        let stages: Vec<Stage> = deps.events().await.into_iter().map(|(stage, _)| stage).collect();
        assert_eq!(stages, vec![Stage::DownloadingAudio]);

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A cached audio file skips the download stage entirely (design §5.3).
    #[tokio::test]
    async fn cached_audio_skips_the_download() {
        let dir = scratch_dir("cached");
        let audio = dir.join("cached.m4a");
        std::fs::write(&audio, b"audio").unwrap();

        let deps = fake(media(&[]), &[]).await;
        *deps.cached.lock().await = Some(audio.clone());
        let params = params(&dir, "srt");

        let outcome = run(&deps, &params, None).await.unwrap();
        assert!(matches!(outcome, Outcome::Queued { .. }));
        assert!(
            deps.events().await.is_empty(),
            "a cached file must not emit a downloading_audio stage"
        );
        assert!(deps.downloaded.lock().await.is_none(), "the download seam must not run");

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// `skipExistingSubtitle` marks the run skipped and writes nothing.
    #[tokio::test]
    async fn skip_existing_does_not_rewrite_the_file() {
        let dir = scratch_dir("skip");
        let existing = dir.join("Title [BV1GJ411x7h7].srt");
        std::fs::write(&existing, "PRE-EXISTING").unwrap();

        let deps = fake(media(&[("zh-CN", "srt", Some("inline"))]), &[("zh-CN", &cue_srt("新"))]).await;
        let mut params = params(&dir, "srt");
        params.request.skip_existing = true;

        let outcome = run(&deps, &params, None).await.unwrap();
        match outcome {
            Outcome::Subtitle { skipped, paths, .. } => {
                assert!(skipped);
                assert_eq!(paths, vec![existing.to_string_lossy().into_owned()]);
            }
            other => panic!("expected a skipped subtitle outcome, got {other:?}"),
        }
        assert_eq!(
            std::fs::read_to_string(&existing).unwrap(),
            "PRE-EXISTING",
            "the pre-existing file must not be rewritten"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Prefer-local skips selection: no `select`, no fetch, straight to fallback
    /// (design §7-2).
    #[tokio::test]
    async fn prefer_local_recognition_skips_the_subtitle_lookup() {
        let dir = scratch_dir("prefer-local");
        let audio = dir.join("a.m4a");
        std::fs::write(&audio, b"audio").unwrap();

        // A perfectly good Chinese track exists; it must still be ignored.
        let deps = fake(media(&[("zh-Hans", "srt", Some("inline"))]), &[("zh-Hans", &cue_srt("你好"))]).await;
        *deps.downloaded.lock().await = Some(Ok(audio));
        let mut params = params(&dir, "srt");
        params.request.prefer_local_recognition = Some(true);

        let outcome = run(&deps, &params, None).await.unwrap();
        assert!(matches!(outcome, Outcome::Queued { .. }));
        assert!(deps.fetched_tags().await.is_empty());
        assert!(!dir.join("Title [BV1GJ411x7h7].srt").exists());

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A track that resolves but parses to nothing is a fallback, not a failure.
    #[tokio::test]
    async fn an_unparseable_track_falls_back() {
        let dir = scratch_dir("unparseable");
        let audio = dir.join("a.m4a");
        std::fs::write(&audio, b"audio").unwrap();

        let deps = fake(media(&[("zh-CN", "srt", Some("inline"))]), &[("zh-CN", "not srt at all")]).await;
        *deps.downloaded.lock().await = Some(Ok(audio));
        let params = params(&dir, "srt");

        let outcome = run(&deps, &params, None).await.unwrap();
        assert!(matches!(outcome, Outcome::Queued { .. }));
        assert!(!dir.join("Title [BV1GJ411x7h7].srt").exists());

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A login wall is never degraded into a fallback: the audio fetch would hit
    /// the same wall, so the user gets the browser picker instead.
    #[tokio::test]
    async fn a_login_wall_is_reported_not_swallowed() {
        let dir = scratch_dir("login");
        let deps = FakeDeps::default();
        *deps.resolve_error.lock().await = Some(ImportError::NeedsCookies);
        let params = params(&dir, "srt");

        let error = run(&deps, &params, None).await.unwrap_err();
        assert_eq!(error, ImportError::NeedsCookies);
        assert_eq!(error.status(), crate::site::NEEDS_COOKIES_STATUS);

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A multi-P video without `?p=` is rejected before anything is fetched.
    #[tokio::test]
    async fn a_multipart_video_without_a_part_is_rejected() {
        let dir = scratch_dir("multipart");
        let mut multi = media(&[("zh-CN", "srt", Some("inline"))]);
        multi.id = "BV1G5po6zECY_p1".to_string();
        let deps = fake(multi, &[("zh-CN", &cue_srt("你好"))]).await;
        let params = params(&dir, "srt");

        let error = run(&deps, &params, None).await.unwrap_err();
        assert_eq!(error, ImportError::MultiPart);
        assert!(deps.fetched_tags().await.is_empty());

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The `login_hint` from the Resolve stderr reaches the `Queued` outcome.
    #[tokio::test]
    async fn the_login_hint_travels_with_the_queued_outcome() {
        let dir = scratch_dir("hint");
        let audio = dir.join("a.m4a");
        std::fs::write(&audio, b"audio").unwrap();

        let deps = FakeDeps::default();
        *deps.resolved.lock().await = Some(Resolved {
            media: media(&[]),
            stderr: "WARNING: [BiliBili] Subtitles are only available when logged in. Use --cookies-from-browser".to_string(),
        });
        *deps.downloaded.lock().await = Some(Ok(audio));
        let params = params(&dir, "srt");

        match run(&deps, &params, None).await.unwrap() {
            Outcome::Queued { login_hint, .. } => assert!(login_hint),
            other => panic!("expected Queued, got {other:?}"),
        }

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// `auto` never selects a track, so it always transcribes (design §4.2).
    #[tokio::test]
    async fn auto_language_always_falls_back() {
        let dir = scratch_dir("auto");
        let audio = dir.join("a.m4a");
        std::fs::write(&audio, b"audio").unwrap();

        let deps = fake(media(&[("zh-CN", "srt", Some("inline")), ("en-US", "srt", Some("inline"))]), &[]).await;
        *deps.downloaded.lock().await = Some(Ok(audio));
        let mut params = params(&dir, "srt");
        params.request.language = "auto".to_string();

        assert!(matches!(run(&deps, &params, None).await.unwrap(), Outcome::Queued { .. }));
        assert!(deps.fetched_tags().await.is_empty());

        let _ = std::fs::remove_dir_all(&dir);
    }
}
