import { create } from 'zustand'

export type VideoStatus = 'pending' | 'processing' | 'completed' | 'error' | 'paused'
export type StorageMode = 'same' | 'custom'

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
}

export const useStore = create<AppState>((set) => ({
  // File queue
  files: [],
  addFiles: (newFiles) => set((state) => ({
    files: [...state.files, ...newFiles]
  })),
  removeFile: (id) => set((state) => ({
    files: state.files.filter(f => f.id !== id)
  })),
  clearFiles: () => set({ files: [] }),
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
}))
