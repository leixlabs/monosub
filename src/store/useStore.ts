import { create } from 'zustand'
import { persist } from 'zustand/middleware'

export type VideoStatus = 'pending' | 'processing' | 'completed' | 'skipped' | 'error'
export type StorageMode = 'same' | 'custom'
export type SubtitleFormat = 'srt' | 'vtt' | 'txt' | 'md' | string  // string allows multi-format like "srt,txt"
export type OutputLanguage = 'auto' | 'en' | 'zh' | 'es' | 'ja' | 'de' | 'fr' | 'pt' | 'zh-tw' | 'ko'
/**
 * Language a local translation model is asked for (translation-design §7).
 *
 * `auto` is deliberately absent: it is a *source*-detection value, and
 * "translate into auto" has no meaning. The Rust side skips such a run rather
 * than guessing (`translate::language::should_translate`).
 */
export type TranslateTargetLanguage = Exclude<OutputLanguage, 'auto'>
/** Speed preference used only to rank same-capability models (design §5.3). */
export type SpeedPreference = 'fast' | 'balanced' | 'accurate'
/** Step roles a user may pin to a specific model id (design §8.1). */
export type ModelOverrideSlot = 'asr' | 'vad' | 'align' | 'diar'

export interface StepProgress {
  currentStep: number
  totalSteps: number
  stepName: string
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
  stepProgress?: StepProgress
  totalTime?: number  // total processing time in seconds (set on completion)
  error?: string
  subtitlePath?: string
  /**
   * Per-file speaker separation. Diarization is a property of the recording,
   * not of the app: a single-narrator file can skip it, which also avoids
   * loading the diar / passthrough model at all (much faster).
   * Seeded from `settings.defaultDiarization` when the file is queued.
   */
  diarization: boolean
}

export interface AppSettings {
  // Output settings
  outputFormat: SubtitleFormat
  outputLanguage: OutputLanguage

  // Processing settings
  skipExistingSubtitle: boolean

  // UI settings
  showCompletedNotification: boolean

  // Recognition plan requirements (v3; design §10.1)
  requireWordTimestamps: boolean
  speedPreference: SpeedPreference

  /**
   * Default speaker separation for newly queued files. Each file keeps its own
   * toggle afterwards (see `VideoFile.diarization`), so one narrator-only file
   * can skip the diar model without changing the app-wide default.
   */
  defaultDiarization: boolean

  // Translation (v4; translation-design §7). Translation is an enhancement: it
  // is off by default, and a run that never enables it behaves exactly as it
  // did before the feature existed.
  /** Whether recognition is followed by a translation pass. */
  translateEnabled: boolean
  /** Language the subtitles are translated into (never `auto`). */
  translateTargetLanguage: TranslateTargetLanguage
  /** Pinned translation-model id; `''` means "use the catalogue default". */
  translationModelId: string
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
  speedPreference: 'balanced',
  defaultDiarization: true,
  translateEnabled: false,
  // `en` is the non-source default the design names (translation-design §7). The
  // source is usually Chinese, so translating *into* English is the common case;
  // a user whose source is already English picks another target.
  translateTargetLanguage: 'en',
  translationModelId: '',
}

const SPEED_PREFERENCES: SpeedPreference[] = ['fast', 'balanced', 'accurate']
const MODEL_OVERRIDE_SLOTS: ModelOverrideSlot[] = ['asr', 'vad', 'align', 'diar']

const isSpeedPreference = (value: unknown): value is SpeedPreference =>
  typeof value === 'string' && (SPEED_PREFERENCES as string[]).includes(value)

/**
 * Targets the UI offers, ordered like the recognition languages above.
 * A translation model may support more languages than this; these are the ones
 * the app also knows how to name the translated file for (translation-design §5).
 */
export const TRANSLATE_TARGET_LANGUAGES: TranslateTargetLanguage[] = [
  'en', 'zh', 'zh-tw', 'ja', 'ko', 'es', 'fr', 'de', 'pt',
]

const isTranslateTargetLanguage = (value: unknown): value is TranslateTargetLanguage =>
  typeof value === 'string' && (TRANSLATE_TARGET_LANGUAGES as string[]).includes(value)

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
}

// Persisted slice. Kept in sync with the `partialize` option below.
type PersistedAppState = Pick<
  AppState,
  'storageMode' | 'customStoragePath' | 'settings' | 'modelOverrides'
>

// v3 adds the recognition-plan requirements and `modelOverrides` (design §10.3);
// v4 adds the translation settings (translation-design §7). Both must only ADD:
// unlike v1→v2 they drop nothing.
const PERSIST_VERSION = 4

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
    speedPreference: isSpeedPreference(persisted.speedPreference) ? persisted.speedPreference : defaultSettings.speedPreference,
    defaultDiarization: typeof persisted.defaultDiarization === 'boolean' ? persisted.defaultDiarization : defaultSettings.defaultDiarization,
    // v3 blobs carry none of these; defaults are applied and nothing already
    // present in the blob is dropped.
    translateEnabled: typeof persisted.translateEnabled === 'boolean' ? persisted.translateEnabled : defaultSettings.translateEnabled,
    translateTargetLanguage: isTranslateTargetLanguage(persisted.translateTargetLanguage) ? persisted.translateTargetLanguage : defaultSettings.translateTargetLanguage,
    translationModelId: typeof persisted.translationModelId === 'string' ? persisted.translationModelId : defaultSettings.translationModelId,
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
