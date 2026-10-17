//! Batched translation prompts.
//!
//! Task: T3-2. See docs/tech-spec/translation-design.md §4 ("batch + numbering",
//! "the template follows the model, not Rust").
//!
//! Everything here is pure: the prompt wording is data, the template kind comes
//! from `models.json` (`prompt_template`), and the ZH ⇄ XX vs. XX ⇄ XX split is a
//! property of the language pair. Nothing in this file names HY-MT.

use super::language::{self, language_label};

/// Template kind used when `models.json` declares no `prompt_template`.
///
/// The English `XX ⇄ XX` instruction from the model card is the safest generic
/// wording; a model that wants something else says so via `prompt_template`.
pub const DEFAULT_TEMPLATE: &str = "generic";

/// The `prompt_template` kind the HY-MT model card documents.
pub const HY_MT_TEMPLATE: &str = "hy_mt";

/// Why this prompt does NOT prepend the model card's single-segment template.
///
/// The card documents two prompts: a per-segment one ("将以下文本翻译为X…") and a
/// contextual one. Stacking the per-segment template on top of the batched
/// numbering contract hands the model two competing instructions, and a 1.8B
/// model resolves that by dropping the numbering it was just given. Measured on
/// an M4 with HY-MT1.5-1.8B-Q4_K_M, zh→en, 40 requests:
///
/// | batch | card line + batch contract | this contract |
/// |-------|----------------------------|---------------|
/// | 1     | 0 numbered lines           | 4/4 exact     |
/// | 20    | 0 numbered lines, and the answer was the SOURCE text echoed untranslated | 3/3 exact |
///
/// The echoed-source variant is the dangerous one: it is non-empty, so a naive
/// check would accept it as a successful translation.
///
/// The contract below has three load-bearing parts: an explicit count, the count
/// sentence as its OWN PARAGRAPH (folding it into the instruction sentence made
/// the model stop numbering), and a two-line worked example of the shape.
///
/// The example is not cosmetic. Measured zh→en with the count but no example,
/// batches of 2–4 cues failed on every attempt (one prose line came back instead
/// of n numbered lines) and size 14 failed intermittently; with the example,
/// 27/27 requests across sizes 2–20 were exact. The example is written in the
/// target language, which was verified not to leak into other targets (ja/fr/ko/de
/// all translated correctly with the English example).
///
/// Numbered lines remain the only contract this model honours at all —
/// `response_format`/`json_schema` echoed the source untranslated or returned an
/// empty list, and a GBNF grammar returned `[]` (PoC §T0-4).
const BATCH_INSTRUCTION_ZH: &str = concat!(
    "将以下 {count} 条带编号的字幕逐条翻译为{target_language}，",
    "逐条对应，保持编号（1. 2. 3. …），每行一条，不要合并、不要漏条、不要额外解释：",
    // The blank line is load-bearing: the count sentence must be its own
    // paragraph. Written as `concat!` pieces so a Rust line continuation cannot
    // silently swallow it.
    "\n\n",
    "输出必须严格为 {count} 条。例如 2 条的输入会得到 2 行输出：\n1. First.\n2. Second.",
);

const BATCH_INSTRUCTION_EN: &str = concat!(
    "Translate the following {count} numbered subtitle lines into {target_language}. ",
    "Keep the numbering (1. 2. 3. …), one line per cue, in the same order. ",
    "Do not merge lines, do not omit lines, do not add explanations:",
    "\n\n",
    "The output must be exactly {count} lines. ",
    "For example, 2 input lines give 2 numbered output lines:\n1. First.\n2. Second.",
);

/// One numbered cue handed to the model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PromptCue {
    /// 1-based number, continuous across batches of the same file.
    pub index: usize,
    pub text: String,
}

/// Build the prompt for one batch.
///
/// `template` is the model's `prompt_template` (`None`/unknown falls back to
/// [`DEFAULT_TEMPLATE`]); `source` is the detected source language, `None` when
/// unknown — an unknown source cannot be Chinese, so the generic template is
/// used and the model still gets an explicit target.
///
/// The batch form is always used, even for a single cue: it is the shape the
/// verification in [`super::parse`] depends on, so the degraded one-cue retry
/// parses through exactly the same path.
pub fn batch_prompt(
    template: Option<&str>,
    source: Option<&str>,
    target: &str,
    cues: &[PromptCue],
) -> String {
    let chinese_source = source.is_some_and(language::is_chinese);

    match normalized_template(template) {
        HY_MT_TEMPLATE => hy_mt_batch_prompt(chinese_source, source, target, cues),
        _ => generic_batch_prompt(target, cues),
    }
}

/// Unknown or absent template names fall back rather than failing: a new
/// catalogue entry must not break translation because of a typo in
/// `prompt_template`.
fn normalized_template(template: Option<&str>) -> &str {
    match template.map(str::trim) {
        Some(HY_MT_TEMPLATE) => HY_MT_TEMPLATE,
        _ => DEFAULT_TEMPLATE,
    }
}

/// The HY-MT form: the card's translation instruction, then the measured
/// numbering contract, then the numbered lines.
fn hy_mt_batch_prompt(
    chinese_source: bool,
    source: Option<&str>,
    target: &str,
    cues: &[PromptCue],
) -> String {
    // The instruction names the target as "Name (tag)" — the display name is
    // what the card's wording expects, and the tag pins the script, which is
    // what keeps `zh` and `zh-Hant` distinguishable at all.
    let target_label = language_label(target);
    let numbered = number_lines(cues);

    if chinese_source && !cues.is_empty() {
        // `source` is only used for the log-free branch below; keeping the
        // parameter explicit documents that this template is source-driven.
        debug_assert!(source.is_some());
        format!(
            "{}\n\n{}",
            BATCH_INSTRUCTION_ZH
                .replace("{count}", &cues.len().to_string())
                .replace("{target_language}", &target_label),
            numbered
        )
    } else {
        format!(
            "{}\n\n{}",
            BATCH_INSTRUCTION_EN
                .replace("{count}", &cues.len().to_string())
                .replace("{target_language}", &target_label),
            numbered
        )
    }
}

/// A generic instruction for a model with no documented template.
///
/// Same wording as the HY-MT English branch — the numbering contract is the part
/// the parser depends on, so a model without a documented template still gets it.
fn generic_batch_prompt(target: &str, cues: &[PromptCue]) -> String {
    format!(
        "{}\n\n{}",
        BATCH_INSTRUCTION_EN
            .replace("{count}", &cues.len().to_string())
            .replace("{target_language}", &language_label(target)),
        number_lines(cues)
    )
}

/// The model card's SINGLE-SEGMENT templates, used only for a 1-cue window.
///
/// The batched contract above is unreliable for exactly one short cue: measured,
/// the model answered with a single unnumbered line or echoed the instruction.
/// The card's per-segment wording is the shape this model was tuned on. That
/// matters because a 1-cue window is the terminal rung of the retry ladder —
/// the last chance to avoid shipping an untranslated cue.
///
/// The target is named WITH A CHINESE NAME for a Chinese source (see
/// `chinese_name`), not the English label: measured, "翻译为English" produced a
/// CHINESE answer — the model read the English word as content rather than as
/// the target language. With "翻译为英语" all 30 probes translated correctly.
const SINGLE_INSTRUCTION_ZH: &str =
    "将以下文本翻译为{target_language}，注意只需要输出翻译后的结果，不要额外解释：";
const SINGLE_INSTRUCTION_EN: &str =
    "Translate the following segment into {target_language}, without additional explanation.";

/// Prompt for a lone cue: no numbering, because nothing needs to be re-matched.
///
/// The answer is taken verbatim (see `parse::parse_single_translation`), so
/// asking for a number here would only invite the model to invent one.
///
/// For a Chinese source the target is named in CHINESE (`language::chinese_name`),
/// not in English: measured, "翻译为English" made the model answer in Chinese —
/// it treated the English word as content instead of as the target language.
pub fn single_cue_prompt(
    template: Option<&str>,
    source: Option<&str>,
    target: &str,
    text: &str,
) -> String {
    let chinese_source = source.is_some_and(language::is_chinese);
    let instruction = match normalized_template(template) {
        HY_MT_TEMPLATE if chinese_source => {
            SINGLE_INSTRUCTION_ZH.replace("{target_language}", &language::chinese_name(target))
        }
        _ => SINGLE_INSTRUCTION_EN.replace("{target_language}", &language_label(target)),
    };
    format!("{}\n\n{}", instruction, text)
}

/// `1. text` lines, one per cue, using the numbering the parser expects.
fn number_lines(cues: &[PromptCue]) -> String {
    cues.iter()
        .map(|cue| format!("{}. {}", cue.index, cue.text))
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cues(texts: &[&str], start: usize) -> Vec<PromptCue> {
        texts
            .iter()
            .enumerate()
            .map(|(offset, text)| PromptCue {
                index: start + offset,
                text: text.to_string(),
            })
            .collect()
    }

    /// Chinese source → the Chinese batch contract, with the target's display
    /// name and its tag both present.
    ///
    /// Regression (T3-2): the model card's per-segment line must NOT be
    /// prepended. Measured zh→en, a 20-cue batch carrying both instructions came
    /// back with zero numbered lines AND the source text echoed untranslated, so
    /// every cue fell back to its original text.
    #[test]
    fn chinese_source_uses_the_chinese_template() {
        let prompt = batch_prompt(
            Some(HY_MT_TEMPLATE),
            Some("zh"),
            "en",
            &cues(&["你好", "世界"], 1),
        );

        assert!(
            !prompt.contains("将以下文本翻译为"),
            "the per-segment card line must not be stacked on the batch contract: {prompt}"
        );
        assert!(prompt.contains("将以下 2 条带编号的字幕逐条翻译为English (en)"));
        // The count sentence is what holds the numbering at small batches.
        assert!(prompt.contains("输出必须严格为 2 条"), "{prompt}");
        assert!(prompt.contains("1. 你好"));
        assert!(prompt.contains("2. 世界"));
        assert!(!prompt.contains("Translate the following"));
    }

    /// `zh-tw` is Chinese for template purposes, but the TARGET is Traditional —
    /// the two must not be conflated (design §9).
    #[test]
    fn traditional_chinese_source_still_uses_the_chinese_template() {
        let prompt = batch_prompt(
            Some(HY_MT_TEMPLATE),
            Some("zh-tw"),
            "en",
            &cues(&["你好"], 1),
        );
        assert!(prompt.contains("将以下 1 条"));
        assert!(prompt.contains("English (en)"));
    }

    /// Non-Chinese source → the English batch contract.
    #[test]
    fn non_chinese_source_uses_the_english_template() {
        let prompt = batch_prompt(
            Some(HY_MT_TEMPLATE),
            Some("en"),
            "ja",
            &cues(&["hello", "world"], 1),
        );

        assert!(
            !prompt.contains("Translate the following segment into"),
            "the per-segment card line must not be stacked on the batch contract: {prompt}"
        );
        assert!(prompt.contains("into Japanese (ja)"));
        assert!(prompt.contains("must be exactly 2 lines"), "{prompt}");
        assert!(prompt.contains("1. hello"));
        assert!(!prompt.contains('将'));
    }

    /// An unknown/auto source cannot be Chinese, so the generic template is
    /// used — and it must still name the target.
    #[test]
    fn unknown_source_still_names_the_target() {
        for source in [None, Some("auto")] {
            let prompt = batch_prompt(Some(HY_MT_TEMPLATE), source, "de", &cues(&["hallo"], 1));
            assert!(prompt.contains("German"), "source {source:?}: {prompt}");
            assert!(prompt.contains("German (de)"));
            assert!(!prompt.contains('将'));
        }
    }

    /// A missing or misspelled template falls back instead of failing.
    #[test]
    fn unknown_template_falls_back_to_the_generic_form() {
        let generic = batch_prompt(None, Some("en"), "en", &cues(&["x"], 1));
        let misspelled = batch_prompt(Some("hy-mt"), Some("en"), "en", &cues(&["x"], 1));

        assert_eq!(generic, misspelled);
        assert!(generic.contains("numbered subtitle lines"));
        assert!(generic.contains("English (en)"));
        // The count sentence is part of the contract, generic template included.
        assert!(generic.contains("must be exactly 1 line"), "{generic}");
    }

    /// Numbering is continuous across batches: the second batch of a 20-cue
    /// window starts at 21, which is what the parse step validates against.
    #[test]
    fn numbering_starts_at_the_given_index() {
        let prompt = batch_prompt(Some(HY_MT_TEMPLATE), Some("zh"), "en", &cues(&["第二十一条"], 21));

        // Whole-line comparison: "21. X" contains the substring "1. X", so a
        // substring check could not tell the two numberings apart.
        assert!(
            prompt.lines().any(|line| line == "21. 第二十一条"),
            "{prompt}"
        );
        assert!(!prompt.lines().any(|line| line == "1. 第二十一条"));
        // The count is the batch's own size, not the file's cue count.
        assert!(prompt.contains("将以下 1 条"));
    }

    /// Every cue text survives verbatim, including right-to-left and emoji, and
    /// nothing is trimmed or reordered.
    #[test]
    fn cue_texts_are_embedded_verbatim() {
        let texts = ["مرحبا بالعالم", "שלום", "emoji 😀 ok", "  保留空格  "];
        let prompt = batch_prompt(Some(HY_MT_TEMPLATE), Some("zh"), "en", &cues(&texts, 7));

        for (offset, text) in texts.iter().enumerate() {
            assert!(
                prompt.contains(&format!("{}. {}", 7 + offset, text)),
                "cue {} missing from prompt",
                offset
            );
        }
    }

    /// A single-cue prompt still uses the batch shape, so the degraded retry
    /// parses through exactly the same path.
    #[test]
    fn single_cue_prompt_keeps_the_batch_shape() {
        // A Chinese source keeps the measured Chinese batch wording even for a
        // single cue, which is what the degraded one-cue retry sends.
        let prompt = batch_prompt(Some(HY_MT_TEMPLATE), Some("zh"), "fr", &cues(&["only"], 1));
        assert!(prompt.contains("将以下 1 条"), "{prompt}");
        assert!(prompt.lines().any(|line| line == "1. only"));

        // With no source language the generic shape is used instead; it must
        // still be a numbered batch of one.
        let generic = batch_prompt(Some(HY_MT_TEMPLATE), None, "fr", &cues(&["only"], 1));
        assert!(generic.contains("1 numbered subtitle line"), "{generic}");
        assert!(generic.lines().any(|line| line == "1. only"));
    }

    /// A single cue uses the card's per-segment shape, with no numbering asked
    /// for and the target named in the SOURCE's language.
    ///
    /// Regression: naming the target in English for a Chinese source
    /// ("翻译为English") made the model answer in Chinese. The Chinese name is
    /// what makes it actually translate.
    #[test]
    fn single_cue_prompt_names_the_target_in_the_source_language() {
        let zh = single_cue_prompt(Some(HY_MT_TEMPLATE), Some("zh"), "en", "你好");
        assert!(zh.contains("翻译为英语"), "{zh}");
        assert!(!zh.contains("English"), "{zh}");
        assert!(!zh.contains("1. "), "a lone cue must not be numbered: {zh}");
        assert!(zh.ends_with("你好"));

        // A Traditional-Chinese target must keep its own name.
        let zh_hant = single_cue_prompt(Some(HY_MT_TEMPLATE), Some("zh"), "zh-tw", "hello");
        assert!(zh_hant.contains("繁体中文"), "{zh_hant}");

        // A non-Chinese source keeps the English wording, which was reliable
        // for every probe in that direction. The label carries both the name and
        // the tag, so `zh` renders as "Chinese (Simplified) (zh)".
        let en = single_cue_prompt(Some(HY_MT_TEMPLATE), Some("en"), "zh", "hello");
        assert!(en.contains("Chinese") && en.contains("(zh)"), "{en}");
        assert!(!en.contains("1. "), "{en}");

        // An unknown source is not Chinese, so the English wording is used.
        let auto = single_cue_prompt(Some(HY_MT_TEMPLATE), None, "en", "你好");
        assert!(auto.contains("English (en)"), "{auto}");
    }

    /// Every committed language has a Chinese name, so a Chinese-source prompt
    /// never falls back to an English word mid-sentence.
    #[test]
    fn every_committed_language_has_a_chinese_name() {
        let catalogue = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/resources/models.json"
        ))
        .expect("catalogue is committed");
        let parsed: serde_json::Value =
            serde_json::from_str(&catalogue).expect("catalogue is valid json");

        let translate = parsed["models"]
            .as_array()
            .expect("models array")
            .iter()
            .find(|model| {
                model["roles"]
                    .as_array()
                    .is_some_and(|roles| roles.iter().any(|role| role == "translate"))
            })
            .expect("a translate model is committed");

        for language in translate["languages"].as_array().expect("languages array") {
            let tag = language.as_str().expect("language tag");
            let name = language::chinese_name(tag);
            assert!(
                name.chars().any(|c| ('\u{4e00}'..='\u{9fff}').contains(&c)),
                "{tag} has no Chinese name (got {name})"
            );
        }
    }

    /// The count sentence must be its own paragraph, and the worked example must
    /// not be glued to the cue body.
    ///
    /// Regression: with a single newline instead of a blank line the model
    /// folded the count into the instruction sentence and stopped emitting
    /// numbered lines entirely (measured zh→en, 0/20 parsed on every attempt).
    #[test]
    fn the_count_sentence_is_its_own_paragraph() {
        let zh = batch_prompt(Some(HY_MT_TEMPLATE), Some("zh"), "en", &cues(&["a", "b"], 1));
        assert!(zh.contains("不要额外解释：\n\n输出必须严格为 2 条"), "{zh}");
        // The cue body starts after a blank line, never glued to the example.
        assert!(zh.contains("2. Second.\n\n1. a\n2. b"), "{zh}");

        let en = batch_prompt(Some(HY_MT_TEMPLATE), Some("en"), "zh", &cues(&["a", "b"], 1));
        assert!(en.contains("do not add explanations:\n\nThe output must be exactly 2 lines"), "{en}");
        assert!(en.contains("2. Second.\n\n1. a\n2. b"), "{en}");
    }

    /// Every prompt shape states the expected count.
    ///
    /// The count sentence is not decoration: measured zh→en, dropping it made
    /// batches of 2–5 and 14 come back as a single prose line instead of n
    /// numbered lines, which the parser must then reject cue by cue.
    #[test]
    fn every_prompt_shape_states_the_expected_count() {
        for (template, source, target) in [
            (Some(HY_MT_TEMPLATE), Some("zh"), "en"),
            (Some(HY_MT_TEMPLATE), Some("en"), "zh"),
            (None, Some("en"), "ja"),
        ] {
            for count in [1usize, 2, 20] {
                let texts: Vec<String> = (0..count).map(|i| format!("cue {i}")).collect();
                let borrowed: Vec<&str> = texts.iter().map(String::as_str).collect();
                let prompt = batch_prompt(template, source, target, &cues(&borrowed, 1));

                assert!(
                    prompt.contains(&count.to_string()),
                    "the expected count must appear for {template:?}/{source:?}/{count}: {prompt}"
                );
                assert!(
                    prompt.lines().any(|line| line == "1. cue 0"),
                    "the numbered body must survive: {prompt}"
                );
                // The cue body is numbered independently of the worked example,
                // which always shows `1. First.` / `2. Second.` — so match the
                // body's own text rather than counting `1. ` lines, which the
                // example would now double.
                assert_eq!(
                    prompt.lines().filter(|line| line.ends_with("cue 0")).count(),
                    1,
                    "cue 1 must appear exactly once: {prompt}"
                );
                assert!(
                    prompt.contains("1. First.") && prompt.contains("2. Second."),
                    "the worked example must be present: {prompt}"
                );
            }
        }
    }
}
