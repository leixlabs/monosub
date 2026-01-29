import { useState } from 'react';
import { X, RotateCcw, Globe, ChevronDown, Check } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { useStore, SubtitleFormat, OutputLanguage } from '@/store/useStore';
import { supportedLanguages } from '@/i18n';

export function Settings() {
  const { t, i18n } = useTranslation();
  const [isLangMenuOpen, setIsLangMenuOpen] = useState(false);
  const isOpen = useStore((state) => state.isSettingsOpen);
  const setIsOpen = useStore((state) => state.setIsSettingsOpen);
  const settings = useStore((state) => state.settings);
  const updateSettings = useStore((state) => state.updateSettings);
  const resetSettings = useStore((state) => state.resetSettings);

  if (!isOpen) return null;

  const formatOptions: { value: SubtitleFormat; label: string }[] = [
    { value: 'srt', label: 'SRT' },
    { value: 'vtt', label: 'VTT' },
    { value: 'txt', label: 'TXT' },
  ];

  const languageOptions: { value: OutputLanguage; label: string }[] = [
    { value: 'auto', label: t('language.auto') },
    { value: 'en', label: 'English' },
    { value: 'zh', label: '简体中文' },
    { value: 'es', label: 'Español' },
    { value: 'ja', label: '日本語' },
    { value: 'de', label: 'Deutsch' },
    { value: 'fr', label: 'Français' },
    { value: 'pt', label: 'Português' },
    { value: 'zh-tw', label: '繁體中文' },
    { value: 'ko', label: '한국어' },
  ];

  const handleUILanguageChange = (lang: string) => {
    i18n.changeLanguage(lang);
    localStorage.setItem('monosub-language', lang);
    setIsLangMenuOpen(false);
  };

  const getCurrentLanguage = () => {
    const currentLang = i18n.language;
    return supportedLanguages.find(l =>
      currentLang === l.code || currentLang?.startsWith(l.code.split('-')[0])
    ) || supportedLanguages[1]; // Default to English
  };

  const currentLang = getCurrentLanguage();

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center">
      {/* Backdrop */}
      <div
        className="absolute inset-0 bg-black/30 backdrop-blur-sm animate-fade-in"
        onClick={() => {
          setIsOpen(false);
          setIsLangMenuOpen(false);
        }}
      />

      {/* Panel */}
      <div className="relative bg-bg-card rounded-xl shadow-apple-lg w-[480px] max-h-[80vh] overflow-hidden animate-slide-up">
        {/* Header */}
        <div className="flex items-center justify-between px-6 py-4 border-b border-divider">
          <h2 className="text-lg font-semibold text-text-primary">{t('settings.title')}</h2>
          <button
            onClick={() => setIsOpen(false)}
            className="p-1.5 rounded-md hover:bg-bg-secondary transition-colors"
          >
            <X className="w-5 h-5 text-text-secondary" />
          </button>
        </div>

        {/* Content */}
        <div className="p-6 space-y-6 overflow-y-auto max-h-[calc(80vh-120px)]">
          {/* UI Language */}
          <section>
            <div className="flex items-center justify-between">
              <div className="flex items-center gap-2">
                <Globe className="w-4 h-4 text-text-secondary" />
                <label className="text-sm text-text-secondary">Language</label>
              </div>
              <div className="relative">
                <button
                  onClick={() => setIsLangMenuOpen(!isLangMenuOpen)}
                  className="flex items-center gap-2 px-3 py-2 text-sm bg-bg-secondary border border-border rounded-lg text-text-primary hover:bg-bg-tertiary transition-colors min-w-[180px] justify-between"
                >
                  <span className="flex items-center gap-2">
                    <span className="text-base">{currentLang.flag}</span>
                    <span>{currentLang.name}</span>
                  </span>
                  <ChevronDown className={`w-4 h-4 text-text-tertiary transition-transform ${isLangMenuOpen ? 'rotate-180' : ''}`} />
                </button>

                {/* Language Dropdown */}
                {isLangMenuOpen && (
                  <div className="absolute right-0 top-full mt-1 w-[240px] max-h-[320px] overflow-y-auto bg-bg-card border border-border rounded-lg shadow-apple-lg z-10">
                    {supportedLanguages.map((lang) => (
                      <button
                        key={lang.code}
                        onClick={() => handleUILanguageChange(lang.code)}
                        className={`w-full flex items-center gap-3 px-3 py-2.5 text-sm hover:bg-bg-secondary transition-colors ${
                          currentLang.code === lang.code ? 'bg-primary/5' : ''
                        }`}
                      >
                        <span className="text-base">{lang.flag}</span>
                        <span className="flex-1 text-left text-text-primary">{lang.name}</span>
                        {currentLang.code === lang.code && (
                          <Check className="w-4 h-4 text-primary" />
                        )}
                      </button>
                    ))}
                  </div>
                )}
              </div>
            </div>
          </section>

          <div className="h-px bg-divider" />

          {/* Output Settings */}
          <section>
            <h3 className="text-sm font-medium text-text-primary mb-4">{t('settings.output.title')}</h3>
            <div className="space-y-4">
              {/* Format */}
              <div className="flex items-center justify-between">
                <label className="text-sm text-text-secondary">{t('settings.output.format')}</label>
                <div className="flex gap-1 bg-bg-secondary rounded-lg p-1">
                  {formatOptions.map((opt) => (
                    <button
                      key={opt.value}
                      onClick={() => updateSettings({ outputFormat: opt.value })}
                      className={`px-4 py-1.5 text-sm rounded-md transition-all ${
                        settings.outputFormat === opt.value
                          ? 'bg-white text-text-primary shadow-apple'
                          : 'text-text-secondary hover:text-text-primary'
                      }`}
                    >
                      {opt.label}
                    </button>
                  ))}
                </div>
              </div>

              {/* Language */}
              <div className="flex items-center justify-between">
                <label className="text-sm text-text-secondary">{t('settings.output.language')}</label>
                <select
                  value={settings.outputLanguage}
                  onChange={(e) => updateSettings({ outputLanguage: e.target.value as OutputLanguage })}
                  className="px-3 py-1.5 text-sm bg-bg-secondary border border-border rounded-lg text-text-primary focus:outline-none focus:ring-2 focus:ring-primary/20"
                >
                  {languageOptions.map((opt) => (
                    <option key={opt.value} value={opt.value}>
                      {opt.label}
                    </option>
                  ))}
                </select>
              </div>

              {/* Bilingual */}
              <div className="flex items-center justify-between opacity-60">
                <div>
                  <div className="flex items-center gap-2">
                    <label className="text-sm text-text-secondary">{t('settings.output.bilingual')}</label>
                    <span className="px-2 py-0.5 text-[10px] font-medium text-accent bg-accent/10 rounded-full">
                      Coming Soon
                    </span>
                  </div>
                  <p className="text-xs text-text-tertiary mt-0.5">{t('settings.output.bilingualDesc')}</p>
                </div>
                <label className="relative inline-flex items-center cursor-not-allowed">
                  <input
                    type="checkbox"
                    checked={false}
                    disabled={true}
                    className="sr-only peer"
                  />
                  <div className="w-11 h-6 bg-bg-secondary rounded-full peer peer-checked:bg-primary transition-colors after:content-[''] after:absolute after:top-0.5 after:left-[2px] after:bg-white after:rounded-full after:h-5 after:w-5 after:transition-all peer-checked:after:translate-x-full after:shadow-sm" />
                </label>
              </div>

            </div>
          </section>

          {/* Processing Settings */}
          <section>
            <h3 className="text-sm font-medium text-text-primary mb-4">{t('settings.processing.title')}</h3>
            <div className="space-y-4">
              {/* Noise Reduction */}
              <div className="flex items-center justify-between">
                <div>
                  <label className="text-sm text-text-secondary">{t('settings.processing.noiseReduction')}</label>
                  <p className="text-xs text-text-tertiary mt-0.5">{t('settings.processing.noiseReductionDesc')}</p>
                </div>
                <label className="relative inline-flex items-center cursor-pointer">
                  <input
                    type="checkbox"
                    checked={settings.enableNoiseReduction}
                    onChange={(e) => updateSettings({ enableNoiseReduction: e.target.checked })}
                    className="sr-only peer"
                  />
                  <div className="w-11 h-6 bg-bg-secondary rounded-full peer peer-checked:bg-primary transition-colors after:content-[''] after:absolute after:top-0.5 after:left-[2px] after:bg-white after:rounded-full after:h-5 after:w-5 after:transition-all peer-checked:after:translate-x-full after:shadow-sm" />
                </label>
              </div>

              {/* Volume Normalization */}
              <div className="flex items-center justify-between">
                <div>
                  <label className="text-sm text-text-secondary">{t('settings.processing.volumeNormalization')}</label>
                  <p className="text-xs text-text-tertiary mt-0.5">{t('settings.processing.volumeNormalizationDesc')}</p>
                </div>
                <label className="relative inline-flex items-center cursor-pointer">
                  <input
                    type="checkbox"
                    checked={settings.enableVolumeNormalization}
                    onChange={(e) => updateSettings({ enableVolumeNormalization: e.target.checked })}
                    className="sr-only peer"
                  />
                  <div className="w-11 h-6 bg-bg-secondary rounded-full peer peer-checked:bg-primary transition-colors after:content-[''] after:absolute after:top-0.5 after:left-[2px] after:bg-white after:rounded-full after:h-5 after:w-5 after:transition-all peer-checked:after:translate-x-full after:shadow-sm" />
                </label>
              </div>

              {/* Auto Process Queue */}
              <div className="flex items-center justify-between">
                <div>
                  <label className="text-sm text-text-secondary">{t('settings.processing.autoQueue')}</label>
                  <p className="text-xs text-text-tertiary mt-0.5">{t('settings.processing.autoQueueDesc')}</p>
                </div>
                <label className="relative inline-flex items-center cursor-pointer">
                  <input
                    type="checkbox"
                    checked={settings.autoProcessQueue}
                    onChange={(e) => updateSettings({ autoProcessQueue: e.target.checked })}
                    className="sr-only peer"
                  />
                  <div className="w-11 h-6 bg-bg-secondary rounded-full peer peer-checked:bg-primary transition-colors after:content-[''] after:absolute after:top-0.5 after:left-[2px] after:bg-white after:rounded-full after:h-5 after:w-5 after:transition-all peer-checked:after:translate-x-full after:shadow-sm" />
                </label>
              </div>
            </div>
          </section>

          {/* Notification Settings */}
          <section>
            <h3 className="text-sm font-medium text-text-primary mb-4">{t('settings.notifications.title')}</h3>
            <div className="space-y-4">
              <div className="flex items-center justify-between">
                <div>
                  <label className="text-sm text-text-secondary">{t('settings.notifications.showCompleted')}</label>
                  <p className="text-xs text-text-tertiary mt-0.5">{t('settings.notifications.showCompletedDesc')}</p>
                </div>
                <label className="relative inline-flex items-center cursor-pointer">
                  <input
                    type="checkbox"
                    checked={settings.showCompletedNotification}
                    onChange={(e) => updateSettings({ showCompletedNotification: e.target.checked })}
                    className="sr-only peer"
                  />
                  <div className="w-11 h-6 bg-bg-secondary rounded-full peer peer-checked:bg-primary transition-colors after:content-[''] after:absolute after:top-0.5 after:left-[2px] after:bg-white after:rounded-full after:h-5 after:w-5 after:transition-all peer-checked:after:translate-x-full after:shadow-sm" />
                </label>
              </div>
            </div>
          </section>
        </div>

        {/* Footer */}
        <div className="flex items-center justify-between px-6 py-4 border-t border-divider bg-bg-tertiary">
          <button
            onClick={resetSettings}
            className="flex items-center gap-2 px-4 py-2 text-sm text-text-secondary hover:text-text-primary transition-colors"
          >
            <RotateCcw className="w-4 h-4" />
            {t('settings.resetDefaults')}
          </button>
          <button
            onClick={() => setIsOpen(false)}
            className="px-5 py-2 text-sm font-medium text-white bg-primary rounded-lg hover:bg-primary-hover transition-colors"
          >
            {t('settings.done')}
          </button>
        </div>
      </div>
    </div>
  );
}
