//! Translation failures and how they surface.
//!
//! Task: T3-1. See docs/tech-spec/translation-design.md §2 (translation is an
//! enhancement, never a task failure) and §8 (cancellation reclaims the child).
//!
//! This is deliberately a separate enum from `engine::EngineError`: the two
//! engines fail for different reasons and the remediation differs (reinstall the
//! bundled binary vs. download a model vs. retry), and translation must never be
//! reported through `transcribe_error`.

/// Short, user-showable status for a translation that did not fully succeed.
///
/// A plain sentence rather than an i18n key on purpose: the frontend resolves
/// `errors.<status>` when such a key exists and otherwise shows the string
/// verbatim (see `engine_failure` in `commands/transcribe.rs`).
pub const TRANSLATION_FAILED_STATUS: &str = "Translation failed";

/// A failure inside the translation stage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TranslateError {
    /// The bundled `llama-server` is missing (translation-design §3.1).
    BinaryMissing(String),
    /// The chosen translation model is not on disk.
    ModelMissing(String),
    /// The server never became healthy, or died during startup.
    ServerStart(String),
    /// The HTTP request failed, or the server answered with an error status.
    Request(String),
    /// The response was not the documented chat-completion shape.
    Parse(String),
    /// The task was cancelled, or the server is gone and cannot be asked
    /// anything more. The batch loop stops on it.
    Cancelled,
}

impl TranslateError {
    /// Short status, mapped the way `engine_failure` maps [`EngineError`].
    ///
    /// Only three outcomes matter to a user: the translation engine is missing,
    /// the model is missing, or the translation failed. Everything else is a
    /// detail for the log — which matters because translation failures never
    /// reach `transcribe_error` at all (design §2).
    ///
    /// [`EngineError`]: crate::engine::EngineError
    pub fn status(&self) -> &'static str {
        match self {
            TranslateError::BinaryMissing(_) => "engineMissing",
            TranslateError::ModelMissing(_) => "modelNotInstalled",
            _ => TRANSLATION_FAILED_STATUS,
        }
    }
}

impl std::fmt::Display for TranslateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TranslateError::BinaryMissing(m) => write!(f, "llama-server missing: {}", m),
            TranslateError::ModelMissing(m) => write!(f, "translation model missing: {}", m),
            TranslateError::ServerStart(m) => write!(f, "llama-server failed to start: {}", m),
            TranslateError::Request(m) => write!(f, "translation request failed: {}", m),
            TranslateError::Parse(m) => write!(f, "translation response unparsable: {}", m),
            TranslateError::Cancelled => write!(f, "translation cancelled"),
        }
    }
}

impl std::error::Error for TranslateError {}

#[cfg(test)]
mod tests {
    use super::*;

    /// The two statuses that correspond to an existing locale key must be the
    /// exact keys the UI resolves; everything else is a plain sentence the UI
    /// renders verbatim, so a new variant can never silently render `undefined`.
    #[test]
    fn statuses_are_keys_or_sentences() {
        assert_eq!(
            TranslateError::BinaryMissing("nope".into()).status(),
            "engineMissing"
        );
        assert_eq!(
            TranslateError::ModelMissing("nope".into()).status(),
            "modelNotInstalled"
        );
        for error in [
            TranslateError::ServerStart("deadline".into()),
            TranslateError::Request("http 500".into()),
            TranslateError::Parse("no choices".into()),
            TranslateError::Cancelled,
        ] {
            assert_eq!(error.status(), TRANSLATION_FAILED_STATUS);
            // Sentence, not an i18n key: the frontend would resolve `errors.<x>`.
            assert!(error.status().contains(' '), "{}", error.status());
        }
    }

    /// The statuses must stay in sync with the locales, exactly like the engine
    /// ones — a rename here would make every locale entry dead again.
    #[test]
    fn translation_statuses_exist_in_the_locales() {
        let locales = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("src/i18n/locales");

        for entry in std::fs::read_dir(&locales).expect("read locales") {
            let path = entry.expect("entry").path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            let body = std::fs::read_to_string(&path).expect("read locale");
            let json: serde_json::Value =
                serde_json::from_str(&body).unwrap_or_else(|e| panic!("{}: {}", path.display(), e));

            for key in ["engineMissing", "modelNotInstalled"] {
                assert!(
                    json.get("errors")
                        .and_then(|errors| errors.get(key))
                        .is_some(),
                    "{} is missing errors.{}",
                    path.display(),
                    key
                );
            }
        }
    }
}
