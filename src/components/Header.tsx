import { Settings as SettingsIcon, Subtitles } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { motion } from 'framer-motion';
import { useStore } from '@/store/useStore';
import { ThemeToggle } from './ThemeToggle';

export function Header() {
  const { t } = useTranslation();
  const setIsSettingsOpen = useStore((state) => state.setIsSettingsOpen);
  const files = useStore((state) => state.files);
  const processedCount = files.filter(f => f.status === 'completed').length;
  const totalCount = files.length;

  return (
    <header className="bg-bg-secondary dark:bg-zinc-950 h-[52px] flex items-center pl-20 pr-6 sticky top-0 z-40 transition-colors">
      {/* Logo & Title */}
      <div
        data-tauri-drag-region
        className="drag-region flex items-center gap-3 cursor-grab active:cursor-grabbing"
      >
        <div className="w-8 h-8 rounded-lg bg-gradient-to-br from-primary to-accent flex items-center justify-center">
          <Subtitles className="w-4.5 h-4.5 text-white" />
        </div>
        <div>
          <h1 className="window-title text-base font-semibold leading-tight transition-colors">
            {t('app.name')}
          </h1>
          <p className="text-xxs text-text-tertiary dark:text-gray-500 leading-tight">
            {t('app.tagline')}
          </p>
        </div>
      </div>

      <div data-tauri-drag-region className="drag-region flex-1 h-full cursor-grab active:cursor-grabbing" />

      {/* Status & Settings */}
      <div className="flex items-center gap-3 no-drag">
        {totalCount > 0 && (
          <div className="flex items-center gap-2">
            <span className="text-xs text-text-secondary dark:text-gray-400">
              {processedCount}/{totalCount} {t('header.completed')}
            </span>
            {processedCount > 0 && processedCount < totalCount && (
              <div className="w-16 h-1.5 bg-bg-secondary dark:bg-zinc-800 rounded-full overflow-hidden">
                <div
                  className="h-full bg-gradient-to-r from-primary to-accent transition-all duration-500"
                  style={{ width: `${(processedCount / totalCount) * 100}%` }}
                />
              </div>
            )}
          </div>
        )}
        <ThemeToggle />
        <motion.button
          whileTap={{ scale: 0.97 }}
          onClick={() => setIsSettingsOpen(true)}
          className="p-2 rounded-xl hover:bg-bg-secondary dark:hover:bg-zinc-800 transition-colors group no-drag"
          title={t('header.settings')}
        >
          <SettingsIcon className="w-5 h-5 text-text-secondary dark:text-gray-400 group-hover:text-text-primary dark:group-hover:text-white transition-colors" />
        </motion.button>
      </div>
    </header>
  );
}
