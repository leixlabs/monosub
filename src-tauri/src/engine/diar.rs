//! Speaker diarization: the `--task diar` invocation, turn parsing, and the
//! pure-Rust attribution of ASR segments to speaker turns.
//!
//! Tasks: M6 (design §6.4). See docs/tech-spec/model-selection-design.md §6.4
//! and docs/tech-spec/audiocpp-moss-poc.md §3.1 for the measured wire shape.
//!
//! Two consumers:
//! * [`detect_turns`] runs a diar-only model (`sortformer_diar`) when the plan
//!   composes `Asr + Diarize + TurnAttribution`.
//! * [`parse_turns`] / [`label_for`] / [`attribute_turns`] are also used by
//!   `engine/direct.rs`, because MOSS emits the identical `--turns-out` shape.
//!
//! The label mapping and the attribution are the port of the old
//! `core-engine/monosub_core/diarizer.py` (`assign_speakers` /
//! `should_show_speaker_labels`), not new research (design §6.4).

use std::collections::HashMap;
use std::path::Path;

use serde::Deserialize;

use crate::engine::catalog;
use crate::engine::cli::{spawn, Backend, CliInvocation};
use crate::engine::{EngineError, RawChunk};

/// Sample rate audiocpp reports sample offsets at; 16 kHz mono s16le input.
pub const SAMPLE_RATE: u32 = 16_000;

/// audio.cpp task for diar-only models.
pub const TASK: &str = "diar";

/// Wire format of one `--turns-out` element.
///
/// Measured on v0.9.1 (PoC §3.1):
/// `[{"start_sample":1120,"end_sample":159360,"speaker_id":"S01","confidence":0,"text":"…"}]`.
/// `text` is present for the `asr`-family passthrough model and absent for
/// diar-only models, hence `Option`. `confidence` is always `0` in practice and
/// is deliberately not represented: it must never be used as a filter.
#[derive(Debug, Deserialize)]
struct RawTurn {
    start_sample: u64,
    end_sample: u64,
    #[serde(default)]
    speaker_id: Option<String>,
    #[serde(default)]
    text: Option<String>,
}

/// One speaker turn with timing in seconds.
#[derive(Debug, Clone, PartialEq)]
pub struct DiarTurn {
    pub start_s: f64,
    pub end_s: f64,
    /// Raw CLI label, e.g. `S01`.
    pub speaker_id: String,
    /// Turn text when the model provides it (passthrough path).
    pub text: Option<String>,
}

/// Convert a sample offset to seconds at [`SAMPLE_RATE`].
pub fn samples_to_seconds(sample: u64) -> f64 {
    sample as f64 / SAMPLE_RATE as f64
}

/// Parse a `--turns-out` document (design §6.4; PoC §3.1).
///
/// A missing `speaker_id` is a protocol error: without it there is nothing to
/// attribute, and silently inventing one label would merge every speaker.
pub fn parse_turns(json: &str) -> Result<Vec<DiarTurn>, EngineError> {
    let raw: Vec<RawTurn> = serde_json::from_str(json)
        .map_err(|e| EngineError::Protocol(format!("turns.json: {}", e)))?;

    raw.into_iter()
        .map(|turn| {
            if turn.end_sample < turn.start_sample {
                return Err(EngineError::Protocol(format!(
                    "turns.json: end_sample {} before start_sample {}",
                    turn.end_sample, turn.start_sample
                )));
            }
            let speaker_id = turn.speaker_id.filter(|id| !id.is_empty()).ok_or_else(|| {
                EngineError::Protocol(
                    "turns.json: a turn has no speaker_id, so speakers cannot be attributed"
                        .to_string(),
                )
            })?;
            Ok(DiarTurn {
                start_s: samples_to_seconds(turn.start_sample),
                end_s: samples_to_seconds(turn.end_sample),
                speaker_id,
                text: turn.text,
            })
        })
        .collect()
}

/// Display label for the `index`-th distinct speaker, in first-appearance order.
///
/// `Speaker A`, `Speaker B`, … `Speaker Z`; anything past 26 clamps to
/// `Speaker Z` rather than panicking (design §6.4).
pub fn label_for(index: usize) -> String {
    let letter = (b'A' + index.min(25) as u8) as char;
    format!("Speaker {}", letter)
}

/// Build the raw-id → label map in first-appearance order over `turns`.
///
/// Returns `None` when every turn carries the same speaker: a single-speaker
/// recording must not be decorated with labels (old
/// `should_show_speaker_labels`, design §6.4).
pub fn labels_for_turns(turns: &[DiarTurn]) -> Option<HashMap<String, String>> {
    let mut labels: HashMap<String, String> = HashMap::new();
    for turn in turns {
        if !labels.contains_key(&turn.speaker_id) {
            let label = label_for(labels.len());
            labels.insert(turn.speaker_id.clone(), label);
        }
    }

    if labels.len() <= 1 {
        None
    } else {
        Some(labels)
    }
}

/// Time overlap of two intervals, in seconds; `0.0` when they are disjoint.
fn overlap(a_start: f64, a_end: f64, b_start: f64, b_end: f64) -> f64 {
    (a_end.min(b_end) - a_start.max(b_start)).max(0.0)
}

/// Attribute each ASR segment to the speaker turn it overlaps most (design §6.4).
///
/// A segment with no overlap at all keeps whatever speaker it already had
/// (`None` from the VAD-sequence path), and single-speaker input hides labels.
pub fn attribute_turns(segments: &[RawChunk], turns: &[DiarTurn]) -> Vec<RawChunk> {
    let Some(labels) = labels_for_turns(turns) else {
        // Zero or one distinct speaker: no labels anywhere.
        return segments
            .iter()
            .cloned()
            .map(|mut chunk| {
                chunk.speaker = None;
                chunk
            })
            .collect();
    };

    segments
        .iter()
        .cloned()
        .map(|mut chunk| {
            let mut best: Option<(f64, &str)> = None;
            for turn in turns {
                let score = overlap(chunk.start_s, chunk.end_s, turn.start_s, turn.end_s);
                if score <= 0.0 {
                    continue;
                }
                // Strictly greater keeps the earliest turn on a tie, which
                // matches the Python loop's `>` when iterating in order.
                if best.is_none_or(|(best_score, _)| score > best_score) {
                    best = Some((score, turn.speaker_id.as_str()));
                }
            }

            if let Some((_, speaker_id)) = best {
                chunk.speaker = labels.get(speaker_id).cloned();
            }
            chunk
        })
        .collect()
}

/// Run `--task diar` for `model_id` and return its speaker turns.
///
/// The family is resolved from the catalogue rather than hardcoded, so a new
/// diar model is a `models.json` change (design §5, §7.3).
pub async fn detect_turns(
    app: &tauri::AppHandle,
    wav_path: &Path,
    model_id: &str,
    backend: Backend,
    out_path: &Path,
) -> Result<Vec<DiarTurn>, EngineError> {
    let model_file = catalog::model_path(app, model_id)?.ok_or_else(|| {
        EngineError::ModelNotInstalled(format!("model '{}' is not in the catalogue", model_id))
    })?;
    let specs = catalog::catalog(Some(app))?;
    let spec = specs.iter().find(|spec| spec.id == model_id).ok_or_else(|| {
        EngineError::ModelNotInstalled(format!("model '{}' is not in the catalogue", model_id))
    })?;

    // The CLI only writes `--turns-out` if the directory already exists.
    if let Some(parent) = out_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| EngineError::Io(format!("create {}: {}", parent.display(), e)))?;
    }

    let invocation = CliInvocation {
        task: TASK.to_string(),
        family: spec.family.clone(),
        model: model_file,
        audio: Some(wav_path.to_path_buf()),
        backend,
        args: vec![
            "--turns-out".to_string(),
            out_path.to_string_lossy().into_owned(),
        ],
    };

    spawn(Some(app), invocation, |_| {}).await?;

    let raw = std::fs::read_to_string(out_path).map_err(|e| {
        EngineError::Protocol(format!(
            "{} exited successfully but wrote no readable turns at {}: {}",
            spec.family,
            out_path.display(),
            e
        ))
    })?;
    parse_turns(&raw)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Real measured output shape (PoC §3.1), truncated to two turns.
    const TURNS_JSON: &str = r#"[
      {"start_sample":1120,"end_sample":159360,"speaker_id":"S01","confidence":0,
       "text":"大家好，我是婷婷。"},
      {"start_sample":159200,"end_sample":300960,"speaker_id":"S02","confidence":0,
       "text":"我们目前用的是桌面软件。"}
    ]"#;

    fn chunk(start: f64, end: f64, text: &str) -> RawChunk {
        RawChunk {
            start_s: start,
            end_s: end,
            text: text.to_string(),
            speaker: None,
        }
    }

    #[test]
    fn parses_measured_turn_shape_into_seconds() {
        let turns = parse_turns(TURNS_JSON).expect("measured shape parses");
        assert_eq!(turns.len(), 2);
        assert!((turns[0].start_s - 0.07).abs() < 1e-9);
        assert!((turns[0].end_s - 9.96).abs() < 1e-9);
        assert_eq!(turns[0].speaker_id, "S01");
        assert_eq!(turns[1].speaker_id, "S02");
    }

    #[test]
    fn rejects_inverted_ranges_and_missing_speaker_ids() {
        let inverted = r#"[{"start_sample":100,"end_sample":50,"speaker_id":"S01"}]"#;
        assert!(matches!(
            parse_turns(inverted),
            Err(EngineError::Protocol(_))
        ));

        let no_speaker = r#"[{"start_sample":0,"end_sample":100}]"#;
        assert!(matches!(
            parse_turns(no_speaker),
            Err(EngineError::Protocol(_))
        ));

        assert!(matches!(parse_turns("{ not json"), Err(EngineError::Protocol(_))));
    }

    #[test]
    fn labels_follow_first_appearance_and_repeat() {
        let turns = parse_turns(TURNS_JSON).unwrap();
        let labels = labels_for_turns(&turns).expect("two speakers get labels");
        assert_eq!(labels.get("S01").map(String::as_str), Some("Speaker A"));
        assert_eq!(labels.get("S02").map(String::as_str), Some("Speaker B"));
    }

    #[test]
    fn single_speaker_hides_all_labels() {
        let one = r#"[
          {"start_sample":0,"end_sample":16000,"speaker_id":"S01","text":"a"},
          {"start_sample":16000,"end_sample":32000,"speaker_id":"S01","text":"b"}
        ]"#;
        let turns = parse_turns(one).unwrap();
        assert!(labels_for_turns(&turns).is_none());

        let segments = vec![chunk(0.0, 1.0, "a"), chunk(1.0, 2.0, "b")];
        let attributed = attribute_turns(&segments, &turns);
        assert!(attributed.iter().all(|c| c.speaker.is_none()));
    }

    #[test]
    fn attribution_picks_the_max_overlap_turn() {
        let turns = parse_turns(TURNS_JSON).unwrap();
        // Segment mostly inside the second turn wins S02 even though it starts
        // inside the first.
        let segments = vec![chunk(9.9, 12.0, "overlaps both")];
        let attributed = attribute_turns(&segments, &turns);
        assert_eq!(attributed[0].speaker.as_deref(), Some("Speaker B"));
    }

    #[test]
    fn attribution_keeps_none_when_nothing_overlaps() {
        let turns = parse_turns(TURNS_JSON).unwrap();
        let segments = vec![chunk(100.0, 101.0, "far away")];
        let attributed = attribute_turns(&segments, &turns);
        assert!(attributed[0].speaker.is_none());
    }

    #[test]
    fn label_for_never_panics_past_z() {
        assert_eq!(label_for(0), "Speaker A");
        assert_eq!(label_for(25), "Speaker Z");
        assert_eq!(label_for(26), "Speaker Z");
        assert_eq!(label_for(999), "Speaker Z");
    }
}
