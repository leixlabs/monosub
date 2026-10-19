//! Resolve output → `SiteMedia`, plus the multi-P judgement.
//!
//! Task: U2-1. See docs/tech-spec/url-import-design.md §4.1 and §4.4.
//!
//! The resolve call prints a human-readable subtitle table first and the info
//! dict last (measured, U0-9). "Pick the last line that parses as JSON" is not
//! good enough: `123` in a table cell is valid JSON on its own, and so is a
//! quoted string. The rule is therefore "the first line that is a JSON **object**
//! with a string `id`".

use std::collections::BTreeMap;

use serde_json::Value;

use crate::site::error::ImportError;

/// One entry in the platform's subtitle list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubTrack {
    /// Container format as the extractor reports it (`srt`, `xml`, …).
    pub ext: String,
    /// Inline subtitle text, when the extractor already converted it (Bilibili
    /// inlines SRT through `json2srt`; U0-9 confirmed).
    pub data: Option<String>,
}

/// What Resolve learned about one video (url-import-design §4.1).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SiteMedia {
    pub id: String,
    pub title: String,
    pub duration: Option<f64>,
    /// Tag → tracks. A tag usually has exactly one track; the map is keyed by tag
    /// because that is what `--sub-langs` takes.
    pub subs: BTreeMap<String, SubTrack>,
}

impl SiteMedia {
    /// Every tag the platform offered, in sorted order. Used by the parser tests
    /// and by `resolve` diagnostics; production code looks tags up directly.
    #[cfg(test)]
    pub fn tags(&self) -> Vec<&str> {
        self.subs.keys().map(String::as_str).collect()
    }

    #[cfg(test)]
    pub fn has(&self, tag: &str) -> bool {
        self.subs.contains_key(tag)
    }
}

/// Parse the stdout of a `Stage::Resolve` call.
pub fn parse_resolve_output(stdout: &str) -> Result<SiteMedia, ImportError> {
    let info = find_info_dict(stdout).ok_or_else(|| {
        ImportError::Other("yt-dlp printed no info dict".to_string())
    })?;
    from_info_dict(&info)
}

/// The first JSON object line that carries a string `id`.
///
/// Pure and separately tested: this is the rule the whole probe depends on.
pub fn find_info_dict(stdout: &str) -> Option<Value> {
    stdout.lines().find_map(|line| {
        let line = line.trim();
        if !line.starts_with('{') {
            return None;
        }
        let value: Value = serde_json::from_str(line).ok()?;
        is_info_dict(&value).then_some(value)
    })
}

fn is_info_dict(value: &Value) -> bool {
    value.is_object() && value.get("id").and_then(Value::as_str).is_some_and(|id| !id.is_empty())
}

/// The info dict → `SiteMedia`.
pub fn from_info_dict(info: &Value) -> Result<SiteMedia, ImportError> {
    let id = info
        .get("id")
        .and_then(Value::as_str)
        .filter(|id| !id.is_empty())
        .ok_or_else(|| ImportError::Other("info dict has no id".to_string()))?
        .to_string();

    let title = info
        .get("title")
        .and_then(Value::as_str)
        // `fulltitle` carries the part suffix for a multi-P video; `title` alone
        // is the plain video title.
        .or_else(|| info.get("fulltitle").and_then(Value::as_str))
        .unwrap_or(&id)
        .to_string();

    let duration = info.get("duration").and_then(Value::as_f64);

    Ok(SiteMedia {
        id,
        title,
        duration,
        // `subtitles` may be absent or explicitly null; either means "none".
        subs: subtitle_tracks(info),
    })
}

/// `subtitles` → tag → track. `automatic_captions` is deliberately ignored:
/// Bilibili puts everything, including `ai-zh`, under `subtitles` (design §4.2).
fn subtitle_tracks(info: &Value) -> BTreeMap<String, SubTrack> {
    let mut tracks = BTreeMap::new();
    let Some(entries) = info.get("subtitles").and_then(Value::as_object) else {
        return tracks;
    };

    for (tag, value) in entries {
        // Each tag maps to an array of alternative encodings; the first with a
        // usable container wins.
        let Some(formats) = value.as_array() else {
            continue;
        };
        let Some(first) = formats.iter().find_map(track_from_json) else {
            continue;
        };
        tracks.insert(tag.clone(), first);
    }

    tracks
}

fn track_from_json(value: &Value) -> Option<SubTrack> {
    let ext = value.get("ext").and_then(Value::as_str)?.to_string();
    // Bilibili inlines the SRT text in `data` and omits `url` (U0-9/§9.11). An
    // empty `data` is treated as absent so the fetch path takes over.
    let data = value
        .get("data")
        .and_then(Value::as_str)
        .filter(|data| !data.trim().is_empty())
        .map(str::to_string);
    Some(SubTrack { ext, data })
}

/// What one Resolve call produced.
#[derive(Debug, Clone)]
pub struct Resolved {
    pub media: SiteMedia,
    /// The stderr Resolve produced. Kept for the one thing it is needed for: the
    /// "subtitles are only available when logged in" WARNING, which is not an
    /// error but does change what the UI tells the user (design §4.5).
    pub stderr: String,
}

impl Resolved {
    /// Whether the platform would have offered a subtitle behind a login.
    pub fn needs_login_hint(&self) -> bool {
        crate::site::error::needs_login_hint(&self.stderr)
    }
}

/// Run one Resolve call and parse it.
///
/// One call gets everything: the constant flags plus `--list-subs --print-json`
/// (design §4.1). A non-zero exit is classified from stderr rather than taken at
/// face value, because 412 needs a login prompt and 429 needs a retry.
pub async fn run(
    app: &tauri::AppHandle,
    request: &crate::site::UrlImportRequest,
) -> Result<Resolved, ImportError> {
    crate::site::emit_stage(app, &request.task_id, crate::site::Stage::Resolving, None);

    let invocation = crate::site::ytdlp::Invocation {
        stage: crate::site::ytdlp::Stage::Resolve,
        url: request.url.clone(),
        cookies_browser: crate::site::cookies_of(request),
        ffmpeg: Some(crate::site::ffmpeg_of(app)),
    };

    let output = crate::site::with_rate_limit_retry(|| {
        crate::site::ytdlp::run(Some(app), &invocation, |_| {})
    })
    .await?;

    // A 412 exits non-zero AND prints the `ERROR:` line that classifies it, so
    // the stderr verdict is taken before the stdout one: it is the more specific
    // answer, and a login wall is the difference between "retry" and "configure
    // cookies".
    let classified = crate::site::error::classify(&output.stderr);
    if !matches!(classified, ImportError::Other(_)) {
        return Err(classified);
    }

    let media = parse_resolve_output(&output.stdout).map_err(|error| {
        tracing::warn!(
            code = ?output.code,
            stderr = %output.stderr,
            "could not read an info dict from Resolve"
        );
        // An unclassified non-zero exit is still a failure, not "no info dict".
        if output.success() {
            error
        } else {
            classified
        }
    })?;

    Ok(Resolved {
        media,
        stderr: output.stderr,
    })
}

/// Whether the URL asked for a specific part of a multi-P video.
pub fn has_part_selector(url: &str) -> bool {
    match url::Url::parse(url) {
        Ok(url) => url
            .query_pairs()
            .any(|(key, value)| key == "p" && !value.trim().is_empty()),
        Err(_) => false,
    }
}

/// The design §4.4 rule: with `--no-playlist` and no `?p=`, the extractor
/// silently returns part 1 and renames the id to `<BV>_p1`.
///
/// Converting part 1 and calling it the video would be silently wrong, and
/// expanding to every part would silently create dozens of jobs — so the run
/// fails with a message that tells the user to paste the part's own link.
pub fn is_multipart_id(id: &str) -> bool {
    let Some((_, suffix)) = id.rsplit_once("_p") else {
        return false;
    };
    !suffix.is_empty() && suffix.chars().all(|ch| ch.is_ascii_digit())
}

/// Reject a multi-part video that the URL did not pin to one part.
pub fn check_multipart(url: &str, media: &SiteMedia) -> Result<(), ImportError> {
    if !has_part_selector(url) && is_multipart_id(&media.id) {
        return Err(ImportError::MultiPart);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The real shape measured in U0-9, trimmed to the fields we read.
    const RESOLVE_STDOUT: &str = "\
Language Formats
danmaku  xml
zh-CN    srt
zh-Hans  srt
zh-Hant  srt
zh-HK    srt
en-US    srt
{\"id\": \"BV1GJ411x7h7\", \"title\": \"【官方 MV】Never Gonna Give You Up\", \"duration\": 212.393, \"subtitles\": {\"danmaku\": [{\"ext\": \"xml\", \"url\": \"https://comment.bilibili.com/137649199.xml\"}], \"zh-CN\": [{\"ext\": \"srt\", \"data\": \"1\\n00:00:01,000 --> 00:00:02,000\\n你好\\n\"}], \"zh-Hant\": [{\"ext\": \"srt\", \"data\": \"1\\n00:00:01,000 --> 00:00:02,000\\n你好\\n\"}], \"en-US\": [{\"ext\": \"srt\", \"data\": \"1\\n00:00:01,000 --> 00:00:02,000\\nhi\\n\"}]}}
";

    #[test]
    fn parses_the_measured_resolve_output() {
        let media = parse_resolve_output(RESOLVE_STDOUT).expect("info dict");

        assert_eq!(media.id, "BV1GJ411x7h7");
        assert_eq!(media.title, "【官方 MV】Never Gonna Give You Up");
        assert_eq!(media.duration, Some(212.393));
        assert_eq!(media.tags(), vec!["danmaku", "en-US", "zh-CN", "zh-Hant"]);

        let simplified = &media.subs["zh-CN"];
        assert_eq!(simplified.ext, "srt");
        assert!(simplified.data.as_deref().is_some_and(|data| data.contains("你好")));

        // The danmaku track keeps its `xml` container and has no inline data, so
        // `lang::select` can reject it on either ground.
        let danmaku = &media.subs["danmaku"];
        assert_eq!(danmaku.ext, "xml");
        assert_eq!(danmaku.data, None);
    }

    /// Regression: table cells like `123` are valid JSON, and a bare number or
    /// string must never be mistaken for the info dict.
    #[test]
    fn a_scalar_json_line_is_not_an_info_dict() {
        let stdout = "\
Language Formats
123
\"just a string\"
[1, 2, 3]
{\"no_id\": true}
{\"id\": \"\"}
{\"id\": \"BV1real\", \"title\": \"real\"}
";
        let media = parse_resolve_output(stdout).expect("the real dict is found");
        assert_eq!(media.id, "BV1real");
        assert_eq!(media.title, "real");
    }

    #[test]
    fn find_info_dict_skips_a_non_object_json_line() {
        assert!(find_info_dict("123\nnull\ntrue\n").is_none());
        assert!(find_info_dict("").is_none());
        // An object with a non-string id is not it either.
        assert!(find_info_dict("{\"id\": 42}").is_none());
        assert_eq!(
            find_info_dict("{\"id\": \"BV1x\"}").and_then(|value| value["id"].as_str().map(str::to_string)),
            Some("BV1x".to_string())
        );
    }

    #[test]
    fn missing_or_null_subtitles_do_not_panic() {
        for info in [
            serde_json::json!({"id": "BV1x", "title": "t"}),
            serde_json::json!({"id": "BV1x", "title": "t", "subtitles": null}),
            serde_json::json!({"id": "BV1x", "title": "t", "subtitles": {}}),
            serde_json::json!({"id": "BV1x", "title": "t", "subtitles": "nope"}),
        ] {
            let media = from_info_dict(&info).expect("id is present");
            assert!(media.subs.is_empty(), "{info}");
        }
    }

    #[test]
    fn a_track_without_ext_or_with_empty_data_is_skipped_or_has_no_data() {
        let info = serde_json::json!({
            "id": "BV1x",
            "subtitles": {
                "broken": [{"url": "https://example/x.srt"}],
                "empty": [{"ext": "srt", "data": "   "}],
                "good": [{"ext": "srt", "data": "1\n00:00:00,000 --> 00:00:01,000\nhi\n"}]
            }
        });
        let media = from_info_dict(&info).unwrap();
        assert!(!media.has("broken"), "no container format means unusable");
        assert_eq!(media.subs["empty"].data, None, "blank data is treated as absent");
        assert!(media.subs["good"].data.is_some());
    }

    #[test]
    fn falls_back_to_fulltitle_and_then_to_the_id() {
        let with_fulltitle = serde_json::json!({"id": "BV1x", "fulltitle": "full title"});
        assert_eq!(from_info_dict(&with_fulltitle).unwrap().title, "full title");

        let bare = serde_json::json!({"id": "BV1x"});
        assert_eq!(from_info_dict(&bare).unwrap().title, "BV1x");
    }

    #[test]
    fn a_dict_without_an_id_is_an_error() {
        let error = from_info_dict(&serde_json::json!({"title": "t"})).unwrap_err();
        assert!(matches!(error, ImportError::Other(_)), "{error:?}");
    }

    #[test]
    fn multipart_ids_are_detected() {
        assert!(is_multipart_id("BV1G5po6zECY_p1"));
        assert!(is_multipart_id("BV1G5po6zECY_p12"));
        assert!(is_multipart_id("BV1x_p0"));
        assert!(!is_multipart_id("BV1G5po6zECY"));
        assert!(!is_multipart_id("BV1x_part1"));
        assert!(!is_multipart_id("BV1x_p"));
        assert!(!is_multipart_id("BV1x_p1a"));
        // A single-P video whose id happens to contain `_p` is not a multi-P.
        assert!(!is_multipart_id("BV1x_py"));
    }

    #[test]
    fn a_part_selector_is_read_from_the_query() {
        assert!(has_part_selector("https://www.bilibili.com/video/BV1x?p=3"));
        assert!(has_part_selector("https://www.bilibili.com/video/BV1x?p=1"));
        assert!(!has_part_selector("https://www.bilibili.com/video/BV1x"));
        assert!(!has_part_selector("https://www.bilibili.com/video/BV1x?p="));
        assert!(!has_part_selector("https://www.bilibili.com/video/BV1x?spm_id_from=x"));
        // Not a URL at all: no selector, so a multi-P id is rejected.
        assert!(!has_part_selector("BV1x"));
    }

    /// The measured multi-P shape (U0-9): no `?p=` gives `_p1` and a suffixed
    /// title; `?p=3` gives `_p3`.
    #[test]
    fn multipart_is_rejected_only_without_a_part_selector() {
        let part1 = serde_json::json!({
            "id": "BV1G5po6zECY_p1",
            "title": "【盲人无障碍电影】冰雪奇缘第一部 p01 第一部1",
            "duration": 491.133,
            "subtitles": {"danmaku": [{"ext": "xml", "url": "https://comment.bilibili.com/1.xml"}]}
        });
        let media = from_info_dict(&part1).unwrap();

        let no_part = "https://www.bilibili.com/video/BV1G5po6zECY";
        assert_eq!(check_multipart(no_part, &media).unwrap_err(), ImportError::MultiPart);

        let with_part = "https://www.bilibili.com/video/BV1G5po6zECY?p=3";
        assert!(check_multipart(with_part, &media).is_ok());

        // A single-part video is never rejected, with or without `?p=`.
        let single = from_info_dict(&serde_json::json!({"id": "BV1GJ411x7h7"})).unwrap();
        assert!(check_multipart(no_part, &single).is_ok());
    }
}
