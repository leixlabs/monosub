import { useEffect, useMemo, useRef, useState } from 'react';
import {
  X,
  RotateCcw,
  Globe,
  RefreshCw,
  Loader2,
  SlidersHorizontal,
  Cpu,
  ExternalLink,
  Languages,
  Package,
  Download,
  FolderOpen,
  Trash2,
} from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { motion } from 'framer-motion';
import { useStore, SubtitleFormat, OutputLanguage } from '@/store/useStore';
import { supportedLanguages, toSupportedLanguage, UI_LANGUAGE_STORAGE_KEY } from '@/i18n';
import { CustomSelect, type SelectOption } from '@/components/CustomSelect';
import { SettingsModelPlan } from '@/components/SettingsModelPlan';
import { SettingsModelsDir } from '@/components/SettingsModelsDir';
import { SettingsTranslation } from '@/components/SettingsTranslation';
import { getVersion } from '@tauri-apps/api/app';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { SETTINGS_OPEN_CATEGORY_STORAGE_KEY } from '@/lib/license';
import { GlassButton } from '@/components/ui/glass-button';
import { GlassBadge } from '@/components/ui/glass-badge';
import { GlassSwitch } from '@/components/ui/glass-switch';
import {
  MODEL_DOWNLOAD_COMPLETE_EVENT,
  MODEL_DOWNLOAD_ERROR_EVENT,
  MODEL_DOWNLOAD_PROGRESS_EVENT,
  cancelModelDownload,
  downloadPercent,
  downloadModel,
  formatModelSize,
  listModels,
  modelManifestSize,
  openModelsDir,
  recognitionModels,
  removeModel,
  translationModels,
  type ModelDownloadCompletePayload,
  type ModelDownloadErrorPayload,
  type ModelDownloadProgressPayload,
  type ModelStatus,
} from '@/lib/models';

/**
 * Four top-level groups (translation added translation-design §7):
 * * `files`     — interface language, subtitle format/language, processing
 * * `models`    — recognition plan + model management
 * * `translate` — local translation switch, target language, model
 * * `system`    — notifications + about/updates
 */
type SettingsCategory = 'files' | 'models' | 'translate' | 'system';

const RELEASES_URL = 'https://github.com/0xkamalei/monosub/releases';

const isSettingsCategory = (value: string | null): value is SettingsCategory => {
  return value === 'files' || value === 'models' || value === 'translate' || value === 'system';
};

/** Legacy category ids from before the regroup, mapped onto the new groups. */
const LEGACY_CATEGORY_MAP: Record<string, SettingsCategory> = {
  general: 'files',
  output: 'files',
  processing: 'files',
  models: 'models',
  notifications: 'system',
  about: 'system',
};

interface GithubReleaseUpdateResult {
  currentVersion: string;
  latestVersion: string;
  latestTag: string;
  releaseName?: string | null;
  releaseUrl: string;
  publishedAt?: string | null;
  hasUpdate: boolean;
}

interface ModelRowProps {
  model: ModelStatus;
  isDownloading: boolean;
  percent: number;
  onDownload: (id: string) => void;
  onCancel: (id: string) => void;
  onRemove: (id: string) => void;
}

/**
 * One catalogue entry: metadata plus the install / download / remove action.
 *
 * Shared by the recognition and translation groups (translation-design §7), so
 * both behave identically — the only difference is which list they sit in.
 */
function ModelRow({ model, isDownloading, percent, onDownload, onCancel, onRemove }: ModelRowProps) {
  const { t } = useTranslation();
  const displayName = model.display_name || model.id;
  const displaySize = model.installed && model.size_on_disk > 0
    ? model.size_on_disk
    : modelManifestSize(model);
  const licenseLine = model.attribution
    ? `${model.attribution} — ${model.license}`
    : model.license;

  return (
    <div className="p-3 rounded-2xl border border-divider dark:border-zinc-700 bg-bg-secondary dark:bg-zinc-800/50 space-y-3">
      <div className="flex items-start justify-between gap-3">
        <div className="min-w-0">
          <div className="flex items-center gap-2 flex-wrap">
            <p className="text-sm font-semibold text-text-primary dark:text-white truncate">
              {displayName}
            </p>
            <GlassBadge variant={model.installed ? 'success' : 'warning'} size="sm">
              {model.installed ? t('models.installed') : t('models.notInstalled')}
            </GlassBadge>
          </div>
          <p className="text-xs text-text-secondary dark:text-gray-400 mt-1">
            {t('models.size')}: {formatModelSize(displaySize)}
          </p>
          {licenseLine && (
            <p
              className="text-xxs text-text-secondary dark:text-gray-400 mt-0.5 truncate"
              title={licenseLine}
            >
              {licenseLine}
            </p>
          )}
        </div>

        <div className="flex items-center gap-2 shrink-0">
          {!model.installed && !isDownloading && (
            <GlassButton variant="primary" size="sm" onClick={() => onDownload(model.id)}>
              <Download className="w-4 h-4" />
              {t('models.download')}
            </GlassButton>
          )}
          {isDownloading && (
            <GlassButton variant="outline" size="sm" onClick={() => onCancel(model.id)}>
              <X className="w-4 h-4" />
              {t('models.cancel')}
            </GlassButton>
          )}
          {model.installed && (
            <GlassButton
              variant="ghost"
              size="sm"
              onClick={() => onRemove(model.id)}
              className="text-text-secondary dark:text-gray-400 hover:text-danger"
            >
              <Trash2 className="w-4 h-4" />
              {t('models.remove')}
            </GlassButton>
          )}
        </div>
      </div>

      {isDownloading && (
        <div className="space-y-1.5">
          <div className="w-full bg-divider dark:bg-zinc-700 rounded-full h-1.5">
            <motion.div
              initial={{ width: 0 }}
              animate={{ width: `${percent}%` }}
              className="bg-primary h-full rounded-full"
            />
          </div>
          <p className="text-xxs text-text-secondary dark:text-gray-400">{percent}%</p>
        </div>
      )}
    </div>
  );
}

export function Settings() {
  const { t, i18n } = useTranslation();
  const [activeCategory, setActiveCategory] = useState<SettingsCategory>('files');
  const isOpen = useStore((state) => state.isSettingsOpen);
  const setIsOpen = useStore((state) => state.setIsSettingsOpen);
  const settings = useStore((state) => state.settings);
  const updateSettings = useStore((state) => state.updateSettings);
  const resetSettings = useStore((state) => state.resetSettings);

  const [models, setModels] = useState<ModelStatus[]>([]);
  const [isLoadingModels, setIsLoadingModels] = useState(false);
  // `models` is refreshed by the plan panel's downloads too; the list below
  // re-renders from the same state on the next load.
  const [modelsError, setModelsError] = useState('');
  const [downloadPercentById, setDownloadPercentById] = useState<Record<string, number>>({});
  const [downloadingId, setDownloadingId] = useState<string | null>(null);
  const [modelActionError, setModelActionError] = useState('');
  const isMountedRef = useRef(true);
  const ignoreDownloadErrorRef = useRef<string | null>(null);
  const [appVersion, setAppVersion] = useState('');
  const [isCheckingUpdate, setIsCheckingUpdate] = useState(false);
  const [updateResult, setUpdateResult] = useState<GithubReleaseUpdateResult | null>(null);
  const [updateError, setUpdateError] = useState('');

  useEffect(() => {
    // StrictMode mounts effects twice in development; re-arm the guard.
    isMountedRef.current = true;
    return () => {
      isMountedRef.current = false;
    };
  }, []);

  useEffect(() => {
    if (!isOpen) {
      return;
    }

    let initialCategory: SettingsCategory = 'files';
    try {
      const storedCategory = localStorage.getItem(SETTINGS_OPEN_CATEGORY_STORAGE_KEY);
      if (isSettingsCategory(storedCategory)) {
        initialCategory = storedCategory;
      } else if (storedCategory && LEGACY_CATEGORY_MAP[storedCategory]) {
        // A blob written before the regroup still points at an old id.
        initialCategory = LEGACY_CATEGORY_MAP[storedCategory];
      }
      localStorage.removeItem(SETTINGS_OPEN_CATEGORY_STORAGE_KEY);
    } catch (error) {
      console.warn('[Settings] Failed to resolve initial category:', error);
    }
    setActiveCategory(initialCategory);

    let disposed = false;

    const setupListeners = async () => {
      const unlistenProgress = await listen<ModelDownloadProgressPayload>(MODEL_DOWNLOAD_PROGRESS_EVENT, (event) => {
        if (disposed || !isMountedRef.current) return;
        const { id, downloaded, total } = event.payload;
        const percent = downloadPercent(downloaded, total);
        // The Rust downloader throttles at 200ms; skip re-renders while the
        // rounded percentage has not moved.
        setDownloadPercentById((prev) => (prev[id] === percent ? prev : { ...prev, [id]: percent }));
      });

      const unlistenComplete = await listen<ModelDownloadCompletePayload>(MODEL_DOWNLOAD_COMPLETE_EVENT, (event) => {
        if (disposed || !isMountedRef.current) return;
        const { id } = event.payload;
        setModels((prev) => prev.map((model) => (
          model.id === id
            ? { ...model, installed: true, size_on_disk: modelManifestSize(model) }
            : model
        )));
        setDownloadPercentById((prev) => {
          const next = { ...prev };
          delete next[id];
          return next;
        });
        setDownloadingId((current) => (current === id ? null : current));
      });

      const unlistenError = await listen<ModelDownloadErrorPayload>(MODEL_DOWNLOAD_ERROR_EVENT, (event) => {
        if (disposed || !isMountedRef.current) return;
        const { id, message } = event.payload;
        setModelActionError(message);
        setDownloadPercentById((prev) => {
          const next = { ...prev };
          delete next[id];
          return next;
        });
        setDownloadingId((current) => (current === id ? null : current));
      });

      return () => {
        unlistenProgress();
        unlistenComplete();
        unlistenError();
      };
    };

    const cleanup = setupListeners();
    void loadModels();
    void getVersion()
      .then((version) => {
        setAppVersion(version);
      })
      .catch((error) => {
        console.error('[getVersion] Failed:', error);
      });

    return () => {
      disposed = true;
      cleanup.then((fn) => fn && fn());
    };
  }, [isOpen, t]);

  const loadModels = async () => {
    setIsLoadingModels(true);
    setModelsError('');
    try {
      const list = await listModels();
      if (!isMountedRef.current) return;
      setModels(list);
    } catch (error) {
      console.error('[list_models] Failed:', error);
      if (!isMountedRef.current) return;
      setModelsError(String(error));
    } finally {
      if (isMountedRef.current) {
        setIsLoadingModels(false);
      }
    }
  };

  const handleOpenModelsDir = async () => {
    try {
      await openModelsDir();
    } catch (error) {
      console.error('[open_models_dir] Failed:', error);
      setModelActionError(String(error));
    }
  };

  const handleDownloadModel = async (id: string) => {
    setModelActionError('');
    ignoreDownloadErrorRef.current = null;
    setDownloadingId(id);
    setDownloadPercentById((prev) => ({ ...prev, [id]: 0 }));
    try {
      // `download_model` returns as soon as the BACKGROUND task is spawned, not
      // when the transfer finishes. The in-flight state must therefore survive
      // this resolve: it is cleared by the completion/error events (and by the
      // catch below when the request itself is rejected). Clearing it here too
      // made the button flip back to "Download" immediately, so the user saw no
      // change and could click again.
      await downloadModel(id);
    } catch (error) {
      if (isMountedRef.current && ignoreDownloadErrorRef.current !== id) {
        console.error('[download_model] Failed:', error);
        setModelActionError(String(error));
      }
      // The request was rejected, so no events will arrive: reset here.
      ignoreDownloadErrorRef.current = null;
      if (isMountedRef.current) {
        setDownloadingId((current) => (current === id ? null : current));
        setDownloadPercentById((prev) => {
          const next = { ...prev };
          delete next[id];
          return next;
        });
      }
    }
  };

  const handleCancelModelDownload = async (id: string) => {
    ignoreDownloadErrorRef.current = id;
    setModelActionError('');
    try {
      await cancelModelDownload(id);
    } catch (error) {
      console.error('[cancel_model_download] Failed:', error);
      if (isMountedRef.current) {
        setModelActionError(String(error));
      }
    }
    if (isMountedRef.current) {
      setDownloadingId((current) => (current === id ? null : current));
      setDownloadPercentById((prev) => {
        const next = { ...prev };
        delete next[id];
        return next;
      });
    }
  };

  const handleRemoveModel = async (id: string) => {
    setModelActionError('');
    try {
      await removeModel(id);
      if (!isMountedRef.current) return;
      setModels((prev) => prev.map((model) => (
        model.id === id
          ? { ...model, installed: false, size_on_disk: 0 }
          : model
      )));
    } catch (error) {
      console.error('[remove_model] Failed:', error);
      if (isMountedRef.current) {
        setModelActionError(String(error));
      }
    }
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
    { value: 'md', label: 'MD' },
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

  // The catalogue mixes both engines; split it once per render pass so the two
  // lists and their headers stay in step with the loaded models.
  const recognition = useMemo(() => recognitionModels(models), [models]);
  const translation = useMemo(() => translationModels(models), [models]);

  const handleUILanguageChange = (lang: string) => {
    i18n.changeLanguage(lang);
    localStorage.setItem(UI_LANGUAGE_STORAGE_KEY, lang);
  };

  const currentLang = useMemo(() => {
    // `i18n.language` is already normalised by `toSupportedLanguage`, but a
    // stale value read before `init()` resolves would otherwise match nothing.
    const normalized = toSupportedLanguage(i18n.language || '');
    return supportedLanguages.find((lang) => lang.code === normalized) ?? supportedLanguages[1];
  }, [i18n.language]);
  const uiLanguageLabel = t('bottomBar.language').replace(/[:：]\s*$/, '');
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
          <GlassButton
            variant="ghost"
            size="icon"
            onClick={() => setIsOpen(false)}
            className="h-8 w-8"
          >
            <X className="w-5 h-5 text-text-primary dark:text-gray-300" />
          </GlassButton>
        </div>

        <div className="flex flex-1 min-h-0 min-w-0 bg-bg-tertiary/85 dark:bg-zinc-900/60">
          <aside className="w-56 shrink-0 border-r border-glass-border-light dark:border-glass-border-dark bg-bg-tertiary/95 dark:bg-zinc-900/80 p-3">
            <div className="space-y-1">
              {[
                { id: 'files' as const, label: t('settings.files.title'), icon: SlidersHorizontal },
                { id: 'models' as const, label: t('settings.modelsGroup.title'), icon: Package },
                { id: 'translate' as const, label: t('settings.translate.title'), icon: Languages },
                { id: 'system' as const, label: t('settings.system.title'), icon: Cpu },
              ].map((item) => {
                const Icon = item.icon;
                const isActive = activeCategory === item.id;
                return (
                  <GlassButton
                    key={item.id}
                    variant={isActive ? 'default' : 'ghost'}
                    size="sm"
                    data-testid={`settings-category-${item.id}`}
                    onClick={() => setActiveCategory(item.id)}
                    className={`w-full justify-start gap-2.5 px-3 h-9 ${isActive
                        ? 'text-text-primary dark:text-white'
                        : 'text-text-secondary dark:text-gray-400 hover:text-text-primary dark:hover:text-white'
                      }`}
                  >
                    <Icon className="w-4 h-4 shrink-0" />
                    <span className="font-medium truncate">{item.label}</span>
                  </GlassButton>
                );
              })}
            </div>
          </aside>

          <div className="flex-1 min-h-0 min-w-0 flex flex-col bg-bg-tertiary/90 dark:bg-zinc-900/60">
            <div className="px-8 py-5 border-b border-glass-border-light dark:border-glass-border-dark bg-bg-tertiary/95 dark:bg-zinc-900/70">
              <h3 className="text-base font-bold text-text-primary dark:text-white">
                {
                  {
                    files: t('settings.files.title'),
                    models: t('settings.modelsGroup.title'),
                    translate: t('settings.translate.title'),
                    system: t('settings.system.title'),
                  }[activeCategory]
                }
              </h3>
            </div>

            <div className="flex-1 min-h-0 min-w-0 overflow-y-auto overflow-x-hidden px-8 py-6 bg-bg-secondary/60 dark:bg-zinc-900/50">
              {activeCategory === 'files' && (
                <section className="space-y-4">
                  {/* Subtitle output */}
                  <div className="p-4 rounded-2xl border border-divider dark:border-zinc-700 bg-bg-secondary/70 dark:bg-zinc-900/50 space-y-4">
                    <div className="flex items-center justify-between">
                      <label className="text-sm font-semibold text-text-primary dark:text-gray-200">
                        {t('settings.output.format')}
                      </label>
                      <div className="flex gap-1 bg-bg-secondary dark:bg-zinc-800 rounded-lg p-1">
                        {formatOptions.map((opt) => (
                          <GlassButton
                            key={opt.value}
                            variant={settings.outputFormat === opt.value ? 'default' : 'ghost'}
                            size="sm"
                            onClick={() => updateSettings({ outputFormat: opt.value })}
                            className="px-4 h-7 text-sm"
                          >
                            {opt.label}
                          </GlassButton>
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
                  </div>

                  {/* Processing */}
                  <div className="p-4 rounded-2xl border border-divider dark:border-zinc-700 bg-bg-secondary/70 dark:bg-zinc-900/50">
                    <div className="flex items-center justify-between">
                      <div>
                        <label className="text-sm font-semibold text-text-primary dark:text-gray-200">
                          {t('settings.processing.skipExisting')}
                        </label>
                        <p className="text-xs text-text-secondary dark:text-gray-400 mt-0.5">
                          {t('settings.processing.skipExistingDesc')}
                        </p>
                      </div>
                      <GlassSwitch
                        checked={settings.skipExistingSubtitle}
                        onCheckedChange={(checked) => updateSettings({ skipExistingSubtitle: checked })}
                      />
                    </div>
                  </div>
                </section>
              )}

              {activeCategory === 'models' && (
                <section className="space-y-4">
                  <SettingsModelPlan />
                  <SettingsModelsDir onChanged={() => void loadModels()} />

                  <div className="p-4 rounded-2xl border border-divider dark:border-zinc-700 bg-bg-secondary/70 dark:bg-zinc-900/50 space-y-4">
                    <div className="flex items-start justify-between gap-4">
                      <p className="text-xs text-text-secondary dark:text-gray-400 leading-relaxed">
                        {t('models.description')}
                      </p>
                      <div className="flex items-center gap-2 shrink-0">
                        {/* Re-reads install state from disk. Useful after copying
                            models in by hand or changing the storage location. */}
                        <GlassButton
                          variant="outline"
                          size="sm"
                          onClick={() => void loadModels()}
                          disabled={isLoadingModels}
                          title={t('models.rescanHint')}
                        >
                          <RefreshCw className={`w-4 h-4 ${isLoadingModels ? 'animate-spin' : ''}`} />
                          {t('models.rescan')}
                        </GlassButton>
                        <GlassButton
                          variant="outline"
                          size="sm"
                          onClick={() => void handleOpenModelsDir()}
                        >
                          <FolderOpen className="w-4 h-4" />
                          {t('models.openFolder')}
                        </GlassButton>
                      </div>
                    </div>

                    {isLoadingModels && (
                      <div className="flex items-center gap-2 text-sm text-text-secondary dark:text-gray-400">
                        <Loader2 className="w-4 h-4 animate-spin" />
                        {t('models.loading')}
                      </div>
                    )}

                    {!isLoadingModels && modelsError && (
                      <div className="flex items-center justify-between gap-3 p-3 rounded-2xl border border-divider dark:border-zinc-700 bg-bg-secondary dark:bg-zinc-800/50">
                        <p className="text-xs text-danger">
                          {t('models.loadFailed')}: {modelsError}
                        </p>
                        <GlassButton variant="outline" size="sm" onClick={() => void loadModels()}>
                          <RefreshCw className="w-4 h-4" />
                          {t('models.retry')}
                        </GlassButton>
                      </div>
                    )}

                    {!isLoadingModels && !modelsError && models.length === 0 && (
                      <p className="text-xs text-text-secondary dark:text-gray-400">{t('models.empty')}</p>
                    )}

                    {/* The catalogue holds two unrelated kinds of model: the
                        recognition plan (audio.cpp) and the local translation
                        model (llama.cpp). They are grouped rather than merged,
                        because picking a translation model is not part of
                        solving the recognition plan (translation-design §9). */}
                    {!isLoadingModels && !modelsError && recognition.length > 0 && (
                      <>
                        <h3 className="text-xs font-semibold text-text-secondary dark:text-gray-400 uppercase tracking-wide pt-1">
                          {t('models.recognitionTitle')}
                        </h3>
                        {recognition.map((model) => (
                          <ModelRow
                            key={model.id}
                            model={model}
                            isDownloading={downloadingId === model.id}
                            percent={downloadPercentById[model.id] ?? 0}
                            onDownload={(id) => void handleDownloadModel(id)}
                            onCancel={(id) => void handleCancelModelDownload(id)}
                            onRemove={(id) => void handleRemoveModel(id)}
                          />
                        ))}
                      </>
                    )}

                    {!isLoadingModels && !modelsError && translation.length > 0 && (
                      <>
                        <h3 className="text-xs font-semibold text-text-secondary dark:text-gray-400 uppercase tracking-wide pt-1">
                          {t('models.translationTitle')}
                        </h3>
                        {translation.map((model) => (
                          <ModelRow
                            key={model.id}
                            model={model}
                            isDownloading={downloadingId === model.id}
                            percent={downloadPercentById[model.id] ?? 0}
                            onDownload={(id) => void handleDownloadModel(id)}
                            onCancel={(id) => void handleCancelModelDownload(id)}
                            onRemove={(id) => void handleRemoveModel(id)}
                          />
                        ))}
                      </>
                    )}

                    {modelActionError && (
                      <p className="text-xs text-danger">{modelActionError}</p>
                    )}
                  </div>

                  <div className="p-4 rounded-2xl border border-divider dark:border-zinc-700 bg-bg-secondary/70 dark:bg-zinc-900/50 space-y-2">
                    <p className="text-sm font-semibold text-text-primary dark:text-gray-200">
                      {t('models.credits.title')}
                    </p>
                    <p className="text-xs text-text-secondary dark:text-gray-400 leading-relaxed">
                      {t('models.credits.audioCpp')}
                    </p>
                    <p className="text-xs text-text-secondary dark:text-gray-400 leading-relaxed">
                      {t('models.credits.sensevoice')}
                    </p>
                  </div>
                </section>
              )}

              {activeCategory === 'translate' && (
                <SettingsTranslation
                  models={models}
                  downloadingId={downloadingId}
                  downloadPercentById={downloadPercentById}
                  onDownload={(id) => void handleDownloadModel(id)}
                  onCancelDownload={(id) => void handleCancelModelDownload(id)}
                  onRemove={(id) => void handleRemoveModel(id)}
                />
              )}

              {activeCategory === 'system' && (
                <section className="space-y-4">
                  {/* Interface language */}
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

                  {/* Notifications */}
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
                      <GlassSwitch
                        checked={settings.showCompletedNotification}
                        onCheckedChange={(checked) => updateSettings({ showCompletedNotification: checked })}
                      />
                    </div>
                  </div>

                  {/* About + updates */}
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
                        <GlassButton
                          variant="primary"
                          size="sm"
                          onClick={() => void handleCheckUpdate()}
                          disabled={isCheckingUpdate}
                        >
                          {isCheckingUpdate ? (
                            <Loader2 className="w-4 h-4 animate-spin" />
                          ) : (
                            <RefreshCw className="w-4 h-4" />
                          )}
                          {isCheckingUpdate ? t('settings.about.checkingUpdate') : t('settings.about.checkUpdate')}
                        </GlassButton>

                        <GlassButton
                          variant="outline"
                          size="sm"
                          onClick={() => void handleOpenReleasePage()}
                        >
                          <ExternalLink className="w-4 h-4" />
                          {t('settings.about.openReleasePage')}
                        </GlassButton>
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
                        {updateResult.hasUpdate && (
                          <GlassButton
                            variant="primary"
                            size="sm"
                            onClick={() => void handleOpenReleasePage()}
                          >
                            <ExternalLink className="w-4 h-4" />
                            {t('settings.about.openReleasePage')}
                          </GlassButton>
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
          <GlassButton
            variant="ghost"
            size="sm"
            onClick={resetSettings}
            className="text-text-primary dark:text-gray-200 hover:text-primary dark:hover:text-primary"
          >
            <RotateCcw className="w-4 h-4" />
            {t('settings.resetDefaults')}
          </GlassButton>
          <GlassButton
            variant="primary"
            size="sm"
            onClick={() => setIsOpen(false)}
          >
            {t('settings.done')}
          </GlassButton>
        </div>
      </motion.div>
    </div>
  );
}
