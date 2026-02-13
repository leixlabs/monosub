import { create } from 'zustand'
import { persist } from 'zustand/middleware'

export type VideoStatus = 'pending' | 'processing' | 'completed' | 'error' | 'paused'
export type StorageMode = 'same' | 'custom'
export type SubtitleFormat = 'srt' | 'vtt' | 'txt'
export type OutputLanguage = 'auto' | 'en' | 'zh' | 'es' | 'ja' | 'de' | 'fr' | 'pt' | 'zh-tw' | 'ko'

export interface VideoFile {
  id: string
  name: string
  path: string
  size: number
  duration: number
  thumbnail?: string
  status: VideoStatus
  progress: number
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
      }),
    }
  )
)
