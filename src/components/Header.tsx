import { Settings as SettingsIcon, Subtitles } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { useStore } from '@/store/useStore';

export function Header() {
  const { t } = useTranslation();
  const setIsSettingsOpen = useStore((state) => state.setIsSettingsOpen);
  const files = useStore((state) => state.files);
  const processedCount = files.filter(f => f.status === 'completed').length;
  const totalCount = files.length;

  return (
    <header className="bg-bg-card/80 backdrop-blur-lg border-b border-divider h-[52px] flex items-center justify-between px-6 sticky top-0 z-40">
      {/* Logo & Title */}
      <div className="flex items-center gap-3">
        <div className="w-8 h-8 rounded-lg bg-gradient-to-br from-primary to-accent flex items-center justify-center">
          <Subtitles className="w-4.5 h-4.5 text-white" />
        </div>
        <div>
          <h1 className="text-base font-semibold text-text-primary leading-tight">
            {t('app.name')}
          </h1>
          <p className="text-xxs text-text-tertiary leading-tight">
            {t('app.tagline')}
          </p>
        </div>
      </div>

      {/* Status & Settings */}
      <div className="flex items-center gap-4">
        {totalCount > 0 && (
          <div className="flex items-center gap-2">
            <span className="text-xs text-text-secondary">
              {processedCount}/{totalCount} {t('header.completed')}
            </span>
            {processedCount > 0 && processedCount < totalCount && (
              <div className="w-16 h-1.5 bg-bg-secondary rounded-full overflow-hidden">
                <div
                  className="h-full bg-gradient-to-r from-primary to-accent transition-all duration-500"
                  style={{ width: `${(processedCount / totalCount) * 100}%` }}
                />
              </div>
            )}
          </div>
        )}
        <button
          onClick={() => setIsSettingsOpen(true)}
          className="p-2 rounded-lg hover:bg-bg-secondary transition-colors group"
          title={t('header.settings')}
        >
          <SettingsIcon className="w-5 h-5 text-text-secondary group-hover:text-text-primary transition-colors" />
        </button>
      </div>
    </header>
  );
}
