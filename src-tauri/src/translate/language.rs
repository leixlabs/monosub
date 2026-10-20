//! Language normalisation: which tag identifies a translation target, and
//! whether a run needs translating at all.
//!
//! Task: T3-5. See docs/tech-spec/translation-design.md §5 (when to skip) and §9
//! (`zh` and `zh-tw` are different targets). Pure functions: no I/O, no model.
//!
//! Two tag vocabularies meet here. The app spells languages the way
//! `src/store/useStore.ts` does (`zh`, `zh-tw`, `en`, …); the translation model
//! card spells its 38 supported languages its own way (`zh`, `zh-Hant`, `en`,
//! …). [`canonical_tag`] is the single place the two are reconciled.

/// The tag the model uses for traditional Chinese. Reached from the app's
/// `zh-tw`/`zh-hk` and from the model's own `zh-Hant` — and it must NOT collapse
/// onto `zh`: the model converts between the scripts, so they are different
/// targets (design §9).
pub const TRADITIONAL_CHINESE: &str = "zh-Hant";

/// Simplify a language tag for comparison and for the prompt.
///
/// Lower-cases, normalises `_` to `-`, and maps every traditional-Chinese
/// spelling onto [`TRADITIONAL_CHINESE`]. Everything else is passed through
/// lower-cased, so an unknown tag still compares equal to itself.
pub fn canonical_tag(language: &str) -> String {
    let normalized = language.trim().to_lowercase().replace('_', "-");

    match normalized.as_str() {
        "zh-tw" | "zh-hk" | "zh-hant" => TRADITIONAL_CHINESE.to_string(),
        other => other.to_string(),
    }
}

/// True when `language` carries no actual language information: an empty tag or
/// the app's `auto`. This is the branch design §5 calls "unknown source".
pub fn is_unknown_language(language: &str) -> bool {
    let canonical = canonical_tag(language);
    canonical.is_empty() || canonical == "auto"
}

/// Whether this run actually needs translating.
///
/// * Source unknown (`None`, `auto`, empty) → **translate anyway**. The design is
///   explicit (§5): a same-language input mostly comes straight back, so the
///   cost of asking is only time, while skipping a needed translation is a
///   silent wrong result.
/// * Source == target → skip.
/// * Any other pair → translate.
///
/// A missing/`auto` *target* means there is nothing to translate into, so the
/// run is skipped rather than asked for an undefined language.
pub fn should_translate(source: Option<&str>, target: &str) -> bool {
    if is_unknown_language(target) {
        return false;
    }
    match source {
        None => true,
        Some(source) if is_unknown_language(source) => true,
        Some(source) => canonical_tag(source) != canonical_tag(target),
    }
}

/// The target language tag as it goes into a file name.
///
/// `<stem>.<tag>.<ext>` (design §7), so the tag must be path-safe on every
/// platform we ship: everything but ASCII letters, digits and `-` becomes `-`,
/// and an empty result falls back to `translated` rather than producing a file
/// called `clip..srt`.
pub fn file_tag(language: &str) -> String {
    let sanitized: String = canonical_tag(language)
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' {
                ch
            } else {
                '-'
            }
        })
        .collect();

    if sanitized.is_empty() {
        "translated".to_string()
    } else {
        sanitized
    }
}

/// True when `language` is a Chinese tag (`zh` or `zh-Hant`).
///
/// This is the model card's own split: the `ZH ⇄ XX` template is used whenever
/// the *source* is Chinese, and `XX ⇄ XX` otherwise. It is a property of the
/// language pair, not of a model family, so it lives here rather than in the
/// prompt builder.
pub fn is_chinese(language: &str) -> bool {
    canonical_tag(language).starts_with("zh")
}

/// The model card's name for each of its 38 supported languages, in card order.
const LANGUAGE_NAMES: &[(&str, &str)] = &[
    ("zh", "Chinese (Simplified)"),
    ("en", "English"),
    ("fr", "French"),
    ("pt", "Portuguese"),
    ("es", "Spanish"),
    ("ja", "Japanese"),
    ("tr", "Turkish"),
    ("ru", "Russian"),
    ("ar", "Arabic"),
    ("ko", "Korean"),
    ("th", "Thai"),
    ("it", "Italian"),
    ("de", "German"),
    ("vi", "Vietnamese"),
    ("ms", "Malay"),
    ("id", "Indonesian"),
    ("tl", "Filipino"),
    ("hi", "Hindi"),
    ("zh-hant", "Chinese (Traditional)"),
    ("pl", "Polish"),
    ("cs", "Czech"),
    ("nl", "Dutch"),
    ("km", "Khmer"),
    ("my", "Burmese"),
    ("fa", "Persian"),
    ("gu", "Gujarati"),
    ("ur", "Urdu"),
    ("te", "Telugu"),
    ("mr", "Marathi"),
    ("he", "Hebrew"),
    ("bn", "Bengali"),
    ("ta", "Tamil"),
    ("uk", "Ukrainian"),
    ("bo", "Tibetan"),
    ("kk", "Kazakh"),
    ("mn", "Mongolian"),
    ("ug", "Uyghur"),
    ("yue", "Cantonese"),
    // `alma` trains on English↔Icelandic among its 10 directions; the survey
    // catalogue lists it, so the prompt needs a name for it.
    ("is", "Icelandic"),
    // Greek: Cohere Transcribe and Aya list it.
    ("el", "Greek"),
    // Romanian: GigaAM / Parakeet / Cohere all cover it.
    ("ro", "Romanian"),
];

/// Human-readable name for a tag, falling back to the tag itself.
///
/// An unknown tag must still produce a usable prompt, so the fallback is the tag
/// rather than an error or an empty string.
pub fn display_name(language: &str) -> String {
    let canonical = canonical_tag(language);
    LANGUAGE_NAMES
        .iter()
        .find(|(tag, _)| *tag == canonical.to_lowercase())
        .map(|(_, name)| name.to_string())
        .unwrap_or(canonical)
}

/// Chinese display names, used when the prompt itself is in Chinese.
///
/// A Chinese-source single-segment instruction naming the target in English
/// ("翻译为English") makes the model answer IN CHINESE — it reads the English
/// word as content rather than as the target language. Naming the target in the
/// source's own language ("翻译为英语") is what makes it translate. Measured on
/// HY-MT1.5-1.8B: the English form returned Chinese for every probe, while this
/// table's form returned the correct target language for every probe.
///
/// The batch contract does not need this: its longer instruction body is enough
/// context for the English name. Only the bare single-cue prompt is sensitive.
const CHINESE_NAMES: &[(&str, &str)] = &[
    ("zh", "简体中文"),
    ("zh-hant", "繁体中文"),
    ("en", "英语"),
    ("fr", "法语"),
    ("pt", "葡萄牙语"),
    ("es", "西班牙语"),
    ("ja", "日语"),
    ("tr", "土耳其语"),
    ("ru", "俄语"),
    ("ar", "阿拉伯语"),
    ("ko", "韩语"),
    ("th", "泰语"),
    ("it", "意大利语"),
    ("de", "德语"),
    ("vi", "越南语"),
    ("ms", "马来语"),
    ("id", "印尼语"),
    ("tl", "菲律宾语"),
    ("hi", "印地语"),
    ("pl", "波兰语"),
    ("cs", "捷克语"),
    ("nl", "荷兰语"),
    ("km", "高棉语"),
    ("my", "缅甸语"),
    ("fa", "波斯语"),
    ("gu", "古吉拉特语"),
    ("ur", "乌尔都语"),
    ("te", "泰卢固语"),
    ("mr", "马拉地语"),
    ("he", "希伯来语"),
    ("bn", "孟加拉语"),
    ("ta", "泰米尔语"),
    ("uk", "乌克兰语"),
    ("bo", "藏语"),
    ("kk", "哈萨克语"),
    ("mn", "蒙古语"),
    ("ug", "维吾尔语"),
    ("yue", "粤语"),
    ("is", "冰岛语"),
    ("el", "希腊语"),
    ("ro", "罗马尼亚语"),
];

/// The target's name in Chinese, falling back to the English name for a tag this
/// table does not know, and never returning an empty string.
pub fn chinese_name(language: &str) -> String {
    let canonical = canonical_tag(language);
    CHINESE_NAMES
        .iter()
        .find(|(tag, _)| *tag == canonical.to_lowercase())
        .map(|(_, name)| name.to_string())
        .unwrap_or_else(|| display_name(language))
}

/// The target as it goes into a prompt: the display name AND the tag.
///
/// Both are included because the model card's templates place a language *name*
/// where the model expects one, while the tag pins the script (that is what
/// makes `zh` and `zh-Hant` distinguishable at all).
pub fn language_label(language: &str) -> String {
    let name = display_name(language);
    let tag = canonical_tag(language);
    if name == tag {
        tag
    } else {
        format!("{} ({})", name, tag)
    }
}

/// Last-resort source detection for an `auto` request that produced no ASR tag
/// (translation-design §5: use simple script statistics when the tag is absent,
/// and still translate when even that is inconclusive).
///
/// This is a script census, not language identification, so it only answers for
/// scripts that are unambiguous on their own:
///
/// * kana present ⇒ Japanese. A kana-free kanji-only page is indistinguishable
///   from Chinese, so it falls through to the ideograph rule.
/// * hangul ⇒ Korean, checked before the ideograph count because Korean text can
///   carry hanja.
/// * a CJK-ideograph share at least as large as the alphabetic one ⇒ Chinese.
/// * everything else, including all Latin text (`en`/`es`/`de`/`fr` cannot be
///   told apart) ⇒ `None`, which means "unknown" and therefore "translate
///   anyway".
///
/// `None` is always the safe answer: it costs a wasted run on a same-language
/// input, whereas a wrong guess would silently drop a needed translation.
pub fn detect_by_script<'a>(texts: impl IntoIterator<Item = &'a str>) -> Option<String> {
    let mut cjk = 0usize;
    let mut kana = 0usize;
    let mut hangul = 0usize;
    let mut latin = 0usize;

    for text in texts {
        for ch in text.chars() {
            match ch as u32 {
                // Hiragana + katakana, including the phonetic extensions.
                0x3040..=0x30FF | 0x31F0..=0x31FF => kana += 1,
                // Hangul syllables, jamo and compatibility jamo.
                0x1100..=0x11FF | 0x3130..=0x318F | 0xAC00..=0xD7A3 => hangul += 1,
                // CJK unified ideographs and their extensions.
                0x4E00..=0x9FFF | 0x3400..=0x4DBF | 0xF900..=0xFAFF => cjk += 1,
                _ if ch.is_alphabetic() => latin += 1,
                _ => {}
            }
        }
    }

    if kana > 0 {
        return Some("ja".to_string());
    }
    if hangul > 0 {
        return Some("ko".to_string());
    }
    if cjk > 0 && cjk >= latin {
        return Some("zh".to_string());
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_tag_folds_traditional_chinese_spellings() {
        for spelling in ["zh-tw", "zh-TW", "zh_hk", "zh-Hant", " zh-hant "] {
            assert_eq!(
                canonical_tag(spelling),
                TRADITIONAL_CHINESE,
                "canonical_tag({spelling:?})"
            );
        }
        assert_eq!(canonical_tag("zh"), "zh");
        assert_eq!(canonical_tag("EN"), "en");
        assert_eq!(canonical_tag(""), "");
        // An unknown tag is stable, not an error.
        assert_eq!(canonical_tag("xx-YY"), "xx-yy");
    }

    /// `zh` and `zh-tw` are different targets in BOTH directions: the model
    /// converts between the scripts, so neither may be treated as a no-op
    /// (design §9).
    #[test]
    fn zh_and_zh_tw_are_different_targets_both_ways() {
        assert!(should_translate(Some("zh"), "zh-tw"));
        assert!(should_translate(Some("zh-tw"), "zh"));
        assert!(should_translate(Some("zh"), "zh-Hant"));
        assert!(should_translate(Some("zh-hk"), "zh"));
        // Same script, different spelling: skip.
        assert!(!should_translate(Some("zh-tw"), "zh-Hant"));
        assert!(!should_translate(Some("zh-Hant"), "zh-tw"));
    }

    /// The `auto` fallback: unambiguous scripts only, everything else `None`
    /// (which downstream means "translate anyway").
    #[test]
    fn script_detection_answers_only_when_the_script_is_unambiguous() {
        let detect = |texts: &[&str]| detect_by_script(texts.iter().copied());

        assert_eq!(detect(&["开放时间早上九点"]).as_deref(), Some("zh"));
        assert_eq!(detect(&["你好", "世界"]).as_deref(), Some("zh"));
        // Japanese mixes kana with kanji; kana decides.
        assert_eq!(detect(&["こんにちは世界"]).as_deref(), Some("ja"));
        // Korean: hangul is checked before the ideograph count too.
        assert_eq!(detect(&["안녕하세요"]).as_deref(), Some("ko"));
        // Latin, Cyrillic and Arabic text are mutually indistinguishable here,
        // so they stay unknown rather than being guessed.
        assert_eq!(detect(&["Hello there"]), None);
        assert_eq!(detect(&["Hola, ¿qué tal?"]), None);
        assert_eq!(detect(&["Привет"]), None);
        assert_eq!(detect(&["مرحبا"]), None);
        assert_eq!(detect(&[]), None);
        assert_eq!(detect(&["", "   ", "12345"]), None);
        // A Chinese cue with a few latin product names is still Chinese.
        assert_eq!(
            detect(&["我们用 Rust 重写了整个引擎"]).as_deref(),
            Some("zh")
        );
        // ...but latin-dominant text is not, even with one CJK character in it.
        assert_eq!(detect(&["we rewrote the engine in rust 我"]), None);
        // A tie counts as Chinese: the rule is `cjk >= latin`.
        assert_eq!(detect(&["a 我"]).as_deref(), Some("zh"));
        assert_eq!(detect(&["a b 我"]), None);
    }

    /// The file-name tag is path-safe and never empty.
    #[test]
    fn file_tag_is_path_safe() {
        assert_eq!(file_tag("en"), "en");
        assert_eq!(file_tag("zh-tw"), "zh-Hant");
        assert_eq!(file_tag("ZH_Hant"), "zh-Hant");
        assert_eq!(file_tag("pt-BR"), "pt-br");
        assert_eq!(file_tag(""), "translated");
        assert_eq!(file_tag("  "), "translated");
        // Nothing can produce a path separator or a dot.
        for tag in ["a/b", "a\\b", "a.b", "a b"] {
            let sanitized = file_tag(tag);
            assert!(!sanitized.contains('/'), "{sanitized}");
            assert!(!sanitized.contains('\\'), "{sanitized}");
            assert!(!sanitized.contains('.'), "{sanitized}");
            assert!(!sanitized.contains(' '), "{sanitized}");
        }
    }

    #[test]
    fn identical_languages_are_skipped() {
        assert!(!should_translate(Some("zh"), "zh"));
        assert!(!should_translate(Some("zh"), "ZH"));
        assert!(!should_translate(Some("en"), "en"));
        assert!(!should_translate(Some("ja"), "ja"));
    }

    /// `auto` as a source means "unknown": never skip. The design accepts the
    /// wasted run because a same-language input mostly comes straight back (§5).
    #[test]
    fn unknown_source_always_translates() {
        assert!(should_translate(None, "en"));
        assert!(should_translate(Some("auto"), "en"));
        assert!(should_translate(Some("AUTO"), "zh"));
        assert!(should_translate(Some(""), "zh"));
    }

    /// A missing target is not a language: there is nothing to translate into.
    #[test]
    fn missing_or_auto_target_skips() {
        assert!(!should_translate(Some("zh"), ""));
        assert!(!should_translate(Some("zh"), "auto"));
        assert!(!should_translate(None, "auto"));
        assert!(!should_translate(Some("zh"), "  "));
    }

    #[test]
    fn chinese_source_picks_the_chinese_template() {
        assert!(is_chinese("zh"));
        assert!(is_chinese("zh-tw"));
        assert!(is_chinese("zh-Hant"));
        assert!(!is_chinese("en"));
        assert!(!is_chinese("yue"));
        assert!(!is_chinese(""));
        assert!(!is_chinese("auto"));
    }

    /// Every language in `models.json`'s translate entry needs a label, or the
    /// prompt would silently say "de" instead of "German".
    #[test]
    fn every_committed_translate_language_has_a_display_name() {
        let path = std::path::Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/resources/models.json"
        ));
        let specs = crate::engine::catalog::load_catalog(path).expect("committed models.json");
        let translate = crate::engine::catalog::translation_models(&specs);
        assert!(!translate.is_empty(), "no translate entry in the catalogue");

        for spec in translate {
            for language in &spec.languages {
                let name = display_name(language);
                assert_ne!(
                    name,
                    canonical_tag(language),
                    "models.json language {language:?} has no display name"
                );
                // ...and the label carries both spellings.
                let label = language_label(language);
                assert!(label.contains(&name), "{label}");
            }
        }

        assert_eq!(display_name("en"), "English");
        assert_eq!(language_label("en"), "English (en)");
        assert_eq!(display_name("zh-tw"), "Chinese (Traditional)");
        assert_eq!(language_label("zh-tw"), "Chinese (Traditional) (zh-Hant)");
        // Unknown tags degrade to the tag itself rather than failing.
        assert_eq!(display_name("xx"), "xx");
        assert_eq!(language_label("xx"), "xx");
    }
}
