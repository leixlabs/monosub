//! Transcription command: WAV extraction → audio.cpp inference → subtitle files.
//!
//! Tasks: P1-15 (rewrite) and P1-16 (cancel). See
//! docs/tech-spec/refactor-audiocpp.md §3, §5.3, §5.7, §5.8.
//!
//! The event contract is frozen (zero frontend change): the three events
//! `transcribe_progress` / `transcribe_complete` / `transcribe_error` carry
//! [`TranscribeProgress`] and the four [`StepInfo`] steps. What used to be the
//! Python stdout protocol (`STEP:`/`ELAPSED:`/`MSG:`/`SUCCESS:`/`ERROR:`,
//! parsed by `process_output_line`) is gone; the values are measured here and
//! the inference itself is delegated to `engine::transcribe`.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Instant, SystemTime};
use tauri::{AppHandle, Emitter};
use tokio::sync::Mutex;

use crate::commands::models::MODEL_NOT_INSTALLED;
use crate::engine::catalog::{self, ModelSpec};
use crate::engine::plan::{self, ModelPlan, StageModels, StepKind};
use crate::engine::{self, Backend, EngineError, TranscribeOpts};
use crate::subtitle::{self, Format, PostProcessConfig};
use crate::translate;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct StepInfo {
    pub current_step: u32,
    pub total_steps: u32,
    pub step_name: String,
    /// Which part of the step is running (`vad` / `asr` / `diar`), or absent
    /// outside the recognition step. The GUI renders it as a pill under the step
    /// badge; a CLI ignores it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub phase: Option<String>,
    pub step_elapsed: f64,
    pub total_elapsed: f64,
}

/// Process-wide limit on how many CUES/GPU stages may run at once.
///
/// This is the number that honours the user's Settings ▸ 并发数. It is a
/// `RwLock<Arc<Semaphore>>` because a semaphore's capacity is fixed at creation:
/// raising it in Settings swaps in a new semaphore; in-flight tasks keep the old
/// one's permit (they are already running and must not be disturbed), and new
/// tasks acquire from the one set since the last change.
static TRANSCRIBE_LIMIT: std::sync::RwLock<Option<Arc<tokio::sync::Semaphore>>> =
    std::sync::RwLock::new(None);

/// The default concurrency, applied until the frontend sends its setting.
const DEFAULT_TRANSCRIPTION_CONCURRENCY: usize = 1;
/// The allowed range, clamped in [`set_transcription_concurrency`].
const MIN_TRANSCRIPTION_CONCURRENCY: usize = 1;
const MAX_TRANSCRIPTION_CONCURRENCY: usize = 3;

/// The current limit, installing the default if nobody has set one yet.
///
/// Self-healing rather than `expect`-ing: `transcribe_file` is reachable from
/// three entry points (the GUI command, the CLI, tests) and a missing install
/// must not turn into a panic mid-run.
fn transcribe_limit() -> Arc<tokio::sync::Semaphore> {
    let mut guard = TRANSCRIBE_LIMIT.write().unwrap_or_else(|poisoned| poisoned.into_inner());
    guard
        .get_or_insert_with(|| {
            Arc::new(tokio::sync::Semaphore::new(DEFAULT_TRANSCRIPTION_CONCURRENCY))
        })
        .clone()
}

/// Acquire a permit from the current transcription-concurrency semaphore.
async fn transcribe_permit() -> tokio::sync::OwnedSemaphorePermit {
    // Read the current shared handle, then acquire on it. A resize swaps the
    // Arc for a new one, so a permit belongs to the semaphore that was in force
    // when the handle was read — which is why acquiring is never over-subscribed:
    // the permit counts against that same semaphore.
    transcribe_limit()
        .acquire_owned()
        .await
        .expect("the semaphore is never closed")
}

/// Set the number of recognition/translation stages that may run concurrently.
///
/// Called from the frontend when 并发数 in Settings changes. Clamped to
/// [`MIN_TRANSCRIPTION_CONCURRENCY`..`MAX_TRANSCRIPTION_CONCURRENCY`] so a
/// hand-edited or stale blob cannot pin zero (which would deadlock the queue) or
/// a figure that guarantees GPU thrash.
///
/// Resizing replaces the semaphore rather than mutating it, because a
/// `tokio::sync::Semaphore`'s capacity is fixed at creation. Stages already
/// running hold permits from the *old* semaphore, so they finish undisturbed and
/// their release cannot free a slot in the new one; only newly spawned tasks see
/// the new figure. That is the semantics a user expects from lowering a limit:
/// "no more than N from now on", not "kill what is running".
#[tauri::command]
pub fn set_transcription_concurrency(capacity: usize) -> Result<(), String> {
    let capacity = capacity.clamp(MIN_TRANSCRIPTION_CONCURRENCY, MAX_TRANSCRIPTION_CONCURRENCY);
    *TRANSCRIBE_LIMIT.write().unwrap() = Some(Arc::new(tokio::sync::Semaphore::new(capacity)));
    tracing::info!(capacity = %capacity, "transcription concurrency set");
    Ok(())
}

/// Install the default before the first task acquires, so [`transcribe_permit`]
/// never sees `None`.
///
/// Public because both entry points must call it: the GUI from `main`, the CLI
/// from its own `main`. Idempotent, so calling it twice costs a write lock.
pub fn ensure_transcription_limit() {
    // One line by design: `transcribe_limit` owns the install, so there is no
    // second copy of the default to drift.
    drop(transcribe_limit());
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TranscribeProgress {
    pub task_id: String,
    /// Progress within the current step, 0…100.
    pub progress: f64,
    /// Progress across the whole run, 0…100 (absent on non-step events).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub overall: Option<f64>,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub step: Option<StepInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_time: Option<f64>,
    /// What the translation stage did, carried on `transcribe_complete` only.
    ///
    /// Added because "translation was on and no translated file appeared" had no
    /// answer anywhere in the UI. The stage can legitimately write nothing — the
    /// source already being the target language is the common case — and the
    /// only trace was a `tracing::info!` in a log file the user is not reading.
    /// A *successful* translation was equally invisible: the row listed the
    /// original subtitle and nothing else.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub translation: Option<TranslationReport>,
    /// `true` on the one event that says "dispatched, but blocked behind the
    /// concurrency limit". Absent on every other event.
    ///
    /// A flag rather than a fourth value of `status`: the frontend has to
    /// *compare* this, and `status` is a free-form string whose other values are
    /// error keys and sentences. `phase` set the precedent — a closed identifier
    /// the UI can branch on, not prose it has to pattern-match.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub waiting: Option<bool>,
}

/// What the translation stage did, as the UI needs to hear it.
///
/// The IPC shape of [`crate::translate::stage::StageOutcome`]: that type is
/// transport-free (it carries `PathBuf`s and a `SkipReason` enum), so the
/// mapping happens here, at the command boundary, rather than teaching the
/// stage about JSON.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TranslationReport {
    /// Files written (`<stem>.<lang>.<ext>`). Empty when nothing was written.
    pub written: Vec<String>,
    /// Why nothing was written, when the stage decided there was nothing to do.
    ///
    /// A skip is **not** a failure: `sameLanguage` means the target already is
    /// the source, and `noSegments` means the input was silent. Both legitimately
    /// produce no file, which is exactly why the row has to say so instead of
    /// leaving the user to wonder where their translation went.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skipped: Option<String>,
    /// Some cues kept their original text (a batch that would not parse).
    pub degraded: bool,
    /// Cues that received translated text.
    ///
    /// `degraded` alone says "some lines are still in the source language" without
    /// saying whether that is one line out of fifty or three hundred out of five
    /// thousand. The row needs the second number to state the shortfall instead of
    /// leaving the user to wonder how much of the file is untranslated.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub translated: Option<usize>,
    /// Cues that kept their original text.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kept_original: Option<usize>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TranscribeRequest {
    pub id: String,
    pub path: String,
    pub output_path: String,
    pub format: Option<String>,
    pub language: Option<String>,
    pub device: Option<String>,
    pub enable_diarization: Option<bool>,
    // --- v3 model-plan requirements (design §9.1). All optional so an older
    // frontend that sends none of them keeps today's behaviour exactly. ---
    /// Word-level timestamps needed.
    pub require_word_timestamps: Option<bool>,
    /// 语种场景: `mandarin` / `mixed` / `multilingual`.
    pub language_profile: Option<String>,
    /// 显存占用: `small` / `medium` / `large`.
    pub memory_budget: Option<String>,
    /// Manual `role -> model_id` overrides.
    pub model_overrides: Option<HashMap<String, String>>,
    // --- translation (translation-design §7). All optional so an older
    // frontend that sends none of them translates nothing, exactly as before. ---
    /// Whether the user turned translation on for this run.
    pub translate_enabled: Option<bool>,
    /// Target language as the frontend spells it (`en`, `zh-tw`, …).
    pub target_language: Option<String>,
    /// Explicit translation-model id; absent means the catalogue default.
    pub translation_model: Option<String>,
}

/// Step names are part of the frozen contract: `FileItem.tsx` derives the i18n
/// key as `steps.<lowercase, spaces → _>`, so the wording must not change.
///
/// `Translating` was APPENDED (translation-design §7). The first four keep their
/// exact spelling: adding a name is a new i18n key, renaming one would silently
/// blank an existing step on every file in every queue.
const STEP_NAMES: [&str; 5] = [
    "Extracting audio",
    "Loading model",
    "Transcribing",
    "Writing subtitle",
    "Translating",
];

/// Steps a run that does not translate emits — the pre-translation contract.
pub const STEPS_WITHOUT_TRANSLATION: u32 = 4;
/// Steps a run that translates emits (translation-design §7).
pub const STEPS_WITH_TRANSLATION: u32 = 5;

/// ffmpeg filter chain of the previous engine, reproduced verbatim.
///
/// The old Python extractor always ran with `enable_preprocessing=True`,
/// `normalize_volume=True`, `noise_reduction=True` — callers never passed the
/// flags — appending in this order: `highpass=f=80`, `lowpass=f=8000`,
/// `afftdn=nf=-25:nr=12:nt=w`, `loudnorm=I=-16:TP=-1.5:LRA=11`. Keeping it
/// keeps the recognition input identical (spec §3; PoC §7 / P0-9).
const AUDIO_FILTER_CHAIN: &str =
    "highpass=f=80,lowpass=f=8000,afftdn=nf=-25:nr=12:nt=w,loudnorm=I=-16:TP=-1.5:LRA=11";

/// One in-flight task: its abort handle plus everything needed to clean up
/// after a cancel (P1-16).
struct TaskEntry {
    handle: tokio::task::JoinHandle<()>,
    /// Every file this run will write, derived from `output_path` + formats.
    outputs: Vec<PathBuf>,
    started: SystemTime,
    /// Set once the run emitted `transcribe_complete`, so a cancel that races
    /// the final write cannot delete a finished result.
    completed: Arc<std::sync::atomic::AtomicBool>,
}

// Global task manager
type TaskMap = Arc<Mutex<HashMap<String, TaskEntry>>>;

lazy_static::lazy_static! {
    static ref TASKS: TaskMap = Arc::new(Mutex::new(HashMap::new()));
    static ref AUDIO_SEMAPHORE: Arc<tokio::sync::Semaphore> = Arc::new(tokio::sync::Semaphore::new(2));
}

/// Everything one queued file needs, so the spawned task owns no borrows.
pub struct TaskParams {
    pub task_id: String,
    pub input: PathBuf,
    pub output: PathBuf,
    pub formats: Vec<Format>,
    pub language: String,
    pub model_id: String,
    /// VAD model id from the plan, when the plan named one (design §8.2.3).
    /// `None` runs the bundled Silero VAD.
    pub vad_model_id: Option<String>,
    pub backend: Backend,
    /// What the user asked for, independent of what the plan can reach.
    ///
    /// Kept separate from `plan.capabilities`: the solver reports an unmet
    /// requirement as a gap but still returns a usable plan, and the engine must
    /// only attempt alignment when it was actually *asked* for it.
    pub want_word_timestamps: bool,
    /// The plan this run must execute (design §7.2 / M8-4).
    pub plan: ModelPlan,
    /// Resolved before anything is spawned, so `transcribe_file` never has to
    /// re-decide whether this run translates (translation-design §6).
    pub translation: Option<TranslationPlan>,
    pub completed: Arc<std::sync::atomic::AtomicBool>,
}

/// The translation stage, decided at request time.
///
/// U1-13: the type and its pre-flight now live in `translate::stage`, because
/// the URL-import subtitle-hit path runs exactly the same stage
/// (url-import-design §5.6). Re-exported here so this module and its tests keep
/// one spelling.
use crate::translate::stage::TranslationPlan;

/// The naming/writing helpers the tests below pin. They live in
/// `translate::stage` now (U1-13) and are re-imported here so the tests keep
/// asserting against the shared implementation rather than a copy.
#[cfg(test)]
use crate::translate::stage::{
    canonical_source_equals_target, translation_output_path, write_translated_formats,
};

/// A failure that has already been turned into user-showable text.
///
/// `status` is what `transcribe_error` carries (short, rendered by the UI);
/// `detail` is the full chain, logged with `tracing::error`.
pub struct TaskFailure {
    pub context: &'static str,
    pub status: String,
    pub detail: String,
}

impl TaskFailure {
    fn new(context: &'static str, status: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            context,
            status: status.into(),
            detail: detail.into(),
        }
    }
}

/// Start a transcription task.
///
/// Request-level preconditions are checked *before* anything is spawned, so a
/// rejected request cannot leave a half-registered task behind.
#[tauri::command]
pub async fn start_transcription(
    app: AppHandle,
    request: TranscribeRequest,
) -> Result<String, String> {
    // The CLI never calls this command, so the default has to be installed here
    // (GUI path) and in `main` (CLI path); both are idempotent.
    ensure_transcription_limit();
    let TranscribeRequest {
        id: task_id,
        path,
        output_path,
        format,
        language,
        device,
        enable_diarization,
        require_word_timestamps,
        language_profile,
        memory_budget,
        model_overrides,
        translate_enabled,
        target_language,
        translation_model,
    } = request;

    let format = format.unwrap_or_else(|| "srt".to_string());
    let language = language.unwrap_or_else(|| "zh".to_string());
    let backend = resolve_backend(device.as_deref());
    let enable_diarization = enable_diarization.unwrap_or(false);

    tracing::info!(
        task_id = %task_id,
        path = %path,
        output_path = %output_path,
        format = %format,
        language = %language,
        backend = ?backend,
        enable_diarization = %enable_diarization,
        "Starting transcription task"
    );

    let formats = match parse_formats(&format) {
        Ok(formats) => formats,
        Err(status) => {
            tracing::warn!(task_id = %task_id, format = %format, "rejected request");
            emit_error(&app, &task_id, &status);
            return Ok(task_id);
        }
    };

    // The plan is **not** solved here. Settings froze it into `settings.json`
    // when the user answered the four questions, and a run executes exactly that
    // plan: re-solving at run time made the model depend on the install state of
    // the moment, so deleting a model silently swapped in a different one
    // instead of telling the user. A missing model is now reported by the
    // install gate below, which is what turns it into a download prompt.
    let paths = match crate::paths::AppPaths::from_app(&app) {        Ok(paths) => paths,
        Err(error) => {
            tracing::error!(task_id = %task_id, error = %error, "failed to resolve app paths");
            emit_error(&app, &task_id, "engineFailed");
            return Ok(task_id);
        }
    };
    let catalogue = match engine::catalog::catalog(&paths) {
        Ok(catalogue) => catalogue,
        Err(error) => {
            tracing::error!(task_id = %task_id, error = %error, "failed to load the model catalogue");
            emit_error(&app, &task_id, "engineFailed");
            return Ok(task_id);
        }
    };
    let installed = |id: &str| engine::catalog::is_installed(&paths, id);

    let saved = match crate::model_dir::load_settings(&app).recognition_plan {
        Some(plan) => plan,
        None => {
            // No saved plan: the user has never opened Settings ▸ 模型. This is
            // not a missing *model*, it is a missing *decision*, so it must not
            // be reported as MODEL_NOT_INSTALLED — the download UI would open
            // for a model nobody chose.
            tracing::warn!(task_id = %task_id, "no saved recognition plan");
            emit_error(&app, &task_id, "modelPlan.notConfigured");
            return Ok(task_id);
        }
    };

    // Manual overrides are part of the saved plan, not a run-time argument: they
    // are applied once, in Settings, and frozen with everything else. A request
    // that still carries them is logged so a stale frontend is visible rather
    // than silently ignored.
    if model_overrides.as_ref().is_some_and(|map| !map.is_empty()) {
        tracing::warn!(
            task_id = %task_id,
            "the request carries model overrides, which the saved plan already includes; ignoring them"
        );
    }

    // The four plan answers are the same kind of thing: they were answered in
    // Settings and frozen with the plan, so a request carrying different values
    // cannot re-solve anything. Logging the disagreement is the whole value of
    // reading them here — a silent ignore would make a stale frontend
    // undiagnosable.
    let asked_differently = require_word_timestamps.unwrap_or(false) != saved.require_word_timestamps
        || language_profile
            .as_deref()
            .is_some_and(|value| !value.eq_ignore_ascii_case(&saved.language_profile))
        || memory_budget
            .as_deref()
            .is_some_and(|value| !value.eq_ignore_ascii_case(&saved.memory_budget));
    if asked_differently {
        tracing::warn!(
            task_id = %task_id,
            "the request's plan answers differ from the saved plan; the saved plan is what runs"
        );
    }

    let mut plan = match plan::thaw(&saved, &catalogue) {
        Ok(Some(plan)) => plan,
        Ok(None) => {
            // The gap plan was frozen, meaning Settings itself found no model for
            // the request. There is no step to run, so the only useful answer is
            // the download prompt — and it needs a model id to open on. The
            // record does not store one (a gap plan has no steps), so this is the
            // plain sentinel: the row then offers the model list rather than
            // guessing which model the user should have.
            tracing::warn!(task_id = %task_id, "the saved plan has no steps");
            return Err(MODEL_NOT_INSTALLED.to_string());
        }
        Err(detail) => {
            tracing::error!(task_id = %task_id, detail = %detail, "the saved plan cannot be read");
            emit_error(&app, &task_id, "modelPlan.unreadable");
            return Ok(task_id);
        }
    };

    // Recompute the derived fields the runtime reads (`capabilities.timestamps`
    // decides the word-timestamp path) from the catalogue, so a frozen step list
    // and a changed catalogue cannot disagree about what a step can do.
    plan::refresh_from_catalogue(&mut plan, &catalogue);

    // Per-file speaker separation is a *subset* of the frozen plan, not a second
    // solve: turning it off for one file drops the diarization steps and runs the
    // ASR alone. Turning it **on** for a file whose plan has no diar step cannot
    // invent a model — that would be exactly the run-time re-solve this design
    // removed — so it is logged and the file runs without speaker labels.
    if !enable_diarization && plan.steps.iter().any(|step| step.kind == StepKind::Diarize) {
        // Name the model being dropped, not just that something was: this line is
        // read next to `model plan selected`, and without it the two look
        // contradictory — `enable_diarization=false` above a plan whose `diar`
        // field still named a model.
        let dropped = plan
            .stage_models()
            .diar
            .unwrap_or_else(|| StageModels::ABSENT.to_string());
        tracing::info!(
            task_id = %task_id,
            dropped_diar = %dropped,
            "this file has speaker separation off; running the plan without its diarization steps"
        );
        plan.steps.retain(|step| {
            !matches!(step.kind, StepKind::Diarize | StepKind::TurnAttribution)
        });
        plan::refresh_from_catalogue(&mut plan, &catalogue);
    } else if enable_diarization
        && !plan.steps.iter().any(|step| step.kind == StepKind::Diarize)
    {
        let stages = plan.stage_models();
        tracing::warn!(
            task_id = %task_id,
            vad = %stages.vad,
            asr = stages.asr.as_deref().unwrap_or(StageModels::ABSENT),
            diar = stages.diar.as_deref().unwrap_or(StageModels::ABSENT),
            "speaker separation is on for this file, but the saved plan has no diarization step; \
             run without speaker labels (re-save the plan in Settings ▸ 模型 to add one)"
        );
    }

    if plan.steps.is_empty() {
        tracing::warn!(task_id = %task_id, "the saved plan has no usable steps");
        return Err(MODEL_NOT_INSTALLED.to_string());
    }

    // Every model in the saved plan must be present. A model deleted since the
    // plan was saved is reported here — never replaced by another one.
    for model_id in &plan.model_ids {
        // A model this version retired (`status: unsupported`) is still in the
        // catalogue for attribution, and may still be on disk from an older
        // install. Running it is not the answer: v1 diarization silently drops
        // speaker labels past 120 s, and `download_model` would refuse to
        // replace it. The plan is stale, so say that instead of starting.
        if catalogue
            .iter()
            .any(|spec| spec.id == *model_id && !spec.status.is_offered())
        {
            tracing::warn!(
                task_id = %task_id,
                model = %model_id,
                "the saved plan names a model this version does not support"
            );
            emit_error(&app, &task_id, "modelPlan.unsupportedModel");
            return Ok(task_id);
        }
        if !installed(model_id) {
            tracing::warn!(
                task_id = %task_id,
                model = %model_id,
                plan = %plan.id,
                "the saved plan needs a model that is not installed"
            );
            return Err(format!("{}:{}", MODEL_NOT_INSTALLED, model_id));
        }
    }

    let primary_model_id = plan
        .model_ids
        .first()
        .cloned()
        .unwrap_or_else(|| engine::catalog::default_model_id().to_string());

    // A plan may pin a `vad`-role model (design §8.2.3). The solver's templates
    // never emit a Vad step — Silero always runs inside the sequence pipeline —
    // so this is either a manual override or an explicit catalogue pick. When it
    // is absent the bundled Silero VAD stays in charge.
    let vad_model_id = plan
        .steps
        .iter()
        .find(|step| step.kind == StepKind::Vad && !step.model_id.is_empty())
        .map(|step| step.model_id.clone());

    // `plan` is the shape key (`asr+diar+turns:nemotron`), which names the step
    // shape plus the one model that makes it distinctive — so on its own it
    // reads as "nemotron transcribes", when nemotron only supplies the speaker
    // turns. The three stage fields are the unambiguous answer to the question
    // a reader actually has: which model runs VAD, which transcribes, which
    // separates speakers.
    let stages = plan.stage_models();
    tracing::info!(
        task_id = %task_id,
        plan = %plan.id,
        vad = %stages.vad,
        asr = stages.asr.as_deref().unwrap_or(StageModels::ABSENT),
        diar = stages.diar.as_deref().unwrap_or(StageModels::ABSENT),
        "model plan selected"
    );

    let input = PathBuf::from(path);
    let output = PathBuf::from(output_path);

    // Translation pre-flight (T4-3, translation-design §6): when translation is
    // on AND it will actually run, the chosen model must be in the catalogue and
    // installed BEFORE the ASR run starts. Otherwise the user would wait out a
    // whole transcription and only then be told to download a model.
    //
    // The source language is the request's own `auto`-able value here: an `auto`
    // source always translates (§5), so nothing is skipped on a guess.
    let translation = match resolve_translation(
        &catalogue,
        &language,
        translate_enabled,
        target_language.as_deref(),
        translation_model.as_deref(),
        &installed,
        &|id: &str| engine::catalog::model_path(&paths, id).ok().flatten(),
    ) {
        Ok(translation) => translation,
        Err(status) => {
            tracing::warn!(task_id = %task_id, status = %status, "translation pre-flight rejected the request");
            return Err(status);
        }
    };

    let outputs = derive_output_paths(
        &output,
        &formats,
        translation.as_ref().map(|plan| plan.target.as_str()),
    );

    {
        let mut tasks = TASKS.lock().await;
        if let Some(existing) = tasks.get(&task_id) {
            if !existing.handle.is_finished() {
                tracing::warn!(task_id = %task_id, "Task already running");
                return Err("Task already running".to_string());
            }
            // A task that finished before the parent registered its handle
            // would otherwise wedge this id forever.
            tasks.remove(&task_id);
        }
    }

    let params = TaskParams {
        task_id: task_id.clone(),
        input,
        output,
        formats,
        language,
        model_id: primary_model_id,
        vad_model_id,
        backend,
        // What the user asked for when the plan was saved, not what this request
        // carries: the plan is frozen, so its own answer is the one that was
        // solved against. Kept as a separate field because the engine must only
        // align when it was *asked* for it, and a plan can outlive the request
        // that produced it.
        want_word_timestamps: saved.require_word_timestamps,
        plan,
        translation,
        completed: Arc::new(std::sync::atomic::AtomicBool::new(false)),
    };
    let app_for_task = app.clone();
    let completed = Arc::clone(&params.completed);
    let handle = tokio::spawn(async move { run_transcription(app_for_task, params).await });

    TASKS.lock().await.insert(
        task_id.clone(),
        TaskEntry {
            handle,
            outputs,
            started: SystemTime::now(),
            completed,
        },
    );

    Ok(task_id)
}

/// Cancel a running task.
///
/// `engine::cli::spawn` marks the CLI child `kill_on_drop(true)` and
/// `engine::transcribe` owns a `Drop` guard for its temp directory, so aborting
/// the tokio task is sufficient to kill the child process and remove the temp
/// directory — the future is dropped, which drops both guards.
#[tauri::command]
pub async fn cancel_transcription(task_id: String) -> Result<(), String> {
    tracing::info!(task_id = %task_id, "Cancelling transcription task");

    let entry = TASKS.lock().await.remove(&task_id);
    let Some(entry) = entry else {
        tracing::warn!(task_id = %task_id, "Cannot cancel: task not found");
        return Err("Task not found".to_string());
    };

    entry.handle.abort();
    // Awaiting the aborted handle makes the cleanup below deterministic: the
    // future has been dropped, so the CLI child is dead and no writer can still
    // be mid-write.
    let _ = entry.handle.await;

    if entry.completed.load(std::sync::atomic::Ordering::SeqCst) {
        // The run finished while the cancel was in flight; its output is whole.
        tracing::info!(task_id = %task_id, "Task completed before the cancel arrived");
    } else {
        remove_partial_outputs(&entry.outputs, entry.started);
    }

    tracing::info!(task_id = %task_id, "Transcription task cancelled");
    Ok(())
}

/// Resolve the translation stage for a request, or reject it clearly.
///
/// U1-13 moved the gate itself into `translate::stage::resolve`, so the URL
/// import path runs byte-identical logic. This wrapper keeps the local signature
/// (and therefore every existing test) intact.
fn resolve_translation(
    catalogue: &[ModelSpec],
    source_language: &str,
    enabled: Option<bool>,
    target: Option<&str>,
    model_id: Option<&str>,
    installed: &dyn Fn(&str) -> bool,
    model_file: &dyn Fn(&str) -> Option<PathBuf>,
) -> Result<Option<TranslationPlan>, String> {
    crate::translate::stage::resolve(
        catalogue,
        source_language,
        enabled,
        target,
        model_id,
        installed,
        model_file,
    )
}

/// Task body: run one file end to end, then report a single outcome.
async fn run_transcription(app: AppHandle, params: TaskParams) {
    // Resolved once per task: every stage below resolves binaries, the
    // catalogue and the models root from it.
    let paths = match crate::paths::AppPaths::from_app(&app) {
        Ok(paths) => paths,
        Err(error) => {
            emit_error(&app, &params.task_id, "engineFailed");
            tracing::error!(task_id = %params.task_id, error = %error, "failed to resolve app paths");
            return;
        }
    };

    let mut sink = EventSink { app: &app };

    if let Err(failure) = transcribe_file(&paths, &params, &mut sink).await {
        tracing::error!(
            task_id = %params.task_id,
            detail = %failure.detail,
            "{}",
            failure.context
        );
        emit_error(&app, &params.task_id, &failure.status);
    }

    // Only unregister ourselves: a cancel followed by an immediate restart of
    // the same id leaves a newer entry in the map, which must survive.
    let mut tasks = TASKS.lock().await;
    let ours = tasks
        .get(&params.task_id)
        .is_some_and(|entry| Arc::ptr_eq(&entry.completed, &params.completed));
    if ours {
        tasks.remove(&params.task_id);
    }
}

/// Run one file end to end: audio → recognition → post-process → write →
/// (optional) translation.
///
/// Deliberately takes a [`ProgressSink`] rather than an `AppHandle`: this is the
/// whole transcription pipeline minus the transport, which is what lets the CLI
/// reuse it verbatim (`docs/tech-spec/cli-design.md` §3.3). The GUI passes
/// [`EventSink`]; a CLI passes a printer.
pub async fn transcribe_file(
    paths: &crate::paths::AppPaths,
    params: &TaskParams,
    sink: &mut dyn ProgressSink,
) -> Result<(), TaskFailure> {
    let mut steps = Steps::new(
        params.translation.is_some(),
        params.plan.steps.iter().any(|step| step.kind == StepKind::Diarize),
    );
    // Tracks whether the translation stage actually produced files, so the CLI
    // can report "subtitles written but translation failed" separately from a
    // clean success (docs/tech-spec/cli-design.md §5.3).
    let mut translation_written = true;
    sink.queued(&params.task_id);

    // Step 1: 16 kHz mono s16le WAV, cached next to the media file. Audio
    // inputs are converted too — audio.cpp only accepts WAV (spec §3).
    steps.begin(sink, &params.task_id, 1);
    let audio = {
        let _permit = AUDIO_SEMAPHORE.acquire().await;
        extract_wav(paths, &params.input)
            .await
            .map_err(engine_failure)?
    };

    // Steps 2/3: the CLI loads the model inside the ASR session, so "Loading
    // model" is a transition only; the work happens under "Transcribing".
    //
    // `words` is only ever non-empty on the aligned path, where cue timing comes
    // from the words rather than from their chunk window. Declared here and
    // filled inside the block so the post-process call below can borrow it.
    #[allow(unused_assignments)]
    let mut words: Vec<engine::words::Word> = Vec::new();
    // The diarizer's turns, kept past the recognition block on purpose. Chunk
    // labels are a coarse answer — one VAD window is up to 15 s and can span a
    // speaker change — so the finished cues are re-attributed against these
    // (see `diar::attribute_cues`). Empty when the plan has no diarization step.
    let mut diar_turns: Vec<engine::diar::DiarTurn> = Vec::new();
    let chunks = {
        // Report the block *before* waiting for it, so a task parked behind the
        // concurrency limit says so instead of showing the previous step's
        // "启动中…". `acquire` is FIFO, so the row flips to step 2 the moment a
        // slot frees up — the two events are never out of order for one task.
        if transcribe_limit().available_permits() == 0 {
            sink.waiting(&params.task_id);
        }
        let _permit = transcribe_permit().await;
        steps.begin(sink, &params.task_id, 2);
        steps.begin(sink, &params.task_id, 3);

        let opts = TranscribeOpts {
            language: params.language.clone(),
            model_id: Some(params.model_id.clone()),
            backend: params.backend,
            vad_model_id: params.vad_model_id.clone(),
            word_timestamps: params.want_word_timestamps,
        };
        // The engine's `ProgressFn` is `Fn` (shared), while the sink is `&mut`,
        // so the mutable borrow lives behind a mutex. Reports are sequential, so
        // the lock is uncontended.
        let progress_sink = std::sync::Mutex::new(&mut *sink);
        let on_progress = |done: usize, total: usize, phase: engine::ProgressPhase| {
            let percent = if total == 0 {
                0.0
            } else {
                done as f64 * 100.0 / total as f64
            };
            if let Ok(mut sink) = progress_sink.lock() {
                steps.progress(*sink, &params.task_id, percent, Some(phase));
            }
        };

        // Every model runs the same VAD → slice → `--request-sequence` pipeline,
        // then optionally diarization + turn attribution. There is no one-shot
        // path: a model that emits text + timestamps + speakers in a single call
        // (MOSS) is still fed per VAD chunk, which is what keeps a multi-hour
        // file inside a fixed decode budget
        // (docs/tech-spec/audiocpp-moss-poc.md §5).
        {
            let transcription =
                engine::transcribe_words(paths, &params.task_id, audio.path(), &opts, &on_progress)
                    .await
                    .map_err(engine_failure)?;
            let chunks = transcription.chunks;
            words = transcription.words;

            // A composed plan needs a diar model + turn attribution (M6).
            if params
                .plan
                .steps
                .iter()
                .any(|step| step.kind == StepKind::Diarize)
            {
                let diar_id = params
                    .plan
                    .steps
                    .iter()
                    .find(|step| step.kind == StepKind::Diarize)
                    .map(|step| step.model_id.clone());
                match diar_id {
                    Some(diar_id) => {
                        // Scratch file beside the other temp artefacts, never
                        // next to the user's media file.
                        let turns_path = std::env::temp_dir().join(format!(
                            "monosub-diar-turns-{}.json",
                            params.task_id
                        ));
                        // The recognition stage already ran VAD, so the speech
                        // spans are free: they let the diar windows be cut in
                        // silence instead of on a blind time grid. They are *not*
                        // the unit of work — see `diar::detect_turns`.
                        let speech: Vec<(f64, f64)> = chunks
                            .iter()
                            .map(|chunk| (chunk.start_s, chunk.end_s))
                            .collect();
                        // The user's per-model choice, re-validated against the
                        // catalogue: `chosen_mode` answers the model's default
                        // (streaming wherever the app can drive it) when nothing
                        // is stored, and refuses a stored `streaming` for a
                        // model that cannot run it.
                        //
                        // A catalogue miss leaves the mode at the offline
                        // default; `detect_turns` then reports the missing model
                        // itself, so this must not add a second failure path.
                        let diar_mode = catalog::catalog(paths)
                            .ok()
                            .and_then(|specs| specs.into_iter().find(|spec| spec.id == diar_id))
                            .map(|spec| {
                                crate::model_dir::chosen_mode(
                                    &crate::model_dir::read_settings_at(&paths.settings_path()),
                                    &spec,
                                )
                            })
                            .unwrap_or(catalog::RunMode::Offline);
                        match engine::diar::detect_turns(
                            paths,
                            audio.path(),
                            &diar_id,
                            params.backend,
                            &turns_path,
                            &speech,
                            diar_mode,
                            &on_progress,
                        )
                        .await
                        {
                            Ok(turns) if !turns.is_empty() => {
                                let attributed = engine::diar::attribute_turns(&chunks, &turns);
                                // The turns outlive this block: step 4 re-checks
                                // every cue against them, because one chunk can
                                // hold more than one speaker.
                                diar_turns = turns;
                                let _ = std::fs::remove_file(&turns_path);
                                attributed
                            }
                            Ok(_) => {
                                tracing::info!(task_id = %params.task_id, "diarization found no turns");
                                let _ = std::fs::remove_file(&turns_path);
                                chunks
                            }
                            // Diarization is an enhancement: if it fails, the
                            // transcript is still complete and should ship.
                            Err(error) => {
                                tracing::warn!(
                                    task_id = %params.task_id,
                                    error = %error,
                                    "diarization failed; writing the unsplit transcript"
                                );
                                let _ = std::fs::remove_file(&turns_path);
                                chunks
                            }
                        }
                    }
                    None => chunks,
                }
            } else {
                chunks
            }
        }
    };

    // Step 4: post-process and write every requested format.
    steps.begin(sink, &params.task_id, 4);
    if let Some(parent) = params.output.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).map_err(|e| {
                TaskFailure::new(
                    "could not create the output directory",
                    "Could not create the output folder",
                    format!("create {}: {}", parent.display(), e),
                )
            })?;
        }
    }

    // An empty chunk list means "no speech": still write valid empty files.
    //
    // Cue timing comes from `words` when the run aligned them, otherwise from the
    // chunk windows; `subtitle::postprocess` owns that choice. The diarizer's
    // turns go in with them so speaker labels are decided per **cue**, not per
    // VAD chunk — see `diar::attribute_cues` for why a chunk is too coarse.
    let segments = subtitle::postprocess(
        &chunks,
        &words,
        &diar_turns,
        &PostProcessConfig::default(),
    );
    for (index, format) in params.formats.iter().enumerate() {
        let path = subtitle::output_path_for(&params.output, index, *format);
        subtitle::write(&segments, &path, *format).map_err(write_failure)?;
        tracing::info!(
            task_id = %params.task_id,
            path = %path.display(),
            format = %format.extension(),
            "Subtitle written"
        );
    }

    // The word-level sidecar is written from the same words the cues were built
    // from, so an agent cutting the video sees exactly the timing the subtitles
    // claim. It is additive like the translated files: a run that produced no
    // words writes none (see `write_word_sidecar`).
    if let Err(error) = write_word_sidecar(&params.output, &words) {
        // Never fatal: the subtitles are already on disk and the sidecar is an
        // optional artefact for machine consumers.
        tracing::warn!(
            task_id = %params.task_id,
            error = %error,
            "could not write the word-level sidecar"
        );
    }

    // Step 5 is the enhancement, never a precondition: the originals are on disk
    // before it starts, so any failure here still delivers them (design §2).
    //
    // The concurrency semaphore is held across BOTH the recognition and the
    // translation stages (T4-7): llama.cpp and audio.cpp both want the GPU, and
    // one permit per file is what stops two queued files from running one of
    // each at the same time. Its capacity is the user's 并发数 setting
    // (`set_transcription_concurrency`).
    let mut translation_report: Option<TranslationReport> = None;
    if let Some(translation) = params.translation.as_ref() {
        // Same report as above: with concurrency 1, a translating file waits here
        // for whichever run currently holds the GPU.
        if transcribe_limit().available_permits() == 0 {
            sink.waiting(&params.task_id);
        }
        let _permit = transcribe_permit().await;
        steps.begin(sink, &params.task_id, 5);
        // The detected source is resolved from the RAW chunks (`T4-1` reads the
        // ASR's `<|zh|>` tag before cleaning), so `auto` is answered by the ASR
        // rather than by a guess (translation-design §5).
        let detected = detected_language(&chunks, &params.language);
        let translated = translate_and_write(
            paths,
            params,
            translation,
            &segments,
            detected.as_deref(),
            &steps,
            sink,
        )
        .await;
        // Exit 5 means "attempted and produced nothing". A *skip* is a success:
        // a silent input, or a source that is already the target language, both
        // legitimately write no translated file. Collapsing the two made
        // `--translate` on a silent clip report a failure.
        translation_written = translated.skipped.is_some() || !translated.written.is_empty();
        translation_report = Some(TranslationReport {
            written: translated
                .written
                .iter()
                .map(|path| path.display().to_string())
                .collect(),
            skipped: translated.skipped.map(|reason| reason.as_str().to_string()),
            degraded: translated.degraded,
            translated: Some(translated.translated),
            kept_original: Some(translated.kept_original),
        });
    }

    // Exactly one completion, after every format landed.
    params
        .completed
        .store(true, std::sync::atomic::Ordering::SeqCst);
    sink.finished(
        &params.task_id,
        steps.total_seconds(),
        translation_report.as_ref(),
    );

    if !translation_written {
        return Err(TaskFailure::new(
            "translation produced no files",
            "TRANSLATE_FAILED",
            "the subtitles were written, but nothing was translated",
        ));
    }
    Ok(())
}

/// Translate the finished cues and write one file per requested format.
///
/// A failure here is logged and swallowed: the originals are already written and
/// the task completes normally (design §2). The work itself is the shared
/// `translate::stage` function, which the URL import subtitle-hit path calls with
/// exactly the same arguments (U1-13, url-import-design §5.6).
///
/// Private on purpose: this is the GUI/CLI *wiring* around that shared stage
/// (it owns `Steps`, which is a rendering detail of this module), and the only
/// caller is the translating branch of `transcribe_file` below. It was made
/// `pub` during the `monosub-cli` split on the assumption that the CLI would
/// call it directly; the CLI calls `transcribe_file` instead and the URL import
/// calls `translate::stage` itself, so the visibility only produced
/// `private_interfaces` (a `pub` fn taking a private `Steps`).
async fn translate_and_write(
    paths: &crate::paths::AppPaths,
    params: &TaskParams,
    translation: &TranslationPlan,
    segments: &[subtitle::Segment],
    detected: Option<&str>,
    steps: &Steps,
    sink: &mut dyn ProgressSink,
) -> crate::translate::stage::StageOutcome {
    // `Mutex` because the callback handed to the engine must be `Sync`; there is
    // only ever one writer (the translation stage reports sequentially).
    let sink = std::sync::Mutex::new(sink);
    let on_progress = move |done: usize, total: usize| {
        let percent = if total == 0 {
            0.0
        } else {
            done as f64 * 100.0 / total as f64
        };
        if let Ok(mut sink) = sink.lock() {
            steps.progress(*sink, &params.task_id, percent, None);
        }
    };

    let outcome = crate::translate::stage::translate_and_write(
        &params.output,
        &params.formats,
        translation,
        segments,
        detected,
        paths,
        on_progress,
    )
    .await;

    tracing::debug!(
        task_id = %params.task_id,
        written = outcome.written.len(),
        degraded = outcome.degraded,
        "translation stage finished"
    );

    outcome
}

/// The language a run actually recognised.
///
/// Preference order (translation-design §5):
///
/// 1. The dominant tag the ASR itself emitted (`<|zh|>`), folded across the
///    chunks — the only source that is measured rather than inferred. It has to
///    be read from `RawChunk::language`, which `T4-1` populated before
///    `subtitle::postprocess::clean_text` deleted the tags; the cleaned segments
///    no longer carry them.
/// 2. The requested language, when it is not `auto`, so an explicit choice wins
///    over the absence of tags (MOSS emits none).
/// 3. A script census over the transcript (§5's "简单的文字脚本统计兜底"). This
///    is the only case the census exists for: `auto` was requested AND the model
///    gave no tag, which is exactly the MOSS path.
/// 4. `None` — genuinely unknown. The caller then translates rather than skips.
fn detected_language(chunks: &[engine::RawChunk], requested: &str) -> Option<String> {
    let tags = chunks.iter().map(|chunk| chunk.language.as_deref());
    if let Some(tag) = crate::subtitle::postprocess::dominant_language(tags) {
        return Some(tag);
    }
    if !translate::language::is_unknown_language(requested) {
        return Some(requested.to_string());
    }
    translate::language::detect_by_script(chunks.iter().map(|chunk| chunk.text.as_str()))
}

/// Emits the exported step transitions with real elapsed seconds.
///
/// The previous implementation fed `step_elapsed`/`total_elapsed` from the
/// Python `STEP_ELAPSED:`/`ELAPSED:` protocol; they are measured here directly.
struct Steps {
    start: Instant,
    step_start: Instant,
    current: u32,
    /// How many steps THIS run emits: 4 normally, 5 when it translates. Carried
    /// per run rather than read from a constant, because the frontend renders
    /// `current/total` and a fixed 4 would show "5/4" on a translating run
    /// (translation-design §7).
    total: u32,
    /// True when the plan runs diarization after recognition, which is the only
    /// thing that can follow the ASR phase inside the same step.
    diarizing: bool,
}

/// How the bar's 100 % is shared between the steps.
///
/// Equal weights were wrong for the two cheap steps: extracting a WAV and
/// starting the model take a couple of seconds, yet each was worth a quarter of
/// the run, so the bar sat at 0 % and then jumped to 25 % before any recognition
/// had happened, and the last quarter arrived all at once while writing.
///
/// The numbers below are the measured shape of a normal run: transcription
/// dominates, extraction and writing are near-instant, translation is a real
/// stage but shorter than recognition. They are a *rendering* choice — the run
/// reaches 100 % exactly when the last step does, whatever they are — so a
/// machine that extracts unusually slowly is slow in the bar, not wrong about it.
///
/// A translating run renormalises these over 5 steps; see [`Steps::weights`].
const STEP_WEIGHTS_WITHOUT_TRANSLATION: [f64; 4] = [
    0.02, // 1. Extracting audio
    0.03, // 2. Loading model
    0.85, // 3. Transcribing (VAD + ASR + diarization)
    0.10, // 4. Writing subtitle
];

const STEP_WEIGHTS_WITH_TRANSLATION: [f64; 5] = [
    0.02, // 1. Extracting audio
    0.03, // 2. Loading model
    0.70, // 3. Transcribing
    0.10, // 4. Writing subtitle
    0.15, // 5. Translating
];

/// How the recognition step's own 100 % splits between its phases.
///
/// The phases run one after another inside one step, and each reports its own
/// 0…100, so they have to be stitched or the badge's number runs backwards
/// (ASR finishes at 100 %, then diarization starts at 0 %).
///
/// VAD is near-instant and decides everything downstream; the ASR is the bulk of
/// the wall time; diarization is one whole-file CLI call and costs a noticeable
/// fraction when the plan asks for it. Without diarization the ASR simply owns
/// the rest.
const PHASE_WEIGHTS_NO_DIAR: [f64; 2] = [0.05, 0.95];
const PHASE_WEIGHTS_WITH_DIAR: [f64; 3] = [0.05, 0.65, 0.30];

/// Where `phase` at `fraction` % sits within the recognition step, 0…100.
///
/// A pure function so the mapping is testable without a sink, and so the
/// monotonicity it owes the user is pinned by a test rather than by inspection.
fn phase_progress_within_step(
    phase: engine::ProgressPhase,
    fraction: f64,
    diarizing: bool,
) -> f64 {
    use engine::ProgressPhase as P;
    let fraction = (fraction / 100.0).clamp(0.0, 1.0);

    // Index and weight of the running phase, in pipeline order.
    let (index, weights): (usize, &[f64]) = match (phase, diarizing) {
        (P::Vad, false) => (0, &PHASE_WEIGHTS_NO_DIAR),
        (P::Asr, false) => (1, &PHASE_WEIGHTS_NO_DIAR),
        (P::Vad, true) => (0, &PHASE_WEIGHTS_WITH_DIAR),
        (P::Asr, true) => (1, &PHASE_WEIGHTS_WITH_DIAR),
        (P::Diar, true) => (2, &PHASE_WEIGHTS_WITH_DIAR),
        // A diar phase on a plan that did not ask for one cannot happen; if it
        // does, treat it as the tail of the step rather than panic.
        (P::Diar, false) => (1, &PHASE_WEIGHTS_NO_DIAR),
    };

    let finished: f64 = weights.iter().take(index).sum();
    let own = weights.get(index).copied().unwrap_or(0.0);
    ((finished + own * fraction) * 100.0).clamp(0.0, 100.0)
}

impl Steps {
    /// `translating` decides the total up front, so the very first step event
    /// already carries the right denominator.
    ///
    /// `diarizing` is only consulted to split the recognition step's own
    /// percentage between its phases, so a plan without speaker separation never
    /// shows a diarization slice it will not use.
    fn new(translating: bool, diarizing: bool) -> Self {
        let now = Instant::now();
        Self {
            start: now,
            step_start: now,
            current: 0,
            total: if translating {
                STEPS_WITH_TRANSLATION
            } else {
                STEPS_WITHOUT_TRANSLATION
            },
            diarizing,
        }
    }

    /// Enter `step` (1..=5) and publish its transition.
    ///
    /// Takes a sink rather than an `AppHandle`: the transcription orchestration
    /// has to run outside a Tauri process for the CLI, so "who receives the
    /// progress" is the caller's business (`emit_progress` below is the GUI's).
    fn begin(&mut self, sink: &mut dyn ProgressSink, task_id: &str, step: u32) {
        self.current = step;
        self.step_start = Instant::now();
        self.emit(sink, task_id, 0.0, step_name(step));
    }

    /// In-step update; `phase` names which part of the step is running.
    ///
    /// Takes `&self` because the engine's `ProgressFn` is a shared closure
    /// (`Fn`, not `FnMut`): the phase travels on the snapshot rather than being
    /// remembered here, which keeps that closure callable from several places
    /// in the pipeline.
    ///
    /// `progress` is the phase's own fraction and is mapped onto the *step's*
    /// share, because the recognition step runs several phases one after another
    /// and each restarts at 0. Reporting the raw figure would show the badge
    /// going 100 % (ASR finished) → 0 % (diarization starting) → 100 %, i.e. a
    /// number that runs backwards inside one step. The mapping is what makes
    /// "· 42 %" mean "42 % of this step", whatever phase is running.
    ///
    /// `phase` is `None` for a step that has no sub-phases of its own —
    /// translation reports its own percentage and nothing finer.
    fn progress(
        &self,
        sink: &mut dyn ProgressSink,
        task_id: &str,
        progress: f64,
        phase: Option<engine::ProgressPhase>,
    ) {
        let step_progress = match phase {
            Some(phase) => phase_progress_within_step(phase, progress, self.diarizing),
            None => progress,
        };
        self.emit_with_phase(
            sink,
            task_id,
            step_progress,
            step_name(self.current),
            phase,
        );
    }

    fn emit(&self, sink: &mut dyn ProgressSink, task_id: &str, progress: f64, status: &str) {
        self.emit_with_phase(sink, task_id, progress, status, None);
    }

    fn emit_with_phase(
        &self,
        sink: &mut dyn ProgressSink,
        task_id: &str,
        progress: f64,
        status: &str,
        phase: Option<engine::ProgressPhase>,
    ) {
        sink.step(&StepSnapshot {
            task_id: task_id.to_string(),
            progress,
            overall: self.overall(progress),
            status: status.to_string(),
            current_step: self.current,
            total_steps: self.total,
            // Never taken from `status`: the i18n keys depend on it.
            step_name: step_name(self.current),
            phase,
            step_elapsed: self.step_start.elapsed().as_secs_f64(),
            total_elapsed: self.start.elapsed().as_secs_f64(),
        });
    }

    /// The share of the bar each step is worth, in step order.
    fn weights(&self) -> &'static [f64] {
        if self.total == STEPS_WITH_TRANSLATION {
            &STEP_WEIGHTS_WITH_TRANSLATION
        } else {
            &STEP_WEIGHTS_WITHOUT_TRANSLATION
        }
    }

    /// This run's percentage, across every step.
    ///
    /// `progress` is per **step** and restarts at 0 whenever a new step begins, so
    /// a bar driven by it alone jumps back to zero four times in one file. The
    /// whole-run figure sums the weight of every finished step plus the current
    /// step's own fraction of its weight:
    ///
    /// ```text
    /// overall = (Σ finished weights + current_weight * progress/100) * 100
    /// ```
    ///
    /// The weights are unequal on purpose — see
    /// [`STEP_WEIGHTS_WITHOUT_TRANSLATION`]. Equal weights made extracting a WAV
    /// and starting the model worth a quarter of the run each, so the bar jumped
    /// to 50 % before any recognition had happened and then sat still for the
    /// rest of it. The bar still moves monotonically and reaches 100 exactly when
    /// the last step does, which is what a progress bar owes the user.
    fn overall(&self, progress: f64) -> f64 {
        let weights = self.weights();
        if self.total == 0 || weights.is_empty() {
            return 0.0;
        }
        let current = self.current.saturating_sub(1) as usize;
        let finished: f64 = weights.iter().take(current).sum();
        let own = weights.get(current).copied().unwrap_or(0.0);
        let fraction = (progress / 100.0).clamp(0.0, 1.0);
        ((finished + own * fraction) * 100.0).clamp(0.0, 100.0)
    }

    fn total_seconds(&self) -> f64 {
        self.start.elapsed().as_secs_f64()
    }
}

/// One progress tick, independent of any event format.
///
/// The GUI turns this into a `transcribe_progress` event; a CLI prints a line or
/// emits JSON. Keeping the snapshot format-free is what lets both frontends share
/// [`transcribe_file`].
pub struct StepSnapshot {
    pub task_id: String,
    /// Progress **within** the current step, 0…100.
    pub progress: f64,
    /// Progress across the whole run, 0…100. This is what a progress bar shows.
    pub overall: f64,
    pub status: String,
    pub current_step: u32,
    pub total_steps: u32,
    pub step_name: &'static str,
    /// The recognition sub-phase, when step 3 is reporting one. `None` for every
    /// other step and for a run that never entered the VAD/ASR pipeline (a
    /// subtitle hit, or an all-silent input).
    pub phase: Option<engine::ProgressPhase>,
    pub step_elapsed: f64,
    pub total_elapsed: f64,
}

/// Where transcription progress goes.
///
/// A trait rather than a closure so the GUI can also receive the "queued" and
/// "finished" transitions, which are not per-step.
/// `Send + Sync` because the translation stage's progress callback is handed to
/// an async engine that requires it.
pub trait ProgressSink: Send + Sync {
    /// A step started or is progressing.
    fn step(&mut self, snapshot: &StepSnapshot);
    /// The run entered the queue (before audio extraction).
    fn queued(&mut self, task_id: &str);
    /// The run is dispatched but has not been admitted to the GPU yet.
    ///
    /// Distinct from `queued` on purpose: `queued` means "this task exists",
    /// `waiting` means "it exists and is blocked behind the concurrency limit".
    /// Without it the row showed "启动中…" with a spinner while the task sat in
    /// the semaphore queue, i.e. it claimed to be working when it was only
    /// waiting — and the default concurrency is 1, so that is the normal state
    /// for every file after the first.
    fn waiting(&mut self, task_id: &str);
    /// Every requested file was written; `total_seconds` is wall time.
    ///
    /// `translation` is `Some` only when this run had a translation plan, and it
    /// reports what that stage actually did — including "skipped, because the
    /// source already is the target language". The CLI ignores it (its stable
    /// contract is the exit code); the GUI shows it on the finished row, which is
    /// the only way a user learns why no `<stem>.<lang>.<ext>` file appeared.
    fn finished(
        &mut self,
        task_id: &str,
        total_seconds: f64,
        translation: Option<&TranslationReport>,
    );
}

/// The GUI's sink: turns snapshots into the frozen `transcribe_progress` event.
struct EventSink<'a> {
    app: &'a AppHandle,
}

impl ProgressSink for EventSink<'_> {
    fn step(&mut self, snapshot: &StepSnapshot) {
        let _ = self.app.emit(
            "transcribe_progress",
            TranscribeProgress {
                task_id: snapshot.task_id.clone(),
                progress: snapshot.progress,
                overall: Some(snapshot.overall),
                status: snapshot.status.clone(),
                step: Some(StepInfo {
                    current_step: snapshot.current_step,
                    total_steps: snapshot.total_steps,
                    step_name: snapshot.step_name.to_string(),
                    phase: snapshot.phase.map(|phase| phase.as_str().to_string()),
                    step_elapsed: snapshot.step_elapsed,
                    total_elapsed: snapshot.total_elapsed,
                }),
                total_time: None,
                // Only `transcribe_complete` reports translation.
                translation: None,
                waiting: None,
            },
        );
    }

    fn queued(&mut self, task_id: &str) {
        emit_queued(self.app, task_id);
    }

    fn waiting(&mut self, task_id: &str) {
        emit_waiting(self.app, task_id);
    }

    fn finished(
        &mut self,
        task_id: &str,
        total_seconds: f64,
        translation: Option<&TranslationReport>,
    ) {
        let _ = self.app.emit(
            "transcribe_complete",
            TranscribeProgress {
                task_id: task_id.to_string(),
                progress: 100.0,
                overall: Some(100.0),
                status: "completed".to_string(),
                step: None,
                total_time: Some(total_seconds),
                translation: translation.cloned(),
                waiting: None,
            },
        );
    }
}

fn step_name(step: u32) -> &'static str {
    STEP_NAMES
        .get(step.saturating_sub(1) as usize)
        .copied()
        .unwrap_or("Processing")
}

fn emit_queued(app: &AppHandle, task_id: &str) {
    let _ = app.emit(
        "transcribe_progress",
        TranscribeProgress {
            task_id: task_id.to_string(),
            progress: 0.0,
            overall: Some(0.0),
            status: "Pending in queue...".to_string(),
            step: None,
            total_time: None,
            translation: None,
            waiting: None,
        },
    );
}

/// "Dispatched, but not admitted yet."
///
/// Carries `overall: 0` rather than omitting it: the row is a position in the
/// run (the start), not an error or a queue notice, and the bar it draws should
/// read 0 % instead of falling back to a stale figure from a previous attempt.
fn emit_waiting(app: &AppHandle, task_id: &str) {
    let _ = app.emit(
        "transcribe_progress",
        TranscribeProgress {
            task_id: task_id.to_string(),
            progress: 0.0,
            overall: Some(0.0),
            status: "Waiting for a free slot".to_string(),
            step: None,
            total_time: None,
            translation: None,
            waiting: Some(true),
        },
    );
}

fn emit_error(app: &AppHandle, task_id: &str, status: &str) {
    let _ = app.emit(
        "transcribe_error",
        TranscribeProgress {
            task_id: task_id.to_string(),
            progress: 0.0,
            // An error is not a position in the run, so there is no overall
            // figure to report: the row shows the failure, not a stuck bar.
            overall: None,
            status: status.to_string(),
            step: None,
            total_time: None,
            translation: None,
            waiting: None,
        },
    );
}

/// Short status for a failed engine call (spec §5.3).
///
/// These are i18n **keys**, not sentences: the frontend resolves
/// `errors.<status>` when such a key exists and otherwise shows the raw string,
/// so a technical writer message still comes through verbatim. The full detail
/// (including `EngineError::Failed::stderr_tail`) always goes to the log.
fn engine_failure(err: EngineError) -> TaskFailure {
    let status = match &err {
        EngineError::Missing(_) => "engineMissing",
        EngineError::ModelNotInstalled(_) => "modelNotInstalled",
        EngineError::AudioExtractFailed(_) => "audioExtractFailed",
        EngineError::Failed { .. }
        | EngineError::Io(_)
        | EngineError::InvalidAudio(_)
        | EngineError::Protocol(_) => "engineFailed",
    };
    TaskFailure::new("transcription failed", status, engine_error_detail(&err))
}

/// Full error text, including the CLI's stderr tail.
fn engine_error_detail(err: &EngineError) -> String {
    match err {
        EngineError::Failed {
            message,
            stderr_tail,
        } => format!("{} — stderr: {}", message, stderr_tail),
        other => other.to_string(),
    }
}

/// A writer failure is usually user-facing (e.g. `json` without word-level
/// data), so surface the writer's own message; pure I/O failures only carry a
/// path, which stays in the log.
fn write_failure(err: EngineError) -> TaskFailure {
    let status = match &err {
        EngineError::Failed { message, .. } | EngineError::Protocol(message) => message.clone(),
        _ => "Could not write the subtitle file".to_string(),
    };
    TaskFailure::new("writing the subtitle failed", status, engine_error_detail(&err))
}

// --- Step 1: ffmpeg ---------------------------------------------------------

/// A WAV produced for one run.
///
/// The cached copy lives next to the media file and is kept (like the previous
/// engine); when the input has no usable location it goes to the temp dir and
/// is removed on drop, including when the task is cancelled.
struct ExtractedAudio {
    path: PathBuf,
    temporary: bool,
}

impl ExtractedAudio {
    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for ExtractedAudio {
    fn drop(&mut self) {
        if self.temporary {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

fn get_audio_cache_path(media_path: &Path) -> Option<PathBuf> {
    let parent = media_path.parent()?;
    let stem = media_path.file_stem()?.to_str()?;
    Some(parent.join(format!("{}_audio.wav", stem)))
}

fn is_cache_fresh(cache_path: &Path, source_path: &Path) -> bool {
    let cache_meta = match std::fs::metadata(cache_path) {
        Ok(meta) => meta,
        Err(_) => return false,
    };
    let source_meta = match std::fs::metadata(source_path) {
        Ok(meta) => meta,
        Err(_) => return false,
    };

    let cache_mtime = match cache_meta.modified() {
        Ok(time) => time,
        Err(_) => return false,
    };
    let source_mtime = match source_meta.modified() {
        Ok(time) => time,
        Err(_) => return false,
    };

    cache_mtime >= source_mtime
}

async fn extract_wav(
    paths: &crate::paths::AppPaths,
    input: &Path,
) -> Result<ExtractedAudio, EngineError> {
    let (cache, temporary) = match get_audio_cache_path(input) {
        Some(path) => (path, false),
        None => (
            std::env::temp_dir().join(format!("monosub-extract-{}.wav", uuid::Uuid::new_v4())),
            true,
        ),
    };

    if is_cache_fresh(&cache, input) {
        tracing::info!(
            source = %input.display(),
            wav = %cache.display(),
            "reusing cached extracted audio"
        );
        return Ok(ExtractedAudio {
            path: cache,
            temporary,
        });
    }

    let ffmpeg = crate::commands::video::get_ffmpeg_path(paths);
    match run_ffmpeg(&ffmpeg, input, &cache, Some(AUDIO_FILTER_CHAIN)).await {
        Ok(()) => {
            tracing::info!(
                source = %input.display(),
                wav = %cache.display(),
                "audio extracted with the filter chain"
            );
            Ok(ExtractedAudio {
                path: cache,
                temporary,
            })
        }
        Err(filtered) => {
            // Mirrors the old engine's `_extract_basic` fallback: an input
            // ffmpeg cannot filter may still decode without `-af`. Only a
            // genuine ffmpeg failure reaches here.
            tracing::warn!(
                source = %input.display(),
                error = %filtered,
                "filtered extraction failed; retrying without filters"
            );
            let _ = std::fs::remove_file(&cache);
            run_ffmpeg(&ffmpeg, input, &cache, None).await?;
            Ok(ExtractedAudio {
                path: cache,
                temporary,
            })
        }
    }
}

/// The exact ffmpeg argv, shared by the extraction and its no-filter fallback.
fn ffmpeg_args(input: &Path, output: &Path, filters: Option<&str>) -> Vec<std::ffi::OsString> {
    let mut args: Vec<std::ffi::OsString> = vec![std::ffi::OsString::from("-i")];
    args.push(input.into());
    args.push("-vn".into());
    if let Some(filters) = filters {
        args.push("-af".into());
        args.push(filters.into());
    }
    args.extend(
        ["-acodec", "pcm_s16le", "-ar", "16000", "-ac", "1", "-y"]
            .map(std::ffi::OsString::from),
    );
    args.push(output.into());
    args
}

async fn run_ffmpeg(
    ffmpeg: &str,
    input: &Path,
    output: &Path,
    filters: Option<&str>,
) -> Result<(), EngineError> {
    let result = tokio::process::Command::new(ffmpeg)
        .args(ffmpeg_args(input, output, filters))
        .output()
        .await
        .map_err(|e| {
            EngineError::AudioExtractFailed(format!("failed to run ffmpeg: {}", e))
        })?;

    if !result.status.success() {
        return Err(EngineError::AudioExtractFailed(format!(
            "ffmpeg exited with {:?}: {}",
            result.status.code(),
            stderr_tail(&result.stderr)
        )));
    }
    if !output.is_file() {
        return Err(EngineError::AudioExtractFailed(format!(
            "ffmpeg reported success but {} was not created",
            output.display()
        )));
    }
    Ok(())
}

/// Last few stderr lines: enough to identify a failure, without the banner.
fn stderr_tail(stderr: &[u8]) -> String {
    let text = String::from_utf8_lossy(stderr);
    let lines: Vec<&str> = text.lines().filter(|line| !line.trim().is_empty()).collect();
    let start = lines.len().saturating_sub(3);
    lines[start..].join(" | ")
}

// --- Format parsing and output paths (spec §3) ------------------------------

/// Task: U1-12. Moved to `subtitle::parse_formats` so the URL-import path
/// accepts exactly the same format list (url-import-design §5.4).
fn parse_formats(value: &str) -> Result<Vec<Format>, String> {
    subtitle::parse_formats(value)
}

/// `<stem>.words.json` next to the subtitle (word-timestamps §4).
///
/// A distinct name from the `json` output format: that one contains cues, this
/// one contains words and silences. Suffixing `.words` keeps a user who asked for
/// `json` cues from silently getting a different schema, and keeps both
/// derivable from `output_path` alone.
fn word_sidecar_path(output_path: &Path) -> Option<PathBuf> {
    let stem = output_path.file_stem()?.to_string_lossy().into_owned();
    Some(match output_path.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => {
            parent.join(format!("{}.words.json", stem))
        }
        _ => PathBuf::from(format!("{}.words.json", stem)),
    })
}

/// Write the machine-readable word/silence sidecar, when there is anything to
/// write.
///
/// A run with no words writes no file: an empty `{"words": []}` would be
/// indistinguishable from "the aligner ran and found nothing", and an agent
/// reading it would conclude the audio is silent.
fn write_word_sidecar(
    output_path: &Path,
    words: &[engine::words::Word],
) -> Result<(), String> {
    if words.is_empty() {
        return Ok(());
    }
    let Some(path) = word_sidecar_path(output_path) else {
        return Err(format!(
            "cannot derive a word sidecar path from {}",
            output_path.display()
        ));
    };
    let body = subtitle::writer::render_word_sidecar(words);
    std::fs::write(&path, body).map_err(|e| format!("write {}: {}", path.display(), e))?;
    tracing::info!(
        path = %path.display(),
        words = words.len(),
        "word-level sidecar written"
    );
    Ok(())
}

/// Task: U1-12. The rule now lives in `subtitle::output_path_for` so the
/// URL-import subtitle-hit path writes through exactly the same function
/// (url-import-design §5.4). This alias keeps the local call sites and their
/// tests spelling it the way they always have.
#[cfg(test)]
fn output_path_for(output_path: &Path, index: usize, format: Format) -> PathBuf {
    subtitle::output_path_for(output_path, index, format)
}

/// Every path a run may write: the originals first, then — when translation is
/// on — the translated files.
///
/// The cancellation cleanup uses this list, so a cancelled run cannot leave a
/// translated file behind (T4-6). The mtime rule in [`remove_partial_outputs`]
/// is what keeps a pre-existing file safe, and it is deliberately unchanged.
fn derive_output_paths(
    output_path: &Path,
    formats: &[Format],
    target_language: Option<&str>,
) -> Vec<PathBuf> {
    let mut paths = subtitle::output_paths_for(output_path, formats);

    if let Some(target) = target_language {
        paths.extend(crate::translate::stage::translated_paths(
            output_path,
            formats,
            target,
        ));
    }

    // The word sidecar is written whenever the run aligned, so a cancelled run
    // must clean it up too — otherwise a stale `.words.json` outlives the
    // subtitle it describes and an agent reads timing for a file that is gone.
    if let Some(sidecar) = word_sidecar_path(output_path) {
        paths.push(sidecar);
    }

    paths
}

/// Map `small` / `medium` / `large`; anything else is `Medium`.
///
/// Test-only now: the run reads the answers off the frozen plan instead of the
/// request, so this survives only to pin the three wire spellings the Settings
/// DTO also uses.
#[cfg(test)]
fn memory_from_request(value: Option<&str>) -> crate::engine::plan::MemoryBudget {
    crate::engine::plan::MemoryBudget::from_wire(value)
}

// Manual overrides live in the solver module (`plan::apply_model_overrides`),
// because the Settings preview must apply exactly the same pinning the freeze
// does. The run no longer applies them — they are part of the saved plan.
#[cfg(test)]
use crate::engine::plan::apply_model_overrides;

/// The frontend does not send `device` today; `auto`/absent means "the engine
/// picks", which on macOS is Metal (engine retries CPU itself). Other platforms
/// stay on CPU until the Phase 4 platform work.
fn resolve_backend(device: Option<&str>) -> Backend {
    match device.map(|value| value.trim().to_lowercase()) {
        Some(value) if value == "cpu" => Backend::Cpu,
        _ if cfg!(target_os = "macos") => Backend::Metal,
        _ => Backend::Cpu,
    }
}

/// Remove the subtitle files a cancelled run may have left behind.
///
/// A pre-existing file this run never opened (the cancel arrived before the
/// write step) predates `started` and is left alone.
fn remove_partial_outputs(outputs: &[PathBuf], started: SystemTime) {
    for path in outputs {
        let touched_by_this_run = std::fs::metadata(path)
            .and_then(|meta| meta.modified())
            .map(|modified| modified >= started)
            .unwrap_or(false);
        if !touched_by_this_run {
            continue;
        }
        match std::fs::remove_file(path) {
            Ok(()) => tracing::info!(
                path = %path.display(),
                "removed partially written subtitle after cancel"
            ),
            Err(e) => tracing::warn!(
                path = %path.display(),
                error = %e,
                "could not remove partial subtitle"
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::catalog::Speed;
    use std::time::Duration;

    // --- progress -----------------------------------------------------------

    /// `overall` must climb across steps, never jump back to 0 at a step
    /// boundary, and land on exactly 100 in the last step.
    ///
    /// Regression: the bar used to be driven by `progress`, which restarts at 0
    /// for every step, so it fell back to zero four times per file and the tray
    /// percentage (`buildTitle` averages `file.progress`) read 0 % while a
    /// two-hour file was being transcribed.
    #[test]
    fn overall_progress_is_monotonic_across_steps() {
        let steps = Steps::new(false, false);
        assert_eq!(steps.total, 4);

        let mut seen = Vec::new();
        for step in 1..=4u32 {
            let mut steps = Steps::new(false, false);
            steps.current = step;
            for fraction in [0.0, 25.0, 50.0, 100.0] {
                seen.push(steps.overall(fraction));
            }
        }

        assert!(
            seen.windows(2).all(|pair| pair[0] <= pair[1]),
            "overall must never regress: {seen:?}"
        );
        assert_eq!(seen.first(), Some(&0.0));
        assert_eq!(seen.last(), Some(&100.0), "the last step must reach 100");
    }

    /// The two cheap steps must not own half the bar.
    ///
    /// Regression: with equal weights, extracting a WAV and starting the model
    /// were worth 25 % each, so the bar jumped to 50 % before a single word had
    /// been recognised and then sat still for the rest of the run.
    #[test]
    fn extraction_and_model_loading_barely_move_the_bar() {
        let mut steps = Steps::new(false, false);

        steps.current = 1;
        let after_extract = steps.overall(100.0);
        steps.current = 2;
        let after_load = steps.overall(100.0);
        steps.current = 3;
        let before_transcribe = steps.overall(0.0);

        assert_eq!(after_extract, 2.0);
        assert_eq!(after_load, 5.0);
        assert_eq!(
            before_transcribe, 5.0,
            "entering transcription must not jump the bar"
        );
        assert!(
            after_load < 10.0,
            "two seconds of work cannot be worth a tenth of the run: {after_load}"
        );
    }

    /// Each table sums to 1, so the last step's 100 % is the run's 100 %.
    #[test]
    fn every_weight_table_covers_the_whole_bar() {
        let plain: f64 = STEP_WEIGHTS_WITHOUT_TRANSLATION.iter().sum();
        let translating: f64 = STEP_WEIGHTS_WITH_TRANSLATION.iter().sum();
        assert!((plain - 1.0).abs() < 1e-9, "{plain}");
        assert!((translating - 1.0).abs() < 1e-9, "{translating}");
        assert_eq!(STEP_WEIGHTS_WITHOUT_TRANSLATION.len(), 4);
        assert_eq!(STEP_WEIGHTS_WITH_TRANSLATION.len(), 5);
    }

    /// A translating run has five steps and its own weights, so writing the
    /// subtitles no longer finishes the run.
    #[test]
    fn a_translating_run_has_further_to_go_after_writing() {
        let mut plain = Steps::new(false, false);
        plain.current = 4;
        assert_eq!(plain.overall(100.0), 100.0);

        let mut translating = Steps::new(true, false);
        translating.current = 4;
        let after_writing = translating.overall(100.0);
        assert_eq!(after_writing, 85.0);
        translating.current = 5;
        assert_eq!(translating.overall(100.0), 100.0);
        assert!(
            after_writing < 100.0,
            "the translation step is still ahead"
        );
    }

    /// An out-of-range fraction is clamped rather than producing 120 % or -5 %.
    #[test]
    fn overall_progress_clamps_out_of_range_input() {
        let mut steps = Steps::new(false, false);
        steps.current = 2;
        // Steps 1 + 2 are worth 0.02 + 0.03, so the clamp pins the bar to that.
        assert_eq!(steps.overall(-10.0), 2.0);
        assert_eq!(steps.overall(250.0), 5.0);
    }

    /// The badge's percentage must not run backwards inside the recognition
    /// step. Its phases each report their own 0…100, so without the mapping the
    /// badge would read 100 % (ASR finished) and then 0 % (diarization starting).
    #[test]
    fn phase_progress_never_regresses_inside_the_recognition_step() {
        use engine::ProgressPhase as P;

        for diarizing in [false, true] {
            let phases: &[P] = if diarizing {
                &[P::Vad, P::Asr, P::Diar]
            } else {
                &[P::Vad, P::Asr]
            };
            let mut seen = Vec::new();
            for phase in phases {
                for fraction in [0.0, 25.0, 50.0, 100.0] {
                    seen.push(phase_progress_within_step(*phase, fraction, diarizing));
                }
            }
            assert!(
                seen.windows(2).all(|pair| pair[0] <= pair[1]),
                "diarizing={diarizing}: {seen:?}"
            );
            assert_eq!(seen.first(), Some(&0.0));
            assert_eq!(seen.last(), Some(&100.0));
        }
    }

    /// Without diarization the ASR owns the whole tail, so finishing it is
    /// finishing the step; with diarization it stops at 70 %.
    #[test]
    fn the_asr_phase_leaves_room_only_for_diarization() {
        use engine::ProgressPhase as P;
        assert_eq!(phase_progress_within_step(P::Asr, 100.0, false), 100.0);
        assert_eq!(phase_progress_within_step(P::Asr, 100.0, true), 70.0);
        assert_eq!(phase_progress_within_step(P::Diar, 100.0, true), 100.0);
    }

    /// The phase tables cover the step, like the step tables cover the run.
    #[test]
    fn every_phase_weight_table_covers_the_step() {
        let no_diar: f64 = PHASE_WEIGHTS_NO_DIAR.iter().sum();
        let diar: f64 = PHASE_WEIGHTS_WITH_DIAR.iter().sum();
        assert!((no_diar - 1.0).abs() < 1e-9, "{no_diar}");
        assert!((diar - 1.0).abs() < 1e-9, "{diar}");
    }


    // --- M8: request → solver requirement mapping -------------------------

    /// A request that omits the translation fields must decode to "no
    /// translation" — the old-frontend contract (T4-2).
    #[test]
    fn request_without_translation_fields_decodes_to_none() {
        let request: TranscribeRequest = serde_json::from_str(
            r#"{
              "id": "t1",
              "path": "/media/clip.mov",
              "output_path": "/media/clip.srt",
              "format": "srt",
              "language": "zh"
            }"#,
        )
        .expect("an old request still decodes");

        assert_eq!(request.translate_enabled, None);
        assert_eq!(request.target_language, None);
        assert_eq!(request.translation_model, None);

        // ...and that decodes into "do not translate", before any catalogue or
        // disk access.
        let result = resolve_translation(
            &[],
            "zh",
            request.translate_enabled,
            request.target_language.as_deref(),
            request.translation_model.as_deref(),
            &|_| true,
            &|_| Some(PathBuf::from("/models/none.gguf")),
        );
        assert!(result.unwrap().is_none());
    }

    /// A catalogue holding just the committed translate entry, plus install and
    /// path predicates a test controls.
    fn translation_fixture() -> Vec<ModelSpec> {
        let path = Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/resources/models.json"
        ));
        let catalogue = engine::catalog::load_catalog(path).expect("committed models.json");
        assert!(
            engine::catalog::default_translation_model_id(&catalogue).is_some(),
            "the committed catalogue must offer a translation model"
        );
        catalogue
    }

    /// The whole pre-flight gate, including the `MODEL_NOT_INSTALLED:<id>` error
    /// the frontend routes on (T4-3).
    #[test]
    fn preflight_requires_an_installed_translation_model() {
        let catalogue = translation_fixture();
        let default_id = engine::catalog::default_translation_model_id(&catalogue).unwrap();
        let file = |id: &str| Some(PathBuf::from(format!("/models/{}.gguf", id)));

        // Enabled, a real language pair, model installed: a plan comes back.
        let plan = resolve_translation(
            &catalogue,
            "zh",
            Some(true),
            Some("en"),
            None,
            &|id| id == default_id,
            &file,
        )
        .expect("installed model passes the pre-flight")
        .expect("translation is planned");
        assert_eq!(plan.model_id, default_id);
        assert_eq!(plan.target, "en");
        assert_eq!(plan.model.prompt_template.as_deref(), Some("hy_mt"));

        // Not installed: the exact sentinel the download UI matches on.
        let status = resolve_translation(
            &catalogue,
            "zh",
            Some(true),
            Some("en"),
            None,
            &|_| false,
            &file,
        )
        .unwrap_err();
        assert_eq!(status, format!("{}:{}", MODEL_NOT_INSTALLED, default_id));
        assert!(status.starts_with(MODEL_NOT_INSTALLED));

        // Installed but with no file path: still a missing model, never a panic.
        let status = resolve_translation(
            &catalogue,
            "zh",
            Some(true),
            Some("en"),
            None,
            &|_| true,
            &|_| None,
        )
        .unwrap_err();
        assert_eq!(status, format!("{}:{}", MODEL_NOT_INSTALLED, default_id));

        // An explicit id that the catalogue does not know is rejected the same way.
        let status = resolve_translation(
            &catalogue,
            "zh",
            Some(true),
            Some("en"),
            Some("no_such_model"),
            &|_| true,
            &file,
        )
        .unwrap_err();
        assert_eq!(status, format!("{}:no_such_model", MODEL_NOT_INSTALLED));

        // An explicitly chosen RECOGNITION model must be refused: it runs on
        // audio.cpp, so llama-server could not load it (design §11).
        let status = resolve_translation(
            &catalogue,
            "zh",
            Some(true),
            Some("en"),
            Some("sensevoice_small_q8"),
            &|_| true,
            &file,
        )
        .unwrap_err();
        assert_eq!(
            status,
            format!("{}:sensevoice_small_q8", MODEL_NOT_INSTALLED)
        );
    }

    /// The skipped cases must never reach the model gate, so a same-language run
    /// cannot demand a download (T4-3, translation-design §5).
    #[test]
    fn preflight_skips_when_translation_would_not_run() {
        let catalogue = translation_fixture();
        // Nothing is installed and no path resolves: any model access would fail.
        let nothing_installed = |_: &str| false;
        let no_path = |_: &str| None;

        let skipped = |enabled: Option<bool>, source: &str, target: Option<&str>| {
            resolve_translation(&catalogue, source, enabled, target, None, &nothing_installed, &no_path)
                .expect("a skipped run is not an error")
                .is_none()
        };

        assert!(skipped(None, "zh", Some("en")), "flag absent");
        assert!(skipped(Some(false), "zh", Some("en")), "flag off");
        assert!(skipped(Some(true), "zh", Some("zh")), "same language");
        assert!(skipped(Some(true), "zh", Some("ZH")), "same language, other case");
        assert!(skipped(Some(true), "zh-tw", Some("zh-Hant")), "same script");
        assert!(skipped(Some(true), "zh", None), "no target");
        assert!(skipped(Some(true), "zh", Some("auto")), "auto target");
        assert!(skipped(Some(true), "zh", Some("  ")), "blank target");

        // `auto` as a source is unknown, so it must NOT be skipped: a run that
        // needs translating would otherwise be dropped on a guess.
        let status = resolve_translation(
            &catalogue,
            "auto",
            Some(true),
            Some("en"),
            None,
            &nothing_installed,
            &no_path,
        )
        .unwrap_err();
        assert!(status.starts_with(MODEL_NOT_INSTALLED), "{status}");

        // zh vs zh-tw is a real translation in both directions (design §9).
        let status = resolve_translation(
            &catalogue,
            "zh",
            Some(true),
            Some("zh-tw"),
            None,
            &nothing_installed,
            &no_path,
        )
        .unwrap_err();
        assert!(status.starts_with(MODEL_NOT_INSTALLED), "{status}");
    }

    /// A catalogue with no translate entry at all must fail the pre-flight with
    /// the bare sentinel rather than planning a run it cannot perform.
    #[test]
    fn preflight_without_any_translation_model_is_a_clear_error() {
        let recognition_only: Vec<ModelSpec> = translation_fixture()
            .into_iter()
            .filter(|spec| !spec.has_role(engine::catalog::Role::Translate))
            .collect();

        let status = resolve_translation(
            &recognition_only,
            "zh",
            Some(true),
            Some("en"),
            None,
            &|_| true,
            &|_| Some(PathBuf::from("/models/x.gguf")),
        )
        .unwrap_err();
        assert_eq!(status, MODEL_NOT_INSTALLED);
    }

    /// An explicitly identical source/target pair is recognised before the model
    /// gate — including the `zh-tw` → `zh-Hant` equivalence, which must NOT be
    /// confused with `zh-tw` → `zh`.
    #[test]
    fn same_language_pair_is_recognised_before_the_model_gate() {
        assert!(canonical_source_equals_target("zh", "zh"));
        assert!(canonical_source_equals_target("zh", "ZH"));
        assert!(canonical_source_equals_target("zh-tw", "zh-Hant"));
        assert!(canonical_source_equals_target("zh_Hant", "zh-tw"));
        // ...while a genuinely different pair proceeds to the model gate.
        assert!(!canonical_source_equals_target("zh", "en"));
        assert!(!canonical_source_equals_target("zh", "zh-tw"));
        assert!(!canonical_source_equals_target("auto", "auto"));
        assert!(!canonical_source_equals_target("", "auto"));
        assert!(!canonical_source_equals_target("zh", ""));
    }

    /// The detected source language prefers the ASR's own tag, falls back to an
    /// explicit request, then to a script census, and finally reports `None`
    /// (⇒ translate anyway) — the moss path (translation-design §5).
    #[test]
    fn detected_language_prefers_the_asr_tag_then_the_request_then_the_script() {
        let chunk = |language: Option<&str>, text: &str| engine::RawChunk {
            start_s: 0.0,
            end_s: 1.0,
            text: text.to_string(),
            speaker: None,
            language: language.map(str::to_string),
        };

        // The tag wins, even over a contradicting request.
        let chunks = vec![
            chunk(Some("zh"), "你好"),
            chunk(Some("zh"), "世界"),
            chunk(Some("en"), "hello"),
        ];
        assert_eq!(detected_language(&chunks, "en").as_deref(), Some("zh"));

        // No tags: an explicit request answers.
        let chunks = vec![chunk(None, "你好"), chunk(None, "世界")];
        assert_eq!(detected_language(&chunks, "ja").as_deref(), Some("ja"));

        // No tags AND `auto`: the script census is the last resort, and it does
        // NOT override an explicit request (checked above).
        assert_eq!(detected_language(&chunks, "auto").as_deref(), Some("zh"));
        let korean = vec![chunk(None, "안녕하세요")];
        assert_eq!(detected_language(&korean, "auto").as_deref(), Some("ko"));

        // Even the census can be inconclusive: `None`, which means translate
        // anyway rather than skip on a guess.
        let latin = vec![chunk(None, "hello there")];
        assert_eq!(detected_language(&latin, "auto"), None);
        assert_eq!(detected_language(&[], "auto"), None);
    }

    #[test]
    fn memory_request_mapping_defaults_to_medium() {
        assert_eq!(
            memory_from_request(Some("small")),
            crate::engine::plan::MemoryBudget::Small
        );
        assert_eq!(
            memory_from_request(Some("LARGE")),
            crate::engine::plan::MemoryBudget::Large
        );
        assert_eq!(
            memory_from_request(Some("nonsense")),
            crate::engine::plan::MemoryBudget::Medium
        );
        assert_eq!(
            memory_from_request(None),
            crate::engine::plan::MemoryBudget::Medium
        );
    }

    #[test]
    fn override_slots_map_to_step_kinds() {
        use crate::engine::plan::override_slot_kind;
        assert_eq!(override_slot_kind("asr"), Some(StepKind::Asr));
        assert_eq!(override_slot_kind("VAD"), Some(StepKind::Vad));
        assert_eq!(override_slot_kind("diar"), Some(StepKind::Diarize));
        assert_eq!(override_slot_kind("align"), Some(StepKind::Align));
        assert_eq!(override_slot_kind("bogus"), None);
    }

    #[test]
    fn overrides_pin_the_matching_step_and_ignore_unknown_models() {
        let catalogue = vec![
            crate::engine::catalog::ModelSpec {
                id: "sortformer_diar_4spk_q8_0".to_string(),
                family: "sortformer_diar".to_string(),
                display_name: "Sortformer".to_string(),
                target_directory: "d".to_string(),
                model_file: "m.gguf".to_string(),
                files: Vec::new(),
                languages: vec!["auto".to_string()],
                license: "CC-BY-NC-4.0".to_string(),
                attribution: None,
                capabilities_raw: None,
                roles: Vec::new(),
                engine: crate::engine::catalog::Engine::default(),
                prompt_template: None,
                speed: None,
                source: None,
                tier: None,
                status: crate::engine::catalog::SupportStatus::Unverified,
                modes: Vec::new(),
                licenses: Vec::new(),
                requires: Vec::new(),
                language_tokens: Vec::new(),
            max_session_seconds: None,
            },
        ];

        let mut plans = vec![ModelPlan {
            id: "asr".to_string(),
            title_key: "t",
            model_ids: vec!["sensevoice_small_q8".to_string()],
            steps: vec![crate::engine::plan::PlanStep {
                kind: StepKind::Asr,
                model_id: "sensevoice_small_q8".to_string(),
                task: "asr".to_string(),
                family: "sense_asr".to_string(),
            }],
            cost: crate::engine::plan::PlanCost {
                steps: 1,
                speed: Speed::Fast,
                memory: crate::engine::plan::MemoryBudget::Medium,
            },
            capabilities: crate::engine::catalog::Capabilities::default(),
            gaps: Vec::new(),
        }];
        let mut overrides = HashMap::new();
        overrides.insert("diar".to_string(), "sortformer_diar_4spk_q8_0".to_string());
        overrides.insert("asr".to_string(), "does_not_exist".to_string());

        apply_model_overrides(&mut plans, &overrides, &catalogue);

        let plan = &plans[0];
        // A diar pin adds the missing step...
        assert!(plan.steps.iter().any(|step| step.kind == StepKind::Diarize
            && step.model_id == "sortformer_diar_4spk_q8_0"));
        // ...an unknown model id is ignored rather than breaking the run.
        assert_eq!(plan.steps[0].model_id, "sensevoice_small_q8");
        assert!(plan.model_ids.contains(&"sortformer_diar_4spk_q8_0".to_string()));
    }

    /// Regression: a pin must refresh every derived field, not just `model_ids`.
    ///
    /// The Settings preview renders `titleKey` as the plan name, so leaving it
    /// stale showed "SenseVoice + VAD" above a model list reading
    /// `qwen3_asr + sortformer` — the label described a plan that was not there.
    #[test]
    fn overrides_refresh_the_plan_title_capabilities_and_id() {
        use crate::engine::catalog::{Capabilities, TimestampGrade, DiarizationGrade};

        let sortformer = crate::engine::catalog::ModelSpec {
            id: "sortformer_diar_4spk_q8_0".to_string(),
            family: "sortformer_diar".to_string(),
            display_name: "Sortformer".to_string(),
            target_directory: "d".to_string(),
            model_file: "m.gguf".to_string(),
            files: Vec::new(),
            languages: vec!["auto".to_string()],
            license: "CC-BY-NC-4.0".to_string(),
            attribution: None,
            capabilities_raw: Some(Capabilities {
                timestamps: TimestampGrade::Turns,
                diarization: DiarizationGrade::Turns,
                produces_text: false,
            }),
            roles: vec![crate::engine::catalog::Role::Diar],
            engine: crate::engine::catalog::Engine::default(),
            prompt_template: None,
            speed: Some(Speed::Balanced),
            source: None,
            tier: None,
            status: crate::engine::catalog::SupportStatus::Unverified,
            modes: Vec::new(),
            licenses: Vec::new(),
            requires: Vec::new(),
            language_tokens: Vec::new(),
            max_session_seconds: None,
        };

        let mut plans = vec![ModelPlan {
            id: "asr:sense".to_string(),
            title_key: "modelPlan.planAsrVad",
            model_ids: vec!["sensevoice_small_q8".to_string()],
            steps: vec![crate::engine::plan::PlanStep {
                kind: StepKind::Asr,
                model_id: "sensevoice_small_q8".to_string(),
                task: "asr".to_string(),
                family: "sense_asr".to_string(),
            }],
            cost: crate::engine::plan::PlanCost {
                steps: 1,
                speed: Speed::Fast,
                memory: crate::engine::plan::MemoryBudget::Medium,
            },
            capabilities: Capabilities::default(),
            gaps: Vec::new(),
        }];

        let mut overrides = HashMap::new();
        overrides.insert("diar".to_string(), "sortformer_diar_4spk_q8_0".to_string());
        apply_model_overrides(&mut plans, &overrides, &[sortformer]);

        let plan = &plans[0];
        assert_eq!(
            plan.title_key, "modelPlan.planAsrDiar",
            "the title must describe the pinned plan, not the solver's old pick"
        );
        // Pinning `diar` onto a plain-ASR plan adds the Diarize step plus the
        // pure-Rust TurnAttribution step the runtime always runs after it, so
        // the id and capabilities both reflect speaker-labelled output.
        assert_eq!(plan.id, "asr+diar+turns:sortformer");
        assert_eq!(
            plan.capabilities.diarization,
            DiarizationGrade::TurnsWithText,
            "TurnAttribution upgrades the pinned turns to turns-with-text, which
             is what the runtime produces"
        );
        assert_eq!(plan.cost.steps, plan.steps.len());
    }

    /// Creates a unique scratch directory (the crate has no `tempfile`).
    fn scratch_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("monosub-transcribe-{}-{}", tag, uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn touch(path: &Path, offset_secs: i64) {
        let now = SystemTime::now();
        let time = if offset_secs >= 0 {
            now + Duration::from_secs(offset_secs as u64)
        } else {
            now - Duration::from_secs(offset_secs.unsigned_abs())
        };
        std::fs::File::options()
            .write(true)
            .open(path)
            .unwrap()
            .set_modified(time)
            .unwrap();
    }

    #[test]
    fn parses_a_comma_separated_format_list_in_order() {
        assert_eq!(
            parse_formats("srt,vtt,txt").unwrap(),
            vec![Format::Srt, Format::Vtt, Format::Txt]
        );
        assert_eq!(
            parse_formats("srt,vtt,txt,md,json").unwrap(),
            vec![
                Format::Srt,
                Format::Vtt,
                Format::Txt,
                Format::Md,
                Format::Json
            ]
        );
        assert_eq!(parse_formats("srt").unwrap(), vec![Format::Srt]);
    }

    #[test]
    fn format_list_is_trimmed_lowercased_and_deduplicated() {
        assert_eq!(parse_formats("SRT, srt").unwrap(), vec![Format::Srt]);
        assert_eq!(parse_formats(" srt , txt ").unwrap(), vec![Format::Srt, Format::Txt]);
        assert_eq!(
            parse_formats("vtt,srt,vtt,txt,Srt").unwrap(),
            vec![Format::Vtt, Format::Srt, Format::Txt]
        );
        // Empty entries between separators are dropped, not rejected.
        assert_eq!(parse_formats("srt,,vtt,").unwrap(), vec![Format::Srt, Format::Vtt]);
    }

    #[test]
    fn rejects_unknown_and_empty_format_lists() {
        assert!(parse_formats("srt,bogus").is_err());
        assert_eq!(
            parse_formats("srt,bogus").unwrap_err(),
            "Unsupported subtitle format: bogus"
        );
        assert!(parse_formats("").is_err());
        assert!(parse_formats("   ").is_err());
        assert!(parse_formats(",").is_err());
        assert!(parse_formats(" , ,").is_err());
    }

    #[test]
    fn first_format_uses_the_requested_output_path_verbatim() {
        let output = Path::new("/media/clip.srt");
        assert_eq!(output_path_for(output, 0, Format::Srt), PathBuf::from("/media/clip.srt"));
        // Even when the extension disagrees with the first format.
        assert_eq!(output_path_for(output, 0, Format::Vtt), PathBuf::from("/media/clip.srt"));
    }

    #[test]
    fn later_formats_replace_only_the_extension() {
        let output = Path::new("/media/clip.srt");
        assert_eq!(output_path_for(output, 1, Format::Vtt), PathBuf::from("/media/clip.vtt"));
        assert_eq!(output_path_for(output, 2, Format::Txt), PathBuf::from("/media/clip.txt"));
        assert_eq!(output_path_for(output, 3, Format::Md), PathBuf::from("/media/clip.md"));
        assert_eq!(output_path_for(output, 4, Format::Json), PathBuf::from("/media/clip.json"));

        // Multiple dots: only the last extension is replaced.
        let dotted = Path::new("/media/a.b.c.mp4");
        assert_eq!(
            output_path_for(dotted, 1, Format::Srt),
            PathBuf::from("/media/a.b.c.srt")
        );
        assert_eq!(output_path_for(dotted, 0, Format::Srt), dotted.to_path_buf());

        // No extension at all.
        let bare = Path::new("/media/clip");
        assert_eq!(output_path_for(bare, 0, Format::Srt), bare.to_path_buf());
        assert_eq!(output_path_for(bare, 1, Format::Srt), PathBuf::from("/media/clip.srt"));

        // A custom output directory, and a relative path.
        assert_eq!(
            output_path_for(Path::new("/custom/out/clip.srt"), 1, Format::Vtt),
            PathBuf::from("/custom/out/clip.vtt")
        );
        assert_eq!(
            output_path_for(Path::new("clip.srt"), 1, Format::Vtt),
            PathBuf::from("clip.vtt")
        );
    }

    #[test]
    fn derives_one_output_path_per_requested_format() {
        let formats = vec![Format::Srt, Format::Vtt, Format::Txt];
        assert_eq!(
            derive_output_paths(Path::new("/media/clip.srt"), &formats, None),
            vec![
                PathBuf::from("/media/clip.srt"),
                PathBuf::from("/media/clip.vtt"),
                PathBuf::from("/media/clip.txt"),
                // The word/silence sidecar is always derived: a run that aligned
                // writes it, and the cancel cleanup must cover it either way.
                PathBuf::from("/media/clip.words.json"),
            ]
        );
    }

    /// The delivery half of step 5: one translated file per requested format,
    /// next to the original, with the original files untouched (T4-4, design §7).
    #[test]
    fn translated_files_are_written_next_to_the_originals() {
        let dir = scratch_dir("translate-write");
        let output = dir.join("clip.srt");
        let formats = vec![Format::Srt, Format::Vtt, Format::Json];
        let original = vec![subtitle::Segment {
            start: 0.0,
            end: 1.0,
            text: "<原文>".to_string(),
            speaker: Some("Speaker A".to_string()),
        }];

        // What the original stage would already have written.
        for (index, format) in formats.iter().enumerate() {
            subtitle::write(&original, &output_path_for(&output, index, *format), *format).unwrap();
        }
        let originals_before: Vec<String> = formats
            .iter()
            .enumerate()
            .map(|(index, format)| {
                std::fs::read_to_string(output_path_for(&output, index, *format)).unwrap()
            })
            .collect();

        let translated = vec![subtitle::Segment {
            start: 0.0,
            end: 1.0,
            text: "<translated>".to_string(),
            speaker: Some("Speaker A".to_string()),
        }];
        let written = write_translated_formats(&output, &formats, "en", &translated);

        assert_eq!(
            written,
            vec![
                dir.join("clip.en.srt"),
                dir.join("clip.en.vtt"),
                dir.join("clip.en.json"),
            ]
        );
        assert!(written.iter().all(|path| path.is_file()));
        let english = std::fs::read_to_string(dir.join("clip.en.srt")).unwrap();
        assert!(english.contains("<translated>"), "{english}");
        assert!(
            english.contains("[Speaker A]: "),
            "the speaker label must survive translation: {english}"
        );

        // The originals are byte-identical to what they were.
        for (index, format) in formats.iter().enumerate() {
            assert_eq!(
                std::fs::read_to_string(output_path_for(&output, index, *format)).unwrap(),
                originals_before[index],
                "an original file must not be rewritten by the translation stage"
            );
        }

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// `zh-tw` names the file `zh-Hant`, which is the whole point of treating
    /// traditional Chinese as a distinct target (design §9).
    #[test]
    fn translated_file_name_uses_the_canonical_tag() {
        let dir = scratch_dir("translate-tag");
        let output = dir.join("clip.srt");
        let written = write_translated_formats(&output, &[Format::Srt], "zh-tw", &[]);

        assert_eq!(written, vec![dir.join("clip.zh-Hant.srt")]);

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Translated paths sit next to the original as `<stem>.<lang>.<ext>`, and
    /// the originals are still derived first and unchanged (T4-6, design §7).
    #[test]
    fn derives_translated_paths_alongside_the_originals() {
        let formats = vec![Format::Srt, Format::Vtt, Format::Txt];
        assert_eq!(
            derive_output_paths(Path::new("/media/clip.srt"), &formats, Some("en")),
            vec![
                PathBuf::from("/media/clip.srt"),
                PathBuf::from("/media/clip.vtt"),
                PathBuf::from("/media/clip.txt"),
                PathBuf::from("/media/clip.en.srt"),
                PathBuf::from("/media/clip.en.vtt"),
                PathBuf::from("/media/clip.en.txt"),
                PathBuf::from("/media/clip.words.json"),
            ]
        );

        // A single format, a dotted stem, a custom directory and a relative path.
        assert_eq!(
            derive_output_paths(Path::new("/media/clip.srt"), &[Format::Srt], Some("zh-tw")),
            vec![
                PathBuf::from("/media/clip.srt"),
                PathBuf::from("/media/clip.zh-Hant.srt"),
                PathBuf::from("/media/clip.words.json"),
            ]
        );
        assert_eq!(
            derive_output_paths(Path::new("a.b.c.mp4"), &[Format::Srt, Format::Md], Some("ja")),
            vec![
                PathBuf::from("a.b.c.mp4"),
                PathBuf::from("a.b.c.md"),
                PathBuf::from("a.b.c.ja.mp4"),
                PathBuf::from("a.b.c.ja.md"),
                // `.file_stem()` drops only the last extension, so the sidecar is
                // `a.b.c.words.json` rather than `a.words.json`.
                PathBuf::from("a.b.c.words.json"),
            ]
        );
        assert_eq!(
            derive_output_paths(Path::new("/custom/out/clip.srt"), &[Format::Srt], Some("de")),
            vec![
                PathBuf::from("/custom/out/clip.srt"),
                PathBuf::from("/custom/out/clip.de.srt"),
                PathBuf::from("/custom/out/clip.words.json"),
            ]
        );
        // A path with no file name has nowhere to put a translation.
        assert_eq!(
            translation_output_path(Path::new("/"), 0, Format::Srt, "en"),
            None
        );
    }

    /// The translated paths are additive: every original is still derived, at
    /// the same place, with or without translation (design §9).
    #[test]
    fn originals_are_never_affected_by_translation() {
        let formats = vec![Format::Srt, Format::Vtt];
        let without = derive_output_paths(Path::new("/media/clip.srt"), &formats, None);
        let with = derive_output_paths(Path::new("/media/clip.srt"), &formats, Some("en"));

        // Compare the `--format`-derived paths by identity rather than by
        // position: `without` also carries the word sidecar, which is not
        // per-format and must not be mistaken for one.
        for format_path in ["/media/clip.srt", "/media/clip.vtt"] {
            let path = PathBuf::from(format_path);
            assert!(without.contains(&path), "{path:?} missing without translation");
            assert!(with.contains(&path), "{path:?} missing with translation");
        }
        assert!(with.contains(&PathBuf::from("/media/clip.en.srt")));
        assert!(with.len() > without.len(), "translated paths are additive");
    }

    /// A translated path must also survive the cancel cleanup, so a cancelled
    /// run does not leave a newer file behind (T4-6).
    #[test]
    fn cancel_cleanup_covers_translated_paths() {
        let dir = scratch_dir("cancel-translated");
        let started = SystemTime::now();
        let output = dir.join("clip.srt");

        let outputs = derive_output_paths(&output, &[Format::Srt, Format::Txt], Some("en"));
        assert!(outputs.contains(&dir.join("clip.en.srt")));
        assert!(outputs.contains(&dir.join("clip.en.txt")));

        for path in &outputs {
            std::fs::write(path, b"partial").unwrap();
            touch(path, 1);
        }
        // A translated file that pre-dates the run stays put.
        let old = dir.join("clip.de.srt");
        std::fs::write(&old, b"previous run").unwrap();
        touch(&old, -600);

        remove_partial_outputs(&outputs, started);
        for path in &outputs {
            assert!(!path.exists(), "{} must be removed", path.display());
        }
        assert!(old.is_file(), "a pre-existing file must survive the cleanup");

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The word sidecar is derived, written and cleaned up like any other output.
    ///
    /// A stale `.words.json` outliving its subtitle is the failure this guards:
    /// an agent would read timing for a clip whose subtitles a cancel deleted.
    #[test]
    fn word_sidecar_path_is_derived_and_cleaned_up_with_the_run() {
        let dir = scratch_dir("word-sidecar");
        let output = dir.join("clip.srt");

        assert_eq!(
            word_sidecar_path(&output),
            Some(dir.join("clip.words.json")),
            "the sidecar sits beside the subtitle"
        );
        assert!(
            derive_output_paths(&output, &[Format::Srt], None).contains(&dir.join("clip.words.json")),
            "a cancelled run must sweep it"
        );

        // Writing then cancelling removes it.
        let words = vec![crate::engine::words::Word {
            index: 0,
            start: 0.0,
            end: 0.16,
            text: "大".to_string(),
        }];
        write_word_sidecar(&output, &words).expect("sidecar writes");
        assert!(dir.join("clip.words.json").is_file());

        let started = SystemTime::now();
        touch(&dir.join("clip.words.json"), 1);
        remove_partial_outputs(&derive_output_paths(&output, &[Format::Srt], None), started);
        assert!(
            !dir.join("clip.words.json").exists(),
            "a cancelled run must not leave a word sidecar behind"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// No words means no file: an empty `{"words": []}` reads as "silent audio".
    #[test]
    fn word_sidecar_is_not_written_when_there_are_no_words() {
        let dir = scratch_dir("word-sidecar-empty");
        let output = dir.join("clip.srt");

        write_word_sidecar(&output, &[]).expect("absence is not an error");
        assert!(
            !dir.join("clip.words.json").exists(),
            "an empty sidecar must not be written"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn audio_cache_lives_next_to_the_media_file() {
        assert_eq!(
            get_audio_cache_path(Path::new("/media/clip.mov")).unwrap(),
            PathBuf::from("/media/clip_audio.wav")
        );
        // An audio input is converted now, and uses the same rule.
        assert_eq!(
            get_audio_cache_path(Path::new("/media/song.mp3")).unwrap(),
            PathBuf::from("/media/song_audio.wav")
        );
        assert_eq!(
            get_audio_cache_path(Path::new("/media/a.b.c.mp4")).unwrap(),
            PathBuf::from("/media/a.b.c_audio.wav")
        );
        assert!(get_audio_cache_path(Path::new("/")).is_none());
    }

    #[test]
    fn audio_cache_is_fresh_when_not_older_than_the_source() {
        let dir = scratch_dir("cache");
        let source = dir.join("clip.mov");
        let cache = dir.join("clip_audio.wav");
        std::fs::write(&source, b"source").unwrap();
        std::fs::write(&cache, b"cache").unwrap();

        // Equal mtimes (the Python `>=` rule).
        touch(&source, -100);
        touch(&cache, -100);
        assert!(is_cache_fresh(&cache, &source));

        // Newer cache.
        touch(&source, -200);
        touch(&cache, -100);
        assert!(is_cache_fresh(&cache, &source));

        // Stale cache.
        touch(&source, -100);
        touch(&cache, -200);
        assert!(!is_cache_fresh(&cache, &source));

        // Missing cache / missing source.
        assert!(!is_cache_fresh(&dir.join("absent_audio.wav"), &source));
        assert!(!is_cache_fresh(&cache, &dir.join("absent.mov")));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn extracts_wav_with_the_previous_engines_filter_chain() {
        // PoC §7 / P0-9: dropping the filters would change the recognition
        // input, so the exact chain is part of the ported behaviour.
        let args = ffmpeg_args(
            Path::new("/media/clip.mov"),
            Path::new("/media/clip_audio.wav"),
            Some(AUDIO_FILTER_CHAIN),
        );
        let args: Vec<String> = args
            .iter()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect();

        assert_eq!(
            args,
            vec![
                "-i",
                "/media/clip.mov",
                "-vn",
                "-af",
                "highpass=f=80,lowpass=f=8000,afftdn=nf=-25:nr=12:nt=w,loudnorm=I=-16:TP=-1.5:LRA=11",
                "-acodec",
                "pcm_s16le",
                "-ar",
                "16000",
                "-ac",
                "1",
                "-y",
                "/media/clip_audio.wav",
            ]
        );
    }

    #[test]
    fn basic_fallback_extraction_has_no_filter_argument() {
        let args = ffmpeg_args(
            Path::new("/media/clip.mp3"),
            Path::new("/media/clip_audio.wav"),
            None,
        );
        assert!(!args.iter().any(|arg| arg == "-af"));
        // Everything else is unchanged, so the fallback output is still 16 kHz mono.
        let args: Vec<String> = args
            .iter()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect();
        assert!(args.windows(2).any(|pair| pair == ["-ar", "16000"]));
        assert!(args.windows(2).any(|pair| pair == ["-ac", "1"]));
    }

    #[test]
    fn backend_defaults_to_metal_on_macos_and_honours_cpu() {
        assert_eq!(resolve_backend(Some("cpu")), Backend::Cpu);
        assert_eq!(resolve_backend(Some("CPU")), Backend::Cpu);
        let expected = if cfg!(target_os = "macos") {
            Backend::Metal
        } else {
            Backend::Cpu
        };
        assert_eq!(resolve_backend(None), expected);
        assert_eq!(resolve_backend(Some("auto")), expected);
        assert_eq!(resolve_backend(Some("cuda")), expected);
    }

    #[test]
    fn all_silence_still_writes_one_empty_file_per_format() {
        // `engine::transcribe` returns no chunks for an all-silent input (PoC
        // §4.3); that must still produce valid, empty subtitle files.
        let dir = scratch_dir("silence");
        let output = dir.join("clip.srt");
        let formats = vec![
            Format::Srt,
            Format::Vtt,
            Format::Txt,
            Format::Md,
            Format::Json,
        ];

        let segments = subtitle::postprocess(&[], &[], &[], &PostProcessConfig::default());
        assert!(segments.is_empty());

        for (index, format) in formats.iter().enumerate() {
            let path = output_path_for(&output, index, *format);
            subtitle::write(&segments, &path, *format).expect("empty subtitle must still write");
            assert!(path.is_file(), "{} must exist", path.display());
            assert_eq!(path.extension().unwrap(), format.extension());
        }

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The engine-failure statuses are i18n keys, not sentences: the frontend
    /// resolves `errors.<status>` and falls back to the raw string. Pin the keys
    /// so a rename cannot silently make every locale dead again (review P2-2).
    #[test]
    fn engine_failures_emit_i18n_keys_that_exist_in_the_locales() {
        let cases = [
            (
                EngineError::Missing("no binary".into()),
                "engineMissing",
            ),
            (
                EngineError::AudioExtractFailed("ffmpeg".into()),
                "audioExtractFailed",
            ),
            (
                EngineError::Failed {
                    message: "exit 1".into(),
                    stderr_tail: "boom".into(),
                },
                "engineFailed",
            ),
            (EngineError::Io("disk".into()), "engineFailed"),
            (EngineError::InvalidAudio("layout".into()), "engineFailed"),
            (EngineError::Protocol("bad json".into()), "engineFailed"),
        ];

        let locales = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("src/i18n/locales");

        for (err, expected) in cases {
            let failure = engine_failure(err);
            assert_eq!(failure.status, expected);

            // Every locale must define the key, or the fallback hides it forever.
            for entry in std::fs::read_dir(&locales).expect("read locales") {
                let path = entry.expect("entry").path();
                if path.extension().and_then(|e| e.to_str()) != Some("json") {
                    continue;
                }
                let body = std::fs::read_to_string(&path).expect("read locale");
                let json: serde_json::Value = serde_json::from_str(&body)
                    .unwrap_or_else(|e| panic!("{}: {}", path.display(), e));
                assert!(
                    json.get("errors").and_then(|e| e.get(expected)).is_some(),
                    "{} is missing errors.{}",
                    path.display(),
                    expected
                );
            }
        }
    }

    /// Every `title_key` a solved plan can carry must exist as a FLAT key in
    /// both locales.
    ///
    /// Regression: i18next treats `.` as a path separator, so a key spelled
    /// `modelPlan.plan.asrVad` resolves the nested path `modelPlan.plan` (which
    /// does not exist) and the UI renders the raw key string — exactly what the
    /// Settings preview showed instead of “SenseVoice + VAD”.
    #[test]
    fn plan_title_keys_exist_as_flat_locale_keys() {
        let locales = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("src/i18n/locales");

        // Cover every branch of `plan::title_key` plus the empty-plan gap key.
        let keys = [
            "modelPlan.gapTitle",
            "modelPlan.planAsrDiar",
            "modelPlan.planAsrVad",
        ];

        for entry in std::fs::read_dir(&locales).expect("read locales") {
            let path = entry.expect("entry").path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            let body = std::fs::read_to_string(&path).expect("read locale");
            let json: serde_json::Value = serde_json::from_str(&body)
                .unwrap_or_else(|e| panic!("{}: {}", path.display(), e));

            for key in keys {
                // Split on '.' the way i18next would, then walk the JSON.
                let mut node = &json;
                for segment in key.split('.') {
                    node = node
                        .get(segment)
                        .unwrap_or_else(|| panic!("{} is missing {key}", path.display()));
                }
                assert!(node.is_string(), "{key} must be a string in {}", path.display());
            }
        }
    }

    /// A sink sees exactly the transitions the GUI relies on, in order.
    ///
    /// This is the contract the CLI will reuse: if `transcribe_file` ever starts
    /// emitting a step the sink cannot describe, or the 4-vs-5 denominator drifts,
    /// this fails without needing a Tauri app or a real model.
    #[test]
    fn a_sink_receives_queued_then_each_step_then_finished() {
        use std::sync::{Arc, Mutex};

        struct Recorder {
            events: Arc<Mutex<Vec<String>>>,
        }
        impl ProgressSink for Recorder {
            fn step(&mut self, snapshot: &StepSnapshot) {
                self.events.lock().unwrap().push(format!(
                    "step {}/{} {}",
                    snapshot.current_step, snapshot.total_steps, snapshot.step_name
                ));
            }
            fn queued(&mut self, _task_id: &str) {
                self.events.lock().unwrap().push("queued".to_string());
            }
            fn waiting(&mut self, _task_id: &str) {
                self.events.lock().unwrap().push("waiting".to_string());
            }
            fn finished(
                &mut self,
                _task_id: &str,
                _seconds: f64,
                _translation: Option<&TranslationReport>,
            ) {
                self.events.lock().unwrap().push("finished".to_string());
            }
        }

        let events = Arc::new(Mutex::new(Vec::new()));
        let mut sink = Recorder { events: events.clone() };
        let mut steps = Steps::new(false, false);

        sink.queued("t");
        steps.begin(&mut sink, "t", 1);
        steps.begin(&mut sink, "t", 2);
        steps.begin(&mut sink, "t", 3);
        steps.progress(&mut sink, "t", 42.0, Some(engine::ProgressPhase::Asr));
        steps.begin(&mut sink, "t", 4);
        sink.finished("t", 1.0, None);

        let logged = events.lock().unwrap().clone();
        assert_eq!(logged[0], "queued");
        // 4 without translation: a translating run carries 5 (see `Steps::new`).
        assert_eq!(logged[1], "step 1/4 Extracting audio");
        assert_eq!(logged[2], "step 2/4 Loading model");
        assert_eq!(logged[3], "step 3/4 Transcribing");
        assert_eq!(logged[4], "step 3/4 Transcribing", "the in-step tick");
        assert_eq!(logged[5], "step 4/4 Writing subtitle");
        assert_eq!(logged[6], "finished");
    }

    #[test]
    fn step_names_are_the_frozen_contract() {
        for (index, name) in STEP_NAMES.iter().enumerate() {
            assert_eq!(step_name(index as u32 + 1), *name);
            // FileItem.tsx maps these to `steps.<snake_case>` i18n keys.
            let key = name.to_lowercase().replace(' ', "_");
            assert_eq!(
                key,
                [
                    "extracting_audio",
                    "loading_model",
                    "transcribing",
                    "writing_subtitle",
                    "translating"
                ][index]
            );
        }
        assert_eq!(STEP_NAMES.len(), 5);
        assert_eq!(STEPS_WITHOUT_TRANSLATION, 4);
        assert_eq!(STEPS_WITH_TRANSLATION, 5);
        assert_eq!(STEPS_WITH_TRANSLATION, STEP_NAMES.len() as u32);
        // Out of range is still handled rather than panicking.
        assert_eq!(step_name(0), "Extracting audio");
        assert_eq!(step_name(9), "Processing");
    }

    /// The four existing step names must keep their exact spelling: the i18n
    /// keys are derived from them, so a rename would blank a step on every file
    /// (translation-design §7). `Translating` is the only addition.
    #[test]
    fn existing_step_names_are_unchanged_and_translating_is_appended() {
        assert_eq!(
            STEP_NAMES,
            [
                "Extracting audio",
                "Loading model",
                "Transcribing",
                "Writing subtitle",
                "Translating",
            ]
        );
    }

    /// The denominator is per-run: a translating run reports 5, everything else
    /// keeps reporting 4 exactly as before.
    #[test]
    fn step_total_is_dynamic() {
        assert_eq!(Steps::new(false, false).total, STEPS_WITHOUT_TRANSLATION);
        assert_eq!(Steps::new(true, false).total, STEPS_WITH_TRANSLATION);
    }

    #[test]
    fn cancelled_runs_only_remove_files_they_touched() {
        let dir = scratch_dir("cancel");
        let started = SystemTime::now();

        let untouched = dir.join("old.srt");
        std::fs::write(&untouched, b"previous run").unwrap();
        touch(&untouched, -600);

        // Simulate a file this run wrote after it started.
        let partial = dir.join("new.vtt");
        std::fs::write(&partial, b"half").unwrap();
        touch(&partial, 1);
        let missing = dir.join("never.vtt");

        remove_partial_outputs(&[untouched.clone(), partial.clone(), missing.clone()], started);

        assert!(untouched.is_file(), "a pre-existing output must survive");
        assert!(!partial.exists(), "a file written by this run must be removed");
        assert!(!missing.exists());

        let _ = std::fs::remove_dir_all(&dir);
    }
}
