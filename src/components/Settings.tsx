import { useState, useEffect } from 'react';
import { X, RotateCcw, Globe, ChevronDown, Check, ShieldCheck, ShieldAlert, RefreshCw, Loader2 } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { motion, AnimatePresence } from 'framer-motion';
import { useStore, SubtitleFormat, OutputLanguage } from '@/store/useStore';
import { supportedLanguages } from '@/i18n';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

export function Settings() {
  const { t, i18n } = useTranslation();
  const [isLangMenuOpen, setIsLangMenuOpen] = useState(false);
  const isOpen = useStore((state) => state.isSettingsOpen);
  const setIsOpen = useStore((state) => state.setIsSettingsOpen);
  const settings = useStore((state) => state.settings);
  const updateSettings = useStore((state) => state.updateSettings);
  const resetSettings = useStore((state) => state.resetSettings);

  const [envStatus, setEnvStatus] = useState<'idle' | 'checking' | 'healthy' | 'broken' | 'error'>('idle');
  const [isRepairing, setIsRepairing] = useState(false);
  const [repairProgress, setRepairProgress] = useState(0);
  const [repairMsg, setRepairMsg] = useState('');

  useEffect(() => {
    if (!isOpen) return;

    const setupListeners = async () => {
      const unlistenProgress = await listen('install_progress', (event: any) => {
        const { progress, status } = event.payload;
        if (progress > 0) setRepairProgress(progress);
        setRepairMsg(status);
      });

      const unlistenComplete = await listen('install_complete', () => {
        setIsRepairing(false);
        setEnvStatus('healthy');
        setRepairMsg(t('settings.environment.repairSuccess'));
      });

      return () => {
        unlistenProgress();
        unlistenComplete();
      };
    };

    const cleanup = setupListeners();
    checkEnv();

    return () => {
      cleanup.then(fn => fn && fn());
    };
  }, [isOpen]);

  const checkEnv = async () => {
    setEnvStatus('checking');
    try {
      const isHealthy = await invoke<boolean>('check_dependencies');
      setEnvStatus(isHealthy ? 'healthy' : 'broken');
    } catch (error) {
      console.error('Failed to check env:', error);
      setEnvStatus('error');
    }
  };

  const repairEnv = async () => {
    setIsRepairing(true);
    setRepairProgress(0);
    setRepairMsg(t('settings.environment.repairing'));
    try {
      await invoke('install_dependencies');
    } catch (error) {
      console.error('Failed to repair env:', error);
      setIsRepairing(false);
      setRepairMsg(t('settings.environment.repairFailed'));
    }
  };

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
      <motion.div
        initial={{ opacity: 0 }}
        animate={{ opacity: 1 }}
        exit={{ opacity: 0 }}
        className="absolute inset-0 bg-black/40 dark:bg-black/60 backdrop-blur-xl"
        onClick={() => {
          setIsOpen(false);
          setIsLangMenuOpen(false);
        }}
      />

      {/* Panel */}
      <motion.div
        initial={{ opacity: 0, scale: 0.95 }}
        animate={{ opacity: 1, scale: 1 }}
        exit={{ opacity: 0, scale: 0.95 }}
        transition={{ type: "spring", stiffness: 300, damping: 30 }}
        className="relative glass-medium dark:glass-dark-strong rounded-3xl shadow-glass-lg w-[480px] max-h-[80vh] overflow-hidden"
      >
        {/* Header */}
        <div className="flex items-center justify-between px-6 py-4 border-b border-glass-border-light dark:border-glass-border-dark">
          <h2 className="text-lg font-bold text-text-primary dark:text-white">{t('settings.title')}</h2>
          <motion.button
            whileTap={{ scale: 0.95 }}
            onClick={() => setIsOpen(false)}
            className="p-1.5 rounded-xl hover:bg-bg-secondary dark:hover:bg-zinc-800 transition-colors"
          >
            <X className="w-5 h-5 text-text-primary dark:text-gray-300" />
          </motion.button>
        </div>

        {/* Content */}
        <div className="p-6 space-y-6 overflow-y-auto max-h-[calc(80vh-120px)]">
          {/* UI Language */}
          <section>
            <div className="flex items-center justify-between">
              <div className="flex items-center gap-2">
                <Globe className="w-4 h-4 text-text-primary dark:text-gray-300" />
                <label className="text-sm font-semibold text-text-primary dark:text-gray-200">Language</label>
              </div>
              <div className="relative">
                <motion.button
                  whileTap={{ scale: 0.97 }}
                  onClick={() => setIsLangMenuOpen(!isLangMenuOpen)}
                  className="flex items-center gap-2 px-3 py-2 text-sm bg-bg-secondary dark:bg-zinc-800 border border-border dark:border-zinc-700 rounded-xl text-text-primary dark:text-white hover:bg-bg-tertiary dark:hover:bg-zinc-700 transition-colors min-w-[180px] justify-between shadow-sm"
                >
                  <span className="flex items-center gap-2">
                    <span className="text-base">{currentLang.flag}</span>
                    <span className="font-medium">{currentLang.name}</span>
                  </span>
                  <ChevronDown className={`w-4 h-4 text-text-secondary dark:text-gray-400 transition-transform ${isLangMenuOpen ? 'rotate-180' : ''}`} />
                </motion.button>

                {/* Language Dropdown */}
                {isLangMenuOpen && (
                  <div className="absolute right-0 top-full mt-1 w-[240px] max-h-[320px] overflow-y-auto glass-medium dark:glass-dark-strong rounded-xl shadow-glass-lg z-10">
                    {supportedLanguages.map((lang) => (
                      <motion.button
                        key={lang.code}
                        whileTap={{ scale: 0.98 }}
                        onClick={() => handleUILanguageChange(lang.code)}
                        className={`w-full flex items-center gap-3 px-3 py-2.5 text-sm hover:bg-bg-secondary dark:hover:bg-zinc-800 transition-colors ${currentLang.code === lang.code ? 'bg-primary/5 dark:bg-primary/10' : ''
                          }`}
                      >
                        <span className="text-base">{lang.flag}</span>
                        <span className="flex-1 text-left text-text-primary dark:text-white">{lang.name}</span>
                        {currentLang.code === lang.code && (
                          <Check className="w-4 h-4 text-primary" />
                        )}
                      </motion.button>
                    ))}
                  </div>
                )}
              </div>
            </div>
          </section>

          <div className="h-px bg-divider dark:bg-zinc-700" />

          {/* Output Settings */}
          <section>
            <h3 className="text-sm font-bold text-text-primary dark:text-white mb-4">{t('settings.output.title')}</h3>
            <div className="space-y-4">
              {/* Format */}
              <div className="flex items-center justify-between">
                <label className="text-sm font-semibold text-text-primary dark:text-gray-200">{t('settings.output.format')}</label>
                <div className="flex gap-1 bg-bg-secondary dark:bg-zinc-800 rounded-lg p-1">
                  {formatOptions.map((opt) => (
                    <motion.button
                      key={opt.value}
                      whileTap={{ scale: 0.97 }}
                      onClick={() => updateSettings({ outputFormat: opt.value })}
                      className={`px-4 py-1.5 text-sm rounded-md transition-all ${settings.outputFormat === opt.value
                          ? 'bg-white dark:bg-zinc-700 text-text-primary dark:text-white shadow-glass-sm'
                          : 'text-text-secondary dark:text-gray-400 hover:text-text-primary dark:hover:text-white'
                        }`}
                    >
                      {opt.label}
                    </motion.button>
                  ))}
                </div>
              </div>

              {/* Language */}
              <div className="flex items-center justify-between">
                <label className="text-sm font-semibold text-text-primary dark:text-gray-200">{t('settings.output.language')}</label>
                <select
                  value={settings.outputLanguage}
                  onChange={(e) => updateSettings({ outputLanguage: e.target.value as OutputLanguage })}
                  className="px-3 py-1.5 text-sm bg-bg-secondary dark:bg-zinc-800 border border-border dark:border-zinc-700 rounded-xl text-text-primary dark:text-white focus:outline-none focus:ring-2 focus:ring-primary/20 shadow-sm font-medium"
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
                    <label className="text-sm font-semibold text-text-primary dark:text-gray-200">{t('settings.output.bilingual')}</label>
                    <span className="px-2 py-0.5 text-[10px] font-bold text-accent bg-accent/10 dark:bg-accent/20 rounded-full border border-accent/20">
                      Coming Soon
                    </span>
                  </div>
                  <p className="text-xs text-text-secondary dark:text-gray-400 mt-0.5">{t('settings.output.bilingualDesc')}</p>
                </div>
                <label className="relative inline-flex items-center cursor-not-allowed">
                  <input
                    type="checkbox"
                    checked={false}
                    disabled={true}
                    className="sr-only peer"
                  />
                  <div className="w-11 h-6 bg-bg-secondary dark:bg-zinc-800 rounded-full peer peer-checked:bg-primary transition-colors after:content-[''] after:absolute after:top-0.5 after:left-[2px] after:bg-white after:rounded-full after:h-5 after:w-5 after:transition-all peer-checked:after:translate-x-full after:shadow-sm" />
                </label>
              </div>

            </div>
          </section>

          {/* Processing Settings */}
          <section>
            <h3 className="text-sm font-bold text-text-primary dark:text-white mb-4">{t('settings.processing.title')}</h3>
            <div className="space-y-4">
              {/* Noise Reduction */}
              <div className="flex items-center justify-between">
                <div>
                  <label className="text-sm font-semibold text-text-primary dark:text-gray-200">{t('settings.processing.noiseReduction')}</label>
                  <p className="text-xs text-text-secondary dark:text-gray-400 mt-0.5">{t('settings.processing.noiseReductionDesc')}</p>
                </div>
                <label className="relative inline-flex items-center cursor-pointer">
                  <input
                    type="checkbox"
                    checked={settings.enableNoiseReduction}
                    onChange={(e) => updateSettings({ enableNoiseReduction: e.target.checked })}
                    className="sr-only peer"
                  />
                  <div className="w-11 h-6 bg-bg-secondary dark:bg-zinc-800 rounded-full peer peer-checked:bg-primary transition-colors after:content-[''] after:absolute after:top-0.5 after:left-[2px] after:bg-white after:rounded-full after:h-5 after:w-5 after:transition-all peer-checked:after:translate-x-full after:shadow-sm" />
                </label>
              </div>

              {/* Volume Normalization */}
              <div className="flex items-center justify-between">
                <div>
                  <label className="text-sm font-semibold text-text-primary dark:text-gray-200">{t('settings.processing.volumeNormalization')}</label>
                  <p className="text-xs text-text-secondary dark:text-gray-400 mt-0.5">{t('settings.processing.volumeNormalizationDesc')}</p>
                </div>
                <label className="relative inline-flex items-center cursor-pointer">
                  <input
                    type="checkbox"
                    checked={settings.enableVolumeNormalization}
                    onChange={(e) => updateSettings({ enableVolumeNormalization: e.target.checked })}
                    className="sr-only peer"
                  />
                  <div className="w-11 h-6 bg-bg-secondary dark:bg-zinc-800 rounded-full peer peer-checked:bg-primary transition-colors after:content-[''] after:absolute after:top-0.5 after:left-[2px] after:bg-white after:rounded-full after:h-5 after:w-5 after:transition-all peer-checked:after:translate-x-full after:shadow-sm" />
                </label>
              </div>

              {/* Auto Process Queue */}
              <div className="flex items-center justify-between">
                <div>
                  <label className="text-sm font-semibold text-text-primary dark:text-gray-200">{t('settings.processing.autoQueue')}</label>
                  <p className="text-xs text-text-secondary dark:text-gray-400 mt-0.5">{t('settings.processing.autoQueueDesc')}</p>
                </div>
                <label className="relative inline-flex items-center cursor-pointer">
                  <input
                    type="checkbox"
                    checked={settings.autoProcessQueue}
                    onChange={(e) => updateSettings({ autoProcessQueue: e.target.checked })}
                    className="sr-only peer"
                  />
                  <div className="w-11 h-6 bg-bg-secondary dark:bg-zinc-800 rounded-full peer peer-checked:bg-primary transition-colors after:content-[''] after:absolute after:top-0.5 after:left-[2px] after:bg-white after:rounded-full after:h-5 after:w-5 after:transition-all peer-checked:after:translate-x-full after:shadow-sm" />
                </label>
              </div>
            </div>
          </section>

          <div className="h-px bg-divider dark:bg-zinc-700" />

          {/* AI Environment Settings */}
          <section>
            <h3 className="text-sm font-bold text-text-primary dark:text-white mb-4">{t('settings.environment.title')}</h3>
            <div className="space-y-4">
              <div className="flex items-center justify-between p-3 bg-bg-secondary dark:bg-zinc-800/50 rounded-2xl border border-divider dark:border-zinc-700">
                <div className="flex items-center gap-3">
                  {envStatus === 'checking' && <Loader2 className="w-5 h-5 text-primary animate-spin" />}
                  {envStatus === 'healthy' && <ShieldCheck className="w-5 h-5 text-success" />}
                  {envStatus === 'broken' && <ShieldAlert className="w-5 h-5 text-danger" />}
                  {envStatus === 'error' && <ShieldAlert className="w-5 h-5 text-warning" />}
                  {envStatus === 'idle' && <RefreshCw className="w-5 h-5 text-text-tertiary" />}

                  <div>
                    <p className="text-sm font-bold text-text-primary dark:text-white leading-none">
                      {t(`settings.environment.status.${envStatus}`)}
                    </p>
                    {repairMsg && (
                      <p className="text-xxs text-text-secondary dark:text-gray-400 mt-1.5 flex items-center gap-1.5">
                        <span className="w-1 h-1 rounded-full bg-primary/40" />
                        {repairMsg}
                      </p>
                    )}
                  </div>
                </div>

                <div className="flex gap-2">
                  <motion.button
                    whileTap={{ scale: 0.95 }}
                    onClick={checkEnv}
                    disabled={envStatus === 'checking' || isRepairing}
                    className="p-2 rounded-xl bg-white dark:bg-zinc-700 text-text-primary dark:text-white border border-border dark:border-zinc-600 hover:bg-bg-secondary dark:hover:bg-zinc-600 disabled:opacity-50 transition-colors shadow-sm"
                    title={t('settings.environment.check')}
                  >
                    <RefreshCw className={`w-4 h-4 ${envStatus === 'checking' ? 'animate-spin' : ''}`} />
                  </motion.button>
                  <motion.button
                    whileTap={{ scale: 0.95 }}
                    onClick={repairEnv}
                    disabled={isRepairing}
                    className="flex items-center gap-2 px-3 py-2 text-xs font-bold text-white bg-primary rounded-xl hover:bg-primary-hover disabled:opacity-50 transition-colors shadow-glass-sm"
                  >
                    {isRepairing ? <Loader2 className="w-3.5 h-3.5 animate-spin" /> : <RefreshCw className="w-3.5 h-3.5" />}
                    {t('settings.environment.repair')}
                  </motion.button>
                </div>
              </div>

              <AnimatePresence>
                {isRepairing && (
                  <motion.div
                    initial={{ opacity: 0, height: 0 }}
                    animate={{ opacity: 1, height: 'auto' }}
                    exit={{ opacity: 0, height: 0 }}
                    className="overflow-hidden"
                  >
                    <div className="w-full bg-divider dark:bg-zinc-700 rounded-full h-1.5 mt-2">
                      <motion.div
                        initial={{ width: 0 }}
                        animate={{ width: `${repairProgress}%` }}
                        className="bg-primary h-full rounded-full"
                      />
                    </div>
                  </motion.div>
                )}
              </AnimatePresence>

              <p className="text-xs text-text-secondary dark:text-gray-400 leading-relaxed px-1">
                {t('settings.environment.repairDesc')}
              </p>
            </div>
          </section>

          <div className="h-px bg-divider dark:bg-zinc-700" />

          {/* Notification Settings */}
          <section>
            <h3 className="text-sm font-bold text-text-primary dark:text-white mb-4">{t('settings.notifications.title')}</h3>
            <div className="space-y-4">
              <div className="flex items-center justify-between">
                <div>
                  <label className="text-sm font-semibold text-text-primary dark:text-gray-200">{t('settings.notifications.showCompleted')}</label>
                  <p className="text-xs text-text-secondary dark:text-gray-400 mt-0.5">{t('settings.notifications.showCompletedDesc')}</p>
                </div>
                <label className="relative inline-flex items-center cursor-pointer">
                  <input
                    type="checkbox"
                    checked={settings.showCompletedNotification}
                    onChange={(e) => updateSettings({ showCompletedNotification: e.target.checked })}
                    className="sr-only peer"
                  />
                  <div className="w-11 h-6 bg-bg-secondary dark:bg-zinc-800 rounded-full peer peer-checked:bg-primary transition-colors after:content-[''] after:absolute after:top-0.5 after:left-[2px] after:bg-white after:rounded-full after:h-5 after:w-5 after:transition-all peer-checked:after:translate-x-full after:shadow-sm" />
                </label>
              </div>
            </div>
          </section>
        </div>

        {/* Footer */}
        <div className="flex items-center justify-between px-6 py-4 border-t border-glass-border-light dark:border-glass-border-dark bg-bg-tertiary dark:bg-zinc-900/50">
          <motion.button
            whileTap={{ scale: 0.97 }}
            onClick={resetSettings}
            className="flex items-center gap-2 px-4 py-2 text-sm font-bold text-text-primary dark:text-gray-200 hover:text-primary dark:hover:text-primary transition-colors"
          >
            <RotateCcw className="w-4 h-4" />
            {t('settings.resetDefaults')}
          </motion.button>
          <motion.button
            whileTap={{ scale: 0.97 }}
            onClick={() => setIsOpen(false)}
            className="px-5 py-2 text-sm font-medium text-white bg-primary rounded-xl hover:bg-primary-hover transition-colors shadow-glass-sm"
          >
            {t('settings.done')}
          </motion.button>
        </div>
      </motion.div>
    </div>
  );
}
