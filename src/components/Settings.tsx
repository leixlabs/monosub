import { useEffect, useMemo, useState } from 'react';
import {
  X,
  RotateCcw,
  Globe,
  ShieldCheck,
  ShieldAlert,
  RefreshCw,
  Loader2,
  SlidersHorizontal,
  Cpu,
  Bell,
  KeyRound,
  ExternalLink,
  BadgeCheck,
  AlertTriangle,
  Unplug,
  Info,
} from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { motion, AnimatePresence } from 'framer-motion';
import { useStore, SubtitleFormat, OutputLanguage } from '@/store/useStore';
import { supportedLanguages } from '@/i18n';
import { CustomSelect, type SelectOption } from '@/components/CustomSelect';
import { getVersion } from '@tauri-apps/api/app';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import {
  validatePolarLicense,
  activatePolarLicense,
  deactivatePolarLicense,
  openLicenseCheckout,
  SETTINGS_OPEN_CATEGORY_STORAGE_KEY,
} from '@/lib/license';

type SettingsCategory = 'general' | 'output' | 'processing' | 'license' | 'environment' | 'notifications' | 'about';

const RELEASES_URL = 'https://github.com/0xkamalei/monosub/releases';

const safeTrim = (value: unknown) => (typeof value === 'string' ? value.trim() : '');

const isSettingsCategory = (value: string | null): value is SettingsCategory => {
  return value === 'general'
    || value === 'output'
    || value === 'processing'
    || value === 'license'
    || value === 'environment'
    || value === 'notifications'
    || value === 'about';
};

interface InstallProgressPayload {
  progress: number;
  status: string;
}

interface GithubReleaseUpdateResult {
  currentVersion: string;
  latestVersion: string;
  latestTag: string;
  releaseName?: string | null;
  releaseUrl: string;
  publishedAt?: string | null;
  hasUpdate: boolean;
}

export function Settings() {
  const { t, i18n } = useTranslation();
  const [activeCategory, setActiveCategory] = useState<SettingsCategory>('general');
  const isOpen = useStore((state) => state.isSettingsOpen);
  const setIsOpen = useStore((state) => state.setIsSettingsOpen);
  const settings = useStore((state) => state.settings);
  const updateSettings = useStore((state) => state.updateSettings);
  const resetSettings = useStore((state) => state.resetSettings);
  const licenseConfig = useStore((state) => state.licenseConfig);
  const setLicenseKey = useStore((state) => state.setLicenseKey);
  const setLicenseActivationId = useStore((state) => state.setLicenseActivationId);
  const licenseStatus = useStore((state) => state.licenseStatus);
  const licenseMessage = useStore((state) => state.licenseMessage);
  const licenseExpiresAt = useStore((state) => state.licenseExpiresAt);
  const licenseLastCheckedAt = useStore((state) => state.licenseLastCheckedAt);
  const clearLicenseValidation = useStore((state) => state.clearLicenseValidation);

  const [envStatus, setEnvStatus] = useState<'idle' | 'checking' | 'healthy' | 'broken' | 'error'>('idle');
  const [isRepairing, setIsRepairing] = useState(false);
  const [repairProgress, setRepairProgress] = useState(0);
  const [repairMsg, setRepairMsg] = useState('');
  const [isValidatingLicense, setIsValidatingLicense] = useState(false);
  const [isDeactivatingLicense, setIsDeactivatingLicense] = useState(false);
  const [appVersion, setAppVersion] = useState('');
  const [isCheckingUpdate, setIsCheckingUpdate] = useState(false);
  const [updateResult, setUpdateResult] = useState<GithubReleaseUpdateResult | null>(null);
  const [updateError, setUpdateError] = useState('');

  useEffect(() => {
    if (!isOpen) {
      return;
    }

    let initialCategory: SettingsCategory = 'general';
    try {
      const storedCategory = localStorage.getItem(SETTINGS_OPEN_CATEGORY_STORAGE_KEY);
      if (isSettingsCategory(storedCategory)) {
        initialCategory = storedCategory;
      }
      localStorage.removeItem(SETTINGS_OPEN_CATEGORY_STORAGE_KEY);
    } catch (error) {
      console.warn('[Settings] Failed to resolve initial category:', error);
    }
    setActiveCategory(initialCategory);

    const setupListeners = async () => {
      const unlistenProgress = await listen<InstallProgressPayload>('install_progress', (event) => {
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
    void getVersion()
      .then((version) => {
        setAppVersion(version);
      })
      .catch((error) => {
        console.error('[getVersion] Failed:', error);
      });

    return () => {
      cleanup.then((fn) => fn && fn());
    };
  }, [isOpen, t]);

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

  const handleValidateLicense = async () => {
    setIsValidatingLicense(true);
    try {
      if (!safeTrim(licenseConfig?.licenseActivationId)) {
        const activation = await activatePolarLicense();
        if (!activation.activated) {
          return;
        }
      }
      await validatePolarLicense();
    } finally {
      setIsValidatingLicense(false);
    }
  };

  const handleDeactivateLicense = async () => {
    setIsDeactivatingLicense(true);
    try {
      await deactivatePolarLicense();
    } finally {
      setIsDeactivatingLicense(false);
    }
  };

  const handleOpenCheckout = async () => {
    await openLicenseCheckout();
  };

  const handleCheckUpdate = async () => {
    setIsCheckingUpdate(true);
    setUpdateError('');

    try {
      const result = await invoke<GithubReleaseUpdateResult>('check_github_release_update');
      setUpdateResult(result);
    } catch (error) {
      const message = String(error);
      setUpdateError(message);
      console.error('[check_github_release_update] Failed:', error);
    } finally {
      setIsCheckingUpdate(false);
    }
  };

  const handleOpenReleasePage = async () => {
    const target = updateResult?.releaseUrl || RELEASES_URL;
    try {
      await invoke('open_external_url', { url: target });
    } catch (error) {
      console.error('[open_external_url] Failed:', error);
      try {
        window.open(target, '_blank');
      } catch (fallbackError) {
        console.error('[window.open] Failed:', fallbackError);
      }
    }
  };

  const formatOptions: { value: SubtitleFormat; label: string }[] = [
    { value: 'srt', label: 'SRT' },
    { value: 'vtt', label: 'VTT' },
    { value: 'txt', label: 'TXT' },
  ];

  const languageOptions: SelectOption<OutputLanguage>[] = [
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

  const uiLanguageOptions = useMemo<SelectOption<string>[]>(() => (
    supportedLanguages.map((lang) => ({
      value: lang.code,
      label: `${lang.flag} ${lang.name}`,
    }))
  ), []);

  const handleUILanguageChange = (lang: string) => {
    i18n.changeLanguage(lang);
    localStorage.setItem('monosub-language', lang);
  };

  const currentLang = useMemo(() => {
    const currentLanguage = i18n.language;
    return (
      supportedLanguages.find(
        (lang) => currentLanguage === lang.code || currentLanguage?.startsWith(lang.code.split('-')[0])
      ) || supportedLanguages[1]
    );
  }, [i18n.language]);
  const uiLanguageLabel = t('bottomBar.language').replace(/[:：]\s*$/, '');
  const hasActivationId = safeTrim(licenseConfig?.licenseActivationId).length > 0;
  const isLicenseChecking = isValidatingLicense || licenseStatus === 'checking';
  const hasLicenseChecked = Boolean(licenseLastCheckedAt);
  const normalizedLicenseExpiresAt = safeTrim(licenseExpiresAt);
  const licenseExpiryDisplay = useMemo(() => {
    if (!normalizedLicenseExpiresAt) {
      return '';
    }

    const expiresAtMs = Date.parse(normalizedLicenseExpiresAt);
    if (Number.isNaN(expiresAtMs)) {
      return normalizedLicenseExpiresAt;
    }

    return new Intl.DateTimeFormat(i18n.language || undefined, {
      year: 'numeric',
      month: '2-digit',
      day: '2-digit',
      hour: '2-digit',
      minute: '2-digit',
      second: '2-digit',
    }).format(new Date(expiresAtMs));
  }, [i18n.language, normalizedLicenseExpiresAt]);
  const licenseSummary = licenseStatus === 'valid'
    ? t('settings.license.summary.valid')
    : isLicenseChecking
      ? t('settings.license.summary.checking')
      : t('settings.license.summary.notActivated');
  const licenseDetail = licenseStatus === 'valid'
    ? ''
    : (licenseMessage || t('settings.license.summary.hint'));
  const updatePublishedDisplay = useMemo(() => {
    const publishedAt = updateResult?.publishedAt;
    if (!publishedAt) {
      return '';
    }

    const publishedMs = Date.parse(publishedAt);
    if (Number.isNaN(publishedMs)) {
      return publishedAt;
    }

    return new Intl.DateTimeFormat(i18n.language || undefined, {
      year: 'numeric',
      month: '2-digit',
      day: '2-digit',
      hour: '2-digit',
      minute: '2-digit',
    }).format(new Date(publishedMs));
  }, [i18n.language, updateResult?.publishedAt]);

  if (!isOpen) return null;

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center">
      <motion.div
        initial={{ opacity: 0 }}
        animate={{ opacity: 1 }}
        exit={{ opacity: 0 }}
        className="absolute inset-0 bg-black/40 dark:bg-black/60 backdrop-blur-xl"
        onClick={() => setIsOpen(false)}
      />

      <motion.div
        initial={{ opacity: 0, scale: 0.95 }}
        animate={{ opacity: 1, scale: 1 }}
        exit={{ opacity: 0, scale: 0.95 }}
        transition={{ type: 'spring', stiffness: 300, damping: 30 }}
        className="relative glass-medium dark:glass-dark-strong rounded-3xl shadow-glass-lg w-[860px] max-w-[94vw] h-[82vh] max-h-[700px] min-h-[540px] overflow-hidden flex flex-col"
      >
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

        <div className="flex flex-1 min-h-0 bg-bg-tertiary/85 dark:bg-zinc-900/60">
          <aside className="w-56 border-r border-glass-border-light dark:border-glass-border-dark bg-bg-tertiary/95 dark:bg-zinc-900/80 p-3">
            <div className="space-y-1">
              {[
                { id: 'general' as const, label: uiLanguageLabel, icon: Globe },
                { id: 'output' as const, label: t('settings.output.title'), icon: SlidersHorizontal },
                { id: 'processing' as const, label: t('settings.processing.title'), icon: Cpu },
                { id: 'license' as const, label: t('settings.license.title'), icon: KeyRound },
                { id: 'environment' as const, label: t('settings.environment.title'), icon: RefreshCw },
                { id: 'notifications' as const, label: t('settings.notifications.title'), icon: Bell },
                { id: 'about' as const, label: t('settings.about.title'), icon: Info },
              ].map((item) => {
                const Icon = item.icon;
                const isActive = activeCategory === item.id;
                return (
                  <motion.button
                    key={item.id}
                    whileTap={{ scale: 0.98 }}
                    onClick={() => setActiveCategory(item.id)}
                    className={`w-full flex items-center gap-2.5 px-3 py-2 rounded-xl text-left text-sm transition-colors ${isActive
                      ? 'bg-white/90 dark:bg-zinc-800/90 text-text-primary dark:text-white shadow-glass-sm'
                      : 'text-text-secondary dark:text-gray-400 hover:bg-white/70 dark:hover:bg-zinc-800/70 hover:text-text-primary dark:hover:text-white'
                      }`}
                  >
                    <Icon className="w-4 h-4 shrink-0" />
                    <span className="font-medium truncate">{item.label}</span>
                  </motion.button>
                );
              })}
            </div>
          </aside>

          <div className="flex-1 min-h-0 flex flex-col bg-bg-tertiary/90 dark:bg-zinc-900/60">
            <div className="px-8 py-5 border-b border-glass-border-light dark:border-glass-border-dark bg-bg-tertiary/95 dark:bg-zinc-900/70">
              <h3 className="text-base font-bold text-text-primary dark:text-white">
                {
                  {
                    general: uiLanguageLabel,
                    output: t('settings.output.title'),
                    processing: t('settings.processing.title'),
                    license: t('settings.license.title'),
                    environment: t('settings.environment.title'),
                    notifications: t('settings.notifications.title'),
                    about: t('settings.about.title'),
                  }[activeCategory]
                }
              </h3>
            </div>

            <div className="flex-1 min-h-0 overflow-y-auto px-8 py-6 bg-bg-secondary/60 dark:bg-zinc-900/50">
              {activeCategory === 'general' && (
                <section className="space-y-6">
                  <div className="p-4 rounded-2xl border border-divider dark:border-zinc-700 bg-bg-secondary/70 dark:bg-zinc-900/50">
                    <div className="flex items-center justify-between gap-4">
                      <div className="flex items-center gap-2">
                        <Globe className="w-4 h-4 text-text-primary dark:text-gray-300" />
                        <label className="text-sm font-semibold text-text-primary dark:text-gray-200">{uiLanguageLabel}</label>
                      </div>
                      <CustomSelect
                        value={currentLang.code}
                        onChange={handleUILanguageChange}
                        options={uiLanguageOptions}
                        className="w-[260px]"
                      />
                    </div>
                  </div>
                </section>
              )}

              {activeCategory === 'output' && (
                <section className="space-y-4">
                  <div className="p-4 rounded-2xl border border-divider dark:border-zinc-700 bg-bg-secondary/70 dark:bg-zinc-900/50 space-y-4">
                    <div className="flex items-center justify-between">
                      <label className="text-sm font-semibold text-text-primary dark:text-gray-200">
                        {t('settings.output.format')}
                      </label>
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

                    <div className="flex items-center justify-between gap-4">
                      <label className="text-sm font-semibold text-text-primary dark:text-gray-200">
                        {t('settings.output.language')}
                      </label>
                      <CustomSelect
                        value={settings.outputLanguage}
                        onChange={(value) => updateSettings({ outputLanguage: value })}
                        options={languageOptions}
                        className="w-[220px]"
                      />
                    </div>

                    <div className="flex items-center justify-between opacity-60">
                      <div>
                        <div className="flex items-center gap-2">
                          <label className="text-sm font-semibold text-text-primary dark:text-gray-200">
                            {t('settings.output.bilingual')}
                          </label>
                          <span className="px-2 py-0.5 text-[10px] font-bold text-accent bg-accent/10 dark:bg-accent/20 rounded-full border border-accent/20">
                            Coming Soon
                          </span>
                        </div>
                        <p className="text-xs text-text-secondary dark:text-gray-400 mt-0.5">
                          {t('settings.output.bilingualDesc')}
                        </p>
                      </div>
                      <label className="relative inline-flex items-center cursor-not-allowed">
                        <input type="checkbox" checked={false} disabled={true} className="sr-only peer" />
                        <div className="w-11 h-6 bg-bg-secondary dark:bg-zinc-800 rounded-full peer peer-checked:bg-primary transition-colors after:content-[''] after:absolute after:top-0.5 after:left-[2px] after:bg-white after:rounded-full after:h-5 after:w-5 after:transition-all peer-checked:after:translate-x-full after:shadow-sm" />
                      </label>
                    </div>
                  </div>
                </section>
              )}

              {activeCategory === 'processing' && (
                <section className="space-y-4">
                  <div className="p-4 rounded-2xl border border-divider dark:border-zinc-700 bg-bg-secondary/70 dark:bg-zinc-900/50 space-y-4">
                    <div className="flex items-center justify-between">
                      <div>
                        <label className="text-sm font-semibold text-text-primary dark:text-gray-200">
                          {t('settings.processing.noiseReduction')}
                        </label>
                        <p className="text-xs text-text-secondary dark:text-gray-400 mt-0.5">
                          {t('settings.processing.noiseReductionDesc')}
                        </p>
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

                    <div className="flex items-center justify-between">
                      <div>
                        <label className="text-sm font-semibold text-text-primary dark:text-gray-200">
                          {t('settings.processing.volumeNormalization')}
                        </label>
                        <p className="text-xs text-text-secondary dark:text-gray-400 mt-0.5">
                          {t('settings.processing.volumeNormalizationDesc')}
                        </p>
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

                    <div className="flex items-center justify-between">
                      <div>
                        <label className="text-sm font-semibold text-text-primary dark:text-gray-200">
                          {t('settings.processing.skipExisting')}
                        </label>
                        <p className="text-xs text-text-secondary dark:text-gray-400 mt-0.5">
                          {t('settings.processing.skipExistingDesc')}
                        </p>
                      </div>
                      <label className="relative inline-flex items-center cursor-pointer">
                        <input
                          type="checkbox"
                          checked={settings.skipExistingSubtitle}
                          onChange={(e) => updateSettings({ skipExistingSubtitle: e.target.checked })}
                          className="sr-only peer"
                        />
                        <div className="w-11 h-6 bg-bg-secondary dark:bg-zinc-800 rounded-full peer peer-checked:bg-primary transition-colors after:content-[''] after:absolute after:top-0.5 after:left-[2px] after:bg-white after:rounded-full after:h-5 after:w-5 after:transition-all peer-checked:after:translate-x-full after:shadow-sm" />
                      </label>
                    </div>


                  </div>
                </section>
              )}

              {activeCategory === 'license' && (
                <section className="space-y-4">
                  <div className="p-4 rounded-2xl border border-divider dark:border-zinc-700 bg-bg-secondary/70 dark:bg-zinc-900/50 space-y-4">
                    <div className="grid grid-cols-1 gap-3">
                      <label className="text-xs font-semibold text-text-secondary dark:text-gray-400 uppercase tracking-wide">
                        {t('settings.license.licenseKey')}
                      </label>
                      <input
                        type="password"
                        value={licenseConfig.licenseKey}
                        onChange={(e) => setLicenseKey(e.target.value)}
                        className="px-3 py-2 text-sm bg-bg-secondary dark:bg-zinc-800 border border-border dark:border-zinc-700 rounded-xl text-text-primary dark:text-white focus:outline-none focus:ring-2 focus:ring-primary/20 shadow-sm font-medium"
                        placeholder="pol_xxx..."
                      />
                    </div>

                    <div className="flex flex-wrap items-center gap-2">
                      <motion.button
                        whileTap={{ scale: 0.97 }}
                        onClick={handleValidateLicense}
                        disabled={isValidatingLicense}
                        className="inline-flex items-center gap-2 px-4 py-2 text-sm font-medium text-white bg-primary rounded-xl hover:bg-primary-hover disabled:opacity-50 transition-colors shadow-glass-sm"
                      >
                        {isValidatingLicense ? (
                          <Loader2 className="w-4 h-4 animate-spin" />
                        ) : (
                          <RefreshCw className="w-4 h-4" />
                        )}
                        {isValidatingLicense ? t('settings.license.validating') : t('settings.license.validate')}
                      </motion.button>

                      <motion.button
                        whileTap={{ scale: 0.97 }}
                        onClick={handleOpenCheckout}
                        className="inline-flex items-center gap-2 px-4 py-2 text-sm font-medium text-text-primary dark:text-white border border-border dark:border-zinc-700 rounded-xl hover:bg-bg-secondary dark:hover:bg-zinc-800 transition-colors"
                      >
                        <ExternalLink className="w-4 h-4" />
                        {t('settings.license.openCheckout')}
                      </motion.button>

                      {hasActivationId && (
                        <motion.button
                          whileTap={{ scale: 0.97 }}
                          onClick={handleDeactivateLicense}
                          disabled={isDeactivatingLicense}
                          className="inline-flex items-center gap-2 px-4 py-2 text-sm font-medium text-text-primary dark:text-white border border-border dark:border-zinc-700 rounded-xl hover:bg-bg-secondary dark:hover:bg-zinc-800 transition-colors disabled:opacity-50"
                        >
                          {isDeactivatingLicense ? (
                            <Loader2 className="w-4 h-4 animate-spin" />
                          ) : (
                            <Unplug className="w-4 h-4" />
                          )}
                          {t('settings.license.deactivate')}
                        </motion.button>
                      )}

                      <motion.button
                        whileTap={{ scale: 0.97 }}
                        onClick={() => {
                          setLicenseKey('');
                          setLicenseActivationId('');
                          clearLicenseValidation();
                        }}
                        className="inline-flex items-center gap-2 px-3 py-2 text-xs font-semibold text-text-secondary dark:text-gray-400 hover:text-danger hover:bg-danger/10 rounded-xl transition-colors"
                      >
                        {t('settings.license.clearKey')}
                      </motion.button>
                    </div>

                    <div className="p-3 rounded-2xl border border-divider dark:border-zinc-700 bg-bg-secondary dark:bg-zinc-800/50">
                      <div className="flex items-center gap-2 text-sm font-semibold text-text-primary dark:text-white">
                        {licenseStatus === 'valid' ? (
                          <BadgeCheck className="w-4 h-4 text-success" />
                        ) : isLicenseChecking ? (
                          <Loader2 className="w-4 h-4 text-primary animate-spin" />
                        ) : (
                          <AlertTriangle className="w-4 h-4 text-warning" />
                        )}
                        {licenseSummary}
                      </div>
                      {licenseDetail && (
                        <p className="text-xs text-text-secondary dark:text-gray-400 mt-2">{licenseDetail}</p>
                      )}
                      {hasLicenseChecked && (
                        <p className="text-xs text-text-secondary dark:text-gray-400 mt-2">
                          {t('settings.license.expiresAt')}: {normalizedLicenseExpiresAt ? licenseExpiryDisplay : t('settings.license.unlimited')}
                        </p>
                      )}
                    </div>
                  </div>
                </section>
              )}

              {activeCategory === 'environment' && (
                <section className="space-y-4">
                  <div className="p-4 rounded-2xl border border-divider dark:border-zinc-700 bg-bg-secondary/70 dark:bg-zinc-900/50 space-y-4">
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
                          {isRepairing ? (
                            <Loader2 className="w-3.5 h-3.5 animate-spin" />
                          ) : (
                            <RefreshCw className="w-3.5 h-3.5" />
                          )}
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
                          <div className="w-full bg-divider dark:bg-zinc-700 rounded-full h-1.5">
                            <motion.div
                              initial={{ width: 0 }}
                              animate={{ width: `${repairProgress}%` }}
                              className="bg-primary h-full rounded-full"
                            />
                          </div>
                        </motion.div>
                      )}
                    </AnimatePresence>

                    <p className="text-xs text-text-secondary dark:text-gray-400 leading-relaxed">
                      {t('settings.environment.repairDesc')}
                    </p>
                  </div>
                </section>
              )}

              {activeCategory === 'notifications' && (
                <section className="space-y-4">
                  <div className="p-4 rounded-2xl border border-divider dark:border-zinc-700 bg-bg-secondary/70 dark:bg-zinc-900/50">
                    <div className="flex items-center justify-between">
                      <div>
                        <label className="text-sm font-semibold text-text-primary dark:text-gray-200">
                          {t('settings.notifications.showCompleted')}
                        </label>
                        <p className="text-xs text-text-secondary dark:text-gray-400 mt-0.5">
                          {t('settings.notifications.showCompletedDesc')}
                        </p>
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
              )}

              {activeCategory === 'about' && (
                <section className="space-y-4">
                  <div className="p-4 rounded-2xl border border-divider dark:border-zinc-700 bg-bg-secondary/70 dark:bg-zinc-900/50 space-y-4">
                    <div className="flex items-center justify-between gap-4">
                      <div>
                        <p className="text-sm font-semibold text-text-primary dark:text-gray-200">
                          {t('settings.about.appVersion')}
                        </p>
                        <p className="mt-1 text-xs text-text-secondary dark:text-gray-400">
                          v{appVersion || t('settings.about.unknownVersion')}
                        </p>
                      </div>

                      <div className="flex items-center gap-2">
                        <motion.button
                          whileTap={{ scale: 0.97 }}
                          onClick={handleCheckUpdate}
                          disabled={isCheckingUpdate}
                          className="inline-flex items-center gap-2 px-4 py-2 text-sm font-medium text-white bg-primary rounded-xl hover:bg-primary-hover disabled:opacity-50 transition-colors shadow-glass-sm"
                        >
                          {isCheckingUpdate ? (
                            <Loader2 className="w-4 h-4 animate-spin" />
                          ) : (
                            <RefreshCw className="w-4 h-4" />
                          )}
                          {isCheckingUpdate ? t('settings.about.checkingUpdate') : t('settings.about.checkUpdate')}
                        </motion.button>

                        <motion.button
                          whileTap={{ scale: 0.97 }}
                          onClick={handleOpenReleasePage}
                          className="inline-flex items-center gap-2 px-4 py-2 text-sm font-medium text-text-primary dark:text-white border border-border dark:border-zinc-700 rounded-xl hover:bg-bg-secondary dark:hover:bg-zinc-800 transition-colors"
                        >
                          <ExternalLink className="w-4 h-4" />
                          {t('settings.about.openReleasePage')}
                        </motion.button>
                      </div>
                    </div>

                    {updateError && (
                      <p className="text-xs text-danger">
                        {t('settings.about.checkFailed')}: {updateError}
                      </p>
                    )}

                    {updateResult && (
                      <div className="p-3 rounded-2xl border border-divider dark:border-zinc-700 bg-bg-secondary dark:bg-zinc-800/50 space-y-2">
                        <p className="text-sm font-semibold text-text-primary dark:text-white">
                          {updateResult.hasUpdate
                            ? t('settings.about.newVersionAvailable', { version: updateResult.latestTag || updateResult.latestVersion })
                            : t('settings.about.upToDate')}
                        </p>
                        <p className="text-xs text-text-secondary dark:text-gray-400">
                          {t('settings.about.latestVersion')}: v{updateResult.latestVersion}
                        </p>
                        {updatePublishedDisplay && (
                          <p className="text-xs text-text-secondary dark:text-gray-400">
                            {t('settings.about.publishedAt')}: {updatePublishedDisplay}
                          </p>
                        )}

                      </div>
                    )}

                    {!updateResult && !updateError && (
                      <p className="text-xs text-text-secondary dark:text-gray-400">
                        {t('settings.about.hint')}
                      </p>
                    )}
                  </div>
                </section>
              )}
            </div>
          </div>
        </div>

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
