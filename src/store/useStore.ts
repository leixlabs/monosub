import { create } from 'zustand'
import { persist } from 'zustand/middleware'

export type VideoStatus = 'pending' | 'processing' | 'completed' | 'error' | 'paused'
export type StorageMode = 'same' | 'custom'
export type SubtitleFormat = 'srt' | 'vtt' | 'txt'
export type OutputLanguage = 'auto' | 'en' | 'zh' | 'es' | 'ja' | 'de' | 'fr' | 'pt' | 'zh-tw' | 'ko'
export type LicenseStatus = 'unknown' | 'checking' | 'valid' | 'invalid' | 'error'

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
}

export interface AppSettings {
  // Output settings
  outputFormat: SubtitleFormat
  outputLanguage: OutputLanguage
  enableBilingual: boolean
  bilingualTargetLang: 'en' | 'zh'

  // Processing settings
  enableNoiseReduction: boolean
  enableVolumeNormalization: boolean
  autoProcessQueue: boolean

  // UI settings
  showCompletedNotification: boolean
}

export interface LicenseConfig {
  licenseKey: string
  licenseActivationId: string
  licenseDeviceId: string
}

// Detect system language
const detectSystemLanguage = (): OutputLanguage => {
  const lang = navigator.language || (navigator as any).userLanguage || 'auto'
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
  enableBilingual: false,
  bilingualTargetLang: 'en',
  enableNoiseReduction: true,
  enableVolumeNormalization: true,
  autoProcessQueue: true,
  showCompletedNotification: true,
}

const defaultLicenseConfig: LicenseConfig = {
  licenseKey: '',
  licenseActivationId: '',
  licenseDeviceId: '',
}

interface AppState {
  // File queue management
  files: VideoFile[]
  addFiles: (files: VideoFile[]) => void
  removeFile: (id: string) => void
  clearFiles: () => void
  updateFile: (id: string, updates: Partial<VideoFile>) => void

  // Processing settings
  storageMode: StorageMode
  setStorageMode: (mode: StorageMode) => void
  customStoragePath: string
  setCustomStoragePath: (path: string) => void

  // Speaker diarization (multi-speaker recognition)
  enableSpeakerDiarization: boolean
  setEnableSpeakerDiarization: (enabled: boolean) => void

  // License / Polar
  licenseConfig: LicenseConfig
  updateLicenseConfig: (updates: Partial<LicenseConfig>) => void
  setLicenseKey: (licenseKey: string) => void
  setLicenseActivationId: (activationId: string) => void
  setLicenseDeviceId: (deviceId: string) => void
  licenseStatus: LicenseStatus
  licenseMessage: string
  licenseExpiresAt: string | null
  licenseLastCheckedAt: string | null
  setLicenseStatus: (status: LicenseStatus) => void
  setLicenseValidation: (updates: {
    status: LicenseStatus
    message: string
    expiresAt?: string | null
    checkedAt?: string
  }) => void
  clearLicenseValidation: () => void

  // Global state
  isProcessing: boolean
  setIsProcessing: (isProcessing: boolean) => void
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

      // Settings
      storageMode: 'same',
      setStorageMode: (mode) => set({ storageMode: mode }),
      customStoragePath: '',
      setCustomStoragePath: (path) => set({ customStoragePath: path }),

      // Speaker diarization
      enableSpeakerDiarization: false,
      setEnableSpeakerDiarization: (enabled) => set({ enableSpeakerDiarization: enabled }),

      // License / Polar
      licenseConfig: defaultLicenseConfig,
      updateLicenseConfig: (updates) => set((state) => ({
        licenseConfig: { ...state.licenseConfig, ...updates }
      })),
      setLicenseKey: (licenseKey) => set((state) => {
        const normalizedCurrent = typeof state.licenseConfig.licenseKey === 'string'
          ? state.licenseConfig.licenseKey.trim()
          : '';
        const normalizedNext = typeof licenseKey === 'string' ? licenseKey.trim() : '';
        const shouldResetActivation = normalizedCurrent !== normalizedNext;
        return {
          licenseConfig: {
            ...state.licenseConfig,
            licenseKey,
            licenseActivationId: shouldResetActivation ? '' : state.licenseConfig.licenseActivationId,
          },
          licenseStatus: shouldResetActivation ? 'unknown' : state.licenseStatus,
          licenseMessage: shouldResetActivation ? '' : state.licenseMessage,
          licenseExpiresAt: shouldResetActivation ? null : state.licenseExpiresAt,
          licenseLastCheckedAt: shouldResetActivation ? null : state.licenseLastCheckedAt,
        };
      }),
      setLicenseActivationId: (licenseActivationId) => set((state) => ({
        licenseConfig: { ...state.licenseConfig, licenseActivationId }
      })),
      setLicenseDeviceId: (licenseDeviceId) => set((state) => ({
        licenseConfig: { ...state.licenseConfig, licenseDeviceId }
      })),
      licenseStatus: 'unknown',
      licenseMessage: '',
      licenseExpiresAt: null,
      licenseLastCheckedAt: null,
      setLicenseStatus: (status) => set({ licenseStatus: status }),
      setLicenseValidation: ({ status, message, expiresAt = null, checkedAt }) => set({
        licenseStatus: status,
        licenseMessage: message,
        licenseExpiresAt: expiresAt,
        licenseLastCheckedAt: checkedAt ?? new Date().toISOString(),
      }),
      clearLicenseValidation: () => set({
        licenseStatus: 'unknown',
        licenseMessage: '',
        licenseExpiresAt: null,
        licenseLastCheckedAt: null,
      }),

      // Global state
      isProcessing: false,
      setIsProcessing: (isProcessing) => set({ isProcessing }),
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
      partialize: (state) => ({
        storageMode: state.storageMode,
        customStoragePath: state.customStoragePath,
        settings: state.settings,
        licenseConfig: state.licenseConfig,
      }),
    }
  )
)
