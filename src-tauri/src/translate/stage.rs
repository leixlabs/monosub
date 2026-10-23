//! The Tauri-free half of a translation stage: decide <em>whether</em> a run
//! translates, then write the translated files next to the originals.
//!
//! Task: U1-13. See docs/tech-spec/url-import-design.md §5.6.
//!
//! Two callers run a translation stage — the transcription pipeline
//! (`commands::transcribe`) and the URL import subtitle-hit path
//! (`site::import`) — and both must behave identically: same pre-flight gate,
//! same `<stem>.<lang>.<ext>` naming, same "a failure is a warning, never a
//! task failure" semantics. Before this module existed the naming and the
//! per-format write loop lived inside `commands/transcribe.rs`, so a second
//! caller had no choice but to copy them; that is exactly how two paths drift.
//!
//! What stays in `commands/transcribe.rs` is only the Tauri-specific part: the
//! step/progress events and the `TaskParams` it already owns.

use std::path::{Path, PathBuf};

use crate::commands::models::MODEL_NOT_INSTALLED;
use crate::engine;
use crate::engine::catalog::{Engine, ModelSpec, Role};
use crate::subtitle::{self, Format};
use crate::translate::{self, TranslateError};

/// The translation stage, decided before any work is spawned.
///
/// `model_path` is resolved at request time rather than at the end of the run,
/// so a missing model fails the request before a single process starts.
#[derive(Debug, Clone)]
pub struct TranslationPlan {
    pub model_id: String,
    pub model_path: PathBuf,
    /// The catalogue entry, which carries the prompt-template kind.
    pub model: ModelSpec,
    /// Target language as the frontend spelled it.
    pub target: String,
}

/// Why the pre-flight returned no plan.
///
/// Both cases are "nothing to do", not errors: the caller simply does not
/// translate. They are distinguished so a log line can say which happened.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkipReason {
    /// The request did not ask for translation.
    Disabled,
    /// The requested pair is the same language, so a translated file would be a
    /// misleading duplicate of the original.
    SameLanguage,
    /// There is no usable target language at all.
    NoTarget,
}

/// Whether this run needs translating, and into what.
///
/// Returns `Ok(None)` when there is nothing to do. The `Err` arm is the
/// `MODEL_NOT_INSTALLED:<id>` sentinel the frontend routes on.
///
/// `installed` and `model_file` are injected so the whole gate is a pure
/// function of (catalogue, install state, paths): it can be unit-tested without
/// a Tauri `AppHandle` or a real models directory.
#[allow(clippy::too_many_arguments)]
pub fn resolve(
    catalogue: &[ModelSpec],
    source_language: &str,
    enabled: Option<bool>,
    target: Option<&str>,
    model_id: Option<&str>,
    installed: &dyn Fn(&str) -> bool,
    model_file: &dyn Fn(&str) -> Option<PathBuf>,
) -> Result<Option<TranslationPlan>, String> {
    gate(catalogue, source_language, enabled, target, model_id, installed, model_file)
        .map_err(|(status, detail)| {
            // The detail is a human sentence for the log; the status is what the
            // UI renders. Keeping them separate is what lets `status` stay a
            // stable sentinel.
            tracing::warn!(status = %status, detail = %detail, "translation pre-flight rejected the request");
            status
        })
}

/// [`resolve`] with the diagnostic kept, so callers that log can say why.
#[allow(clippy::too_many_arguments)]
pub fn gate(
    catalogue: &[ModelSpec],
    source_language: &str,
    enabled: Option<bool>,
    target: Option<&str>,
    model_id: Option<&str>,
    installed: &dyn Fn(&str) -> bool,
    model_file: &dyn Fn(&str) -> Option<PathBuf>,
) -> Result<Option<TranslationPlan>, (String, String)> {
    let Some(reason) = skip_reason(source_language, enabled, target) else {
        // Translate: fall through to the model gate.
        return plan(catalogue, target.unwrap_or_default(), model_id, installed, model_file);
    };

    if reason != SkipReason::Disabled {
        tracing::info!(
            source = %source_language,
            target = ?target,
            reason = ?reason,
            "translation skipped"
        );
    }
    Ok(None)
}

/// The skip half of the gate, pure and total.
pub fn skip_reason(
    source_language: &str,
    enabled: Option<bool>,
    target: Option<&str>,
) -> Option<SkipReason> {
    if !enabled.unwrap_or(false) {
        return Some(SkipReason::Disabled);
    }
    let target = target.unwrap_or_default();

    // `language` may be `auto`; the ASR's own tag (and the fallback rule) decides
    // later whether the real source equals the target. Here only an EXPLICIT
    // equal pair is short-circuited, because skipping on a guess would silently
    // drop a translation the user asked for (translation-design §5).
    if canonical_source_equals_target(source_language, target) {
        return Some(SkipReason::SameLanguage);
    }
    if !translate::language::should_translate(Some(source_language), target) {
        // The source is `auto` and the target is missing/auto: nothing to ask for.
        return Some(SkipReason::NoTarget);
    }
    None
}

fn plan(
    catalogue: &[ModelSpec],
    target: &str,
    model_id: Option<&str>,
    installed: &dyn Fn(&str) -> bool,
    model_file: &dyn Fn(&str) -> Option<PathBuf>,
) -> Result<Option<TranslationPlan>, (String, String)> {
    let model_id = match model_id.map(str::trim).filter(|id| !id.is_empty()) {
        Some(id) => id.to_string(),
        None => engine::catalog::default_translation_model_id(catalogue).ok_or_else(|| {
            (
                MODEL_NOT_INSTALLED.to_string(),
                "the catalogue offers no translation model".to_string(),
            )
        })?,
    };

    let Some(model) = catalogue.iter().find(|spec| spec.id == model_id).cloned() else {
        return Err((
            format!("{}:{}", MODEL_NOT_INSTALLED, model_id),
            format!("translation model {} is not in the catalogue", model_id),
        ));
    };

    // A translate entry must run on llama.cpp; anything else would be handed to
    // `llama-server`, which cannot load an audio.cpp GGUF (design §3.2, §11).
    if model.engine != Engine::Llamacpp || !model.has_role(Role::Translate) {
        return Err((
            format!("{}:{}", MODEL_NOT_INSTALLED, model_id),
            format!(
                "{} is not a llama.cpp translate entry (engine {}); llama-server cannot load it",
                model_id,
                model.engine.as_str()
            ),
        ));
    }

    if !installed(&model_id) {
        return Err((
            format!("{}:{}", MODEL_NOT_INSTALLED, model_id),
            format!("translation model {} is not installed", model_id),
        ));
    }
    let Some(model_path) = model_file(&model_id) else {
        return Err((
            format!("{}:{}", MODEL_NOT_INSTALLED, model_id),
            format!("translation model {} has no usable file path", model_id),
        ));
    };

    tracing::info!(
        model = %model_id,
        target = %target,
        path = %model_path.display(),
        "translation enabled"
    );

    Ok(Some(TranslationPlan {
        model_id,
        model_path,
        model,
        target: target.to_string(),
    }))
}

/// True when the two languages are the same target — `zh` and `zh-tw` are not
/// (translation-design §9).
///
/// `auto`/empty is "unknown", never "the same language": treating the two as
/// equal would silently drop a translation the user asked for.
pub fn canonical_source_equals_target(source: &str, target: &str) -> bool {
    use crate::translate::language::{canonical_tag, is_unknown_language};

    if is_unknown_language(source) || is_unknown_language(target) {
        return false;
    }
    source.trim().eq_ignore_ascii_case(target.trim()) || canonical_tag(source) == canonical_tag(target)
}

/// What a completed translation stage produced.
#[derive(Debug, Default)]
pub struct StageOutcome {
    /// Files actually written; empty when nothing was translated.
    pub written: Vec<PathBuf>,
    /// True when the run degraded (some cues kept their original text).
    pub degraded: bool,
    /// Cues that received translated text.
    ///
    /// Carried out of the stage so a *degraded* run can say how large the shortfall
    /// is: "partly untranslated" on a 4884-cue file could mean one line or three
    /// hundred, and only the second is worth the user's attention. `degraded`
    /// alone is a boolean that hides a number the pipeline already computed.
    pub translated: usize,
    /// Cues that kept their original text.
    pub kept_original: usize,
    /// Set when the stage decided there was nothing to do and wrote nothing.
    ///
    /// This is **not** a failure: `SameLanguage` means a translated file would be
    /// a misleading duplicate, and `NoSegments` means the input was silent. Only
    /// a `None` here with `written` empty means the translation was attempted and
    /// produced nothing. Callers that report an outcome to a user (the CLI) need
    /// that distinction; the GUI does not care, which is why it was not here
    /// before.
    pub skipped: Option<crate::translate::SkipReason>,
}

/// `(source, target)` pairs on which a run would translate — the condition the
/// URL-import path uses to decide whether to resolve the model at all.
pub fn will_translate(source_language: &str, enabled: Option<bool>, target: Option<&str>) -> bool {
    skip_reason(source_language, enabled, target).is_none()
}

/// Translate `segments` and write one translated file per requested format.
///
/// A failure here is logged and swallowed: the originals are already on disk and
/// the caller's run still completes (design §2). Nothing is written when the run
/// skipped, when it translated nothing, or when the model could not be reached —
/// a `<stem>.<lang>.<ext>` file whose name promises a language it does not
/// contain is worse than no file at all.
pub async fn translate_and_write(
    output: &Path,
    formats: &[Format],
    transplan: &TranslationPlan,
    segments: &[subtitle::Segment],
    source: Option<&str>,
    paths: &crate::paths::AppPaths,
    on_progress: impl Fn(usize, usize) + Send + Sync,
) -> StageOutcome {
    let (translated, outcome) = translate::translate_segments(
        segments,
        source,
        &transplan.target,
        &transplan.model,
        &transplan.model_path,
        paths,
        on_progress,
    )
    .await;

    if let Some(reason) = outcome.skipped {
        // Nothing was sent, so nothing is written: an "already in the target
        // language" file would be a misleading duplicate of the original.
        tracing::info!(
            reason = ?reason,
            source = ?source,
            target = %transplan.target,
            "translation skipped"
        );
        return StageOutcome {
            skipped: Some(reason),
            ..StageOutcome::default()
        };
    }

    if outcome.degraded() {
        warn_degraded(transplan, &outcome);
    }

    if outcome.translated == 0 {
        // The originals are already delivered; the warn above is the report.
        tracing::warn!(
            model = %transplan.model_id,
            "translation produced nothing; no translated file written"
        );
        return StageOutcome {
            skipped: None,
            written: Vec::new(),
            degraded: outcome.degraded(),
            translated: outcome.translated,
            kept_original: outcome.kept_original,
        };
    }

    let written = write_translated_formats(output, formats, &transplan.target, &translated);
    for path in &written {
        tracing::info!(
            path = %path.display(),
            translated = outcome.translated,
            kept_original = outcome.kept_original,
            "Translated subtitle written"
        );
    }

    StageOutcome {
        skipped: None,
        written,
        degraded: outcome.degraded(),
        translated: outcome.translated,
        kept_original: outcome.kept_original,
    }
}

fn warn_degraded(transplan: &TranslationPlan, outcome: &translate::TranslationOutcome) {
    let status = outcome
        .last_error
        .as_ref()
        .map(TranslateError::status)
        .unwrap_or(translate::TRANSLATION_FAILED_STATUS);
    tracing::warn!(
        model = %transplan.model_id,
        status = %status,
        error = ?outcome.last_error,
        translated = outcome.translated,
        kept_original = outcome.kept_original,
        "translation degraded; those cues keep their original language"
    );
}

/// Write one translated file per requested format next to the original.
///
/// Split out so the naming, the per-format loop and the write itself are
/// testable without a Tauri handle or a model. The caller has already
/// established that there is something worth writing.
pub fn write_translated_formats(
    output: &Path,
    formats: &[Format],
    target_language: &str,
    segments: &[subtitle::Segment],
) -> Vec<PathBuf> {
    let mut written = Vec::new();

    for (index, format) in formats.iter().enumerate() {
        let Some(path) = translation_output_path(output, index, *format, target_language) else {
            continue;
        };
        match subtitle::write(segments, &path, *format) {
            Ok(()) => written.push(path),
            Err(error) => tracing::warn!(
                path = %path.display(),
                error = %error,
                "could not write a translated subtitle file"
            ),
        }
    }

    written
}

/// `<stem>.<lang>.<ext>` next to the original (translation-design §7).
///
/// The first format keeps the original's exact extension (like
/// `subtitle::output_path_for`), later ones replace it. `None` when a path has
/// no file name at all, in which case there is nowhere to put a translation.
pub fn translation_output_path(
    output_path: &Path,
    index: usize,
    format: Format,
    target_language: &str,
) -> Option<PathBuf> {
    let stem = output_path.file_stem()?.to_string_lossy().into_owned();
    let extension = if index == 0 {
        output_path
            .extension()
            .map(|extension| extension.to_string_lossy().into_owned())
            .unwrap_or_else(|| format.extension().to_string())
    } else {
        format.extension().to_string()
    };
    let name = format!(
        "{}.{}.{}",
        stem,
        translate::language::file_tag(target_language),
        extension
    );

    Some(match output_path.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent.join(name),
        _ => PathBuf::from(name),
    })
}

/// Every translated path a run may write, derived from `output` + formats.
///
/// Used by the URL-import path to answer "did anything land on disk?" without
/// re-deriving the naming rule, and by the transcription cancel cleanup so a
/// cancelled run cannot leave a translated file behind (T4-6).
pub fn translated_paths(output_path: &Path, formats: &[Format], target: &str) -> Vec<PathBuf> {
    formats
        .iter()
        .enumerate()
        .filter_map(|(index, format)| translation_output_path(output_path, index, *format, target))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("monosub-stage-{}-{}", tag, uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn catalogue() -> Vec<ModelSpec> {
        let path = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/resources/models.json"));
        engine::catalog::load_catalog(path).expect("committed models.json")
    }

    fn cue(text: &str) -> subtitle::Segment {
        subtitle::Segment {
            start: 0.0,
            end: 1.0,
            text: text.to_string(),
            speaker: Some("Speaker A".to_string()),
        }
    }

    #[test]
    fn write_translated_formats_names_files_after_the_target_tag() {
        let dir = scratch_dir("write");
        let output = dir.join("clip.srt");
        let formats = [Format::Srt, Format::Vtt, Format::Json];

        let written = write_translated_formats(&output, &formats, "en", &[cue("<translated>")]);

        assert_eq!(
            written,
            vec![
                dir.join("clip.en.srt"),
                dir.join("clip.en.vtt"),
                dir.join("clip.en.json"),
            ]
        );
        assert!(written.iter().all(|path| path.is_file()));
        let english = std::fs::read_to_string(dir.join("clip.en.srt")).unwrap();
        assert!(english.contains("<translated>"), "{english}");
        assert!(
            english.contains("[Speaker A]: "),
            "the speaker label must survive translation: {english}"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// `zh-tw` names the file `zh-Hant`, which is the whole point of treating
    /// traditional Chinese as a distinct target (design §9).
    #[test]
    fn translated_file_name_uses_the_canonical_tag() {
        let dir = scratch_dir("tag");
        let output = dir.join("clip.srt");
        let written = write_translated_formats(&output, &[Format::Srt], "zh-tw", &[]);

        assert_eq!(written, vec![dir.join("clip.zh-Hant.srt")]);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn translation_output_path_mirrors_output_path_for() {
        let output = Path::new("/media/clip.srt");
        assert_eq!(
            translation_output_path(output, 0, Format::Vtt, "en"),
            Some(PathBuf::from("/media/clip.en.srt")),
            "the first format keeps the original extension"
        );
        assert_eq!(
            translation_output_path(output, 1, Format::Vtt, "en"),
            Some(PathBuf::from("/media/clip.en.vtt"))
        );
        // A path with no file name has nowhere to put a translation.
        assert_eq!(translation_output_path(Path::new("/"), 0, Format::Srt, "en"), None);
    }

    #[test]
    fn translated_paths_match_the_write_loop() {
        let output = Path::new("/media/clip.srt");
        let formats = [Format::Srt, Format::Txt];
        assert_eq!(
            translated_paths(output, &formats, "de"),
            vec![
                PathBuf::from("/media/clip.de.srt"),
                PathBuf::from("/media/clip.de.txt"),
            ]
        );
    }

    #[test]
    fn skip_reason_is_total() {
        assert_eq!(skip_reason("zh", None, Some("en")), Some(SkipReason::Disabled));
        assert_eq!(skip_reason("zh", Some(false), Some("en")), Some(SkipReason::Disabled));
        assert_eq!(skip_reason("zh", Some(true), Some("zh")), Some(SkipReason::SameLanguage));
        assert_eq!(skip_reason("zh", Some(true), Some("ZH")), Some(SkipReason::SameLanguage));
        assert_eq!(
            skip_reason("zh-tw", Some(true), Some("zh-Hant")),
            Some(SkipReason::SameLanguage)
        );
        assert_eq!(skip_reason("zh", Some(true), None), Some(SkipReason::NoTarget));
        assert_eq!(skip_reason("zh", Some(true), Some("auto")), Some(SkipReason::NoTarget));
        assert_eq!(skip_reason("zh", Some(true), Some("  ")), Some(SkipReason::NoTarget));
        // `auto` as a source is unknown, so the run proceeds.
        assert_eq!(skip_reason("auto", Some(true), Some("en")), None);
        assert_eq!(skip_reason("zh", Some(true), Some("en")), None);
        // zh vs zh-tw is a real translation in both directions (design §9).
        assert_eq!(skip_reason("zh", Some(true), Some("zh-tw")), None);
        assert!(will_translate("zh", Some(true), Some("en")));
        assert!(!will_translate("zh", Some(true), Some("zh")));
    }

    #[test]
    fn gate_requires_an_installed_translation_model() {
        let catalogue = catalogue();
        let default_id = engine::catalog::default_translation_model_id(&catalogue).unwrap();
        let file = |id: &str| Some(PathBuf::from(format!("/models/{}.gguf", id)));

        let plan = resolve(&catalogue, "zh", Some(true), Some("en"), None, &|id| id == default_id, &file)
            .expect("installed model passes the pre-flight")
            .expect("translation is planned");
        assert_eq!(plan.model_id, default_id);
        assert_eq!(plan.target, "en");
        assert_eq!(plan.model.prompt_template.as_deref(), Some("hy_mt"));

        let status = resolve(&catalogue, "zh", Some(true), Some("en"), None, &|_| false, &file).unwrap_err();
        assert_eq!(status, format!("{}:{}", MODEL_NOT_INSTALLED, default_id));

        let status = resolve(&catalogue, "zh", Some(true), Some("en"), None, &|_| true, &|_| None).unwrap_err();
        assert_eq!(status, format!("{}:{}", MODEL_NOT_INSTALLED, default_id));

        let status = resolve(&catalogue, "zh", Some(true), Some("en"), Some("no_such"), &|_| true, &file)
            .unwrap_err();
        assert_eq!(status, format!("{}:no_such", MODEL_NOT_INSTALLED));

        // A recognition model must be refused: it runs on audio.cpp.
        let status = resolve(
            &catalogue,
            "zh",
            Some(true),
            Some("en"),
            Some("sensevoice_small_q8"),
            &|_| true,
            &file,
        )
        .unwrap_err();
        assert_eq!(status, format!("{}:sensevoice_small_q8", MODEL_NOT_INSTALLED));
    }

    #[test]
    fn gate_never_reaches_the_model_for_a_skipped_run() {
        let catalogue = catalogue();
        let nothing_installed = |_: &str| false;
        let no_path = |_: &str| None;

        for (enabled, source, target) in [
            (None, "zh", Some("en")),
            (Some(false), "zh", Some("en")),
            (Some(true), "zh", Some("zh")),
            (Some(true), "zh", None),
        ] {
            let result = resolve(&catalogue, source, enabled, target, None, &nothing_installed, &no_path)
                .expect("a skipped run is not an error");
            assert!(result.is_none(), "{source} -> {target:?} should skip");
        }
    }

    #[test]
    fn catalogue_without_a_translation_model_reports_the_bare_sentinel() {
        let recognition_only: Vec<ModelSpec> = catalogue()
            .into_iter()
            .filter(|spec| !spec.has_role(Role::Translate))
            .collect();

        let status = resolve(
            &recognition_only,
            "zh",
            Some(true),
            Some("en"),
            None,
            &|_| true,
            &|_| Some(PathBuf::from("/models/x.gguf")),
        )
        .unwrap_err();
        assert_eq!(status, MODEL_NOT_INSTALLED);
    }

    #[test]
    fn canonical_pair_check_distinguishes_the_two_chinese_scripts() {
        assert!(canonical_source_equals_target("zh", "zh"));
        assert!(canonical_source_equals_target("zh-tw", "zh-Hant"));
        assert!(canonical_source_equals_target("zh_Hant", "zh-tw"));
        assert!(!canonical_source_equals_target("zh", "zh-tw"));
        assert!(!canonical_source_equals_target("auto", "auto"));
        assert!(!canonical_source_equals_target("", "auto"));
    }
}
