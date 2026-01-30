import { Moon, Sun } from 'lucide-react'
import { useTheme } from '@/hooks/useTheme'
import { motion } from 'framer-motion'

export function ThemeToggle() {
  const { effectiveTheme, setTheme } = useTheme()

  const toggleTheme = () => {
    setTheme(effectiveTheme === 'light' ? 'dark' : 'light')
  }

  return (
    <motion.button
      whileTap={{ scale: 0.95 }}
      onClick={toggleTheme}
      className="p-2 rounded-xl hover:bg-bg-tertiary dark:hover:bg-zinc-800 transition-colors no-drag"
      aria-label={`Switch to ${effectiveTheme === 'light' ? 'dark' : 'light'} mode`}
    >
      {effectiveTheme === 'light' ? (
        <Moon size={20} className="text-text-secondary dark:text-gray-400" />
      ) : (
        <Sun size={20} className="text-text-secondary dark:text-gray-400" />
      )}
    </motion.button>
  )
}
