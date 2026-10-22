//! Passthrough inference: one unsplit `moss_transcribe_diarize` call that
//! returns text + segment timestamps + speaker labels together.
//!
//! Tasks: M5 (design §6.1 / §6.3). Measured behaviour is recorded in
//! docs/tech-spec/audiocpp-moss-poc.md; this module implements exactly that
//! shape rather than guessing.
//!
//! Why the whole VAD → slice → `requests.json` → sequence pipeline is skipped:
//! the model emits its own timestamps and speaker labels, so there is nothing to
//! estimate and no turns↔text attribution error to make. The price is the loss
//! of VAD's segmentation stability and a documented collapse of speaker labels
//! under rapid alternation (PoC §6.1) — which is why it is a selectable plan,
//! never a forced default (design §12 M10).

use std::path::Path;

use crate::paths::AppPaths;
use crate::engine::catalog;
use crate::engine::cli::{spawn, Backend, CliInvocation};
use crate::engine::diar::{labels_for_turns, parse_turns, DiarTurn, SAMPLE_RATE};
use crate::engine::{EngineError, RawChunk};

/// audio.cpp task/family for the passthrough model.
const TASK: &str = "asr";
const FAMILY: &str = "moss_transcribe_diarize";

/// Token budget per second of audio used to size `--max-tokens`.
///
/// MOSS decodes autoregressively and truncates at a default budget (design
/// §13-4). Measured on v0.9.1 (see audiocpp-moss-poc.md §5): the default
/// budget of 5120 is fine on its own, and a 10.4-minute clip still completes.
/// Raising the rate from 6 to 8 tokens/s and flooring the estimate at the
/// model's own 5120 default is what keeps a 5-minute+ clip from being sent a
/// *smaller* budget than the model default would have used. Chinese speech
/// decoded at ~6.5-7 tokens/s in measurement, so 8 leaves a margin.
const TOKENS_PER_SECOND: f64 = 8.0;
/// Minimum budget, so a very short clip is not starved.
///
/// Deliberately equal to the model's own default (5120): flooring at 1024
/// meant a 5-14 minute clip was given *less* than the model would have used on
/// its own, so the truncation was self-inflicted (measured on a 300 s clip:
/// our 1801 token budget failed, the model's 5120 default would not have).
const MIN_MAX_TOKENS: u32 = 5120;
/// Upper bound; beyond this the decode cache grows without buying accuracy.
///
/// 16k covers ~30 minutes of speech at 8 tokens/s; above this the KV-cache
/// memory climbs ~0.33 MB/token (measured: 4 MB at 8192, 12 GB at 32768)
/// without buying accuracy.
const MAX_MAX_TOKENS: u32 = 16_384;

/// Estimate the `--max-tokens` budget for `duration_s` seconds of audio.
///
/// Monotonic in `duration_s`, clamped to `[MIN_MAX_TOKENS, MAX_MAX_TOKENS]`.
pub fn estimate_max_tokens(duration_s: f64) -> u32 {
    if !duration_s.is_finite() || duration_s <= 0.0 {
        return MIN_MAX_TOKENS;
    }
    let raw = (duration_s * TOKENS_PER_SECOND).ceil();
    if raw <= MIN_MAX_TOKENS as f64 {
        return MIN_MAX_TOKENS;
    }
    if raw >= MAX_MAX_TOKENS as f64 {
        return MAX_MAX_TOKENS;
    }
    raw as u32
}

/// Duration of a 16 kHz mono s16le WAV, in seconds.
///
/// A WAV is read here rather than shelling out to ffprobe because the pipeline
/// already guarantees this exact format (16 kHz mono s16le) before inference.
pub fn wav_duration_seconds(wav_path: &Path) -> Result<f64, EngineError> {
    let bytes = std::fs::metadata(wav_path)
        .map(|meta| meta.len())
        .map_err(|e| EngineError::Io(format!("stat {}: {}", wav_path.display(), e)))?;
    // 44-byte canonical header; the extraction step always writes one.
    let data_bytes = bytes.saturating_sub(44);
    Ok(data_bytes as f64 / (SAMPLE_RATE as f64 * 2.0))
}

/// Output paths for one passthrough run.
pub struct DirectOutputs {
    pub text: std::path::PathBuf,
    pub segments: std::path::PathBuf,
    pub turns: std::path::PathBuf,
}

impl DirectOutputs {
    pub fn in_dir(dir: &Path) -> Self {
        Self {
            text: dir.join("direct_transcript.txt"),
            segments: dir.join("direct_segments.json"),
            turns: dir.join("direct_turns.json"),
        }
    }
}

/// Run one unsplit passthrough call and return speaker-labelled chunks.
///
/// Mirrors `direct_outputs` → CLI → parse. A successful call that wrote no
/// turns is not an error: an all-silent input returns an empty list with exit
/// code 0, which becomes an empty subtitle (PoC §4.3).
///
/// Scratch files go into the task temp directory (removed on every exit path),
/// never next to the user's media file.
pub async fn transcribe_direct(
    paths: &AppPaths,
    task_id: &str,
    wav_path: &Path,
    model_id: &str,
    backend: Backend,
    on_progress: &crate::engine::ProgressFn<'_>,
) -> Result<Vec<RawChunk>, EngineError> {
    let model_file = catalog::model_path(paths, model_id)?.ok_or_else(|| {
        EngineError::ModelNotInstalled(format!("model '{}' is not in the catalogue", model_id))
    })?;

    let work_dir = super::TempWorkDir::create(task_id)?;
    let duration_s = wav_duration_seconds(wav_path)?;
    let max_tokens = estimate_max_tokens(duration_s);
    let out = DirectOutputs::in_dir(work_dir.path());

    on_progress(0, 0, "Loading model");

    let invocation = CliInvocation {
        task: TASK.to_string(),
        family: FAMILY.to_string(),
        model: model_file,
        audio: Some(wav_path.to_path_buf()),
        backend,
        args: direct_args(&out, max_tokens),
    };

    spawn(Some(paths), invocation, |_| {}).await?;

    let raw = std::fs::read_to_string(&out.turns).map_err(|e| {
        EngineError::Protocol(format!(
            "{} exited successfully but wrote no readable turns at {}: {}",
            FAMILY,
            out.turns.display(),
            e
        ))
    })?;

    let turns = parse_turns(&raw)?;
    if turns.is_empty() {
        return Ok(Vec::new());
    }

    // Prefer each turn's own text (the model emits it); fall back to the
    // transcript file only if a future revision stops including it.
    let chunks = turns_into_chunks(&turns);
    on_progress(chunks.len(), chunks.len(), "Transcribing");
    Ok(chunks)
}

/// The passthrough-specific flags, appended after the common ones.
///
/// `--max-tokens` is a plain CLI argument on this single-request path: the PoC
/// §4.1 `--request-option` / `--request-sequence` conflict does not apply here
/// because no request sequence is used (design §6.3).
fn direct_args(out: &DirectOutputs, max_tokens: u32) -> Vec<String> {
    [
        ("--text-out", out.text.to_string_lossy().into_owned()),
        ("--segments-out", out.segments.to_string_lossy().into_owned()),
        ("--turns-out", out.turns.to_string_lossy().into_owned()),
        ("--max-tokens", max_tokens.to_string()),
    ]
    .into_iter()
    .flat_map(|(flag, value)| [flag.to_string(), value])
    .collect()
}

/// Turn the parsed turns into speaker-labelled [`RawChunk`]s.
///
/// Labels are assigned in first-appearance order and hidden entirely when the
/// recording has a single speaker (design §6.4), matching the composed path.
pub fn turns_into_chunks(turns: &[DiarTurn]) -> Vec<RawChunk> {
    let labels = labels_for_turns(turns);
    turns
        .iter()
        .map(|turn| RawChunk {
            start_s: turn.start_s,
            end_s: turn.end_s,
            text: turn.text.clone().unwrap_or_default(),
            speaker: labels
                .as_ref()
                .and_then(|labels| labels.get(&turn.speaker_id).cloned()),
            // The passthrough model emits text + times + speakers, never a
            // `<|lang|>` tag, so the source language stays unknown here. That is
            // the documented "translate anyway" branch (translation-design §5),
            // not a failure.
            language: None,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn turn(start_s: f64, end_s: f64, speaker: &str, text: &str) -> DiarTurn {
        DiarTurn {
            start_s,
            end_s,
            speaker_id: speaker.to_string(),
            text: Some(text.to_string()),
        }
    }

    #[test]
    fn max_tokens_is_monotonic_and_clamped() {
        assert_eq!(estimate_max_tokens(0.0), MIN_MAX_TOKENS);
        assert_eq!(estimate_max_tokens(-5.0), MIN_MAX_TOKENS);
        assert_eq!(estimate_max_tokens(f64::NAN), MIN_MAX_TOKENS);

        let short = estimate_max_tokens(30.0);
        let long = estimate_max_tokens(600.0);
        assert!(long >= short, "budget must not shrink as audio grows");
        assert_eq!(short, MIN_MAX_TOKENS);
        // 600s * 8 = 4800, still under the 5120 floor.
        assert_eq!(long, MIN_MAX_TOKENS);
        assert_eq!(estimate_max_tokens(1_000_000.0), MAX_MAX_TOKENS);
    }

    /// The regression this file exists to prevent: a 5-minute clip must never be
    /// sent a budget **smaller** than the model's own default.
    ///
    /// Measured on a real 300 s Chinese clip: the old formula (6 tokens/s,
    /// floor 1024) asked for 1801 and `audiocpp_cli` exited 1 with
    /// `MOSS-Transcribe-Diarize reached max_tokens before EOS`, while the
    /// model's own 5120 default would have finished. A 1037 s clip needed 6928.
    #[test]
    fn a_measured_clip_is_never_given_less_than_the_model_default() {
        const MODEL_DEFAULT: u32 = 5120;

        // 300 s: failed at 1801 under the old constants.
        assert!(estimate_max_tokens(300.0) >= MODEL_DEFAULT);
        // 1037 s: needed 6928 measured.
        assert!(estimate_max_tokens(1037.05) >= 6928);
    }

    #[test]
    fn direct_args_carry_measured_flags_and_max_tokens() {
        let out = DirectOutputs::in_dir(Path::new("/tmp/work"));
        let args = direct_args(&out, 2048);

        assert!(args.windows(2).any(|w| w[0] == "--turns-out"));
        assert!(args.windows(2).any(|w| w[0] == "--segments-out"));
        assert!(args.windows(2).any(|w| w[0] == "--text-out"));
        assert!(args
            .windows(2)
            .any(|w| w[0] == "--max-tokens" && w[1] == "2048"));
        // The incompatible flag must never appear on this path.
        assert!(!args.iter().any(|arg| arg == "--request-option"));
        assert!(!args.iter().any(|arg| arg == "--request-sequence"));
    }

    #[test]
    fn two_speakers_get_labels_and_keep_their_text() {
        let turns = vec![
            turn(0.0, 2.0, "S01", "hello"),
            turn(2.0, 4.0, "S02", "world"),
            turn(4.0, 6.0, "S01", "again"),
        ];
        let chunks = turns_into_chunks(&turns);

        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0].speaker.as_deref(), Some("Speaker A"));
        assert_eq!(chunks[1].speaker.as_deref(), Some("Speaker B"));
        assert_eq!(chunks[2].speaker.as_deref(), Some("Speaker A"));
        assert_eq!(chunks[1].text, "world");
        assert!((chunks[1].start_s - 2.0).abs() < 1e-9);
    }

    #[test]
    fn one_speaker_hides_labels_on_the_direct_path() {
        let turns = vec![turn(0.0, 2.0, "S01", "a"), turn(2.0, 4.0, "S01", "b")];
        let chunks = turns_into_chunks(&turns);
        assert!(chunks.iter().all(|chunk| chunk.speaker.is_none()));
    }

    #[test]
    fn missing_turn_text_becomes_empty_not_panicking() {
        let turns = vec![DiarTurn {
            start_s: 0.0,
            end_s: 1.0,
            speaker_id: "S01".to_string(),
            text: None,
        }];
        let chunks = turns_into_chunks(&turns);
        assert_eq!(chunks[0].text, "");
        assert_eq!(chunks[0].language, None);
    }

    /// Regression: the passthrough path carries no language tag, and that must
    /// be `None` rather than an error or a guess (T4-1).
    #[test]
    fn direct_path_reports_no_language() {
        let turns = vec![turn(0.0, 2.0, "S01", "<|zh|>你好")];
        let chunks = turns_into_chunks(&turns);

        assert!(
            chunks.iter().all(|chunk| chunk.language.is_none()),
            "{chunks:?}"
        );
        // The raw tag stays in the text; downstream `clean_text` still strips it.
        assert_eq!(chunks[0].text, "<|zh|>你好");
    }
}
