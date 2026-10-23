//! Speaker diarization: the `--task diar` invocation, turn parsing, and the
//! pure-Rust attribution of ASR segments to speaker turns.
//!
//! Tasks: M6 (design §6.4). See docs/tech-spec/model-selection-design.md §6.4
//! and docs/tech-spec/audiocpp-moss-poc.md §3.1 for the measured wire shape.
//!
//! Two consumers:
//! * [`detect_turns`] runs a diar-only model (`nemotron_3_diar` by default) when
//!   the plan composes `Asr + Diarize + TurnAttribution`.
//! * [`parse_turns`] / [`label_for`] / [`attribute_turns`] are also used by
//!   `engine/direct.rs`, because MOSS emits the identical `--turns-out` shape.
//!
//! The label mapping and the attribution are the port of the old
//! `core-engine/monosub_core/diarizer.py` (`assign_speakers` /
//! `should_show_speaker_labels`), not new research (design §6.4).

use std::collections::HashMap;
use std::path::Path;

use serde::Deserialize;

use crate::paths::AppPaths;
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
/// `text` is present for `moss_transcribe_diarize` and absent for
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
    /// Turn text when the model provides it (MOSS).
    pub text: Option<String>,
}

/// Convert a sample offset to seconds at [`SAMPLE_RATE`].
pub fn samples_to_seconds(sample: u64) -> f64 {
    sample as f64 / SAMPLE_RATE as f64
}

/// Duration of a 16 kHz mono s16le WAV, in seconds.
///
/// Read from the file length rather than by shelling out to ffprobe: the pipeline
/// guarantees this exact format before any inference runs (see
/// [`crate::engine::wav`]). A file that cannot be stat'ed yields `0.0`, which the
/// caller clamps — a diarization attempt is not worth failing over a stat.
fn wav_duration_seconds(wav_path: &Path) -> f64 {
    const HEADER_BYTES: u64 = 44;
    match std::fs::metadata(wav_path) {
        Ok(meta) => {
            let data = meta.len().saturating_sub(HEADER_BYTES);
            data as f64 / (SAMPLE_RATE as f64 * 2.0)
        }
        Err(_) => 0.0,
    }
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
///
/// Audio longer than the model's [`catalog::ModelSpec::max_session_seconds`] is
/// diarized **window by window** and the windows are stitched: the ceiling is
/// architectural (`sortformer_diar` sizes its encoder graph from
/// `tf_encoder.max_source_positions`, measured at 120 s on audio.cpp 0.9.1), so
/// no session option can raise it and a single call would fail the whole file.
/// See [`diarize_long`] for the stitching rules.
///
/// `speech` is the VAD segmentation the recognition stage already computed
/// (`(start_s, end_s)` in file-relative seconds). It is used only to place window
/// boundaries in silence — see [`plan_speech_windows`] — and is never sent to the
/// model as a unit of work: per-chunk runs would each number their speakers from
/// zero, so `S01` in one chunk would have no measurable relationship to `S01` in
/// the next and the whole file would show dozens of "speakers".
pub async fn detect_turns(
    paths: &AppPaths,
    wav_path: &Path,
    model_id: &str,
    backend: Backend,
    out_path: &Path,
    speech: &[(f64, f64)],
) -> Result<Vec<DiarTurn>, EngineError> {
    let model_file = catalog::model_path(paths, model_id)?.ok_or_else(|| {
        EngineError::ModelNotInstalled(format!("model '{}' is not in the catalogue", model_id))
    })?;
    let specs = catalog::catalog(paths)?;
    let spec = specs.iter().find(|spec| spec.id == model_id).ok_or_else(|| {
        EngineError::ModelNotInstalled(format!("model '{}' is not in the catalogue", model_id))
    })?;

    // The CLI only writes `--turns-out` if the directory already exists.
    if let Some(parent) = out_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| EngineError::Io(format!("create {}: {}", parent.display(), e)))?;
    }

    let duration_s = wav_duration_seconds(wav_path);
    let session_len = (duration_s.ceil() + 1.0).max(1.0);

    let run = DiarRun {
        paths,
        wav_path,
        spec,
        model_file: &model_file,
        backend,
        speech,
    };

    // Audio that fits one session is the plain single call, unchanged.
    let ceiling = spec.max_session_seconds;
    if ceiling.is_none_or(|max| duration_s <= max) {
        return detect_turns_once(&run, out_path, session_len).await;
    }

    diarize_long(&run, duration_s, ceiling.unwrap_or(duration_s)).await
}

/// Everything one diarization run needs, so the two paths can share it without
/// threading eight positional arguments through both.
struct DiarRun<'a> {
    paths: &'a AppPaths,
    wav_path: &'a Path,
    spec: &'a catalog::ModelSpec,
    model_file: &'a Path,
    backend: Backend,
    /// VAD speech spans, file-relative seconds. Empty means "cut on the blind
    /// grid", which is what a caller without a recognition run passes.
    speech: &'a [(f64, f64)],
}

/// One `--task diar` call over the whole file.
async fn detect_turns_once(
    run: &DiarRun<'_>,
    out_path: &Path,
    session_len: f64,
) -> Result<Vec<DiarTurn>, EngineError> {
    let invocation = CliInvocation {
        task: TASK.to_string(),
        family: run.spec.family.clone(),
        model: run.model_file.to_path_buf(),
        audio: Some(run.wav_path.to_path_buf()),
        backend: run.backend,
        args: session_args(run.spec, run.model_file, out_path, session_len),
    };

    spawn(Some(run.paths), invocation, |_| {}).await?;
    read_turns(out_path, &run.spec.family)
}

/// The `--turns-out … --session-option …` tail shared by both diar paths.
///
/// `session_len_sec` is sent **only when the model declares it**, because an
/// undeclared session option fails the whole run rather than being ignored:
///
/// ```text
/// audiocpp_cli failed: unknown Sortformer diar session option: sortformer_diar.bogus_option
/// ```
///
/// It is not cosmetic either way:
///
/// * `sortformer_diar` (v1) **needs** it — its graph is fixed-capacity and sized
///   by this value, defaulting to 20 s, so omitting it fails any real recording
///   with `request exceeds fixed graph capacity` (measured: 19 s succeeds, 20 s
///   fails).
/// * `nemotron_3_diar` has **no** such option: its geometry comes from
///   `latency_profile`, and it has no fixed position table to overflow.
///
/// Sending the duration of the audio rather than the ceiling is deliberate for
/// v1: the graph is sized to the session, and a smaller session than the audio
/// truncates it.
fn session_args(
    spec: &catalog::ModelSpec,
    model_file: &Path,
    out_path: &Path,
    session_len: f64,
) -> Vec<String> {
    let mut args = vec![
        "--turns-out".to_string(),
        out_path.to_string_lossy().into_owned(),
    ];
    if super::spec::session_options(Some(model_file)).session_len_sec {
        args.push("--session-option".to_string());
        args.push(format!("{}.session_len_sec={}", spec.family, session_len));
    }
    args
}

/// Read and parse a `--turns-out` document.
fn read_turns(out_path: &Path, family: &str) -> Result<Vec<DiarTurn>, EngineError> {
    let raw = std::fs::read_to_string(out_path).map_err(|e| {
        EngineError::Protocol(format!(
            "{} exited successfully but wrote no readable turns at {}: {}",
            family,
            out_path.display(),
            e
        ))
    })?;
    parse_turns(&raw)
}

// --- long audio: windows + label chaining ----------------------------------

/// Overlap between two adjacent diarization windows, in seconds.
///
/// The overlap exists for exactly one reason: a window boundary lands mid-turn,
/// and the only thing that can join `SPEAKER_01` in window N to `SPEAKER_00` in
/// window N+1 is a stretch of audio both windows saw. Measured on a 17-minute
/// two-speaker podcast: every pair of adjacent windows agreed by a 2–5× margin
/// in the overlap, so a majority vote resolves the permutation cleanly.
const WINDOW_OVERLAP_SECONDS: f64 = 20.0;

/// Turn fragments shorter than this are dropped after stitching.
///
/// `sortformer_diar` emits sub-second blips at window edges (measured: a 0.48 s
/// `SPEAKER_02` turn in a two-speaker recording). They are artefacts of the
/// overlap being truncated, not speakers, and attributing a subtitle cue to one
/// would put a phantom third speaker in the output.
const MIN_TURN_SECONDS: f64 = 0.5;

/// A window of the source audio: `[start_s, end_s)` in file-relative seconds.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DiarWindow {
    pub start_s: f64,
    pub end_s: f64,
}

impl DiarWindow {
    /// The part of this window that only this window is responsible for.
    ///
    /// The cut between two windows is the **midpoint of their overlap**, which
    /// gives each neighbour half of it. It is derived from the neighbours rather
    /// than from a fixed half-overlap because the last window is nudged backwards
    /// to stay full-length ([`plan_windows`]), so the hop is not uniform: a fixed
    /// half-overlap would leave the final two cores overlapping and count that
    /// audio twice.
    ///
    /// The first window keeps its opening and the last keeps its ending, so no
    /// audio is lost at the file's own edges.
    fn core(&self, previous_end: Option<f64>, next_start: Option<f64>) -> (f64, f64) {
        let start = previous_end.map_or(self.start_s, |end| (end + self.start_s) / 2.0);
        let end = next_start.map_or(self.end_s, |start| (self.end_s + start) / 2.0);
        (start, end)
    }
}

/// Split `duration_s` into windows of at most `max_s`, overlapping by `overlap`.
///
/// Every window is `<= max_s`, which is the model's hard ceiling — the point of
/// the split. `overlap` is clamped to half a window so a tiny ceiling cannot
/// produce a window that is entirely overlap.
pub fn plan_windows(duration_s: f64, max_s: f64, overlap: f64) -> Vec<DiarWindow> {
    if duration_s <= 0.0 || max_s <= 0.0 {
        return Vec::new();
    }
    if duration_s <= max_s {
        return vec![DiarWindow {
            start_s: 0.0,
            end_s: duration_s,
        }];
    }

    let overlap = overlap.min(max_s / 2.0).max(0.0);
    let hop = max_s - overlap;
    let mut windows = Vec::new();
    let mut start = 0.0_f64;
    loop {
        let end = (start + max_s).min(duration_s);
        windows.push(DiarWindow {
            start_s: start,
            end_s: end,
        });
        // The final window must reach the end of the file; nudging it back to
        // `duration_s - max_s` keeps it full-length instead of emitting a
        // sliver that would have no overlap to chain labels through.
        if end >= duration_s {
            break;
        }
        let next = start + hop;
        if next + max_s >= duration_s {
            windows.push(DiarWindow {
                start_s: (duration_s - max_s).max(0.0),
                end_s: duration_s,
            });
            break;
        }
        start = next;
    }
    windows
}

/// Padding added around a speech span before it is used as a window edge.
///
/// VAD reports the speech itself, so a cut exactly at its last sample clips the
/// final phoneme's tail. A quarter second is well under the shortest gap VAD
/// merges (`VAD_MERGE_GAP_SECONDS`), so this can never bridge two utterances.
const WINDOW_PADDING_SECONDS: f64 = 0.25;

/// Smallest forward step a window may take, so the loop cannot stall on a very
/// short span near the ceiling.
const MIN_HOP_SECONDS: f64 = 1.0;

/// Shortest window a speech-guided cut may produce.
///
/// A cut is only taken at a span end that leaves at least `overlap + MIN_HOP`
/// seconds of window behind it. Two things go wrong below that: the window is a
/// sliver that carries almost no speaker evidence, and the next window would open
/// at (or past) this window's end, so the two would share no audio and
/// [`chain_labels`] would have nothing to vote on — measured as a 1 s window at
/// 1073.25 s followed by one that started exactly where it ended.
fn min_window_seconds(overlap: f64) -> f64 {
    overlap + MIN_HOP_SECONDS
}

/// Split the file into windows whose **edges land in silence**, using the VAD
/// segmentation the ASR stage already produced.
///
/// This is [`plan_windows`] with a better place to cut. The fixed grid cuts at
/// `start + max_s`, which lands mid-utterance whenever the ceiling falls inside
/// speech; the model then sees a truncated word on both sides of the boundary
/// and can attribute the two halves to different speakers. Cutting at the last
/// VAD span end below the ceiling means every window begins and ends between
/// utterances.
///
/// What this is **not**: one `--task diar` call per VAD chunk. Per-chunk runs are
/// each numbered from zero and share no audio, so `S01` in chunk 7 has no
/// measurable relationship to `S01` in chunk 8 — the ids would be incomparable
/// and the whole file would show dozens of "speakers". The windows here still
/// overlap by [`WINDOW_OVERLAP_SECONDS`], and that shared audio is what
/// [`chain_labels`] votes on.
///
/// `speech` is `(start_s, end_s)` in file-relative seconds. Empty or unusable
/// spans fall back to the blind grid, so a file VAD found no speech in behaves
/// exactly as before.
pub fn plan_speech_windows(
    speech: &[(f64, f64)],
    duration_s: f64,
    max_s: f64,
    overlap: f64,
) -> Vec<DiarWindow> {
    if duration_s <= 0.0 || max_s <= 0.0 {
        return Vec::new();
    }
    if duration_s <= max_s {
        return vec![DiarWindow {
            start_s: 0.0,
            end_s: duration_s,
        }];
    }

    let overlap = overlap.min(max_s / 2.0).max(0.0);
    // A span with `end <= start` carries no audio; trusting it as a cut point
    // would place a boundary inside nothing.
    let spans: Vec<(f64, f64)> = speech
        .iter()
        .copied()
        .filter(|(start, end)| end > start)
        .map(|(start, end)| {
            (
                (start - WINDOW_PADDING_SECONDS).max(0.0),
                (end + WINDOW_PADDING_SECONDS).min(duration_s),
            )
        })
        .collect();
    if spans.is_empty() {
        return plan_windows(duration_s, max_s, overlap);
    }

    let mut windows = Vec::new();
    let mut start = 0.0_f64;
    // Spans that end before the current window starts can never be a cut point
    // again, so the scan resumes instead of restarting.
    let mut first = 0usize;

    loop {
        if start >= duration_s {
            break;
        }
        let limit = (start + max_s).min(duration_s);

        while first < spans.len() && spans[first].1 <= start {
            first += 1;
        }

        // The last silence below the ceiling, when there is one. No span fits
        // (a single utterance longer than the ceiling) means the grid's cut is
        // the only option, and `core()` still assigns that utterance to exactly
        // one window.
        //
        // A span end too close to `start` is not usable as a cut: the window it
        // would close is too short to hold speaker evidence, and the next window
        // could not overlap it. `min_window_seconds` is the floor.
        let shortest = start + min_window_seconds(overlap);
        let mut end: Option<f64> = None;
        for &(_, span_end) in &spans[first..] {
            if span_end > limit {
                break;
            }
            if span_end >= shortest {
                end = Some(span_end);
            }
        }
        let end = end.unwrap_or(limit);

        windows.push(DiarWindow {
            start_s: start,
            end_s: end,
        });
        if end >= duration_s {
            break;
        }

        // The next window opens `overlap` seconds before this one closes, so the
        // two share audio for [`chain_labels`] to vote on. It must still move
        // forward when the overlap is larger than the window.
        let next = (end - overlap)
            .max(start + MIN_HOP_SECONDS)
            .min(end);
        if next <= start {
            break;
        }
        start = next;
    }

    windows
}

/// How much wall-clock overlap two label sets share, per `(new, old)` pair.
///
/// Keyed by the pair so the caller can pick a maximum-weight matching: the
/// windows are independent runs, so their speaker ids are only comparable
/// through the audio they both saw.
fn overlap_votes(
    previous: &[DiarTurn],
    previous_offset: f64,
    current: &[DiarTurn],
    current_offset: f64,
) -> HashMap<(String, String), f64> {
    let mut votes: HashMap<(String, String), f64> = HashMap::new();
    for old in previous {
        for new in current {
            let seconds = overlap(
                old.start_s + previous_offset,
                old.end_s + previous_offset,
                new.start_s + current_offset,
                new.end_s + current_offset,
            );
            if seconds > 0.0 {
                *votes
                    .entry((new.speaker_id.clone(), old.speaker_id.clone()))
                    .or_insert(0.0) += seconds;
            }
        }
    }
    votes
}

/// Map one window's speaker ids onto the ids already used by earlier windows.
///
/// Greedy maximum-weight matching, which is enough because the overlap votes are
/// measured to be decisive (2–5× margins). A greedy pick rather than a full
/// assignment problem: a wrong pairing here costs one label swap inside the
/// overlap, while a hung solver would cost the whole run. A current id that
/// matches nothing keeps its own name with a window suffix, so an unmatched
/// speaker stays a *distinct* speaker instead of being merged into an existing
/// one.
fn chain_labels(
    previous: &[DiarTurn],
    previous_offset: f64,
    current: &[DiarTurn],
    current_offset: f64,
    window_index: usize,
) -> HashMap<String, String> {
    let votes = overlap_votes(previous, previous_offset, current, current_offset);
    let mut pairs: Vec<((String, String), f64)> = votes.into_iter().collect();
    // Deterministic order: by weight, then by name, so two runs of the same file
    // produce the same labels.
    pairs.sort_by(|a, b| {
        b.1.partial_cmp(&a.1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.0.cmp(&b.0))
    });

    let mut mapping: HashMap<String, String> = HashMap::new();
    let mut claimed: std::collections::HashSet<String> = std::collections::HashSet::new();
    for ((new_id, old_id), _) in pairs {
        if mapping.contains_key(&new_id) || claimed.contains(&old_id) {
            continue;
        }
        mapping.insert(new_id, old_id.clone());
        claimed.insert(old_id);
    }

    for turn in current {
        mapping
            .entry(turn.speaker_id.clone())
            .or_insert_with(|| format!("{}@w{}", turn.speaker_id, window_index));
    }
    mapping
}

/// Diarize audio longer than one session by running it in windows.
///
/// Four rules make the result equivalent to one call over the whole file:
///
/// 1. **Every window fits the ceiling**, so no call can fail on graph capacity.
/// 2. **Boundaries land in silence** ([`plan_speech_windows`]), using the VAD
///    segmentation the ASR stage already computed — a cut inside an utterance
///    truncates the same word on both sides of the boundary, and the model can
///    then attribute the two halves to different speakers.
/// 3. **Labels are chained through the overlap** ([`chain_labels`]): a speaker
///    keeps one identity across the file even though each window numbers its own
///    speakers from zero.
/// 4. **Each second is emitted once**, from the window that owns it
///    ([`DiarWindow::core`]), so a turn spanning a boundary is not duplicated.
async fn diarize_long(
    run: &DiarRun<'_>,
    duration_s: f64,
    ceiling_s: f64,
) -> Result<Vec<DiarTurn>, EngineError> {
    let windows = plan_speech_windows(run.speech, duration_s, ceiling_s, WINDOW_OVERLAP_SECONDS);
    let work_dir = super::TempWorkDir::create(&format!(
        "diar-{}",
        uuid::Uuid::new_v4().simple()
    ))?;
    let dir = work_dir.path();
    tracing::info!(
        windows = windows.len(),
        duration_s = duration_s,
        ceiling_s = ceiling_s,
        speech_spans = run.speech.len(),
        family = %run.spec.family,
        "diarization exceeds one session; splitting into overlapping windows"
    );

    // Per-window turns, each already shifted onto the file timeline.
    let mut per_window: Vec<(DiarWindow, Vec<DiarTurn>)> = Vec::with_capacity(windows.len());
    for (index, window) in windows.iter().enumerate() {
        let window_wav = dir.join(format!("window_{:03}.wav", index));
        let window_turns = dir.join(format!("window_{:03}.turns.json", index));

        // A ceiling of 120 s at 16 kHz mono is 7.7 MB of PCM per window; the
        // copy streams it rather than decoding the whole source.
        super::wav::copy_range(
            run.wav_path,
            (window.start_s * SAMPLE_RATE as f64).round() as u64,
            (window.end_s * SAMPLE_RATE as f64).round() as u64,
            &window_wav,
        )?;

        // The session length still has to cover the window: it sizes the graph
        // for the families that use one, and a window is by construction at or
        // below the ceiling that limit allows.
        let session_len = (window.end_s - window.start_s).ceil().max(1.0);
        let invocation = CliInvocation {
            task: TASK.to_string(),
            family: run.spec.family.clone(),
            model: run.model_file.to_path_buf(),
            audio: Some(window_wav.clone()),
            backend: run.backend,
            args: session_args(run.spec, run.model_file, &window_turns, session_len),
        };
        spawn(Some(run.paths), invocation, |_| {}).await?;

        // A silent window writes no file at all, exactly like a silent whole-file
        // run: that is a window with no speakers, not a failure.
        let turns = match read_turns(&window_turns, &run.spec.family) {
            Ok(turns) => turns,
            Err(EngineError::Protocol(message)) if !window_turns.exists() => {
                tracing::debug!(
                    window = index,
                    reason = %message,
                    "diarization window contains no speech"
                );
                Vec::new()
            }
            Err(error) => return Err(error),
        };
        per_window.push((*window, turns));
    }

    // Chain the labels across windows, then keep each window's core.
    let mut stitched: Vec<DiarTurn> = Vec::new();
    let mut previous: Option<(&DiarWindow, &[DiarTurn])> = None;
    for (index, (window, turns)) in per_window.iter().enumerate() {
        let mapping = match previous {
            Some((prev_window, prev_turns)) if !prev_turns.is_empty() && !turns.is_empty() => {
                chain_labels(
                    prev_turns,
                    prev_window.start_s,
                    turns,
                    window.start_s,
                    index,
                )
            }
            // Nothing to chain through: a window with no turns before this one,
            // or this window itself is silent. Its own ids are the identity.
            _ => turns
                .iter()
                .map(|turn| (turn.speaker_id.clone(), turn.speaker_id.clone()))
                .collect(),
        };

        let (core_start, core_end) = window.core(
            per_window
                .get(index.wrapping_sub(1))
                .filter(|_| index > 0)
                .map(|(prev, _)| prev.end_s),
            per_window.get(index + 1).map(|(next, _)| next.start_s),
        );
        for turn in turns {
            // Turns are file-relative inside a window; shift them out.
            let start_s = turn.start_s + window.start_s;
            let end_s = turn.end_s + window.start_s;
            let start_s = start_s.max(core_start);
            let end_s = end_s.min(core_end);
            if end_s - start_s < MIN_TURN_SECONDS {
                continue;
            }
            stitched.push(DiarTurn {
                start_s,
                end_s,
                speaker_id: mapping
                    .get(&turn.speaker_id)
                    .cloned()
                    .unwrap_or_else(|| turn.speaker_id.clone()),
                text: turn.text.clone(),
            });
        }

        previous = Some((window, turns));
    }

    tracing::info!(
        windows = per_window.len(),
        turns = stitched.len(),
        speakers = stitched
            .iter()
            .map(|turn| turn.speaker_id.as_str())
            .collect::<std::collections::HashSet<_>>()
            .len(),
        "diarization windows stitched"
    );
    Ok(stitched)
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
            language: None,
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

    // --- long audio: windows and label chaining ----------------------------
    //
    // Every fixture below is the measured `sortformer_diar` behaviour: 120 s is
    // the hard ceiling (`tf_encoder.max_source_positions`), and its per-window
    // speaker ids restart at zero, so the same voice is `SPEAKER_00` in one
    // window and `SPEAKER_01` in the next.

    fn turn(start: f64, end: f64, speaker: &str) -> DiarTurn {
        DiarTurn {
            start_s: start,
            end_s: end,
            speaker_id: speaker.to_string(),
            text: None,
        }
    }

    #[test]
    fn audio_within_the_ceiling_is_one_window() {
        let windows = plan_windows(60.0, 120.0, WINDOW_OVERLAP_SECONDS);
        assert_eq!(
            windows,
            vec![DiarWindow {
                start_s: 0.0,
                end_s: 60.0
            }]
        );
    }

    #[test]
    fn a_17_minute_file_splits_into_windows_that_all_fit_the_ceiling() {
        // 1037.05 s is the measured podcast that failed the whole run.
        let windows = plan_windows(1037.049, 120.0, WINDOW_OVERLAP_SECONDS);
        assert!(windows.len() > 1, "long audio must be split: {:?}", windows);
        for window in &windows {
            assert!(
                window.end_s - window.start_s <= 120.0,
                "window {:?} exceeds the model ceiling",
                window
            );
        }
        // Coverage: every second of the file is inside at least one window.
        assert_eq!(windows[0].start_s, 0.0);
        assert!((windows.last().unwrap().end_s - 1037.049).abs() < 1e-6);
        for pair in windows.windows(2) {
            assert!(
                pair[1].start_s < pair[0].end_s,
                "adjacent windows must overlap: {:?}",
                pair
            );
        }
    }

    #[test]
    fn the_final_window_is_full_length_not_a_sliver() {
        // 250 s with a 120 s ceiling: naive stepping would leave a 10 s tail
        // with no usable overlap to chain labels through.
        let windows = plan_windows(250.0, 120.0, WINDOW_OVERLAP_SECONDS);
        let last = windows.last().unwrap();
        assert!(
            last.end_s - last.start_s >= 120.0 - 1e-9,
            "final window is a sliver: {:?}",
            last
        );
        assert!((last.end_s - 250.0).abs() < 1e-9);
    }

    #[test]
    fn a_tiny_ceiling_cannot_make_a_window_all_overlap() {
        let windows = plan_windows(100.0, 4.0, 20.0);
        assert!(!windows.is_empty());
        for window in &windows {
            assert!(
                window.end_s - window.start_s <= 4.0 + 1e-9,
                "overlap must be clamped below the ceiling: {:?}",
                window
            );
        }
    }

    // --- VAD-guided window edges ------------------------------------------

    #[test]
    fn windows_end_at_the_last_silence_below_the_ceiling() {
        // Speech runs 0–100 s, then silence until 200 s, then more speech.
        // The blind grid would cut at 120 s — inside the second utterance —
        // while the VAD-guided plan cuts at the 100 s span end.
        let speech = vec![(0.0, 40.0), (40.0, 100.0), (200.0, 260.0)];
        let windows = plan_speech_windows(&speech, 300.0, 120.0, WINDOW_OVERLAP_SECONDS);

        assert!(windows.len() >= 2, "long audio must still be split: {windows:?}");
        let first = windows[0];
        assert!(
            (first.end_s - (100.0 + WINDOW_PADDING_SECONDS)).abs() < 1e-9,
            "the first cut must land in the silence after the 100 s utterance, got {first:?}"
        );
        // The point of the change: the boundary is not the blind ceiling.
        assert!(first.end_s < 120.0, "the cut is still on the blind grid: {first:?}");
        assert!(
            first.end_s - first.start_s <= 120.0,
            "the window still has to fit the ceiling: {first:?}"
        );
    }

    #[test]
    fn every_speech_guided_window_fits_the_ceiling_and_the_file_is_covered() {
        let speech: Vec<(f64, f64)> = (0..60)
            .map(|index| {
                let start = index as f64 * 18.0;
                (start, start + 12.0)
            })
            .collect();
        let duration = 1079.0;
        let windows = plan_speech_windows(&speech, duration, 120.0, WINDOW_OVERLAP_SECONDS);

        assert!(windows.len() > 1);
        assert_eq!(windows[0].start_s, 0.0);
        assert!(
            (windows.last().unwrap().end_s - duration).abs() < 1e-9,
            "the last window must reach the end of the file"
        );
        for window in &windows {
            assert!(
                window.end_s - window.start_s <= 120.0 + 1e-9,
                "window exceeds the ceiling: {window:?}"
            );
            assert!(window.end_s > window.start_s, "empty window: {window:?}");
        }
        for pair in windows.windows(2) {
            assert!(
                pair[1].start_s < pair[0].end_s,
                "adjacent windows must overlap for label chaining: {pair:?}"
            );
        }
    }

    #[test]
    fn without_vad_spans_the_blind_grid_is_unchanged() {
        // A file VAD found no speech in must behave exactly as before, so the
        // old path stays the fallback rather than a second implementation.
        assert_eq!(
            plan_speech_windows(&[], 1037.049, 120.0, WINDOW_OVERLAP_SECONDS),
            plan_windows(1037.049, 120.0, WINDOW_OVERLAP_SECONDS)
        );
        assert_eq!(
            plan_speech_windows(&[(5.0, 5.0)], 1037.049, 120.0, WINDOW_OVERLAP_SECONDS),
            plan_windows(1037.049, 120.0, WINDOW_OVERLAP_SECONDS),
            "a zero-length span is not a cut point"
        );
    }

    #[test]
    fn a_single_utterance_longer_than_the_ceiling_still_splits() {
        // Continuous speech with no silence to cut on: the ceiling is the only
        // option, and the loop must still terminate.
        let windows = plan_speech_windows(&[(0.0, 500.0)], 500.0, 120.0, WINDOW_OVERLAP_SECONDS);
        assert!(windows.len() > 1, "must still split: {windows:?}");
        for window in &windows {
            assert!(
                window.end_s - window.start_s <= 120.0 + 1e-9,
                "window exceeds the ceiling: {window:?}"
            );
        }
        assert!((windows.last().unwrap().end_s - 500.0).abs() < 1e-9);
    }

    #[test]
    fn short_audio_is_one_window_whatever_the_speech_says() {
        let windows =
            plan_speech_windows(&[(0.0, 10.0)], 60.0, 120.0, WINDOW_OVERLAP_SECONDS);
        assert_eq!(
            windows,
            vec![DiarWindow {
                start_s: 0.0,
                end_s: 60.0
            }]
        );
    }

    #[test]
    fn cores_tile_the_file_without_gaps_or_double_counting() {
        let windows = plan_windows(400.0, 120.0, WINDOW_OVERLAP_SECONDS);
        let mut cores = Vec::new();
        for (index, window) in windows.iter().enumerate() {
            cores.push(window.core(
                if index == 0 {
                    None
                } else {
                    Some(windows[index - 1].end_s)
                },
                windows.get(index + 1).map(|next| next.start_s),
            ));
        }
        assert_eq!(cores[0].0, 0.0, "the first window keeps the file's opening");
        assert_eq!(
            cores.last().unwrap().1,
            windows.last().unwrap().end_s,
            "the last window keeps the file's ending"
        );
        for pair in cores.windows(2) {
            assert!(
                (pair[0].1 - pair[1].0).abs() < 1e-9,
                "cores must meet exactly, got {:?}",
                pair
            );
        }
    }

    #[test]
    fn labels_chain_across_a_window_boundary() {
        // Window 0 calls the first voice SPEAKER_00; window 1 starts numbering
        // again, so the same voice is SPEAKER_01 there. The overlap decides.
        let previous = vec![
            turn(0.0, 30.0, "SPEAKER_01"),
            turn(30.0, 110.0, "SPEAKER_00"),
        ];
        let current = vec![
            turn(0.0, 18.0, "SPEAKER_01"), // 90–108 s: same voice as SPEAKER_00
            turn(18.0, 20.0, "SPEAKER_00"), // 108–110 s: a short blip
            turn(20.0, 60.0, "SPEAKER_02"), // beyond the overlap: a new speaker
        ];

        let mapping = chain_labels(&previous, 0.0, &current, 90.0, 1);

        assert_eq!(
            mapping.get("SPEAKER_01").map(String::as_str),
            Some("SPEAKER_00"),
            "the 18 s vote dominates: this is the same voice"
        );
        assert_eq!(
            mapping.get("SPEAKER_00").map(String::as_str),
            Some("SPEAKER_00@w1"),
            "a 2 s blip has no vote of its own; inventing a pairing for it \
             would overwrite the decisive match"
        );
        assert_eq!(
            mapping.get("SPEAKER_02").map(String::as_str),
            Some("SPEAKER_02@w1"),
            "a voice never heard before stays distinct"
        );
    }

    #[test]
    fn a_speaker_outside_the_overlap_stays_a_distinct_speaker() {
        let previous = vec![turn(0.0, 60.0, "SPEAKER_00")];
        // Window 1 starts at 60 s, so its only turn (60–65 s) shares no audio
        // with the previous window's turn, which ended at 60 s. Merging them
        // would invent a speaker the audio does not support.
        let current = vec![turn(0.0, 5.0, "SPEAKER_01")];
        let mapping = chain_labels(&previous, 0.0, &current, 60.0, 1);

        assert_eq!(mapping.len(), 1);
        assert_eq!(
            mapping.get("SPEAKER_01").map(String::as_str),
            Some("SPEAKER_01@w1"),
            "an unmatched id must not be merged into an existing speaker"
        );
    }

    #[test]
    fn chaining_is_deterministic_when_two_pairs_tie() {
        // Equal overlap for both pairings: the tie-break is by name, so the same
        // file always produces the same labels.
        let previous = vec![
            turn(0.0, 5.0, "SPEAKER_00"),
            turn(0.0, 5.0, "SPEAKER_01"),
        ];
        let current = vec![
            turn(0.0, 5.0, "SPEAKER_01"),
            turn(0.0, 5.0, "SPEAKER_00"),
        ];
        let first = chain_labels(&previous, 0.0, &current, 90.0, 1);
        let second = chain_labels(&previous, 0.0, &current, 90.0, 1);
        assert_eq!(first, second);
    }

    /// Repo root when this is a dev checkout with weights and binaries present.
    fn repo_root() -> Option<std::path::PathBuf> {
        let cwd = std::env::current_dir().ok()?;
        let candidate = if cwd.join("models").is_dir() {
            cwd
        } else {
            cwd.parent()?.to_path_buf()
        };
        (candidate.join("models").is_dir() && candidate.join("src-tauri/resources/bin").is_dir())
            .then_some(candidate)
    }

    /// The measured failure this whole module exists to prevent, end to end:
    /// a long recording must come back with turns instead of an engine error.
    ///
    /// Before windowing, `detect_turns` passed the whole duration as
    /// `session_len_sec` and the CLI failed the entire run with
    /// `session_len_sec exceeds tf_encoder.max_source_positions`, so every
    /// recording over 120 s silently lost its speaker labels (measured on a
    /// 1037 s podcast, 2026-10-22). The assertion is deliberately about *labels
    /// surviving a window boundary* — a run that produced turns from one window
    /// only would pass a bare "not empty" check.
    ///
    /// **`#[ignore]`: this one loads a real model and runs inference**, which is
    /// minutes of GPU/CPU on a long fixture. The default `cargo test` must stay
    /// fast and must not compete with the running app for the GPU, so the pure
    /// windowing/labelling tests above carry the regression cover. Run it on
    /// purpose when changing the stitching:
    ///
    /// ```text
    /// cargo test --lib engine::diar -- --ignored --nocapture
    /// ```
    #[ignore = "loads a real model and runs inference; run explicitly with --ignored"]
    #[tokio::test]
    async fn a_long_file_is_diarized_in_windows_and_keeps_one_identity_per_speaker() {
        let Some(root) = repo_root() else {
            eprintln!("skip: not a dev checkout (models/ and resources/bin not found)");
            return;
        };
        let model =
            root.join("models/Sortformer-Diar-4spk-v1-GGUF/sortformer-diar-4spk-v1-q8_0.gguf");
        // `poc/` is gitignored (like every other audio fixture here), so a fresh
        // clone skips this rather than failing.
        let wav = root.join("poc/moss/long10.wav");
        if !model.is_file() {
            eprintln!("skip: {} is not downloaded", model.display());
            return;
        }
        if !wav.is_file() {
            eprintln!("skip: the long-speech fixture is absent at {}", wav.display());
            return;
        }

        let out = std::env::temp_dir().join(format!(
            "monosub-diar-long-{}-{}.json",
            std::process::id(),
            uuid::Uuid::new_v4()
        ));
        let paths = crate::paths::AppPaths::from_env().expect("app paths");

        let turns = detect_turns(
            &paths,
            &wav,
            "sortformer_diar_4spk_q8_0",
            // CPU, not Metal: `sortformer_diar` is measured to fail on the Metal
            // backend once the graph is large (`Sortformer diar graph compute
            // failed` at 110 s), and this test is about the windowing, not about
            // which backend runs. The pipeline's Metal→CPU retry covers the app.
            Backend::Cpu,
            &out,
            // No VAD spans here: the fixture is a WAV, not a recognition run, so
            // this exercises the blind-grid fallback of `plan_speech_windows`.
            &[],
        )
        .await
        .expect("a file over the session ceiling must still diarize");
        let _ = std::fs::remove_file(&out);

        let duration = wav_duration_seconds(&wav);
        assert!(
            duration > 120.0,
            "the fixture must exceed the model ceiling to exercise windowing"
        );
        assert!(
            turns.len() > 20,
            "a long multi-speaker recording must produce many turns, got {}",
            turns.len()
        );

        // Timing is file-relative, ordered, and inside the audio.
        for pair in turns.windows(2) {
            assert!(
                pair[0].start_s <= pair[1].start_s + 1e-9,
                "turns must be ordered: {:?} then {:?}",
                pair[0],
                pair[1]
            );
        }
        for turn in &turns {
            assert!(
                turn.end_s > turn.start_s && turn.end_s <= duration + 1.0,
                "turn outside the audio: {:?}",
                turn
            );
        }

        // Every window was covered, not just the first one.
        let last_start = turns.last().expect("turns").start_s;
        assert!(
            last_start > duration * 0.9,
            "diarization stopped at {:.0}s of a {:.0}s file — later windows were skipped",
            last_start,
            duration
        );

        // The point of the fix: one identity per voice across the whole file.
        // A per-window numbering would show up as many more ids than speakers,
        // and a broken chain as one speaker alternating with itself.
        let ids: std::collections::BTreeSet<&str> =
            turns.iter().map(|turn| turn.speaker_id.as_str()).collect();
        assert!(
            ids.len() <= 4,
            "a small cast must not produce {} speakers: {:?}",
            ids.len(),
            ids
        );
    }
}
