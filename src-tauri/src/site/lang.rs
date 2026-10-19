//! `select`: which single subtitle track the current setting language wants.
//!
//! Task: U1-7. See docs/tech-spec/url-import-design.md §4.2.
//!
//! The rules here are all product promises, not taste:
//!
//! * **Strict matching.** A `zh-Hans` track never stands in for `zh-tw` and an
//!   `ai-en` machine-translated track never stands in for `en`: taking the wrong
//!   one silently delivers a subtitle in the wrong language, which the user has
//!   no way to notice before reading it.
//! * **Human over AI** within one language.
//! * **Only the original-language AI track** (`ai-zh`) is accepted; the other
//!   `ai-*` tracks are machine translations *of* `ai-zh` and are therefore in
//!   the wrong language for anyone asking for them (U0-10 confirms this).
//! * **`auto` selects nothing.** "Let the engine decide" cannot answer "which
//!   language do you want the platform subtitle in", and guessing would pick a
//!   machine-translated track as often as not.
//! * Exactly one result, so the download step has one tag to pass to
//!   `--sub-langs`.

/// The app's setting language. Defined here rather than imported from the
/// frontend contract because the Rust side only ever sees the string form; this
/// enum exists so `select` can be an exhaustive `match` instead of a string
/// whitelist that a new language would silently fall through.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputLanguage {
    Auto,
    Zh,
    ZhTw,
    En,
    Ja,
    Ko,
    Es,
    De,
    Fr,
    Pt,
}

impl OutputLanguage {
    /// Parse the value the frontend sends (`settings.outputLanguage`).
    ///
    /// An unknown string is `Auto` — the conservative choice, because it falls
    /// back to transcription rather than to a subtitle in a language nobody asked
    /// for.
    pub fn parse(value: &str) -> OutputLanguage {
        match value.trim().to_ascii_lowercase().as_str() {
            "zh" => OutputLanguage::Zh,
            "zh-tw" | "zh-hant" | "zh-hk" => OutputLanguage::ZhTw,
            "en" => OutputLanguage::En,
            "ja" => OutputLanguage::Ja,
            "ko" => OutputLanguage::Ko,
            "es" => OutputLanguage::Es,
            "de" => OutputLanguage::De,
            "fr" => OutputLanguage::Fr,
            "pt" => OutputLanguage::Pt,
            _ => OutputLanguage::Auto,
        }
    }
}

pub use crate::site::resolve::SiteMedia;

/// Whether a selected track is the platform's own CC or its ASR.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SubKind {
    Human,
    Ai,
}

impl SubKind {
    pub fn as_str(self) -> &'static str {
        match self {
            SubKind::Human => "human",
            SubKind::Ai => "ai",
        }
    }
}

/// The one track to fetch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pick {
    /// The platform's tag, passed to `--sub-langs`.
    pub tag: String,
    pub kind: SubKind,
    /// The language of the *text*, handed to the translation stage so
    /// `should_translate` can skip a same-language run exactly (design §5.6).
    pub source_lang: &'static str,
}

/// The tag that is never a subtitle: `danmaku` is the XML bullet-comment track.
const DANMAKU: &str = "danmaku";

/// Choose the one track `setting` asks for, or `None` to fall back to
/// transcription.
///
/// The candidate lists are explicit and the `match` is exhaustive with no `_`
/// arm: adding an `OutputLanguage` must be a compile error here, because the
/// alternative is silently transcribing a language the user asked to fetch.
pub fn select(setting: OutputLanguage, media: &SiteMedia) -> Option<Pick> {
    let candidates: &[(&str, SubKind, &'static str)] = match setting {
        // `auto` cannot answer "which language do you want the subtitle in"
        // (design §4.2 rule 4), so it always falls back to recognition.
        OutputLanguage::Auto => return None,
        OutputLanguage::Zh => &[
            // Human tracks first (rule 2), simplified only (rule 1).
            ("zh-Hans", SubKind::Human, "zh"),
            ("zh-CN", SubKind::Human, "zh"),
            ("zh", SubKind::Human, "zh"),
            // `ai-zh` is Bilibili's own ASR of the original audio, which is
            // in the original language — the one AI track that is acceptable.
            ("ai-zh", SubKind::Ai, "zh"),
        ],
        OutputLanguage::ZhTw => &[
            ("zh-Hant", SubKind::Human, "zh-tw"),
            ("zh-TW", SubKind::Human, "zh-tw"),
            ("zh-HK", SubKind::Human, "zh-tw"),
        ],
        OutputLanguage::En => &[
            ("en", SubKind::Human, "en"),
            ("en-US", SubKind::Human, "en"),
            ("en-GB", SubKind::Human, "en"),
        ],
        OutputLanguage::Ja => &[("ja", SubKind::Human, "ja"), ("ja-JP", SubKind::Human, "ja")],
        OutputLanguage::Ko => &[("ko", SubKind::Human, "ko"), ("ko-KR", SubKind::Human, "ko")],
        OutputLanguage::Es => &[
            ("es", SubKind::Human, "es"),
            ("es-ES", SubKind::Human, "es"),
            ("es-419", SubKind::Human, "es"),
        ],
        OutputLanguage::De => &[("de", SubKind::Human, "de"), ("de-DE", SubKind::Human, "de")],
        OutputLanguage::Fr => &[("fr", SubKind::Human, "fr"), ("fr-FR", SubKind::Human, "fr")],
        OutputLanguage::Pt => &[
            ("pt", SubKind::Human, "pt"),
            ("pt-BR", SubKind::Human, "pt"),
            ("pt-PT", SubKind::Human, "pt"),
        ],
    };

    candidates
        .iter()
        .find(|(tag, kind, _)| accepts(media, tag, *kind))
        .map(|(tag, kind, source_lang)| Pick {
            tag: (*tag).to_string(),
            kind: *kind,
            source_lang,
        })
}

/// A candidate only counts when the track exists, is not the danmaku XML, and
/// is the kind the list says it is (an `ai-*` tag must not be picked as human).
fn accepts(media: &SiteMedia, tag: &str, kind: SubKind) -> bool {
    if tag == DANMAKU {
        return false;
    }
    let Some(track) = media.subs.get(tag) else {
        return false;
    };
    // `xml` is the danmaku container even when the tag is something else; a
    // subtitle we cannot parse is not a hit.
    if !is_subtitle_ext(&track.ext) {
        return false;
    }
    is_ai_tag(tag) == (kind == SubKind::Ai)
}

/// Extensions we can read as subtitle text. `best` never appears here; it is only
/// ever a `--sub-format` fallback.
fn is_subtitle_ext(ext: &str) -> bool {
    matches!(ext.trim().to_ascii_lowercase().as_str(), "srt" | "vtt")
}

/// Bilibili puts its ASR tracks under an `ai-` prefix (design §4.2).
pub fn is_ai_tag(tag: &str) -> bool {
    tag.trim().to_ascii_lowercase().starts_with("ai-")
}

/// The app language a platform tag names, if it names one unambiguously.
///
/// Only used for diagnostics and tests: `select` deliberately matches tags by
/// hand rather than by mapping them, because `zh-Hans` and `zh-Hant` must not
/// collapse into one another.
#[allow(dead_code)]
pub fn source_lang_for(tag: &str) -> Option<&'static str> {
    match tag.trim() {
        "zh" | "zh-CN" | "zh-Hans" | "ai-zh" => Some("zh"),
        "zh-Hant" | "zh-TW" | "zh-HK" => Some("zh-tw"),
        "en" | "en-US" | "en-GB" => Some("en"),
        "ja" | "ja-JP" => Some("ja"),
        "ko" | "ko-KR" => Some("ko"),
        "es" | "es-ES" | "es-419" => Some("es"),
        "de" | "de-DE" => Some("de"),
        "fr" | "fr-FR" => Some("fr"),
        "pt" | "pt-BR" | "pt-PT" => Some("pt"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::site::resolve::SubTrack;
    use std::collections::BTreeMap;

    fn media(tracks: &[(&str, &str)]) -> SiteMedia {
        let mut subs = BTreeMap::new();
        for (tag, ext) in tracks {
            subs.insert(
                (*tag).to_string(),
                SubTrack {
                    ext: (*ext).to_string(),
                    data: None,
                },
            );
        }
        SiteMedia {
            id: "BV1GJ411x7h7".to_string(),
            title: "t".to_string(),
            duration: Some(1.0),
            subs,
        }
    }

    #[test]
    fn auto_never_selects_a_track() {
        let media = media(&[("zh-CN", "srt"), ("en-US", "srt"), ("ai-zh", "srt")]);
        assert_eq!(select(OutputLanguage::Auto, &media), None);
    }

    #[test]
    fn simplified_and_traditional_never_fall_back_to_each_other() {
        // Only traditional on offer: `zh` must NOT take it.
        let traditional = media(&[("zh-Hant", "srt"), ("zh-HK", "srt")]);
        assert_eq!(select(OutputLanguage::Zh, &traditional), None, "zh must not take zh-Hant");
        assert_eq!(
            select(OutputLanguage::ZhTw, &traditional).map(|pick| pick.tag),
            Some("zh-Hant".to_string())
        );

        // Only simplified on offer: `zh-tw` must NOT take it.
        let simplified = media(&[("zh-CN", "srt"), ("zh-Hans", "srt")]);
        assert_eq!(select(OutputLanguage::ZhTw, &simplified), None, "zh-tw must not take zh-CN");
        assert_eq!(
            select(OutputLanguage::Zh, &simplified).map(|pick| pick.tag),
            Some("zh-Hans".to_string())
        );
    }

    #[test]
    fn ai_zh_is_accepted_for_chinese_and_other_ai_tracks_are_not() {
        let ai_only = media(&[("ai-zh", "srt")]);
        let pick = select(OutputLanguage::Zh, &ai_only).expect("ai-zh is an acceptable Chinese track");
        assert_eq!(pick.tag, "ai-zh");
        assert_eq!(pick.kind, SubKind::Ai);
        assert_eq!(pick.source_lang, "zh");

        // `ai-en` is a machine translation OF ai-zh, so it is not English.
        let ai_english = media(&[("ai-en", "srt")]);
        assert_eq!(select(OutputLanguage::En, &ai_english), None);

        // ...and the traditional setting must not take `ai-zh` either.
        assert_eq!(select(OutputLanguage::ZhTw, &ai_only), None);
    }

    #[test]
    fn human_tracks_win_over_ai_within_a_language() {
        let both = media(&[("ai-zh", "srt"), ("zh-CN", "srt")]);
        let pick = select(OutputLanguage::Zh, &both).unwrap();
        assert_eq!(pick.tag, "zh-CN");
        assert_eq!(pick.kind, SubKind::Human);
    }

    #[test]
    fn danmaku_is_never_selected() {
        // Even when a candidate list somehow mentioned it, the guard refuses.
        let danmaku = media(&[("danmaku", "xml")]);
        for setting in [
            OutputLanguage::Auto,
            OutputLanguage::Zh,
            OutputLanguage::ZhTw,
            OutputLanguage::En,
            OutputLanguage::Ja,
            OutputLanguage::Ko,
            OutputLanguage::Es,
            OutputLanguage::De,
            OutputLanguage::Fr,
            OutputLanguage::Pt,
        ] {
            assert_eq!(select(setting, &danmaku), None, "{setting:?}");
        }
        assert!(!is_subtitle_ext("xml"));
    }

    #[test]
    fn every_language_selects_its_own_tag_and_reports_the_source() {
        let cases = [
            ("zh-Hans", OutputLanguage::Zh, "zh"),
            ("zh-Hant", OutputLanguage::ZhTw, "zh-tw"),
            ("en-US", OutputLanguage::En, "en"),
            ("ja", OutputLanguage::Ja, "ja"),
            ("ko-KR", OutputLanguage::Ko, "ko"),
            ("es-419", OutputLanguage::Es, "es"),
            ("de-DE", OutputLanguage::De, "de"),
            ("fr", OutputLanguage::Fr, "fr"),
            ("pt-BR", OutputLanguage::Pt, "pt"),
        ];
        for (tag, setting, source_lang) in cases {
            let media = media(&[(tag, "srt")]);
            let pick = select(setting, &media)
                .unwrap_or_else(|| panic!("{tag} should satisfy {setting:?}"));
            assert_eq!(pick.tag, tag);
            assert_eq!(pick.kind, SubKind::Human);
            assert_eq!(pick.source_lang, source_lang);
        }
    }

    #[test]
    fn only_one_pick_is_ever_returned() {
        // A video with many Chinese-ish tags: exactly one comes back, the first
        // candidate the list names.
        let media = media(&[
            ("zh-Hant", "srt"),
            ("zh-HK", "srt"),
            ("zh-CN", "srt"),
            ("zh-Hans", "srt"),
            ("ai-zh", "srt"),
        ]);
        let pick = select(OutputLanguage::Zh, &media).unwrap();
        assert_eq!(pick.tag, "zh-Hans", "the documented order decides, not the map order");

        assert_eq!(select(OutputLanguage::ZhTw, &media).unwrap().tag, "zh-Hant");
    }

    #[test]
    fn a_track_we_cannot_parse_does_not_count_as_a_hit() {
        // Wrong container for the tag: fall through rather than "hit" and then
        // fail to parse, which would be a wasted network round trip.
        let media = media(&[("zh-CN", "json3"), ("zh-Hans", "srt")]);
        assert_eq!(select(OutputLanguage::Zh, &media).unwrap().tag, "zh-Hans");
    }

    #[test]
    fn empty_media_selects_nothing() {
        let empty = SiteMedia::default();
        for setting in [
            OutputLanguage::Zh,
            OutputLanguage::ZhTw,
            OutputLanguage::En,
            OutputLanguage::Ja,
            OutputLanguage::Ko,
            OutputLanguage::Es,
            OutputLanguage::De,
            OutputLanguage::Fr,
            OutputLanguage::Pt,
        ] {
            assert_eq!(select(setting, &empty), None, "{setting:?}");
        }
    }

    #[test]
    fn source_lang_for_keeps_the_two_chinese_scripts_apart() {
        assert_eq!(source_lang_for("zh-Hans"), Some("zh"));
        assert_eq!(source_lang_for("zh-Hant"), Some("zh-tw"));
        assert_eq!(source_lang_for("ai-zh"), Some("zh"));
        assert_eq!(source_lang_for("whatever"), None);
    }
}
