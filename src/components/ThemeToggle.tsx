import { Moon, Sun } from 'lucide-react'
import { AnimatePresence, motion } from 'framer-motion'
import { useTheme } from '@/hooks/useTheme'
import { GlassButton } from '@/components/ui/glass-button'

export function ThemeToggle() {
  const { effectiveTheme, setTheme } = useTheme()
  const isLight = effectiveTheme === 'light'

  return (
    <GlassButton
      variant="ghost"
      size="icon"
      onClick={() => setTheme(isLight ? 'dark' : 'light')}
      aria-label={`Switch to ${isLight ? 'dark' : 'light'} mode`}
      className="no-drag overflow-hidden"
    >
      {/* Icon cross-fades and rotates instead of snapping. */}
      <AnimatePresence mode="wait" initial={false}>
        <motion.span
          key={effectiveTheme}
          initial={{ opacity: 0, rotate: -60, scale: 0.7 }}
          animate={{ opacity: 1, rotate: 0, scale: 1 }}
          exit={{ opacity: 0, rotate: 60, scale: 0.7 }}
          transition={{ duration: 0.18, ease: [0.4, 0, 0.2, 1] }}
          className="grid place-items-center"
        >
          {isLight ? <Moon className="w-[18px] h-[18px]" /> : <Sun className="w-[18px] h-[18px]" />}
        </motion.span>
      </AnimatePresence>
    </GlassButton>
  )
}
