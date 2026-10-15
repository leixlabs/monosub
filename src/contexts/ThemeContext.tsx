import React, { createContext, useEffect, useState } from 'react'
import { getCurrentWindow } from '@tauri-apps/api/window'

type Theme = 'light' | 'dark' | 'system'

interface ThemeContextType {
  theme: Theme
  setTheme: (theme: Theme) => void
  effectiveTheme: 'light' | 'dark'
}

export const ThemeContext = createContext<ThemeContextType | undefined>(
  undefined
)

export function ThemeProvider({ children }: { children: React.ReactNode }) {
  const [theme, setTheme] = useState<Theme>('system')
  const [effectiveTheme, setEffectiveTheme] = useState<'light' | 'dark'>('light')

  // Load theme from localStorage on mount
  useEffect(() => {
    const stored = localStorage.getItem('monosub-theme') as Theme | null
    if (stored && ['light', 'dark', 'system'].includes(stored)) {
      setTheme(stored)
    }
  }, [])

  // Apply theme to document and update effectiveTheme
  useEffect(() => {
    const root = window.document.documentElement
    root.classList.remove('light', 'dark')

    let resolvedTheme: 'light' | 'dark'

    if (theme === 'system') {
      const systemTheme = window.matchMedia('(prefers-color-scheme: dark)')
        .matches
        ? 'dark'
        : 'light'
      resolvedTheme = systemTheme
      root.classList.add(systemTheme)
    } else {
      resolvedTheme = theme
      root.classList.add(theme)
    }

    setEffectiveTheme(resolvedTheme)
    localStorage.setItem('monosub-theme', theme)

    // Listen for system theme changes when using 'system' mode
    if (theme === 'system') {
      const mediaQuery = window.matchMedia('(prefers-color-scheme: dark)')
      const handleChange = (e: MediaQueryListEvent) => {
        const newTheme = e.matches ? 'dark' : 'light'
        root.classList.remove('light', 'dark')
        root.classList.add(newTheme)
        setEffectiveTheme(newTheme)
      }

      mediaQuery.addEventListener('change', handleChange)
      return () => mediaQuery.removeEventListener('change', handleChange)
    }
  }, [theme])

  // Synchronize native window theme
  useEffect(() => {
    const syncNativeTheme = async () => {
      // Only run in Tauri environment
      if (typeof window !== 'undefined' && (window as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__) {
        try {
          const win = getCurrentWindow()
          if (win && typeof win.setTheme === 'function') {
            await win.setTheme(effectiveTheme)
          }
        } catch (error) {
          console.error('Failed to sync native theme:', error)
        }
      }
    }
    syncNativeTheme()
  }, [effectiveTheme])

  return (
    <ThemeContext.Provider value={{ theme, setTheme, effectiveTheme }}>
      {children}
    </ThemeContext.Provider>
  )
}
