//! Local subtitle translation through llama.cpp.
//!
//! Task: T3. See docs/tech-spec/translation-design.md §2 (where it hangs off the
//! pipeline), §4 (batch + numbering, validate, degrade) and §11 (a separate
//! process, never a linked library).
//!
//! This module is deliberately a sibling of `engine/`, with **no** dependency on
//! it: `engine/` owns audio.cpp and the recognition plan, while everything here
//! turns an already-timed `Vec<Segment>` into a translated one. The only thing
//! the two share is the bundled-binary resolution rule in `engine::cli`.
//!
//! Shape of a run:
//!
//! ```text
//! should_translate? ── no ──▶ return the input untouched
//!         │ yes
//!         ▼
//! start llama-server ──▶ batch cues (20) ──▶ validate count/indices
//!                                 │ mismatch
//!                                 ▼
//!                        retry at half size … down to 1 cue, keep original on failure
//! ```
//!
//! Nothing here ever fails the transcription task: a translation error is
//! reported through [`TranslationOutcome`] and the caller logs it (§2).

pub mod error;
pub mod language;
pub mod parse;
pub mod prompt;
pub mod server;
/// Tauri-free translation stage shared by the transcription and URL-import
/// paths (U1-13, url-import-design §5.6).
pub mod stage;

use std::path::Path;

use crate::engine::catalog::ModelSpec;
use crate::subtitle::Segment;

pub use error::{TranslateError, TRANSLATION_FAILED_STATUS};
pub use server::{Completer, LlamaServer};

/// Cues per request. Measured: 10, 20 and 40 all returned exact cue counts, so
/// 20 is chosen for bounded blast radius — a bad batch costs at most 20 cues and
/// one retry, while a single bad 40-cue batch would degrade twice as much.
pub const BATCH_SIZE: usize = 20;

/// Token budget floor for one batch.
const MIN_BATCH_TOKENS: u32 = 256;
/// Token budget ceiling for one batch; measured decode is ~42–53 tok/s, so a
/// runaway generation would otherwise dominate the whole translation stage.
const MAX_BATCH_TOKENS: u32 = 4096;
/// Output tokens allowed per input character. Translations are roughly the same
/// length as their source, and CJK needs >1 token per character (PoC).
const TOKENS_PER_CHAR: u32 = 4;

/// Why a run produced no translation at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkipReason {
    /// Source and target are the same language (design §5).
    SameLanguage,
    /// There was nothing to translate.
    NoSegments,
}

impl SkipReason {
    /// The IPC spelling, for the report the GUI shows.
    ///
    /// Same reasoning as `engine::ProgressPhase::as_str()`: the frontend needs to
    /// *compare* this value to decide which sentence to show, and comparing
    /// prose is exactly what this codebase refuses to do. One closed set of
    /// identifiers serves as both the payload value and the i18n key suffix.
    pub fn as_str(self) -> &'static str {
        match self {
            SkipReason::SameLanguage => "sameLanguage",
            SkipReason::NoSegments => "noSegments",
        }
    }
}

/// What a translation stage actually did.
///
/// Translation is an enhancement (§2), so this is a report rather than a
/// `Result`: the caller logs it and delivers the original files regardless.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TranslationOutcome {
    /// Set when no request was sent at all.
    pub skipped: Option<SkipReason>,
    /// Cues that received translated text.
    pub translated: usize,
    /// Cues that kept their original text (failed batch, or an ignored extra).
    pub kept_original: usize,
    /// Batches that mismatched and were retried at a smaller size.
    pub retried_batches: usize,
    /// Batches that still mismatched at one cue.
    pub failed_batches: usize,
    /// Last error seen, for the caller's `tracing::warn!`.
    pub last_error: Option<TranslateError>,
}

impl TranslationOutcome {
    /// True when at least one cue could not be translated.
    pub fn degraded(&self) -> bool {
        self.kept_original > 0 || self.failed_batches > 0
    }
}

/// Translate `segments` into `target`, starting one `llama-server` for the run.
///
/// Times and speaker labels are never touched: only `text` may change, and a cue
/// whose translation is unavailable keeps its original text. `model` supplies
/// the prompt-template kind, so no prompt wording is hardcoded for a model.
///
/// `app` is only used to locate the bundled binary (`engine::cli` rules).
pub async fn translate_segments(
    segments: &[Segment],
    source: Option<&str>,
    target: &str,
    model: &ModelSpec,
    model_path: &Path,
    paths: &crate::paths::AppPaths,
    on_progress: impl Fn(usize, usize) + Send + Sync,
) -> (Vec<Segment>, TranslationOutcome) {
    if segments.is_empty() {
        return (
            segments.to_vec(),
            TranslationOutcome {
                skipped: Some(SkipReason::NoSegments),
                ..TranslationOutcome::default()
            },
        );
    }
    if !language::should_translate(source, target) {
        return (
            segments.to_vec(),
            TranslationOutcome {
                skipped: Some(SkipReason::SameLanguage),
                ..TranslationOutcome::default()
            },
        );
    }

    let binary = match crate::engine::cli::resolve_llamacpp_path(paths) {
        Ok(binary) => binary,
        Err(error) => {
            return (
                segments.to_vec(),
                TranslationOutcome {
                    kept_original: segments.len(),
                    last_error: Some(TranslateError::BinaryMissing(error.to_string())),
                    ..TranslationOutcome::default()
                },
            );
        }
    };

    let server = match LlamaServer::start(&binary, model_path).await {
        Ok(server) => server,
        Err(error) => {
            return (
                segments.to_vec(),
                TranslationOutcome {
                    kept_original: segments.len(),
                    last_error: Some(error),
                    ..TranslationOutcome::default()
                },
            );
        }
    };

    let outcome = translate_with(
        &server,
        model.prompt_template.as_deref(),
        segments,
        source,
        target,
        &on_progress,
    )
    .await;

    // The guard is dropped explicitly so the log line ordering is obvious; the
    // `Drop` impl is what actually reclaims the process on cancel (§8).
    drop(server);

    outcome
}

/// The batching/validation/degradation core, written against [`Completer`] so it
/// is exercised end to end against a local mock server.
///
/// **No cross-batch context is sent.** The measured, verified prompt is exactly
/// the instruction plus the numbered lines for one batch; adding "the previous
/// batch ended with …" would change wording whose behaviour was measured, and
/// short cues are the known weak spot (design §11) that a later phase addresses
/// by merging sentences, not by prompting around it.
pub async fn translate_with(
    completer: &dyn Completer,
    template: Option<&str>,
    segments: &[Segment],
    source: Option<&str>,
    target: &str,
    on_progress: &(dyn Fn(usize, usize) + Send + Sync),
) -> (Vec<Segment>, TranslationOutcome) {
    let total = segments.len();
    let mut texts: Vec<Option<String>> = vec![None; total];
    let mut outcome = TranslationOutcome::default();
    let mut done = 0usize;

    let batches: Vec<(usize, usize)> = (0..total)
        .step_by(BATCH_SIZE)
        .map(|start| (start, (start + BATCH_SIZE).min(total)))
        .collect();

    for (start, end) in batches {
        // 1-based numbering, continuous across the file: the parser matches the
        // response back by number, not by position.
        run_batch(
            completer,
            template,
            segments,
            source,
            target,
            start,
            end,
            &mut texts,
            &mut outcome,
        )
        .await;

        for slot in texts.iter().take(end).skip(start) {
            if slot.is_some() {
                done += 1;
            }
        }
        on_progress(done, total);
    }

    let translated = texts.iter().filter(|text| text.is_some()).count();
    outcome.translated = translated;
    outcome.kept_original = total - translated;

    let out = segments
        .iter()
        .zip(texts)
        .map(|(segment, text)| match text {
            Some(text) => Segment {
                // Only `text` may change: the timeline and the speaker label are
                // the ASR's output and are never re-derived (design §1).
                text,
                ..segment.clone()
            },
            None => segment.clone(),
        })
        .collect();

    (out, outcome)
}

/// Translate `[start, end)` of `segments`, retrying at half size on a mismatch.
///
/// Writes the accepted translations into `texts` and records the outcome. A
/// single cue that still fails keeps its original text and is counted, so one
/// stubborn line never invalidates the rest of the file.
#[allow(clippy::too_many_arguments)]
async fn run_batch(
    completer: &dyn Completer,
    template: Option<&str>,
    segments: &[Segment],
    source: Option<&str>,
    target: &str,
    start: usize,
    end: usize,
    texts: &mut [Option<String>],
    outcome: &mut TranslationOutcome,
) {
    if start >= end {
        return;
    }

    let cues: Vec<prompt::PromptCue> = (start..end)
        .map(|index| prompt::PromptCue {
            index: index + 1,
            text: segments[index].text.clone(),
        })
        .collect();

    // A lone cue uses the model card's single-segment template instead of the
    // batched numbering contract: measured, the numbered shape is unreliable for
    // exactly one short cue, and this is the terminal rung of the retry ladder.
    let single = cues.len() == 1;
    let prompt = if single {
        prompt::single_cue_prompt(template, source, target, &cues[0].text)
    } else {
        prompt::batch_prompt(template, source, target, &cues)
    };
    let answer = completer.complete(&prompt, batch_max_tokens(&cues)).await;

    let answer = match answer {
        Ok(answer) => answer,
        Err(error) => {
            // A cancelled task must stop immediately and keep whatever it has;
            // anything else is worth one smaller retry, because a timeout or a
            // single bad request says nothing about the other cues.
            if matches!(error, TranslateError::Cancelled) {
                outcome.last_error = Some(error);
                return;
            }
            return retry_at_half_size(
                completer, template, segments, source, target, start, end, texts, outcome, error,
            )
            .await;
        }
    };

    if single {
        match parse::parse_single_translation(&answer, &cues[0].text) {
            Some(text) => {
                texts[start] = Some(text);
                return;
            }
            None => {
                // Not a usable answer; there is no smaller request to try.
                tracing::warn!(
                    cue = start + 1,
                    "a single cue came back unusable; keeping the original text"
                );
                outcome.failed_batches += 1;
                outcome.last_error = Some(TranslateError::Parse(
                    "single-cue answer was unusable".to_string(),
                ));
                return;
            }
        }
    }

    let parsed = parse::parse_translations(&answer, start + 1);
    if parsed.len() != cues.len() {
        return retry_at_half_size(
            completer,
            template,
            segments,
            source,
            target,
            start,
            end,
            texts,
            outcome,
            TranslateError::Parse(format!(
                "expected {} numbered cue(s), got {}",
                cues.len(),
                parsed.len()
            )),
        )
        .await;
    }

    for cue in &cues {
        match parsed.get(&cue.index) {
            Some(text) if !text.trim().is_empty() => texts[cue.index - 1] = Some(text.clone()),
            // A cue that came back blank is treated as a miss rather than as an
            // empty subtitle line.
            _ => {
                outcome.last_error = Some(TranslateError::Parse(format!(
                    "cue {} came back empty",
                    cue.index
                )));
            }
        }
    }
}

/// Split a failing window in half and try again; at one cue, give up.
///
/// Degradation stops at one cue because there is nothing smaller to ask for. A
/// single cue is the least reliable request shape this model accepts — measured,
/// it sometimes answers with one unnumbered line and sometimes echoes the
/// instruction — but it still succeeds often enough that splitting a failed
/// 2-cue window into two 1-cue requests is worth the extra call.
#[allow(clippy::too_many_arguments)]
async fn retry_at_half_size(
    completer: &dyn Completer,
    template: Option<&str>,
    segments: &[Segment],
    source: Option<&str>,
    target: &str,
    start: usize,
    end: usize,
    texts: &mut [Option<String>],
    outcome: &mut TranslationOutcome,
    error: TranslateError,
) {
    if end - start <= 1 {
        tracing::warn!(
            cue = start + 1,
            error = %error,
            "translation failed for a single cue; keeping the original text"
        );
        outcome.failed_batches += 1;
        outcome.last_error = Some(error);
        return;
    }

    outcome.retried_batches += 1;
    outcome.last_error = Some(error);

    let mid = start + (end - start) / 2;
    Box::pin(run_batch(
        completer, template, segments, source, target, start, mid, texts, outcome,
    ))
    .await;
    Box::pin(run_batch(
        completer, template, segments, source, target, mid, end, texts, outcome,
    ))
    .await;
}

/// Output-token budget for one batch, proportional to the source length.
fn batch_max_tokens(cues: &[prompt::PromptCue]) -> u32 {
    let chars: u32 = cues.iter().map(|cue| cue.text.chars().count() as u32).sum();
    let estimate = (chars * TOKENS_PER_CHAR) + 64;
    estimate.clamp(MIN_BATCH_TOKENS, MAX_BATCH_TOKENS)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::subtitle::Segment as Cue;
    use std::collections::HashMap;
    use std::io::{BufRead, BufReader, Write};
    use std::net::{TcpListener, TcpStream};
    use std::sync::{Arc, Mutex};

    fn cue(index: usize, text: &str) -> Cue {
        Cue {
            start: index as f64,
            end: index as f64 + 0.9,
            text: text.to_string(),
            speaker: if index.is_multiple_of(3) {
                Some("Speaker A".to_string())
            } else {
                None
            },
        }
    }

    fn cues(count: usize, prefix: &str) -> Vec<Cue> {
        (0..count)
            .map(|index| cue(index, &format!("{prefix} {index}")))
            .collect()
    }

    /// Canned response for one prompt body, or a status-only failure.
    enum Reply {
        Body(String),
        Status(u16),
    }

    /// Minimal blocking HTTP/1.1 listener on loopback that answers
    /// `/v1/chat/completions` with a canned completion.
    ///
    /// The real request path is used (`server::post_completion`), so the JSON
    /// body, the `.choices[0].message.content` extraction and the numbered-line
    /// parser are all covered — only the model is fake.
    struct MockServer {
        base_url: String,
        /// Requests seen, in order, for assertions about batching.
        requests: Arc<Mutex<Vec<String>>>,
    }

    impl MockServer {
        /// `responder` maps a request body to a canned response, so a test can
        /// under-return for one batch and answer correctly for the next.
        fn start(responder: impl Fn(&str) -> Reply + Send + Sync + 'static) -> Self {
            let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock server");
            let addr = listener.local_addr().expect("mock addr");
            let requests: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
            let seen = Arc::clone(&requests);

            std::thread::spawn(move || {
                for stream in listener.incoming() {
                    let Ok(mut stream) = stream else { continue };
                    let Some((path, body)) = read_request(&stream) else {
                        continue;
                    };
                    if path != "/v1/chat/completions" {
                        let _ = write_response(&mut stream, 404, "");
                        continue;
                    }
                    seen.lock().unwrap().push(body.clone());
                    match responder(&body) {
                        Reply::Body(content) => {
                            let payload = serde_json::json!({
                                "choices": [{ "message": { "role": "assistant", "content": content } }]
                            });
                            let _ = write_response(&mut stream, 200, &payload.to_string());
                        }
                        Reply::Status(status) => {
                            let _ = write_response(&mut stream, status, "{\"error\":\"boom\"}");
                        }
                    }
                }
            });

            Self {
                base_url: format!("http://{}", addr),
                requests,
            }
        }

        fn request_count(&self) -> usize {
            self.requests.lock().unwrap().len()
        }
    }

    /// A [`Completer`] that talks to the mock over real HTTP.
    struct RemoteCompleter {
        client: reqwest::Client,
        url: String,
    }

    impl RemoteCompleter {
        fn new(server: &MockServer) -> Self {
            Self {
                client: reqwest::Client::builder()
                    .timeout(std::time::Duration::from_secs(10))
                    .build()
                    .expect("test client"),
                url: format!("{}/v1/chat/completions", server.base_url),
            }
        }
    }

    impl Completer for RemoteCompleter {
        fn complete<'a>(
            &'a self,
            prompt: &'a str,
            max_tokens: u32,
        ) -> server::BoxFuture<'a, Result<String, TranslateError>> {
            Box::pin(server::post_completion(
                &self.client,
                &self.url,
                prompt,
                max_tokens,
            ))
        }
    }

    fn read_request(stream: &TcpStream) -> Option<(String, String)> {
        let mut reader = BufReader::new(stream.try_clone().ok()?);
        let mut request_line = String::new();
        reader.read_line(&mut request_line).ok()?;
        let path = request_line.split(' ').nth(1)?.to_string();

        let mut length = 0usize;
        loop {
            let mut header = String::new();
            match reader.read_line(&mut header) {
                Ok(0) => break,
                Ok(_) => {}
                Err(_) => break,
            }
            if header == "\r\n" || header == "\n" {
                break;
            }
            if let Some(value) = header.to_ascii_lowercase().strip_prefix("content-length:") {
                length = value.trim().parse().unwrap_or(0);
            }
        }

        let mut body = vec![0u8; length];
        if length > 0 {
            std::io::Read::read_exact(&mut reader, &mut body).ok()?;
        }
        Some((path, String::from_utf8_lossy(&body).into_owned()))
    }

    fn write_response(stream: &mut TcpStream, status: u16, body: &str) -> std::io::Result<()> {
        write!(
            stream,
            "HTTP/1.1 {} X\r\nContent-Length: {}\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n{}",
            status,
            body.len(),
            body
        )?;
        stream.flush()
    }

    /// Extract the numbered source lines from a request body so the mock can
    /// answer as the model would.
    ///
    /// Only the FINAL block is the cue body. The instruction above it contains a
    /// worked example (`1. First.` / `2. Second.`) that is part of the prompt
    /// contract, so scanning the whole prompt would mistake those example lines
    /// for input cues and answer with more translations than were asked for.
    fn numbered_lines(body: &str) -> Vec<(usize, String)> {
        let value: serde_json::Value = serde_json::from_str(body).expect("request json");
        let prompt = value["messages"][0]["content"].as_str().expect("prompt");
        let cue_body = prompt
            .rsplit_once("\n\n")
            .map(|(_, body)| body)
            .unwrap_or(prompt);
        cue_body
            .lines()
            .filter_map(|line| {
                let trimmed = line.trim_start();
                let digits = trimmed.find(|c: char| !c.is_ascii_digit())?;
                if digits == 0 {
                    return None;
                }
                let index: usize = trimmed[..digits].parse().ok()?;
                let rest = trimmed[digits..].trim_start();
                let rest = rest.strip_prefix('.').or_else(|| rest.strip_prefix(')'))?;
                Some((index, rest.trim().to_string()))
            })
            .collect()
    }

    /// The lone cue of a single-cue request: everything after the final blank
    /// line, which is where `prompt::single_cue_prompt` puts the source text.
    fn single_cue_source(body: &str) -> String {
        let value: serde_json::Value = serde_json::from_str(body).expect("request json");
        let prompt = value["messages"][0]["content"].as_str().expect("prompt");
        prompt
            .rsplit_once("\n\n")
            .map(|(_, body)| body.trim().to_string())
            .unwrap_or_default()
    }

    /// Answer as the model would, for either request shape.
    ///
    /// A request with no numbered cue body is a lone cue: it was sent the
    /// non-numbered single-segment shape, so the translation comes back
    /// unnumbered too. Anything else is the batched shape and is answered with
    /// `index. <text>` lines.
    fn mock_answer(body: &str) -> String {
        let lines = numbered_lines(body);
        if lines.is_empty() {
            return format!("<{}>", single_cue_source(body));
        }
        lines
            .iter()
            .map(|(index, text)| format!("{}. <{text}>", index))
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// The invariant that matters most: cue count, timings and speakers are
    /// byte-identical; only `text` changes.
    #[tokio::test]
    async fn translations_change_only_the_text() {
        let mock = MockServer::start(|body| Reply::Body(mock_answer(body)));
        let completer = RemoteCompleter::new(&mock);
        let input = cues(25, "原文");

        let (out, outcome) = translate_with(
            &completer,
            Some("hy_mt"),
            &input,
            Some("zh"),
            "en",
            &|_, _| {},
        )
        .await;

        assert_eq!(out.len(), input.len(), "cue count must not change");
        assert_eq!(outcome.translated, input.len());
        assert_eq!(outcome.kept_original, 0);
        assert!(!outcome.degraded());
        for (before, after) in input.iter().zip(out.iter()) {
            assert_eq!(after.start, before.start, "start must not change");
            assert_eq!(after.end, before.end, "end must not change");
            assert_eq!(after.speaker, before.speaker, "speaker must not change");
            assert_eq!(
                after.text,
                format!("<{}>", before.text),
                "text must be the translation"
            );
        }

        // A 25-cue file is two batches at 20.
        assert_eq!(mock.request_count(), 2);
    }

    /// A batch that under-returns must be retried smaller, and the cues the
    /// model does answer must still land.
    #[tokio::test]
    async fn under_returning_batch_is_retried_at_half_size() {
        let calls = Arc::new(Mutex::new(0usize));
        let counter = Arc::clone(&calls);
        let mock = MockServer::start(move |body| {
            let lines = numbered_lines(body);
            let mut n = counter.lock().unwrap();
            *n += 1;
            if lines.len() > 1 {
                // Deliberately drop the last cue of any multi-cue request.
                let out = lines[..lines.len() - 1]
                    .iter()
                    .map(|(index, text)| format!("{}. <{text}>", index))
                    .collect::<Vec<_>>()
                    .join("\n");
                Reply::Body(out)
            } else {
                // Either a lone cue (unnumbered shape) or a batch that has
                // already been reduced to one entry.
                Reply::Body(mock_answer(body))
            }
        });
        let completer = RemoteCompleter::new(&mock);
        let input = cues(4, "原文");

        let (out, outcome) = translate_with(
            &completer,
            Some("hy_mt"),
            &input,
            Some("zh"),
            "en",
            &|_, _| {},
        )
        .await;

        assert_eq!(out.len(), 4);
        assert!(
            outcome.retried_batches > 0,
            "the mismatch must trigger a retry"
        );
        // Every cue was ultimately translated, one by one if necessary.
        for (before, after) in input.iter().zip(out.iter()) {
            assert_eq!(after.text, format!("<{}>", before.text));
            assert_eq!(after.start, before.start);
            assert_eq!(after.speaker, before.speaker);
        }
        assert_eq!(outcome.translated, 4);
        assert!(mock.request_count() > 1, "the smaller retries must be real requests");
    }

    /// A server error never panics and never loses a cue: the file comes back
    /// with its original texts and a recorded error.
    #[tokio::test]
    async fn server_error_keeps_every_original_segment() {
        let mock = MockServer::start(|_| Reply::Status(500));
        let completer = RemoteCompleter::new(&mock);
        let input = cues(3, "原文");

        let (out, outcome) = translate_with(
            &completer,
            Some("hy_mt"),
            &input,
            Some("zh"),
            "en",
            &|_, _| {},
        )
        .await;

        assert_eq!(out, input, "a 500 must yield the original segments verbatim");
        assert_eq!(outcome.translated, 0);
        assert_eq!(outcome.kept_original, 3);
        assert!(outcome.degraded());
        assert!(matches!(
            outcome.last_error,
            Some(TranslateError::Request(_))
        ));
        assert_eq!(
            outcome.last_error.as_ref().unwrap().status(),
            TRANSLATION_FAILED_STATUS
        );
    }

    /// An empty response is a count mismatch, not an empty subtitle.
    #[tokio::test]
    async fn empty_response_keeps_the_original_text() {
        let mock = MockServer::start(|_| Reply::Body(String::new()));
        let completer = RemoteCompleter::new(&mock);
        let input = cues(2, "原文");

        let (out, outcome) = translate_with(
            &completer,
            None,
            &input,
            Some("zh"),
            "en",
            &|_, _| {},
        )
        .await;

        assert_eq!(out, input);
        assert_eq!(outcome.translated, 0);
        assert_eq!(outcome.failed_batches, 2, "one failure per single cue");
    }

    /// Progress climbs to `total` and never past it.
    #[tokio::test]
    async fn progress_reaches_total() {
        let mock = MockServer::start(|body| {
            let lines = numbered_lines(body);
            if lines.is_empty() {
                return Reply::Body(mock_answer(body));
            }
            Reply::Body(
                lines
                    .iter()
                    .map(|(index, text)| format!("{}. <{text}>", index))
                    .collect::<Vec<_>>()
                    .join("\n"),
            )
        });
        let completer = RemoteCompleter::new(&mock);
        let input = cues(45, "原文");
        let samples: Mutex<Vec<(usize, usize)>> = Mutex::new(Vec::new());

        let _ = translate_with(
            &completer,
            Some("hy_mt"),
            &input,
            Some("zh"),
            "en",
            &|done, total| samples.lock().unwrap().push((done, total)),
        )
        .await;

        let samples = samples.into_inner().unwrap();
        assert_eq!(samples.last(), Some(&(45, 45)));
        assert!(samples.iter().all(|(done, total)| done <= total));
        assert!(samples.windows(2).all(|pair| pair[0].0 <= pair[1].0));
    }

    /// The skip decision lives in `language`, and `translate_with` deliberately
    /// has no opinion about it: it is a batching core, and the caller
    /// (`translate_segments`) decides whether to call it at all.
    #[test]
    fn same_language_decision_is_in_language_not_in_the_batcher() {
        assert!(!language::should_translate(Some("zh"), "zh"));
        assert!(!language::should_translate(Some("ZH"), "zh"));
        assert!(!language::should_translate(Some("zh-tw"), "zh-Hant"));
        assert!(language::should_translate(Some("zh"), "en"));
        assert!(language::should_translate(Some("zh"), "zh-tw"));
    }

    /// `translate_segments` short-circuits before spawning anything when there
    /// is nothing to translate, so a same-language run costs nothing.
    #[tokio::test]
    async fn translate_segments_skips_when_nothing_to_do() {
        let input = cues(2, "原文");
        let spec = crate::engine::catalog::ModelSpec {
            id: "hy_mt_1_8b_q4_k_m".to_string(),
            family: "hy_mt".to_string(),
            display_name: "HY-MT".to_string(),
            target_directory: "HY-MT1.5-1.8B-GGUF".to_string(),
            model_file: "HY-MT1.5-1.8B-Q4_K_M.gguf".to_string(),
            files: Vec::new(),
            languages: vec!["zh".to_string(), "en".to_string()],
            license: "Tencent HY Community License Agreement".to_string(),
            attribution: None,
            capabilities_raw: None,
            roles: vec![crate::engine::catalog::Role::Translate],
            engine: crate::engine::catalog::Engine::Llamacpp,
            prompt_template: Some("hy_mt".to_string()),
            speed: None,
            source: None,
            tier: None,
            status: crate::engine::catalog::SupportStatus::Supported,
            modes: vec![crate::engine::catalog::RunMode::Offline],
            licenses: Vec::new(),
            requires: Vec::new(),
            language_tokens: Vec::new(),
            max_session_seconds: None,
        };
        let missing = Path::new("/nowhere/HY-MT1.5-1.8B-Q4_K_M.gguf");

        let (out, outcome) = translate_segments(
            &input,
            Some("zh"),
            "zh",
            &spec,
            missing,
            &crate::paths::AppPaths::from_env().expect("paths"),
            |_, _| {},
        )
        .await;
        assert_eq!(out, input);
        assert_eq!(outcome.skipped, Some(SkipReason::SameLanguage));

        let (out, outcome) = translate_segments(&[], Some("zh"), "en", &spec, missing, &crate::paths::AppPaths::from_env().expect("paths"), |_, _| {}).await;
        assert!(out.is_empty());
        assert_eq!(outcome.skipped, Some(SkipReason::NoSegments));

        // A missing model is reported, never a panic, and the cues are kept.
        let (out, outcome) = translate_segments(
            &input,
            Some("zh"),
            "en",
            &spec,
            missing,
            &crate::paths::AppPaths::from_env().expect("paths"),
            |_, _| {},
        )
        .await;
        assert_eq!(out, input);
        assert!(matches!(
            outcome.last_error,
            Some(TranslateError::ModelMissing(_)) | Some(TranslateError::BinaryMissing(_))
        ));
        assert_eq!(outcome.kept_original, 2);
    }

    /// The batch budget grows with the source and stays inside its bounds.
    #[test]
    fn batch_budget_is_bounded() {
        let small = vec![prompt::PromptCue {
            index: 1,
            text: "hi".to_string(),
        }];
        assert_eq!(batch_max_tokens(&small), MIN_BATCH_TOKENS);

        let large = vec![prompt::PromptCue {
            index: 1,
            text: "字".repeat(10_000),
        }];
        assert_eq!(batch_max_tokens(&large), MAX_BATCH_TOKENS);

        let mid = vec![prompt::PromptCue {
            index: 1,
            text: "字".repeat(100),
        }];
        assert!(batch_max_tokens(&mid) > MIN_BATCH_TOKENS);
    }

    /// The measured single-cue failure mode: the model hands the source back
    /// unchanged. It is non-empty, so it must be recognised as "not a
    /// translation" and the cue must keep its original text rather than
    /// multiplying requests.
    #[tokio::test]
    async fn single_cue_echoing_the_source_keeps_the_original() {
        let mock = MockServer::start(|body| Reply::Body(single_cue_source(body)));
        let completer = RemoteCompleter::new(&mock);
        let input = cues(1, "原文");

        let (out, outcome) = translate_with(
            &completer,
            Some("hy_mt"),
            &input,
            Some("zh"),
            "en",
            &|_, _| {},
        )
        .await;

        assert_eq!(out, input);
        assert_eq!(outcome.failed_batches, 1);
        assert_eq!(mock.request_count(), 1, "one cue gets exactly one request");
    }

    /// A 1-cue window uses the model card's single-segment shape, and its answer
    /// is taken verbatim — no numbering is requested, so none is expected.
    #[test]
    fn single_cue_translation_is_taken_verbatim() {
        assert_eq!(
            parse::parse_single_translation("Hello there.", "你好"),
            Some("Hello there.".to_string())
        );
        // A model that numbers its answer anyway must not leak `1. ` into the cue.
        assert_eq!(
            parse::parse_single_translation("1. Hello there.", "你好"),
            Some("Hello there.".to_string())
        );
        // A wrapped answer is joined rather than truncated.
        assert_eq!(
            parse::parse_single_translation("Hello\nthere.", "你好"),
            Some("Hello there.".to_string())
        );
    }

    /// The two measured non-answers must be rejected rather than shipped.
    #[test]
    fn single_cue_rejects_echoes_and_instructions() {
        // The source echoed back, translated or not, is not a translation.
        assert_eq!(parse::parse_single_translation("你好", "你好"), None);
        // The prompt's own instruction coming back.
        assert_eq!(
            parse::parse_single_translation("输出必须严格为 1 条。", "你好"),
            None
        );
        assert_eq!(
            parse::parse_single_translation(
                "The output must be exactly 1 lines.",
                "Hello"
            ),
            None
        );
        assert_eq!(parse::parse_single_translation("   ", "你好"), None);
        // A long sentence that merely mentions a marker is still a translation.
        let long = format!("This is a translation. {}", "word ".repeat(40));
        assert!(parse::parse_single_translation(&long, "你好").is_some());
    }

    /// A map of expectations used by the helpers above; kept so an accidental
    /// change to `numbered_lines` is visible.
    #[test]
    fn mock_reader_understands_the_prompt_shape() {
        let body = serde_json::json!({
            "messages": [{ "role": "user", "content": "instr\n\n1. hello\n2. world\n" }]
        })
        .to_string();
        let parsed: HashMap<usize, String> = numbered_lines(&body).into_iter().collect();

        assert_eq!(parsed[&1], "hello");
        assert_eq!(parsed[&2], "world");
    }
}
