//! audio.cpp inference engine orchestration (see docs/tech-spec/refactor-audiocpp.md §4).
//!
//! Owning tasks: P1-5..P1-9.
//!
//! Pinned baseline — the exports below are the frozen interface between the
//! `engine` module (P1-5..P1-9) and `commands/transcribe.rs` (P1-15).

pub mod aligned;
pub mod catalog;
pub mod cli;
pub mod diar;
pub mod plan;
pub mod spec;
pub mod vad;
pub mod wav;
pub mod words;

use crate::engine::diar::SAMPLE_RATE;
use crate::paths::AppPaths;
use std::path::PathBuf;

pub use cli::{Backend, CliInvocation, CliOutput, EngineError};

/// One VAD segment or ASR transcript with absolute media-relative times.
#[derive(Debug, Clone, PartialEq)]
pub struct RawChunk {
    pub start_s: f64,
    pub end_s: f64,
    pub text: String,
    /// Already-formatted speaker label (`Speaker A`), or `None` when the source
    /// produced no speaker information (design §6.5).
    pub speaker: Option<String>,
    /// The ASR's own language tag (`<|zh|>` → `zh`), extracted before
    /// `subtitle::postprocess::clean_text` deletes it (translation-design §5).
    /// `None` when the source produced no tag — MOSS does not,
    /// and that means "unknown", never an error.
    pub language: Option<String>,
}

/// Result of one file's recognition stage.
///
/// `words` is empty unless word-aligned inference ran; when it is non-empty the
/// subtitle stage derives cue boundaries from it instead of estimating them.
#[derive(Debug, Clone, Default)]
pub struct Transcription {
    pub chunks: Vec<RawChunk>,
    pub words: Vec<words::Word>,
}

impl Transcription {
    fn from_chunks(chunks: Vec<RawChunk>) -> Self {
        Self {
            chunks,
            words: Vec::new(),
        }
    }
}

/// Options for one file's transcription run.
#[derive(Debug, Clone)]
pub struct TranscribeOpts {
    /// App language (`zh`, `zh-tw`, `en`, …); mapped via `catalog::map_language`.
    pub language: String,
    /// Model id from `models.json`; `None` uses the catalogue default.
    pub model_id: Option<String>,
    /// Requested backend; `auto` resolves to metal on macOS with a cpu retry.
    pub backend: Backend,
    /// VAD model id from `models.json`; `None` uses the bundled Silero VAD.
    ///
    /// A `vad`-role catalogue entry selects a downloadable VAD instead. The
    /// bundled Silero is a `.safetensors` directory rather than a GGUF, so it
    /// is not a catalogue entry — it stays the default and the fallback.
    pub vad_model_id: Option<String>,
    /// Ask for word-level timestamps.
    ///
    /// Only honoured when the selected model declares the capability *and* its
    /// `requires` aligner is installed ([`aligned::can_align`]); otherwise the
    /// run proceeds with estimated timing and the solver has already reported the
    /// reason as a gap.
    pub word_timestamps: bool,
}

impl Default for TranscribeOpts {
    fn default() -> Self {
        Self {
            language: "auto".to_string(),
            model_id: None,
            backend: Backend::Metal,
            vad_model_id: None,
            word_timestamps: false,
        }
    }
}

/// Resolve which VAD to run: the catalogue model when one is pinned, otherwise
/// the bundled Silero directory.
///
/// An unknown or file-less id falls back to Silero rather than failing the run:
/// segmentation is not the user-visible goal, and a broken download should not
/// make transcription impossible. The fallback is logged so it is diagnosable.
fn resolve_vad_model(
    paths: &AppPaths,
    vad_model_id: Option<&str>,
) -> Result<vad::VadModel, EngineError> {
    let requested = vad_model_id.map(str::trim).filter(|id| !id.is_empty());

    if let Some(id) = requested {
        let specs = catalog::catalog(paths)?;
        match specs.iter().find(|spec| spec.id == id) {
            Some(spec) if spec.has_role(catalog::Role::Vad) => {
                match catalog::model_path(paths, id) {
                    Ok(Some(path)) => {
                        return Ok(vad::VadModel {
                            family: spec.family.clone(),
                            model: path,
                        })
                    }
                    _ => tracing::warn!(
                        vad = %id,
                        "selected VAD model is not installed; falling back to the bundled Silero VAD"
                    ),
                }
            }
            Some(_) => tracing::warn!(
                vad = %id,
                "selected model has no 'vad' role; falling back to the bundled Silero VAD"
            ),
            None => tracing::warn!(
                vad = %id,
                "selected VAD model is not in the catalogue; falling back to the bundled Silero VAD"
            ),
        }
    }

    Ok(vad::VadModel {
        family: vad::DEFAULT_FAMILY.to_string(),
        model: crate::engine::cli::resolve_vad_path(paths)?,
    })
}

/// Progress callback payload: `(done, total, message)`.
///
/// Both numbers are **weighted units**, not item counts: the ASR stage reports
/// the sample length of the audio it has finished rather than the number of VAD
/// chunks, so a 15 s chunk advances the bar 30× as far as a 0.5 s one. Counting
/// chunks made the bar jump in unpredictable steps — the VAD segmentation
/// already knows each chunk's real length, so using it costs nothing and is
/// what makes the percentage track the work.
pub type ProgressFn<'a> = dyn Fn(usize, usize, &str) + Send + Sync + 'a;

/// audio.cpp task for ASR. The `family` always comes from the selected model's
/// `ModelSpec`, never from a constant here: a GGUF embeds its own family and
/// `audiocpp_cli` rejects a mismatch with
/// "GGUF embeds model spec for family 'X', not 'Y'".
const ASR_TASK: &str = "asr";

/// Per-request option written into `requests.json`.
///
/// PoC §4.1: v0.9.1 rejects `--request-option` together with `--request-sequence`
/// (`unknown options`), so every per-request option has to travel inside the
/// JSON document instead of on the command line.
const REQUEST_CHUNK_MODE: &str = "none";

/// Stem of the ASR `--text-out` path. The CLI appends `_<safe_id>` plus `.txt`
/// and prints `text_out[<id>]=<path>`, which we prefer over guessing the name.
const TEXT_OUT_STEM: &str = "seg.txt";

/// Stem of the ASR `--segments-out` path, named by the same rule
/// (`<stem>_<safe_id>.json`). Only segment-capable models write it.
const SEGMENTS_OUT_STEM: &str = "segs.json";

/// How many chunks go into one `--request-sequence` invocation (spec §7 risk 9).
const MAX_CHUNKS_PER_BATCH: usize = 200;

/// Run the full pipeline for one file: VAD → slice → `requests.json` →
/// `--request-sequence` ASR → per-chunk text.
///
/// Temporary files live in `temp_dir()/monosub-<task_id>/` and are removed on
/// all exits (success, failure, cancel) via a `Drop` guard.
///
/// Task: P1-9. Cancellation is handled by the caller aborting the owning tokio
/// task: `cli::spawn` marks the child `kill_on_drop`, so aborting drops the
/// future and kills the child process.
/// Recognise `wav_path`, returning the chunks **and** the word spans when the
/// selected model can produce them.
///
/// Word timing takes a different route by design (see [`aligned`]): one unsplit
/// CLI call that chunks internally, instead of the VAD → slice → `requests.json`
/// → sequence pipeline. Both end up as [`RawChunk`]s, so nothing downstream
/// except `subtitle::postprocess` has to know which ran.
///
/// The fallback is deliberate: when alignment is requested but the aligner is
/// missing or the model cannot align, the run continues with estimated timing
/// rather than failing. Recognition is the goal; word timing is an improvement.
pub async fn transcribe_words(
    paths: &AppPaths,
    task_id: &str,
    wav_path: &std::path::Path,
    opts: &TranscribeOpts,
    on_progress: &ProgressFn<'_>,
) -> Result<Transcription, EngineError> {
    // Resolve what to run: `--model` is the GGUF file for ASR, the VAD
    // directory is passed through `--session-option`.
    let model_id = opts
        .model_id
        .clone()
        .unwrap_or_else(|| catalog::default_model_id().to_string());
    let specs = catalog::catalog(paths)?;
    let spec = specs.iter().find(|m| m.id == model_id).ok_or_else(|| {
        EngineError::ModelNotInstalled(format!("model '{}' is not in the catalogue", model_id))
    })?;
    let language = catalog::map_language(spec, &opts.language);

    if opts.word_timestamps {
        if aligned::can_align(paths, spec) {
            match aligned::transcribe_aligned(
                paths,
                task_id,
                wav_path,
                spec,
                &language,
                opts.backend,
                on_progress,
            )
            .await
            {
                Ok(aligned) => {
                    tracing::info!(
                        task_id = %task_id,
                        words = aligned.words.len(),
                        "word-aligned transcription complete"
                    );
                    let chunks = aligned::group_into_chunks(&aligned.words, &aligned.text);
                    return Ok(Transcription {
                        chunks,
                        words: aligned.words,
                    });
                }
                // An empty-word result is not a failure (silence, or a language
                // the aligner cannot tokenise) — fall through to the estimate
                // path so the subtitle still gets written.
                Err(error) => tracing::warn!(
                    task_id = %task_id,
                    error = %error,
                    "word-aligned transcription failed; retrying with estimated timing"
                ),
            }
        } else {
            tracing::warn!(
                task_id = %task_id,
                model = %model_id,
                "word timestamps requested but the model cannot align; estimating timing"
            );
        }
    }

    let chunks = transcribe_estimated(paths, task_id, wav_path, spec, &language, opts, on_progress)
        .await?;
    Ok(Transcription::from_chunks(chunks))
}

/// The VAD → slice → `requests.json` → sequence pipeline, with timing estimated
/// per chunk by `subtitle::postprocess`.
async fn transcribe_estimated(
    paths: &AppPaths,
    task_id: &str,
    wav_path: &std::path::Path,
    spec: &catalog::ModelSpec,
    language: &str,
    opts: &TranscribeOpts,
    on_progress: &ProgressFn<'_>,
) -> Result<Vec<RawChunk>, EngineError> {
    let work_dir = TempWorkDir::create(task_id)?;
    let dir = work_dir.path();

    let model_file = catalog::model_path(paths, &spec.id)?.ok_or_else(|| {
        EngineError::ModelNotInstalled(format!("model '{}' is not in the catalogue", spec.id))
    })?;

    // Step 2: VAD. Cheap (no model load) and decides everything downstream.
    on_progress(0, 0, "Detecting speech");
    let vad_model = resolve_vad_model(paths, opts.vad_model_id.as_deref())?;
    tracing::info!(task_id = %task_id, family = %vad_model.family, "VAD stage starting");
    let chunks = vad::detect(paths, wav_path, dir, &vad_model).await?;

    // All-silent input: exit code 0 with zero chunks (PoC §4.3). Not an error —
    // the caller writes an empty subtitle.
    if chunks.is_empty() {
        tracing::info!(task_id = %task_id, "VAD found no speech; writing an empty subtitle");
        on_progress(0, 0, "No speech detected");
        return Ok(Vec::new());
    }

    let total = chunks.len();
    tracing::info!(task_id = %task_id, chunks = total, "VAD chunk plan ready");

    // Progress is weighted by audio length, not by chunk count. A VAD plan is
    // typically a few long utterances plus many short interjections, so counting
    // chunks made the bar jump 1 % for a 0.4 s "嗯" and then stall for seconds on
    // a 15 s sentence — the percentage described the bookkeeping, not the work.
    // The lengths are already in the chunk list, so this costs nothing.
    // Keyed by chunk index, not position: the index comes from the CLI and the
    // batch loop walks `chunks` in order, but nothing guarantees they agree.
    let weights: std::collections::HashMap<usize, usize> = chunks
        .iter()
        .map(|chunk| (chunk.index, chunk_progress_units(chunk)))
        .collect();
    let total_units = weights.values().sum::<usize>().max(1);

    // Step 3a: slice the 16 kHz mono WAV at the sample offsets VAD reported.
    let audio = wav::read(wav_path)?;
    on_progress(0, total_units, "Segmenting audio");
    for chunk in &chunks {
        let out_path = wav::chunk_path(dir, chunk.index);
        wav::slice(&audio, chunk.start_sample, chunk.end_sample, &out_path)?;
    }

    // Which per-request fields this family accepts. Resolved once per file: it
    // is a property of the model, not of the batch.
    let options = spec::request_options(&spec.family, Some(&model_file));
    tracing::debug!(
        task_id = %task_id,
        family = %spec.family,
        language_option = options.language,
        chunk_mode_option = options.audio_chunk_mode,
        "resolved per-request options"
    );

    // Step 3b: transcribe. Each batch is one session, so the model loads once.
    let mut texts: Vec<String> = Vec::with_capacity(total);
    // Real cue boundaries per chunk, when the model emitted them.
    let mut spans_by_chunk: std::collections::HashMap<usize, Vec<SegmentSpan>> =
        std::collections::HashMap::new();
    let mut done = 0usize;
    for batch in chunks.chunks(MAX_CHUNKS_PER_BATCH) {
        let (results, segments) = transcribe_batch(
            paths,
            dir,
            &model_file,
            // The family comes from the SELECTED model, never a constant: the
            // solver may pick `qwen3_asr` for es/de/fr/pt or a manual override,
            // and the GGUF must be loaded by its own family.
            &spec.family,
            &language,
            opts.backend,
            batch,
            &weights,
            total_units,
            done,
            options,
            on_progress,
        )
        .await?;

        for chunk in batch {
            texts.push(results.get(&chunk.index).cloned().unwrap_or_default());
        }
        for (index, spans) in segments {
            spans_by_chunk.insert(index, spans);
        }
        // The batch is done as a unit: every chunk in it has finished, so the
        // bar advances by the whole batch's audio length in one step rather than
        // by a per-chunk count.
        done += batch
            .iter()
            .map(|chunk| weights.get(&chunk.index).copied().unwrap_or(0))
            .sum::<usize>();
        on_progress(done, total_units, "Transcribing");
    }

    // Step 4 is `subtitle::postprocess`. A chunk whose model emitted real
    // segment boundaries is expanded into one `RawChunk` per segment, so the
    // subtitle stage uses *measured* cue times; a chunk without them stays a
    // single chunk covering the VAD window, which the post-processor then splits
    // and estimates within.
    //
    // Expanding rather than re-timing keeps `RawChunk` the one currency of the
    // pipeline: `subtitle::postprocess` sees more, shorter chunks and needs no
    // knowledge of where the boundaries came from.
    let mut out = Vec::with_capacity(chunks.len());
    for (chunk, text) in chunks.iter().zip(texts) {
        match spans_by_chunk.get(&chunk.index) {
            Some(spans) if !spans.is_empty() => {
                for span in spans {
                    // Segment offsets are relative to the chunk, so the chunk's
                    // own start is the base. Clamped to the chunk so a model
                    // that overshoots cannot place a cue past the audio it was
                    // given.
                    let start_s = chunk.start_s() + span.start_sample as f64 / SAMPLE_RATE as f64;
                    let end_s = chunk.start_s() + span.end_sample as f64 / SAMPLE_RATE as f64;
                    out.push(RawChunk {
                        start_s: start_s.min(chunk.end_s()),
                        end_s: end_s.clamp(start_s, chunk.end_s()),
                        // Segments carry their own text; a span without text
                        // falls back to the chunk's transcript.
                        text: if span.text.trim().is_empty() {
                            text.clone()
                        } else {
                            span.text.clone()
                        },
                        language: crate::subtitle::postprocess::extract_language_tag(&text),
                        speaker: None,
                    });
                }
            }
            _ => out.push(RawChunk {
                start_s: chunk.start_s(),
                end_s: chunk.end_s(),
                // The language tag must be read BEFORE it is cleaned away further
                // down the pipeline (translation-design §5); `clean_text` runs in
                // `subtitle::postprocess`, not here, so `text` is still raw.
                language: crate::subtitle::postprocess::extract_language_tag(&text),
                text,
                // The VAD-sequence path has no speaker model, so no label here.
                speaker: None,
            }),
        }
    }

    Ok(out)
}

/// Transcribe one batch of chunks in a single CLI session.
///
/// Retries once on the CPU backend when a `metal` run fails with a backend-ish
/// error (spec §5.3).
#[allow(clippy::too_many_arguments)]
async fn transcribe_batch(
    paths: &AppPaths,
    dir: &std::path::Path,
    model_file: &std::path::Path,
    family: &str,
    language: &str,
    backend: Backend,
    batch: &[vad::VadChunk],
    weights: &std::collections::HashMap<usize, usize>,
    total_units: usize,
    done_before: usize,
    options: spec::RequestOptions,
    on_progress: &ProgressFn<'_>,
) -> Result<
    (
        std::collections::HashMap<usize, String>,
        std::collections::HashMap<usize, Vec<SegmentSpan>>,
    ),
    EngineError,
> {
    let vad_dir = cli::resolve_vad_path(paths)?;
    let requests_path = write_requests(dir, batch, language, options)?;

    // `done` may only ever increase for a given file. A Metal attempt can emit
    // metrics and *then* fail, so without a shared floor the retry would report a
    // lower value and the progress bar would visibly run backwards. One
    // monotonic floor across both attempts makes that impossible.
    let floor = std::sync::atomic::AtomicUsize::new(done_before);
    // Same 3-arg shape as `ProgressFn`, so `run_sequence` stays unchanged.
    let report = |done: usize, total: usize, message: &str| {
        if should_report(&floor, done, total) {
            on_progress(done.min(total), total, message);
        }
    };

    match run_sequence(
        paths,
        dir,
        model_file,
        family,
        &vad_dir,
        &requests_path,
        backend,
        batch,
        weights,
        total_units,
        done_before,
        &report,
    )
    .await
    {
        Ok(results) => Ok(results),
        Err(err @ EngineError::Failed { .. }) if backend == Backend::Metal && is_backend_failure(&err) => {
            tracing::warn!(
                error = %err,
                "metal backend failed; retrying once on cpu"
            );
            // Announce only if the notice itself would not move progress back.
            if floor.load(std::sync::atomic::Ordering::SeqCst) <= done_before {
                on_progress(done_before, total_units, "Metal unavailable, retrying on CPU");
            }
            run_sequence(
                paths,
                dir,
                model_file,
                family,
                &vad_dir,
                &requests_path,
                Backend::Cpu,
                batch,
                weights,
                total_units,
                done_before,
                &report,
            )
            .await
        }
        Err(err) => Err(err),
    }
}

/// Progress weight of one VAD chunk: its length in samples, floored so a
/// zero-length chunk still counts as something.
///
/// Progress is reported in these units rather than in chunk counts because a VAD
/// plan mixes 15 s sentences with sub-second interjections. Counting chunks made
/// the bar advance 1 % for a 0.4 s "嗯" and then sit still for several seconds on
/// a long sentence, i.e. it described the bookkeeping instead of the work.
fn chunk_progress_units(chunk: &vad::VadChunk) -> usize {
    chunk.end_sample.saturating_sub(chunk.start_sample) as usize
}

/// True when `done` advances the monotonic floor, recording the new high-water
/// mark. Extracted so the "progress never regresses" rule is unit-testable
/// without an `AppHandle`.
fn should_report(
    floor: &std::sync::atomic::AtomicUsize,
    done: usize,
    total: usize,
) -> bool {
    let done = done.min(total);
    // `fetch_max` returns the previous value, so this fires only on an increase.
    floor.fetch_max(done, std::sync::atomic::Ordering::SeqCst) < done
}

/// One `--request-sequence` invocation plus its `text_out` files.
#[allow(clippy::too_many_arguments)]
async fn run_sequence(
    paths: &AppPaths,
    dir: &std::path::Path,
    model_file: &std::path::Path,
    family: &str,
    vad_dir: &std::path::Path,
    requests_path: &std::path::Path,
    backend: Backend,
    batch: &[vad::VadChunk],
    weights: &std::collections::HashMap<usize, usize>,
    total_units: usize,
    done_before: usize,
    on_progress: &ProgressFn<'_>,
) -> Result<
    (
        std::collections::HashMap<usize, String>,
        std::collections::HashMap<usize, Vec<SegmentSpan>>,
    ),
    EngineError,
> {
    let out_dir = dir.join("out");
    let text_out = out_dir.join(TEXT_OUT_STEM);
    let segments_out = out_dir.join(SEGMENTS_OUT_STEM);

    let invocation = CliInvocation {
        task: ASR_TASK.to_string(),
        family: family.to_string(),
        model: model_file.to_path_buf(),
        // Sequence mode names every chunk inside requests.json.
        audio: None,
        backend,
        args: asr_args(requests_path, &text_out, &segments_out, vad_dir, family),
    };

    // Ids are `c0000`-style, so the CLI's `safe_output_name` leaves them alone.
    let expected: std::collections::HashMap<String, usize> = batch
        .iter()
        .map(|chunk| (chunk_id(chunk.index), chunk.index))
        .collect();

    let mut reported: std::collections::HashMap<String, PathBuf> = std::collections::HashMap::new();
    let mut metrics_seen = 0usize;
    // Audio length finished inside THIS batch, in the same units as `weights`.
    let mut done_units = 0usize;
    // Seeded from `done_before`, not 0: on a Metal→CPU retry some metrics may
    // already have advanced inside the failed attempt, so starting at 0 would
    // re-emit an already-reported value and let the percentage visibly regress.
    let mut last_reported_done = done_before;

    {
        let mut on_line = |line: &str| {
            // `text_out[<id>]=<path>` — the authoritative output path. It is
            // absolute when `--text-out` was absolute (always the case here),
            // but the CLI echoes whatever we passed, so resolve defensively
            // against the process cwd the child inherited.
            if let Some((id, path)) = parse_text_out(line) {
                reported.insert(id, resolve_reported_path(&path));
                return;
            }
            // `metrics[<id>].…` — one block per finished chunk, streamed live
            // (PoC §4.2), which is what drives progress.
            if let Some(id) = parse_metric_id(line) {
                if let Some(&index) = expected.get(&id) {
                    // Guard against counting a duplicate id twice.
                    if !reported.contains_key(&format!("__metric__{}", index)) {
                        reported.insert(format!("__metric__{}", index), PathBuf::new());
                        metrics_seen += 1;
                        // Each finished chunk advances by its own audio length,
                        // so the bar tracks minutes transcribed rather than
                        // chunks returned. `weights` is keyed by the chunk's own
                        // index, which is what the CLI reports.
                        done_units += weights.get(&index).copied().unwrap_or(0);
                        let done = (done_before + done_units).min(total_units);
                        if done != last_reported_done {
                            last_reported_done = done;
                            on_progress(done, total_units, "Transcribing");
                        }
                    }
                } else {
                    tracing::debug!(line = %line, "metrics line for an unknown request id");
                }
            }
        };

        let output: CliOutput = cli::spawn(Some(paths), invocation, &mut on_line).await?;
        tracing::debug!(
            stdout_lines = output.stdout.lines().count(),
            stderr_tail = %output.stderr_tail,
            "audiocpp_cli sequence finished"
        );
    }

    // Read each chunk's text, preferring the path the CLI printed.
    let mut results = std::collections::HashMap::with_capacity(batch.len());
    let mut missing = Vec::new();
    for chunk in batch {
        let id = chunk_id(chunk.index);
        let path = reported
            .get(&id)
            .cloned()
            .unwrap_or_else(|| default_text_out_path(&text_out, &id));

        match std::fs::read_to_string(&path) {
            Ok(text) => {
                results.insert(chunk.index, text.trim().to_string());
            }
            Err(e) => {
                tracing::warn!(
                    chunk = chunk.index,
                    path = %path.display(),
                    error = %e,
                    "no transcript written for chunk; treating as silent"
                );
                missing.push(chunk.index);
            }
        }
    }

    if missing.len() == batch.len() {
        return Err(EngineError::Protocol(format!(
            "audiocpp_cli reported success but wrote no transcript for any of the {} chunk(s)",
            batch.len()
        )));
    }

    // Real per-cue boundaries, when the model emitted them. Absent for models
    // without segment support (`sense_asr` writes no `segs_*.json` at all), and
    // absence is success: the caller falls back to estimated timing.
    let segments = read_segments(&out_dir, &segments_out, batch);

    Ok((results, segments))
}

/// Read the `--segments-out` documents for one batch.
///
/// Each file is `<stem>_<safe_id>.json`, the same naming rule as `--text-out`.
/// A missing or unparseable file is logged and skipped rather than failing the
/// run: the transcript is already complete without it.
fn read_segments(
    out_dir: &std::path::Path,
    segments_out: &std::path::Path,
    batch: &[vad::VadChunk],
) -> std::collections::HashMap<usize, Vec<SegmentSpan>> {
    let mut segments = std::collections::HashMap::new();

    for chunk in batch {
        let id = chunk_id(chunk.index);
        let path = unsafe_output_name(segments_out, &id);
        let raw = match std::fs::read_to_string(&path) {
            Ok(raw) => raw,
            // The common case for a model without segment support.
            Err(_) => continue,
        };
        match parse_segments(&raw) {
            Ok(spans) if !spans.is_empty() => {
                segments.insert(chunk.index, spans);
            }
            Ok(_) => {}
            Err(error) => tracing::warn!(
                chunk = chunk.index,
                path = %path.display(),
                error = %error,
                "unreadable segments file; estimating this chunk's timing"
            ),
        }
    }

    let _ = out_dir;
    segments
}

/// One segment as the CLI reports it: chunk-relative sample offsets plus text.
///
/// Measured shape (MOSS via `--request-sequence`):
/// `[{"start_sample":0,"end_sample":96320,"confidence":0,"text":"…"}]`.
/// Offsets are **relative to the chunk**, so the caller adds the chunk start.
#[derive(Debug, Clone, PartialEq)]
pub struct SegmentSpan {
    pub start_sample: u64,
    pub end_sample: u64,
    pub text: String,
}

#[derive(Debug, serde::Deserialize)]
struct RawSegmentSpan {
    start_sample: u64,
    end_sample: u64,
    #[serde(default)]
    text: String,
}

/// Parse a `--segments-out` document.
fn parse_segments(raw: &str) -> Result<Vec<SegmentSpan>, String> {
    let raw_spans: Vec<RawSegmentSpan> =
        serde_json::from_str(raw).map_err(|e| format!("parse segments: {}", e))?;
    Ok(raw_spans
        .into_iter()
        .map(|span| SegmentSpan {
            start_sample: span.start_sample,
            end_sample: span.end_sample,
            text: span.text,
        })
        .collect())
}

/// `--request-sequence` arguments. Deliberately contains no `--request-option`
/// (PoC §4.1).
///
/// `family` also namespaces the session option: `sense_asr.vad_model_path` and
/// `qwen3_asr.vad_model_path` are different keys, so the prefix must be the
/// selected model's family too. Only families that actually take a VAD path get
/// the option; the rest would see an unknown key.
fn asr_args(
    requests_path: &std::path::Path,
    text_out: &std::path::Path,
    segments_out: &std::path::Path,
    vad_dir: &std::path::Path,
    family: &str,
) -> Vec<String> {
    let mut args = vec![
        "--request-sequence".to_string(),
        requests_path.to_string_lossy().into_owned(),
        "--text-out".to_string(),
        text_out.to_string_lossy().into_owned(),
        // Asked of every family, not only the ones known to emit it.
        // `--segments-out` is a global CLI flag: a model without segment
        // support accepts it and simply writes no file (measured with
        // `sense_asr`, which produced only `seg_c*.txt`). Passing it
        // conditionally would need a per-family capability table that does not
        // exist, and the read side already treats a missing file as "fall back
        // to estimated timing".
        "--segments-out".to_string(),
        segments_out.to_string_lossy().into_owned(),
    ];

    if family_takes_vad_model_path(family) {
        args.push("--session-option".to_string());
        args.push(format!("{}.vad_model_path={}", family, vad_dir.to_string_lossy()));
    }

    args.push("--threads".to_string());
    args.push(thread_count().to_string());
    args.push("--metrics".to_string());
    args
}

/// Families whose spec declares a `vad_model_path` session option.
///
/// A family without it is not the same as "no VAD": the option is opt-in and
/// passing it under an unknown family/key would be ignored at best. Verified
/// against `audiocpp_cli --list-loaders --json` for v0.9.1.
fn family_takes_vad_model_path(family: &str) -> bool {
    matches!(family, "sense_asr" | "fun_asr_nano")
}

/// Write `requests.json` for one batch.
///
/// Relative `audio` paths resolve against the document's directory (PoC §4.2),
/// so the bare chunk file name is enough — everything lives in `dir`.
///
/// `options` decides which per-request fields may be written. `audiocpp_cli`
/// rejects an unknown one and fails the whole run, and 7 of the 30 ASR families
/// declare neither `language` nor `audio_chunk_mode` (see [`spec`]), so writing
/// them unconditionally made those models unusable.
fn write_requests(
    dir: &std::path::Path,
    batch: &[vad::VadChunk],
    language: &str,
    options: spec::RequestOptions,
) -> Result<PathBuf, EngineError> {
    let requests: Vec<serde_json::Value> = batch
        .iter()
        .map(|chunk| {
            let mut request = serde_json::json!({
                "id": chunk_id(chunk.index),
                "audio": wav::chunk_file_name(chunk.index),
            });

            if options.language {
                request["language"] = serde_json::Value::String(language.to_string());
            }
            if options.audio_chunk_mode {
                request["options"] =
                    serde_json::json!({ "audio_chunk_mode": REQUEST_CHUNK_MODE });
            }

            request
        })
        .collect();

    let document = serde_json::json!({ "requests": requests });
    let path = dir.join("requests.json");
    let body = serde_json::to_string(&document)
        .map_err(|e| EngineError::Protocol(format!("failed to serialize requests.json: {}", e)))?;
    std::fs::write(&path, body)
        .map_err(|e| EngineError::Io(format!("write {}: {}", path.display(), e)))?;
    Ok(path)
}

/// Request id for one chunk; already `safe_output_name`-clean.
fn chunk_id(index: usize) -> String {
    format!("c{:04}", index)
}

/// Fallback path when the CLI did not print `text_out[<id>]`.
///
/// Mirrors `safe_output_name`: `<stem>_<safe_id><ext>`.
fn default_text_out_path(text_out: &std::path::Path, id: &str) -> PathBuf {
    unsafe_output_name(text_out, id)
}

fn unsafe_output_name(text_out: &std::path::Path, id: &str) -> PathBuf {
    let stem = text_out
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "seg".to_string());
    let safe_id: String = id
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    let ext = text_out
        .extension()
        .map(|e| e.to_string_lossy().into_owned())
        .unwrap_or_else(|| "txt".to_string());
    let name = format!("{}_{}.{}", stem, safe_id, ext);
    match text_out.parent() {
        Some(parent) => parent.join(name),
        None => PathBuf::from(name),
    }
}

/// Parse `text_out[<id>]=<path>`.
fn parse_text_out(line: &str) -> Option<(String, String)> {
    let rest = line.strip_prefix("text_out[")?;
    let (id, path) = rest.split_once("]=")?;
    Some((id.to_string(), path.trim().to_string()))
}

/// The CLI echoes the `--text-out` path it was given, so a relative path is
/// relative to the cwd the child inherited from us.
fn resolve_reported_path(path: &str) -> PathBuf {
    let path = PathBuf::from(path);
    if path.is_absolute() {
        return path;
    }
    match std::env::current_dir() {
        Ok(cwd) => cwd.join(path),
        Err(_) => path,
    }
}

/// Parse the request id out of `metrics[<id>].<key>=<value>`.
fn parse_metric_id(line: &str) -> Option<String> {
    let rest = line.strip_prefix("metrics[")?;
    let (id, _) = rest.split_once(']')?;
    Some(id.to_string())
}

/// A `metal` run that failed for backend reasons is worth one CPU retry.
fn is_backend_failure(err: &EngineError) -> bool {
    let EngineError::Failed { stderr_tail, message } = err else {
        return false;
    };
    let haystack = format!("{}\n{}", message, stderr_tail).to_lowercase();
    haystack.contains("metal") || haystack.contains("backend") || haystack.contains("gpu")
}

/// Physical cores, used for `--threads`.
pub(crate) fn thread_count() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
}

/// `temp_dir()/monosub-<task_id>/`, removed on drop for every exit path.
pub(super) struct TempWorkDir {
    path: PathBuf,
}

impl TempWorkDir {
    pub(super) fn create(task_id: &str) -> Result<Self, EngineError> {
        let safe: String = task_id
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                    c
                } else {
                    '_'
                }
            })
            .collect();
        let path = std::env::temp_dir().join(format!("monosub-{}", safe));

        // A leftover directory from a crashed run would poison this one.
        if path.exists() {
            let _ = std::fs::remove_dir_all(&path);
        }
        std::fs::create_dir_all(&path)
            .map_err(|e| EngineError::Io(format!("create {}: {}", path.display(), e)))?;

        Ok(Self { path })
    }

    pub(super) fn path(&self) -> &std::path::Path {
        &self.path
    }
}

impl Drop for TempWorkDir {
    fn drop(&mut self) {
        if let Err(e) = std::fs::remove_dir_all(&self.path) {
            tracing::warn!(
                path = %self.path.display(),
                error = %e,
                "failed to remove the transcription temp directory"
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_text_out_line() {
        assert_eq!(
            parse_text_out("text_out[c0000]=/tmp/monosub-t1/out/seg_c0000.txt"),
            Some((
                "c0000".to_string(),
                "/tmp/monosub-t1/out/seg_c0000.txt".to_string()
            ))
        );
        assert_eq!(parse_text_out("text_output=hello"), None);
        assert_eq!(parse_text_out("metrics[c0000].wall_ms=1.0"), None);
    }

    #[test]
    fn resolves_a_relative_reported_path_against_the_cwd() {
        // Observed live: `text_out[c0000]=out/seg_c0000.txt` when `--text-out`
        // is relative, so the raw string is not always usable as-is.
        let absolute = resolve_reported_path("/tmp/x/out/seg_c0000.txt");
        assert_eq!(absolute, PathBuf::from("/tmp/x/out/seg_c0000.txt"));

        let relative = resolve_reported_path("out/seg_c0000.txt");
        assert!(relative.is_absolute(), "{relative:?}");
        assert!(relative.ends_with("out/seg_c0000.txt"));
        // It must not be interpreted as a bare cwd-relative name.
        assert_ne!(relative, PathBuf::from("out/seg_c0000.txt"));
    }

    #[test]
    fn parses_metric_ids() {
        assert_eq!(
            parse_metric_id("metrics[c0001].wall_ms=62.6556"),
            Some("c0001".to_string())
        );
        assert_eq!(parse_metric_id("metrics[c0001].rtf=0.02"), Some("c0001".to_string()));
        assert_eq!(parse_metric_id("text_out[c0001]=/x"), None);
        assert_eq!(parse_metric_id("audiocpp_cli failed: nope"), None);
    }

    #[test]
    fn chunk_ids_are_padded_and_safe() {
        assert_eq!(chunk_id(0), "c0000");
        assert_eq!(chunk_id(42), "c0042");
        // Already `safe_output_name`-clean, so the CLI will not rewrite it.
        assert!(chunk_id(7)
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'));
    }

    #[test]
    fn default_text_out_path_matches_the_cli_convention() {
        let path = default_text_out_path(std::path::Path::new("/w/out/seg.txt"), "c0000");
        assert_eq!(path, PathBuf::from("/w/out/seg_c0000.txt"));
    }

    /// Measured shape from MOSS through `--request-sequence`.
    #[test]
    fn segments_parse_the_measured_wire_shape() {
        let raw = r#"[{"start_sample":0,"end_sample":96320,"confidence":0,"text":"第一句。"},
                      {"start_sample":100000,"end_sample":150000,"confidence":0,"text":"第二句。"}]"#;
        let spans = parse_segments(raw).expect("parse");
        assert_eq!(spans.len(), 2);
        assert_eq!(spans[0].start_sample, 0);
        assert_eq!(spans[0].end_sample, 96320);
        assert_eq!(spans[0].text, "第一句。");
        assert_eq!(spans[1].start_sample, 100000);
    }

    #[test]
    fn segments_tolerate_a_missing_text_field() {
        // The shape is shared with `--turns-out`, where diar-only models omit
        // `text`; a missing field must not fail the whole chunk.
        let spans = parse_segments(r#"[{"start_sample":10,"end_sample":20}]"#).expect("parse");
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].text, "");
    }

    #[test]
    fn segments_naming_matches_the_text_out_convention() {
        // `<stem>_<safe_id>.json`, exactly as `--text-out` does it.
        let out = std::path::Path::new("/w/out/segs.json");
        assert_eq!(
            unsafe_output_name(out, "c0000"),
            PathBuf::from("/w/out/segs_c0000.json")
        );
    }

    #[test]
    fn a_missing_segments_file_is_not_a_failure() {
        // `sense_asr` writes no segments at all; that must read as "no real
        // timing available", never as an error.
        let dir = std::env::temp_dir().join(format!("monosub-segs-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let batch = vec![vad::VadChunk { index: 0, start_sample: 0, end_sample: 16000 }];
        let segments = read_segments(&dir, &dir.join("segs.json"), &batch);
        assert!(segments.is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn sequence_args_never_pass_request_option() {
        // PoC §4.1: `--request-option` + `--request-sequence` is rejected.
        let args = asr_args(
            std::path::Path::new("/w/requests.json"),
            std::path::Path::new("/w/out/seg.txt"),
            std::path::Path::new("/w/out/segs.json"),
            std::path::Path::new("/res/models/silero_vad"),
            "sense_asr",
        );

        assert!(args.iter().any(|a| a == "--request-sequence"));
        assert!(args.iter().any(|a| a == "--metrics"));
        assert!(
            !args.iter().any(|a| a.starts_with("--request-option")),
            "{args:?}"
        );
        // The VAD path must be spelled out; the CLI ships a relative default.
        assert!(args
            .iter()
            .any(|a| a == "sense_asr.vad_model_path=/res/models/silero_vad"));
    }

    /// Regression: the session-option prefix is family-scoped, so a plan that
    /// selected a different ASR family must not be handed
    /// `sense_asr.vad_model_path`. `qwen3_asr` has no such option at all.
    #[test]
    fn sequence_args_namespace_the_vad_option_by_family() {
        let qwen = asr_args(
            std::path::Path::new("/w/requests.json"),
            std::path::Path::new("/w/out/seg.txt"),
            std::path::Path::new("/w/out/segs.json"),
            std::path::Path::new("/res/models/silero_vad"),
            "qwen3_asr",
        );
        assert!(
            !qwen.iter().any(|a| a.contains("vad_model_path")),
            "qwen3_asr declares no vad_model_path; got {qwen:?}"
        );
        // The sequence/text plumbing must survive regardless of family.
        assert!(qwen.iter().any(|a| a == "--request-sequence"));
        assert!(qwen.iter().any(|a| a == "--metrics"));

        let fun = asr_args(
            std::path::Path::new("/w/requests.json"),
            std::path::Path::new("/w/out/seg.txt"),
            std::path::Path::new("/w/out/segs.json"),
            std::path::Path::new("/res/models/silero_vad"),
            "fun_asr_nano",
        );
        assert!(
            fun.iter()
                .any(|a| a == "fun_asr_nano.vad_model_path=/res/models/silero_vad"),
            "fun_asr_nano does declare it, under its own prefix; got {fun:?}"
        );
        assert!(!fun.iter().any(|a| a.starts_with("sense_asr.")));
    }

    #[test]
    fn requests_json_carries_per_request_options() {
        let dir = std::env::temp_dir().join(format!("monosub-req-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();

        let batch = vec![
            vad::VadChunk { index: 0, start_sample: 0, end_sample: 16000 },
            vad::VadChunk { index: 7, start_sample: 80000, end_sample: 240000 },
        ];
        let path = write_requests(&dir, &batch, "zh", spec::RequestOptions::ALL)
            .expect("write requests.json");
        let written: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();

        let requests = written["requests"].as_array().expect("requests array");
        assert_eq!(requests.len(), 2);
        assert_eq!(requests[0]["id"], "c0000");
        // Relative to requests.json, which lives in `dir`.
        assert_eq!(requests[0]["audio"], "chunk_0000.wav");
        assert_eq!(requests[0]["language"], "zh");
        assert_eq!(requests[0]["options"]["audio_chunk_mode"], "none");
        assert_eq!(requests[1]["id"], "c0007");
        assert_eq!(requests[1]["audio"], "chunk_0007.wav");

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The bug this signature exists to fix: a family that declares neither
    /// option must receive neither, because `audiocpp_cli` fails the whole run
    /// on an unknown one.
    ///
    /// Measured against MOSS:
    /// `audiocpp_cli failed: unknown MOSS-Transcribe-Diarize request option: language`.
    #[test]
    fn requests_json_omits_options_the_family_rejects() {
        let dir = std::env::temp_dir().join(format!("monosub-req-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();

        let batch = vec![vad::VadChunk { index: 0, start_sample: 0, end_sample: 16000 }];
        let path = write_requests(&dir, &batch, "zh", spec::RequestOptions::NONE)
            .expect("write requests.json");
        let written: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();

        let request = &written["requests"][0];
        // The two fields that must survive.
        assert_eq!(request["id"], "c0000");
        assert_eq!(request["audio"], "chunk_0000.wav");
        // The two that must not be present at all — an explicit `null` would
        // still be an unknown option to the CLI.
        assert!(request.get("language").is_none(), "language must be absent: {request}");
        assert!(request.get("options").is_none(), "options must be absent: {request}");

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// One option on, the other off: the two are resolved independently, because
    /// several families declare `audio_chunk_mode` without `language`.
    #[test]
    fn requests_json_trims_the_two_options_independently() {
        let dir = std::env::temp_dir().join(format!("monosub-req-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();

        let batch = vec![vad::VadChunk { index: 0, start_sample: 0, end_sample: 16000 }];
        let options = spec::RequestOptions { language: false, audio_chunk_mode: true };
        let path = write_requests(&dir, &batch, "zh", options).expect("write requests.json");
        let written: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();

        let request = &written["requests"][0];
        assert!(request.get("language").is_none());
        assert_eq!(request["options"]["audio_chunk_mode"], "none");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn backend_failure_detection_matches_metal_errors_only() {
        assert!(is_backend_failure(&EngineError::Failed {
            message: "audiocpp_cli exited with code 1".to_string(),
            stderr_tail: "ggml_metal_init: failed to create command queue".to_string(),
        }));
        assert!(is_backend_failure(&EngineError::Failed {
            message: "unsupported backend".to_string(),
            stderr_tail: String::new(),
        }));
        // An unrelated failure must not trigger the CPU retry.
        assert!(!is_backend_failure(&EngineError::Failed {
            message: "could not open WAV input".to_string(),
            stderr_tail: "could not open WAV input: /tmp/x.wav".to_string(),
        }));
        assert!(!is_backend_failure(&EngineError::Protocol("bad json".to_string())));
    }

    #[test]
    fn temp_work_dir_is_removed_on_drop() {
        let path = {
            let dir = TempWorkDir::create("unit-drop").expect("create");
            std::fs::write(dir.path().join("scratch"), b"x").unwrap();
            assert!(dir.path().is_dir());
            dir.path().to_path_buf()
        };
        assert!(!path.exists(), "temp dir must be gone after drop");
    }

    #[test]
    fn temp_work_dir_replaces_a_stale_directory() {
        let first = TempWorkDir::create("unit-stale").expect("create");
        let path = first.path().to_path_buf();
        std::fs::write(path.join("leftover"), b"x").unwrap();
        // Leak the guard so the directory survives, simulating a crashed run.
        std::mem::forget(first);

        let second = TempWorkDir::create("unit-stale").expect("create over stale dir");
        assert_eq!(second.path(), path);
        assert!(!path.join("leftover").exists(), "stale files must be cleared");
    }
}

#[cfg(test)]
mod progress_monotonicity {
    use super::*;
    use std::sync::atomic::AtomicUsize;

    /// Progress must never regress, including across the Metal→CPU retry where a
    /// failed attempt may already have advanced it (review finding P2-1).
    #[test]
    fn reported_progress_is_strictly_monotonic() {
        let floor = AtomicUsize::new(0);
        // Climbing normally.
        for done in 1..=5 {
            assert!(should_report(&floor, done, 10), "{} should advance", done);
        }
        // The retry re-starts this batch's counter and can report a LOWER value;
        // that must be suppressed.
        assert!(!should_report(&floor, 0, 10));
        assert!(!should_report(&floor, 3, 10));
        assert!(!should_report(&floor, 5, 10), "equal value is not an advance");
        // ...and it must recover once the retry passes the old high-water mark.
        assert!(should_report(&floor, 6, 10));
    }

    /// A file whose chunks span several batches must climb across the boundary and
    /// land exactly on `total`, never exceeding it.
    #[test]
    fn progress_spans_batches_and_clamps_at_total() {
        let total = 300;
        // Batch 1 reports 1..=200.
        let floor = AtomicUsize::new(0);
        let mut emitted = Vec::new();
        for done in 1..=200 {
            if should_report(&floor, done, total) {
                emitted.push(done.min(total));
            }
        }
        assert_eq!(emitted.last(), Some(&200));
        // Batch 2 re-anchors on done_before=200.
        let floor = AtomicUsize::new(200);
        for done in 201..=300 {
            if should_report(&floor, done, total) {
                emitted.push(done.min(total));
            }
        }
        assert_eq!(emitted.first(), Some(&1));
        assert_eq!(emitted.last(), Some(&300), "must reach exactly total");
        assert!(emitted.windows(2).all(|w| w[0] < w[1]), "strictly increasing");
        // An over-count can never exceed `total`.
        assert!(!should_report(&floor, 999, total));
        assert_eq!(floor.load(std::sync::atomic::Ordering::SeqCst), total);
    }

    /// A zero-chunk (all-silence) run must not produce a bogus advance.
    #[test]
    fn zero_total_never_advances() {
        let floor = AtomicUsize::new(0);
        assert!(!should_report(&floor, 0, 0));
        assert!(!should_report(&floor, 5, 0));
    }
}
