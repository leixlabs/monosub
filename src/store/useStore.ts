import { create } from 'zustand'
import { persist } from 'zustand/middleware'

export type VideoStatus = 'pending' | 'processing' | 'completed' | 'skipped' | 'error'
export type StorageMode = 'same' | 'custom'
export type SubtitleFormat = 'srt' | 'vtt' | 'txt' | 'md' | string  // string allows multi-format like "srt,txt"
export type OutputLanguage = 'auto' | 'en' | 'zh' | 'es' | 'ja' | 'de' | 'fr' | 'pt' | 'zh-tw' | 'ko'
/**
 * Language a local translation model is asked for (translation-design §7).
 *
 * Deliberately wider than [`OutputLanguage`]: recognition supports what the ASR
 * models can transcribe, translation supports what the MT model can *produce*,
 * and those are different sets (HY-MT covers 38 languages; `sense_asr` handles
 * 23 tags but only 6 real tokens). Tying the two together would cap translation
 * at the recognition list for no reason.
 *
 * `auto` is deliberately absent from both: it is a *source*-detection value, and
 * "translate into auto" has no meaning. The Rust side skips such a run rather
 * than guessing (`translate::language::should_translate`).
 */
export type TranslateTargetLanguage =
  | Exclude<OutputLanguage, 'auto'>
  | 'ar' | 'bn' | 'bo' | 'cs' | 'fa' | 'gu' | 'he' | 'hi' | 'id' | 'it'
  | 'kk' | 'km' | 'mn' | 'mr' | 'ms' | 'my' | 'nl' | 'pl' | 'ru' | 'ta'
  | 'te' | 'th' | 'tl' | 'tr' | 'ug' | 'uk' | 'ur' | 'vi' | 'yue'

/**
 * 语种场景 — the first of the four questions the recognition plan asks.
 *
 * It does two jobs (`engine/plan.rs::LanguageProfile`): it owns the family bias
 * table, and it is why `中英混合` does not pick a Chinese-only model.
 */
export type LanguageProfile = 'mandarin' | 'mixed' | 'multilingual'
/**
 * 显存占用 — how much memory the user is willing to spend.
 *
 * This replaced the `fast`/`balanced`/`accurate` speed preference: in this
 * catalogue a model's weight size decides both its footprint and its speed, so
 * one question answers both. `small` ≈ 254 MB class, `large` = anything goes.
 */
export type MemoryBudget = 'small' | 'medium' | 'large'
/** Step roles a user may pin to a specific model id (design §8.1). */
export type ModelOverrideSlot = 'asr' | 'vad' | 'align' | 'diar'

/**
 * Browser whose Bilibili cookies yt-dlp may read (`--cookies-from-browser`).
 *
 * `''` means "not configured", which is the default: reading a browser's cookie
 * store must be an explicit choice, never something that happens because the user
 * pasted a link (url-import-design §4.5).
 */
export type CookiesBrowser = '' | 'chrome' | 'edge' | 'firefox' | 'safari'

/** The browsers the login prompt offers, in the order it lists them. */
export const COOKIES_BROWSERS: Exclude<CookiesBrowser, ''>[] = ['chrome', 'edge', 'firefox', 'safari']

/**
 * One in-flight URL import.
 *
 * Kept in its own, **non-persisted** slice rather than in `files`: an import is
 * not a file, has no path until it finishes, and must not take part in "start
 * all" or the batch counters (url-import-design §5.2). A queued import leaves
 * this slice and becomes a normal `VideoFile`.
 */
export interface UrlImportState {
  taskId: string
  /** The URL exactly as it was pasted, for the row label. */
  input: string
  status: 'pending' | 'resolving' | 'fetching_subtitles' | 'downloading_audio' | 'writing' | 'translating' | 'done' | 'error'
  /** 0–100, or `undefined` when the size is unknown (indeterminate). */
  progress?: number
  /** Set on `done` when a platform subtitle was written. */
  languageTag?: string
  /** Whether the platform track was human CC or its own ASR. */
  languageKind?: 'human' | 'ai'
  /** Files written, for "show in Finder". */
  paths?: string[]
  /** Every format's path when translation also ran. */
  translatedPaths?: string[]
  /** True when `skipExistingSubtitle` skipped every file. */
  skipped?: boolean
  error?: string
  /** True when the fix is to configure cookies and retry. */
  needsCookies?: boolean
  /** A URL that can be retried verbatim from the row. */
  url: string
}

export interface StepProgress {
  currentStep: number
  totalSteps: number
  stepName: string
  /**
   * Which part of the recognition step is running: `vad` / `asr` / `diar`.
   *
   * Only the recognition step has sub-phases worth naming — it is the one that
   * takes minutes, and "转录中" alone cannot distinguish "finding the speech"
   * from "reading it" from "separating the speakers". Absent on every other
   * step, and on a backend old enough not to send it.
   */
  phase?: string
  stepElapsed: number  // seconds elapsed for current step
  totalElapsed: number // total seconds elapsed since start
}

export interface VideoFile {
  id: string
  name: string
  path: string
  size: number
  duration: number
  thumbnail?: string
  status: VideoStatus
  progress: number
  /**
   * Dispatched to the backend, but blocked behind the concurrency limit.
   *
   * Distinct from `status: 'pending'`, which means "not dispatched yet". Both
   * are "not working", and telling them apart is the whole point: with the
   * default concurrency of 1, every file after the first sits here for most of
   * its wait, and the row used to call that "启动中…" — a spinner claiming work
   * that had not begun.
   */
  waiting?: boolean
  stepProgress?: StepProgress
  totalTime?: number  // total processing time in seconds (set on completion)
  error?: string
  /**
   * The subtitle file that made `skipExistingSubtitle` skip this file.
   *
   * The row says "已跳过" and otherwise leaves the user guessing which file it
   * found — the answer can be in another directory (custom storage mode) and in
   * another format than the primary one, so it is worth naming.
   */
  skippedOutputPath?: string
  /**
   * The primary subtitle path this row's run wrote, set when the request is
   * sent.
   *
   * Stored rather than recomputed: the output *format* is a live setting, so a
   * row that recomputed its path after the user switched `srt` → `vtt` would
   * point at a file that run never wrote. Used by the row's "open subtitle"
   * action; the other formats in the same run share its stem.
   */
  subtitlePath?: string
  /**
   * Translated files this run wrote (`<stem>.<lang>.<ext>`), from the
   * `transcribe_complete` report.
   *
   * Empty/absent when the run translated nothing — which is why the two fields
   * below exist. A finished row with translation on and no file to show is
   * otherwise indistinguishable from a bug.
   */
  translatedPaths?: string[]
  /**
   * Why the translation stage wrote nothing: `sameLanguage` or `noSegments`.
   *
   * Not an error. `sameLanguage` is the common one — the ASR detected a source
   * equal to the configured target, so a translated file would be a duplicate
   * under a name promising a language it does not contain. The row says so
   * instead of silently omitting the file (measured: "开启了翻译的情况下，并没有
   * 生成翻译文件" was exactly this case, with the reason only in the log).
   */
  translationSkipped?: string
  /**
   * Some cues kept their original text because a batch would not parse.
   *
   * The translated file still exists; the row marks it so a reader knows part of
   * it is untranslated rather than mistaking it for a clean translation.
   */
  translationDegraded?: boolean
  /**
   * How many cues kept their original text, and the run's cue total.
   *
   * The badge says "部分未翻译" and then answers "how much": one line out of
   * fifty and three hundred out of five thousand are the same boolean and very
   * different problems. Both are absent when an older backend sends no counts.
   */
  translationKeptOriginal?: number
  translationTotalCues?: number
  /**
   * Per-file speaker separation. Diarization is a property of the recording,
   * not of the app: a single-narrator file can skip it, which also avoids
   * loading the diar / passthrough model at all (much faster).
   * Seeded from `settings.defaultDiarization` when the file is queued.
   */
  diarization: boolean
  /**
   * Per-file translation toggle. Seeded from `settings.translateEnabled` when
   * the file is queued.
   *
   * The mirror of `diarization`: which files get a translated copy is a
   * per-run decision, so it belongs next to the file rather than only in
   * Settings. `settings.translateEnabled` is the default it starts from;
   * `translateTargetLanguage` and `translationModelId` stay app-wide, because
   * "translate into English with model X" does not change from file to file.
   */
  translate: boolean
  /**
   * URL imports only: the output directory for this file's subtitles.
   *
   * "Same directory as the source" is meaningless for an imported file (the
   * source is a cache artefact the user will clear), so the frontend computes
   * this once and `getOutputPath` prefers it (url-import-design §5.4).
   */
  outputDir?: string
  /**
   * URL imports only: the platform would have offered a subtitle behind a login.
   *
   * Set from the `Queued` outcome's `login_hint`, so the row can tell the user
   * that logging in would have avoided the recognition pass entirely.
   */
  loginHint?: boolean
  /**
   * URL imports only: the normalised URL this file came from.
   *
   * Kept so the clipboard watcher can tell "I already imported this" from "this
   * is a new link" — without it, dismissing a pill and importing would race, and
   * the same link could be offered again a second later.
   */
  sourceUrl?: string
  /**
   * Catalogue id parsed out of a `MODEL_NOT_INSTALLED:<id>` failure.
   *
   * Set by both error entry points (`startTranscriptionTask`'s catch and the
   * `transcribe_error` listener) so the row knows *which* model to offer for
   * download instead of sending the user to the model list to guess. `undefined`
   * when the sentinel carried no id, or when the failure was something else.
   *
   * Runtime state, not a preference: `files` is not part of `partialize` (see
   * `PersistedAppState`), so neither this field nor `downloadProgress` below
   * needs a `version` bump, a migrate branch, or a place in `normalizePersistedState`
   * — nothing of the file queue survives a reload in the first place.
   */
  missingModelId?: string
  /**
   * True when the row failed *because a model is missing*.
   *
   * `missingModelId` alone cannot say this: the sentinel is sometimes bare
   * (`MODEL_NOT_INSTALLED` with no id, and the engine's `modelNotInstalled` key),
   * so "a model is missing, but which one is unknown" is a real state. `error`
   * holds the localised message rather than the sentinel, so the row needs this
   * flag to know that the download prompt belongs in its dialog at all.
   *
   * Runtime state for the same reason as `missingModelId` (see above).
   */
  missingModel?: boolean
  /**
   * Live progress of the download offered for `missingModelId`.
   *
   * Mirrored from the `model_download_progress` event by `MissingModelPrompt` so
   * the queue row can show the transfer too, rather than only the dialog. Runtime
   * state for the same reason as `missingModelId` — and a byte counter from a
   * previous session would describe a download that is no longer running.
   */
  downloadProgress?: { downloaded: number; total: number }
}

export interface AppSettings {
  // Output settings
  outputFormat: SubtitleFormat
  outputLanguage: OutputLanguage

  // Processing settings
  skipExistingSubtitle: boolean

  // UI settings
  showCompletedNotification: boolean

  // Recognition plan requirements (v3; design §10.1; reshaped in v6)
  requireWordTimestamps: boolean
  /** 语种场景 — 普通话为主 / 中英混合 / 多语种. */
  languageProfile: LanguageProfile
  /** 显存占用 — 小 / 中 / 大. Replaces the old speed preference. */
  memoryBudget: MemoryBudget

  /**
   * Default speaker separation for newly queued files. Each file keeps its own
   * toggle afterwards (see `VideoFile.diarization`), so one narrator-only file
   * can skip the diar model without changing the app-wide default.
   */
  defaultDiarization: boolean

  // Translation (v4; translation-design §7). Translation is an enhancement: it
  // is off by default, and a run that never enables it behaves exactly as it
  // did before the feature existed.
  //
  // The three fields below are NOT all the same kind of setting:
  //
  // * `translateTargetLanguage` and `translationModelId` are **configure-once**
  //   choices — which language and which model are properties of the user's
  //   setup, not of one run, so they live only in Settings.
  // * `translateEnabled` is the **default** for a new run. The switch that
  //   actually decides whether a file is translated sits in the task area
  //   (queue toolbar + each row), because that is where the question is asked
  //   and where the user can still change their mind. A Settings-only switch
  //   made the app translate files the user had not asked about in that moment.
  /**
   * Default for "translate this run". Seeds each newly queued file's own toggle
   * (see `VideoFile.translate`); the task area can change it per file and for
   * the whole queue, exactly like speaker separation.
   */
  translateEnabled: boolean
  /** Language the subtitles are translated into (never `auto`). */
  translateTargetLanguage: TranslateTargetLanguage
  /** Pinned translation-model id; `''` means "use the catalogue default". */
  translationModelId: string

  // URL import (v5; url-import-design §5.4). `same` storage mode writes here
  // rather than next to the source, because for a URL the "source" is a cache
  // file the user will eventually clear.
  /** Output directory for imported subtitles. `''` means `~/Downloads/MonoSub`. */
  urlImportOutputDir: string
  /** Browser to read Bilibili cookies from; `''` = not configured. */
  bilibiliCookiesBrowser: CookiesBrowser
  /**
   * Skip the platform subtitle and always recognise locally.
   *
   * An escape hatch for users who need the app's own ASR (and its `words.json`
   * sidecar) rather than the platform's machine transcript (design §7-2).
   */
  preferLocalRecognition: boolean
  /**
   * Offer a one-click import when the clipboard holds a Bilibili link.
   *
   * On by default because it is the whole point of the feature, but exposed as a
   * switch: it means reading the clipboard, and a user who copies links for other
   * reasons deserves a way to stop it without any other consequence.
   */
  clipboardWatchEnabled: boolean

  /**
   * How many files may be transcribed at once (1–3).
   *
   * Two things read it, and they must agree: the frontend queue uses it as the
   * number of tasks it dispatches, and the backend semaphore is sized from it
   * (`set_transcription_concurrency`). Before this existed the queue dispatched
   * 3 while the Rust semaphore admitted 1, so the setting would have been a
   * lie — the extra tasks simply waited in the backend with a 0 % row.
   *
   * Defaults to 1: every stage is Metal work on one GPU, so running several is
   * a throughput *trade*, not a free win, and a user who has not asked for it
   * should not get it. 3 is the ceiling because the old hard-coded figure was 3
   * and more than that on one GPU makes every task slower.
   */
  concurrency: number
}

const OUTPUT_LANGUAGES: OutputLanguage[] = ['auto', 'en', 'zh', 'es', 'ja', 'de', 'fr', 'pt', 'zh-tw', 'ko']

const isOutputLanguage = (value: unknown): value is OutputLanguage =>
  typeof value === 'string' && (OUTPUT_LANGUAGES as string[]).includes(value)

// Detect system language
const detectSystemLanguage = (): OutputLanguage => {
  const lang = navigator.language || (navigator as { userLanguage?: string }).userLanguage || 'auto'
  if (lang.startsWith('zh-TW') || lang.startsWith('zh-HK')) return 'zh-tw'
  if (lang.startsWith('zh')) return 'zh'
  if (lang.startsWith('en')) return 'en'
  if (lang.startsWith('es')) return 'es'
  if (lang.startsWith('ja')) return 'ja'
  if (lang.startsWith('de')) return 'de'
  if (lang.startsWith('fr')) return 'fr'
  if (lang.startsWith('pt')) return 'pt'
  if (lang.startsWith('ko')) return 'ko'
  return 'auto'
}

const defaultSettings: AppSettings = {
  outputFormat: 'srt',
  outputLanguage: detectSystemLanguage(),
  skipExistingSubtitle: false,
  showCompletedNotification: true,
  requireWordTimestamps: false,
  // `普通话为主` + `中` are the defaults that resolve to SenseVoice (design §5.4).
  languageProfile: 'mandarin',
  memoryBudget: 'medium',
  defaultDiarization: true,
  translateEnabled: false,
  // `en` is the non-source default the design names (translation-design §7). The
  // source is usually Chinese, so translating *into* English is the common case;
  // a user whose source is already English picks another target.
  translateTargetLanguage: 'en',
  translationModelId: '',
  urlImportOutputDir: '',
  bilibiliCookiesBrowser: '',
  preferLocalRecognition: false,
  clipboardWatchEnabled: true,
  concurrency: 1,
}

const MODEL_OVERRIDE_SLOTS: ModelOverrideSlot[] = ['asr', 'vad', 'align', 'diar']

const LANGUAGE_PROFILES: LanguageProfile[] = ['mandarin', 'mixed', 'multilingual']
const MEMORY_BUDGETS: MemoryBudget[] = ['small', 'medium', 'large']

const isLanguageProfile = (value: unknown): value is LanguageProfile =>
  typeof value === 'string' && (LANGUAGE_PROFILES as string[]).includes(value)

const isMemoryBudget = (value: unknown): value is MemoryBudget =>
  typeof value === 'string' && (MEMORY_BUDGETS as string[]).includes(value)

/** The range the concurrency setting accepts; mirrors the Rust clamp. */
export const MIN_CONCURRENCY = 1
export const MAX_CONCURRENCY = 3

const clampConcurrency = (value: unknown): number => {
  const n = typeof value === 'number' && Number.isFinite(value) ? Math.round(value) : MIN_CONCURRENCY
  return Math.min(MAX_CONCURRENCY, Math.max(MIN_CONCURRENCY, n))
}

/**
 * Map a v5 `speedPreference` onto the v6 显存占用.
 *
 * The two ladders coincide in this catalogue — the fastest model is the 254 MB
 * one and the slowest is the largest — so a user who asked for `fast` wanted
 * the small bucket. Dropping the setting instead would silently reset a
 * preference the user did express.
 */
const memoryBudgetFromSpeed = (value: unknown): MemoryBudget => {
  switch (value) {
    case 'fast':
      return 'small'
    case 'accurate':
      return 'large'
    default:
      return defaultSettings.memoryBudget
  }
}

/**
 * Targets the UI offers: every language the default translation model (HY-MT)
 * can produce, with the ones the app itself is localised into first.
 *
 * The list is the *default* model's coverage, not the intersection over the whole
 * catalogue — an optional 8B model that only speaks 10 languages must not shrink
 * what the recommended 1.8B model can do. The Rust side degrades gracefully for a
 * target a user-selected model does not declare: the prompt still names the
 * target, and a model that cannot produce it comes back with the source text,
 * which `translate::parse` reports as degraded rather than as success
 * (translation-design §5).
 */
export const TRANSLATE_TARGET_LANGUAGES: TranslateTargetLanguage[] = [
  'en', 'zh', 'zh-tw', 'ja', 'ko', 'es', 'fr', 'de', 'pt',
  // The rest of HY-MT's 38 languages, alphabetical by tag.
  'ar', 'bn', 'bo', 'cs', 'fa', 'gu', 'he', 'hi', 'id', 'it',
  'kk', 'km', 'mn', 'mr', 'ms', 'my', 'nl', 'pl', 'ru', 'ta',
  'te', 'th', 'tl', 'tr', 'ug', 'uk', 'ur', 'vi', 'yue',
]

const isTranslateTargetLanguage = (value: unknown): value is TranslateTargetLanguage =>
  typeof value === 'string' && (TRANSLATE_TARGET_LANGUAGES as string[]).includes(value)

const isCookiesBrowser = (value: unknown): value is CookiesBrowser =>
  value === '' || (typeof value === 'string' && (COOKIES_BROWSERS as string[]).includes(value))

interface AppState {
  // File queue management
  files: VideoFile[]
  addFiles: (files: VideoFile[]) => void
  removeFile: (id: string) => void
  clearFiles: () => void
  updateFile: (id: string, updates: Partial<VideoFile>) => void
  /** Toggle speaker separation for one queued file. */
  setFileDiarization: (id: string, enabled: boolean) => void
  /** Apply `settings.defaultDiarization` to every pending file. */
  applyDiarizationToAll: (enabled: boolean) => void
  /** Toggle translation for one queued file. */
  setFileTranslate: (id: string, enabled: boolean) => void
  /** Apply one translation value to every pending file. */
  applyTranslateToAll: (enabled: boolean) => void

  // Processing settings
  storageMode: StorageMode
  setStorageMode: (mode: StorageMode) => void
  customStoragePath: string
  setCustomStoragePath: (path: string) => void

  // Manual model overrides: role -> model id (design §10.2)
  modelOverrides: Partial<Record<ModelOverrideSlot, string>>
  setModelOverride: (slot: ModelOverrideSlot, modelId: string) => void
  clearModelOverrides: () => void

  // Global state
  isProcessing: boolean
  setIsProcessing: (isProcessing: boolean) => void
  isBatchProcessing: boolean
  setIsBatchProcessing: (isBatchProcessing: boolean) => void
  currentTaskId: string | null
  setCurrentTaskId: (id: string | null) => void

  // Settings panel
  settings: AppSettings
  updateSettings: (updates: Partial<AppSettings>) => void
  resetSettings: () => void
  isSettingsOpen: boolean
  setIsSettingsOpen: (isOpen: boolean) => void

  // Queue processing
  processedCount: number
  setProcessedCount: (count: number) => void
  incrementProcessedCount: () => void

  // URL imports (non-persisted; url-import-design §5.2)
  urlImports: Record<string, UrlImportState>
  /**
   * The dialog's "start the transcription automatically" checkbox. Defaults to
   * on; the auto-start itself is pure frontend (design §5.3).
   */
  autoStartImports: boolean
  setAutoStartImports: (autoStart: boolean) => void
  addUrlImport: (state: UrlImportState) => void
  updateUrlImport: (taskId: string, updates: Partial<UrlImportState>) => void
  removeUrlImport: (taskId: string) => void
  clearUrlImports: () => void
}

// Persisted slice. Kept in sync with the `partialize` option below.
type PersistedAppState = Pick<
  AppState,
  'storageMode' | 'customStoragePath' | 'settings' | 'modelOverrides'
>

// v3 adds the recognition-plan requirements and `modelOverrides` (design §10.3);
// v4 adds the translation settings (translation-design §7); v5 adds the URL-import
// settings (url-import-design §5.4); v6 replaces `speedPreference` with the two
// questions that actually drive the solver (`languageProfile`, `memoryBudget`).
// v3–v5 only ADD, so an upgraded blob keeps every preference the user set; v6 is
// the one step that retires a field, and it migrates the value instead of
// dropping it (see `memoryBudgetFromSpeed`).
// v7 added `settings.concurrency`; v6 and older keep every field they had.
const PERSIST_VERSION = 7

const isRecord = (value: unknown): value is Record<string, unknown> =>
  typeof value === 'object' && value !== null

// v0/v1 blobs still carry `enableNoiseReduction`, `enableVolumeNormalization`,
// `enableBilingual` and `bilingualTargetLang`. PoC §7 (P0-9) proved all four are
// dead engine-side settings, so migration rebuilds `settings` from the fields
// that still exist and drops everything else.
const normalizeSettings = (value: unknown): AppSettings => {
  const persisted = isRecord(value) ? value : {}
  return {
    outputFormat: typeof persisted.outputFormat === 'string' ? persisted.outputFormat : defaultSettings.outputFormat,
    outputLanguage: isOutputLanguage(persisted.outputLanguage) ? persisted.outputLanguage : defaultSettings.outputLanguage,
    skipExistingSubtitle: typeof persisted.skipExistingSubtitle === 'boolean' ? persisted.skipExistingSubtitle : defaultSettings.skipExistingSubtitle,
    showCompletedNotification: typeof persisted.showCompletedNotification === 'boolean' ? persisted.showCompletedNotification : defaultSettings.showCompletedNotification,
    // v2 blobs have none of these; defaults are applied, nothing is dropped.
    requireWordTimestamps: typeof persisted.requireWordTimestamps === 'boolean' ? persisted.requireWordTimestamps : defaultSettings.requireWordTimestamps,
    // v6 replaced `speedPreference` with two questions. A v5 blob has no
    // `languageProfile`, and its speed preference is translated into a footprint
    // budget rather than discarded.
    languageProfile: isLanguageProfile(persisted.languageProfile) ? persisted.languageProfile : defaultSettings.languageProfile,
    memoryBudget: isMemoryBudget(persisted.memoryBudget)
      ? persisted.memoryBudget
      : memoryBudgetFromSpeed(persisted.speedPreference),
    defaultDiarization: typeof persisted.defaultDiarization === 'boolean' ? persisted.defaultDiarization : defaultSettings.defaultDiarization,
    // v3 blobs carry none of these; defaults are applied and nothing already
    // present in the blob is dropped.
    translateEnabled: typeof persisted.translateEnabled === 'boolean' ? persisted.translateEnabled : defaultSettings.translateEnabled,
    translateTargetLanguage: isTranslateTargetLanguage(persisted.translateTargetLanguage) ? persisted.translateTargetLanguage : defaultSettings.translateTargetLanguage,
    translationModelId: typeof persisted.translationModelId === 'string' ? persisted.translationModelId : defaultSettings.translationModelId,
    // v4 blobs carry none of these; same add-only rule.
    urlImportOutputDir: typeof persisted.urlImportOutputDir === 'string' ? persisted.urlImportOutputDir : defaultSettings.urlImportOutputDir,
    bilibiliCookiesBrowser: isCookiesBrowser(persisted.bilibiliCookiesBrowser) ? persisted.bilibiliCookiesBrowser : defaultSettings.bilibiliCookiesBrowser,
    preferLocalRecognition: typeof persisted.preferLocalRecognition === 'boolean' ? persisted.preferLocalRecognition : defaultSettings.preferLocalRecognition,
    clipboardWatchEnabled: typeof persisted.clipboardWatchEnabled === 'boolean' ? persisted.clipboardWatchEnabled : defaultSettings.clipboardWatchEnabled,
    // v7 added this. Clamped rather than trusted: the Rust side clamps too, and
    // a blob saying 0 would otherwise stall the queue at a level the backend
    // silently raised to 1.
    concurrency: clampConcurrency(persisted.concurrency),
  }
}

// Unknown slots and non-string ids are discarded so a hand-edited blob cannot
// inject a bogus role or model id.
const normalizeModelOverrides = (value: unknown): Partial<Record<ModelOverrideSlot, string>> => {
  const persisted = isRecord(value) ? value : {}
  const result: Partial<Record<ModelOverrideSlot, string>> = {}
  for (const slot of MODEL_OVERRIDE_SLOTS) {
    const modelId = persisted[slot]
    if (typeof modelId === 'string' && modelId.trim().length > 0) {
      result[slot] = modelId
    }
  }
  return result
}

// Called for every blob whose stored version differs from PERSIST_VERSION, so
// an old localStorage blob can neither crash startup nor resurrect removed keys.
const normalizePersistedState = (persistedState: unknown): PersistedAppState => {
  const persisted = isRecord(persistedState) ? persistedState : {}
  return {
    storageMode: persisted.storageMode === 'custom' ? 'custom' : 'same',
    customStoragePath: typeof persisted.customStoragePath === 'string' ? persisted.customStoragePath : '',
    settings: normalizeSettings(persisted.settings),
    modelOverrides: normalizeModelOverrides(persisted.modelOverrides),
  }
}

export const useStore = create<AppState>()(
  persist(
    (set) => ({
      // File queue
      files: [],
      addFiles: (newFiles) => set((state) => ({
        files: [...state.files, ...newFiles]
      })),
      removeFile: (id) => set((state) => ({
        files: state.files.filter(f => f.id !== id)
      })),
      clearFiles: () => set({ files: [], processedCount: 0 }),
      updateFile: (id, updates) => set((state) => ({
        files: state.files.map(f => f.id === id ? { ...f, ...updates } : f)
      })),
      setFileDiarization: (id, enabled) => set((state) => ({
        files: state.files.map(f => f.id === id ? { ...f, diarization: enabled } : f)
      })),
      applyDiarizationToAll: (enabled) => set((state) => ({
        // Only pending files: a finished or in-flight run must not change.
        files: state.files.map(f => f.status === 'pending' ? { ...f, diarization: enabled } : f)
      })),
      setFileTranslate: (id, enabled) => set((state) => ({
        files: state.files.map(f => f.id === id ? { ...f, translate: enabled } : f)
      })),
      applyTranslateToAll: (enabled) => set((state) => ({
        // Pending only, for the same reason as `applyDiarizationToAll`.
        files: state.files.map(f => f.status === 'pending' ? { ...f, translate: enabled } : f)
      })),

      // Settings
      storageMode: 'same',
      setStorageMode: (mode) => set({ storageMode: mode }),
      customStoragePath: '',
      setCustomStoragePath: (path) => set({ customStoragePath: path }),

      // Manual model overrides (design §10.2)
      modelOverrides: {},
      setModelOverride: (slot, modelId) => set((state) => ({
        modelOverrides: { ...state.modelOverrides, [slot]: modelId }
      })),
      clearModelOverrides: () => set({ modelOverrides: {} }),

      // Global state
      isProcessing: false,
      setIsProcessing: (isProcessing) => set({ isProcessing }),
      isBatchProcessing: false,
      setIsBatchProcessing: (isBatchProcessing) => set({ isBatchProcessing }),
      currentTaskId: null,
      setCurrentTaskId: (id) => set({ currentTaskId: id }),

      // Settings panel
      settings: defaultSettings,
      updateSettings: (updates) => set((state) => ({
        settings: { ...state.settings, ...updates }
      })),
      resetSettings: () => set({ settings: defaultSettings }),
      isSettingsOpen: false,
      setIsSettingsOpen: (isOpen) => set({ isSettingsOpen: isOpen }),

      // Queue processing
      processedCount: 0,
      setProcessedCount: (count) => set({ processedCount: count }),
      incrementProcessedCount: () => set((state) => ({
        processedCount: state.processedCount + 1
      })),

      // URL imports: deliberately absent from `partialize`, so a reload starts
      // with no half-finished import rows (url-import-design §5.2).
      urlImports: {},
      autoStartImports: true,
      setAutoStartImports: (autoStartImports) => set({ autoStartImports }),
      addUrlImport: (urlImport) => set((state) => ({
        urlImports: { ...state.urlImports, [urlImport.taskId]: urlImport },
      })),
      updateUrlImport: (taskId, updates) => set((state) => {
        const existing = state.urlImports[taskId]
        if (!existing) return state
        return {
          urlImports: { ...state.urlImports, [taskId]: { ...existing, ...updates } },
        }
      }),
      removeUrlImport: (taskId) => set((state) => {
        if (!(taskId in state.urlImports)) return state
        const next = { ...state.urlImports }
        delete next[taskId]
        return { urlImports: next }
      }),
      clearUrlImports: () => set({ urlImports: {} }),
    }),
    {
      name: 'monosub-storage',
      version: PERSIST_VERSION,
      migrate: normalizePersistedState,
      // Also normalize on plain hydration: a blob written without a `version`
      // field skips `migrate`, and the default shallow merge would otherwise
      // keep the dropped toggles alive in memory.
      merge: (persistedState, currentState) => ({
        ...currentState,
        ...normalizePersistedState(persistedState),
      }),
      partialize: (state): PersistedAppState => ({
        storageMode: state.storageMode,
        customStoragePath: state.customStoragePath,
        settings: state.settings,
        modelOverrides: state.modelOverrides,
      }),
    }
  )
)
