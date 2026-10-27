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
  Link2,
  Package,
  Download,
  Trash2,
  FolderOpen,
  FileCog,
  Terminal,
  Copy,
  Check,
} from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { motion } from 'framer-motion';
import { useStore, SubtitleFormat, OutputLanguage, MIN_CONCURRENCY, MAX_CONCURRENCY } from '@/store/useStore';
import { supportedLanguages, toSupportedLanguage, UI_LANGUAGE_STORAGE_KEY } from '@/i18n';
import { CustomSelect, type SelectOption } from '@/components/CustomSelect';
import { SettingsModelPlan } from '@/components/SettingsModelPlan';
import { SettingsModelsDir } from '@/components/SettingsModelsDir';
import { SettingsTranslation } from '@/components/SettingsTranslation';
import { SettingsUrlImport } from '@/components/SettingsUrlImport';
import { ModelLicensePrompt } from '@/components/ModelLicensePrompt';
import { getVersion } from '@tauri-apps/api/app';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { SETTINGS_OPEN_CATEGORY_STORAGE_KEY } from '@/lib/license';
import { cn } from '@/lib/utils';
import { getLogDirectory, openLogsDirectory, reportFrontendError } from '@/lib/logging';
import {
  cliInstallCommand,
  getCliInfo,
  getConfigFilePath,
  revealConfigFile,
  writeClipboard,
  type CliInfo,
} from '@/lib/config';
import { GlassButton } from '@/components/ui/glass-button';
import { GlassBadge, glassBadgeVariants } from '@/components/ui/glass-badge';
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
  recognitionModels,
  removeModel,
  sortByAvailability,
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
 * * `urlImport` — imported-subtitle directory, cookie source (browser or
 *   exported `cookies.txt`), prefer-local. Carries a Beta marker.
 * * `system`    — notifications + about/updates
 */
type SettingsCategory = 'files' | 'models' | 'translate' | 'urlImport' | 'system';

/**
 * "Beta" marker for a whole settings group. Deliberately quieter than a status
 * badge: it sits beside a label, so amber-on-transparent at the same text scale,
 * and the label itself comes from i18n (`settings.urlImport.beta`).
 *
 * Rendered as a `<span>` rather than `GlassBadge` because one of its two homes
 * is inside the category button, whose content must stay phrasing content.
 */
function BetaBadge({ className }: { className?: string }) {
  const { t } = useTranslation();
  return (
    <span
      className={cn(
        glassBadgeVariants({ variant: 'warning', size: 'sm' }),
        'uppercase tracking-wide',
        className,
      )}
      data-testid="settings-beta-badge"
    >
      {t('settings.urlImport.beta')}
    </span>
  );
}

/**
 * The only external link the About card offers. MonoSub is source-available
 * (PolyForm Small Business): the repository is where the source is read and
 * where the licence lives, so this is the address worth showing — not a
 * release/download page, and not an update check.
 */
const REPO_URL = 'https://github.com/leixlabs/monosub';

/** How long the CLI card's copy button keeps its "copied" label. */
const COPIED_RESET_MS = 2000;

const isSettingsCategory = (value: string | null): value is SettingsCategory => {
  return value === 'files'
    || value === 'models'
    || value === 'translate'
    || value === 'urlImport'
    || value === 'system';
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
            {/* Exactly one status badge, always present: `status` is a single
                axis, so the row says "supported" *or* "unverified" *or* "not
                supported" — never two of them. Every state is labelled because
                an absent badge is ambiguous: it could mean "verified and fine"
                or "this field did not arrive", and the second is a real case
                (an older backend sends nothing, which reads as `unverified`).
                The three states differ in consequence, not in kind: an
                unverified entry stays downloadable and selectable, and a retired
                one stays listed so the survey reads as one document while
                nothing will ever select it. Saying which is the difference
                between "not recommended" and "not offered". */}
            {model.status === 'supported' && (
              <GlassBadge variant="success" size="sm" title={t('models.supportedHint')}>
                {t('models.supported')}
              </GlassBadge>
            )}
            {model.status === 'unverified' && (
              <GlassBadge variant="default" size="sm" title={t('models.unverifiedHint')}>
                {t('models.unverified')}
              </GlassBadge>
            )}
            {model.status === 'unsupported' && (
              <GlassBadge variant="destructive" size="sm" title={t('models.unsupportedHint')}>
                {t('models.unsupported')}
              </GlassBadge>
            )}
            {/* Commercial-use verdict, shown only when the answer is not a plain
                "allowed". The 78 permissive entries already print their licence
                name two lines below, so a badge on every row would train the eye
                to skip the one place a warning has to land. This is the label
                that carries the risk to the user; the confirmation dialog is
                where it gets spelled out before a download starts. */}
            {model.commercial_use !== 'allowed' && (
              <GlassBadge
                variant={model.commercial_use === 'noncommercial' ? 'destructive' : 'warning'}
                size="sm"
                title={t(`models.commercialUse.${model.commercial_use}Hint`)}
              >
                {t(`models.commercialUse.${model.commercial_use}`)}
              </GlassBadge>
            )}
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
          {/* No download button for a retired entry: the backend refuses it
              (`download_model` gates on `supported`), so offering the button
              would only turn a dead model into a failed click. */}
          {!model.installed && !isDownloading && model.status !== 'unsupported' && (
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

      {/* Run mode, shown as a fact rather than a question: streaming is what the
          app does wherever it can, so there is nothing for the user to decide.
          Only streaming is labelled — printing "offline" on the hundred-odd
          models that only run offline would be noise, and the absence of the
          badge already says it. `effective_mode` is Rust's answer, so a
          hand-edited `modelModes` entry is reflected here instead of the row
          claiming a mode the run will not use. */}
      {model.effective_mode === 'streaming' && (
        <div className="flex items-center gap-2" data-testid={`model-mode-${model.id}`}>
          <span className="text-xs text-text-secondary dark:text-gray-400">
            {t('models.runMode')}
          </span>
          <GlassBadge
            variant="primary"
            size="sm"
            title={t('models.modes.streamingHint')}
            data-testid={`model-mode-${model.id}-value`}
          >
            {t('models.modes.streaming')}
          </GlassBadge>
        </div>
      )}

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
  /**
   * The model whose licence confirmation is on screen, if any.
   *
   * Holds the entry rather than an id so the dialog renders without a second
   * lookup and cannot show one model's terms under another model's name.
   */
  const [pendingLicenseModel, setPendingLicenseModel] = useState<ModelStatus | null>(null);
  const [modelActionError, setModelActionError] = useState('');
  const isMountedRef = useRef(true);
  const ignoreDownloadErrorRef = useRef<string | null>(null);
  const [appVersion, setAppVersion] = useState('');
  // About card: the repository row's copy chip, and a link that would not open.
  const [repoCopied, setRepoCopied] = useState(false);
  const [repoError, setRepoError] = useState('');
  const repoCopyTimerRef = useRef<number | null>(null);
  // Log directory row (Settings ▸ System): the path is read once per open, and
  // a failure to open it is shown in place rather than only in the log.
  const [logDirectory, setLogDirectory] = useState('');
  const [logDirectoryError, setLogDirectoryError] = useState('');
  // Config file row. A separate path from the models directory on purpose:
  // they are unrelated locations, and showing this one is the point of the row.
  const [configFilePath, setConfigFilePath] = useState('');
  const [configFileError, setConfigFileError] = useState('');
  /** The CLI card's copy button: its label reverts after `COPIED_RESET_MS`. */
  const [cliCopied, setCliCopied] = useState(false);
  const [cliCopyError, setCliCopyError] = useState('');
  const cliCopyTimerRef = useRef<number | null>(null);
  // Where the CLI actually is, read from the backend rather than assumed: the
  // card names a real file and builds its command from that path.
  const [cliInfo, setCliInfo] = useState<CliInfo | null>(null);
  const [cliInfoError, setCliInfoError] = useState('');
  const installCommand = useMemo(
    () => (cliInfo ? cliInstallCommand(cliInfo) : null),
    [cliInfo],
  );
  /**
   * What the user types, derived from the bundled file name so the card cannot
   * disagree with the package: `monosub-cli.exe` → `monosub-cli` (a terminal
   * resolves either, but the `.exe` is noise in an instruction).
   */
  const cliCommand = useMemo(
    () => (cliInfo?.fileName ?? 'monosub-cli').replace(/\.exe$/i, ''),
    [cliInfo],
  );

  useEffect(() => {
    // StrictMode mounts effects twice in development; re-arm the guard.
    isMountedRef.current = true;
    return () => {
      isMountedRef.current = false;
    };
  }, []);

  // A pending "copied" reset must not fire into an unmounted component.
  useEffect(() => () => {
    if (cliCopyTimerRef.current !== null) {
      window.clearTimeout(cliCopyTimerRef.current);
      cliCopyTimerRef.current = null;
    }
    if (repoCopyTimerRef.current !== null) {
      window.clearTimeout(repoCopyTimerRef.current);
      repoCopyTimerRef.current = null;
    }
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

    // The row is only useful with the real path, so read it when the panel opens
    // (the directory can be created or removed between opens).
    setLogDirectoryError('');
    void getLogDirectory()
      .then((dir) => {
        if (!disposed && isMountedRef.current) setLogDirectory(dir);
      })
      .catch((error) => {
        console.error('[log_directory] Failed:', error);
        if (!disposed && isMountedRef.current) setLogDirectoryError(String(error));
      });

    // Same treatment for the config file. The command reports the path even
    // when the file does not exist yet, so a failure here is a real one.
    setConfigFileError('');
    // A copy from a previous visit is not news on this one, and its pending
    // reset timer would otherwise flip the label while the panel is closed.
    if (cliCopyTimerRef.current !== null) {
      window.clearTimeout(cliCopyTimerRef.current);
      cliCopyTimerRef.current = null;
    }
    setCliCopied(false);
    setCliCopyError('');
    void getConfigFilePath()
      .then((path) => {
        if (!disposed && isMountedRef.current) setConfigFilePath(path);
      })
      .catch((error) => {
        console.error('[config_file_path] Failed:', error);
        if (!disposed && isMountedRef.current) setConfigFileError(String(error));
      });

    // The CLI card describes a file on disk, which an app update can add or
    // remove, so it is read per open for the same reason as the two rows above.
    setCliInfoError('');
    void getCliInfo()
      .then((info) => {
        if (!disposed && isMountedRef.current) setCliInfo(info);
      })
      .catch((error) => {
        console.error('[cli_info] Failed:', error);
        if (!disposed && isMountedRef.current) setCliInfoError(String(error));
      });

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
        const { id, message } = event.payload;
        // Downloads are one of the flows a user reports as "it didn't work", and
        // the banner is gone on the next open — the log file keeps the message.
        // Reported before the render guard: whether the panel still wants to
        // paint is a UI question, and the failure is worth recording regardless.
        reportFrontendError(message, { context: `model_download_error ${id}` });
        if (disposed || !isMountedRef.current) return;
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

  /**
   * Start a transfer. Everything below the licence gate lands here.
   *
   * Split out of `handleDownloadModel` so the confirmation dialog has somewhere
   * to hand the click back to, and so the gate is the only place that decides
   * whether a model needs acknowledging.
   */
  const startDownloadModel = async (id: string) => {
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
        reportFrontendError(error, { context: `download_model ${id}` });
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

  /**
   * A download request, gated on the model's upstream licence.
   *
   * Anything but a plainly permissive licence (`allowed`) opens
   * `ModelLicensePrompt` first. 51 of the 129 catalogue entries are conditional,
   * non-commercial, or carry no published licence at all, and starting one of
   * those transfers in silence would be the app quietly deciding that the terms
   * do not matter. The dialog names the licence, links to the publisher's own
   * text, and its confirmation is what starts the transfer.
   */
  const handleDownloadModel = (id: string) => {
    const model = models.find((candidate) => candidate.id === id);
    if (model && model.commercial_use !== 'allowed') {
      setPendingLicenseModel(model);
      return;
    }
    void startDownloadModel(id);
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

  const handleOpenLogsDir = async () => {
    setLogDirectoryError('');
    try {
      await openLogsDirectory();
    } catch (error) {
      console.error('[open_logs_dir] Failed:', error);
      reportFrontendError(error, { context: 'open_logs_dir' });
      setLogDirectoryError(String(error));
    }
  };

  const handleRevealConfigFile = async () => {
    setConfigFileError('');
    try {
      await revealConfigFile();
    } catch (error) {
      console.error('[reveal_config_file] Failed:', error);
      reportFrontendError(error, { context: 'reveal_config_file' });
      setConfigFileError(String(error));
    }
  };

  /** Copy the install command. The chip says "copied" for two seconds. */
  const handleCopyCliCommand = async () => {
    if (installCommand === null) return;

    setCliCopyError('');
    try {
      await writeClipboard(installCommand);
      setCliCopied(true);
      if (cliCopyTimerRef.current !== null) window.clearTimeout(cliCopyTimerRef.current);
      cliCopyTimerRef.current = window.setTimeout(() => {
        cliCopyTimerRef.current = null;
        setCliCopied(false);
      }, COPIED_RESET_MS);
    } catch (error) {
      // A failed copy must not leave the previous "copied" label standing.
      if (cliCopyTimerRef.current !== null) {
        window.clearTimeout(cliCopyTimerRef.current);
        cliCopyTimerRef.current = null;
      }
      setCliCopied(false);
      console.error('[cli copy] Failed:', error);
      reportFrontendError(error, { context: 'Settings copy CLI command' });
      setCliCopyError(t('settings.system.cliCopyFailed'));
    }
  };

  /**
   * Open the repository in the system browser.
   *
   * `open_external_url` is what the app uses everywhere else; the `window.open`
   * fallback exists because a WebView without the opener shell (a dev server,
   * for instance) throws instead. Failure is surfaced in the card as well as
   * logged, because the address is also printed there — a dead button with a
   * copyable URL next to it is worse than a button that says why it failed.
   */
  const handleOpenRepoUrl = async () => {
    setRepoError('');
    try {
      await invoke('open_external_url', { url: REPO_URL });
    } catch (error) {
      console.error('[open_external_url] Failed:', error);
      try {
        window.open(REPO_URL, '_blank');
      } catch (fallbackError) {
        console.error('[window.open] Failed:', fallbackError);
        reportFrontendError(error, { context: 'Settings open repository URL' });
        setRepoError(t('settings.about.openFailed'));
      }
    }
  };

  /** Copy the repository address. The chip says "copied" for two seconds. */
  const handleCopyRepoUrl = async () => {
    setRepoError('');
    try {
      await writeClipboard(REPO_URL);
      setRepoCopied(true);
      if (repoCopyTimerRef.current !== null) window.clearTimeout(repoCopyTimerRef.current);
      repoCopyTimerRef.current = window.setTimeout(() => {
        repoCopyTimerRef.current = null;
        setRepoCopied(false);
      }, COPIED_RESET_MS);
    } catch (error) {
      if (repoCopyTimerRef.current !== null) {
        window.clearTimeout(repoCopyTimerRef.current);
        repoCopyTimerRef.current = null;
      }
      setRepoCopied(false);
      console.error('[repo copy] Failed:', error);
      reportFrontendError(error, { context: 'Settings copy repository URL' });
      setRepoError(t('settings.about.copyFailed'));
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
  // lists and their headers stay in step with the loaded models. Both are
  // ordered installed-first: this pane is where a user checks what they have and
  // fetches what they lack, so a downloaded model must not sit below dozens of
  // entries that still need a download.
  const recognition = useMemo(
    () => sortByAvailability(recognitionModels(models), downloadingId),
    [models, downloadingId],
  );
  const translation = useMemo(
    () => sortByAvailability(translationModels(models), downloadingId),
    [models, downloadingId],
  );

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
                { id: 'urlImport' as const, label: t('settings.urlImport.title'), icon: Link2 },
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
                    {item.id === 'urlImport' && <BetaBadge className="ml-auto shrink-0" />}
                  </GlassButton>
                );
              })}
            </div>
          </aside>

          <div className="flex-1 min-h-0 min-w-0 flex flex-col bg-bg-tertiary/90 dark:bg-zinc-900/60">
            <div className="px-8 py-5 border-b border-glass-border-light dark:border-glass-border-dark bg-bg-tertiary/95 dark:bg-zinc-900/70">
              <div className="flex items-center gap-2">
                <h3 className="text-base font-bold text-text-primary dark:text-white">
                  {
                    {
                      files: t('settings.files.title'),
                      models: t('settings.modelsGroup.title'),
                      translate: t('settings.translate.title'),
                      urlImport: t('settings.urlImport.title'),
                      system: t('settings.system.title'),
                    }[activeCategory]
                  }
                </h3>
                {activeCategory === 'urlImport' && <BetaBadge />}
              </div>
              {activeCategory === 'urlImport' && (
                <p className="mt-1 text-xs text-text-tertiary dark:text-gray-500 leading-relaxed">
                  {t('settings.urlImport.betaHint')}
                </p>
              )}
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
                  <div className="p-4 rounded-2xl border border-divider dark:border-zinc-700 bg-bg-secondary/70 dark:bg-zinc-900/50 space-y-4">
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

                    {/* Concurrency: how many files run at once. The same number
                        sizes the Rust semaphore, so the queue and the GPU agree
                        (see `AppSettings.concurrency`). A segmented control
                        rather than a dropdown: the range is 1–3 and seeing all
                        three at once is clearer than a menu with three items. */}
                    <div className="flex items-center justify-between gap-4 pt-3 border-t border-divider dark:border-zinc-700">
                      <div className="min-w-0">
                        <label className="text-sm font-semibold text-text-primary dark:text-gray-200">
                          {t('settings.processing.concurrency')}
                        </label>
                        <p className="text-xs text-text-secondary dark:text-gray-400 mt-0.5">
                          {t('settings.processing.concurrencyDesc')}
                        </p>
                      </div>
                      <div className="flex gap-1 bg-bg-secondary dark:bg-zinc-800 rounded-lg p-1 shrink-0">
                        {Array.from({ length: MAX_CONCURRENCY }, (_, i) => i + MIN_CONCURRENCY).map((n) => (
                          <GlassButton
                            key={n}
                            variant={settings.concurrency === n ? 'default' : 'ghost'}
                            size="sm"
                            onClick={() => updateSettings({ concurrency: n })}
                            className="px-3 h-7 text-sm tabular-nums"
                            data-testid={`concurrency-${n}`}
                          >
                            {n}
                          </GlassButton>
                        ))}
                      </div>
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
                            models in by hand or changing the storage location.

                            There is deliberately no "open folder" button here:
                            `SettingsModelsDir` above already has one, sitting
                            next to "change location", and two buttons with the
                            same label within one screen read as a bug. */}
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

              {activeCategory === 'urlImport' && <SettingsUrlImport />}

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

                  {/* Log directory: the one file to attach to a bug report */}
                  <div className="p-4 rounded-2xl border border-divider dark:border-zinc-700 bg-bg-secondary/70 dark:bg-zinc-900/50 space-y-3">
                    <div className="flex items-start justify-between gap-4">
                      <div className="min-w-0">
                        <div className="flex items-center gap-2">
                          <FolderOpen className="w-4 h-4 text-text-primary dark:text-gray-300" />
                          <span className="text-sm font-semibold text-text-primary dark:text-gray-200">
                            {t('settings.system.logDir')}
                          </span>
                        </div>
                        <p
                          className="text-xs text-text-secondary dark:text-gray-400 mt-1 break-all"
                          title={logDirectory}
                        >
                          {logDirectory || '—'}
                        </p>
                        <p className="text-xxs text-text-tertiary dark:text-gray-500 mt-1">
                          {t('settings.system.logDirHint')}
                        </p>
                      </div>

                      <GlassButton
                        variant="outline"
                        size="sm"
                        data-testid="settings-open-logs"
                        onClick={() => void handleOpenLogsDir()}
                      >
                        <FolderOpen className="w-4 h-4" />
                        {t('settings.system.openLogs')}
                      </GlassButton>
                    </div>

                    {logDirectoryError && (
                      <p className="text-xs text-danger break-all">{logDirectoryError}</p>
                    )}
                  </div>

                  {/*
                    Config file. Deliberately its own row rather than a detail of
                    the models card: `settings.json` and the models directory are
                    two unrelated locations, and the confusion this row prevents
                    is exactly "I changed the models folder, so that is where my
                    settings are". The path is shown even when the file does not
                    exist yet — a fresh install has not written it, and the path
                    is still the right answer to "where would it be?".
                  */}
                  <div className="p-4 rounded-2xl border border-divider dark:border-zinc-700 bg-bg-secondary/70 dark:bg-zinc-900/50 space-y-3">
                    <div className="flex items-start justify-between gap-4">
                      <div className="min-w-0">
                        <div className="flex items-center gap-2">
                          <FileCog className="w-4 h-4 text-text-primary dark:text-gray-300" />
                          <span className="text-sm font-semibold text-text-primary dark:text-gray-200">
                            {t('settings.system.configFile')}
                          </span>
                        </div>
                        {/* Rendered even when the file has not been written yet;
                            `config_file_path` returns the path either way, so an
                            empty value means the IPC call failed and is reported
                            below the row. */}
                        <p
                          className="text-xs text-text-secondary dark:text-gray-400 mt-1 break-all"
                          title={configFilePath}
                          data-testid="settings-config-file-path"
                        >
                          {configFilePath}
                        </p>
                        <p className="text-xxs text-text-tertiary dark:text-gray-500 mt-1">
                          {t('settings.system.configFileHint')}
                        </p>
                      </div>

                      <GlassButton
                        variant="outline"
                        size="sm"
                        data-testid="settings-reveal-config"
                        onClick={() => void handleRevealConfigFile()}
                      >
                        <FolderOpen className="w-4 h-4" />
                        {t('settings.system.configFileOpen')}
                      </GlassButton>
                    </div>

                    {configFileError && (
                      <p className="text-xs text-danger break-all">{configFileError}</p>
                    )}
                  </div>

                  {/*
                    Command-line tool. The "coming soon" badge is gone: the CLI
                    shipped with R4, so a card claiming otherwise was the one
                    piece of UI that contradicted the README.

                    Every fact comes from `cli_info` (paths.rs / engine::cli), not
                    from a constant here: the previous hardcoded `ln -s` named
                    `Contents/MacOS/monosub-cli`, which is the `externalBin`
                    layout the design sketched and no build implements. The
                    bundled binary lives in `resources/bin/`.

                    What the instruction *is* differs per platform because
                    putting a command on PATH does:
                    * Windows — the package declares an execution alias, so the
                      command already works and there is nothing to install.
                      Adding the install directory to PATH instead would break on
                      the next update (MSIX installs under a version-stamped
                      folder).
                    * macOS / Linux — one `ln -s` into `/usr/local/bin`. The app
                      still does not modify PATH itself; the user runs it.
                  */}
                  <div className="p-4 rounded-2xl border border-divider dark:border-zinc-700 bg-bg-secondary/70 dark:bg-zinc-900/50 space-y-3">
                    <div className="flex items-center gap-2 flex-wrap">
                      <Terminal className="w-4 h-4 text-text-primary dark:text-gray-300" />
                      <span className="text-sm font-semibold text-text-primary dark:text-gray-200">
                        {t('settings.system.cliTitle')}
                      </span>
                      {cliInfo?.path && (
                        <GlassBadge variant="success" size="sm" data-testid="settings-cli-available">
                          {t('settings.system.cliAvailable')}
                        </GlassBadge>
                      )}
                      {cliInfo && !cliInfo.path && (
                        <GlassBadge variant="warning" size="sm" data-testid="settings-cli-not-packaged">
                          {t('settings.system.cliNotPackaged')}
                        </GlassBadge>
                      )}
                    </div>

                    <p className="text-xs text-text-secondary dark:text-gray-400 leading-relaxed">
                      {t('settings.system.cliHint')}
                    </p>

                    {cliInfoError && (
                      <p className="text-xs text-danger break-all">{cliInfoError}</p>
                    )}

                    {/* The command, not the file's absolute path. The path is a
                        build artefact inside the bundle (`…/resources/bin/…`)
                        and changes with every install, while the name is what a
                        terminal actually resolves — the execution alias on
                        Windows, the `ln -s` target on macOS. It stays reachable
                        as the chip's tooltip for anyone debugging a build. */}
                    {cliInfo?.path && (
                      <code
                        className="inline-block max-w-full select-text truncate rounded-lg border border-divider bg-black/[0.04] px-2 py-1 font-mono text-xs text-text-primary dark:border-white/10 dark:bg-black/40 dark:text-gray-200"
                        title={cliInfo.path}
                        data-testid="settings-cli-command"
                      >
                        {cliCommand}
                      </code>
                    )}

                    {/* Windows: nothing to install, so say that rather than
                        showing an empty command box. */}
                    {installCommand === null && cliInfo?.path && cliInfo.usesExecutionAlias && (
                      <p className="text-xs text-text-secondary dark:text-gray-400 leading-relaxed">
                        {t('settings.system.cliAliasHint')}
                      </p>
                    )}

                    {installCommand !== null && (
                      <>
                        <p className="text-xs text-text-secondary dark:text-gray-400 leading-relaxed">
                          {t('settings.system.cliInstallHint')}
                        </p>
                        <div className="flex items-center gap-2">
                          <code
                            className="min-w-0 flex-1 select-text break-all rounded-xl border border-divider bg-black/[0.04] px-3 py-2 font-mono text-xs text-text-primary dark:border-white/10 dark:bg-black/40 dark:text-gray-200"
                            data-testid="settings-cli-install-command"
                          >
                            {installCommand}
                          </code>
                          <GlassButton
                            variant="outline"
                            size="sm"
                            data-testid="settings-cli-copy"
                            onClick={() => void handleCopyCliCommand()}
                          >
                            {cliCopied ? <Check className="w-4 h-4" /> : <Copy className="w-4 h-4" />}
                            {cliCopied ? t('settings.system.cliCopied') : t('settings.system.cliCopy')}
                          </GlassButton>
                        </div>

                        {cliCopyError && (
                          <p className="text-xs text-danger break-all">{cliCopyError}</p>
                        )}
                      </>
                    )}
                  </div>

                  {/* About + source */}
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

                      <GlassButton
                        variant="primary"
                        size="sm"
                        onClick={() => void handleOpenRepoUrl()}
                        data-testid="settings-view-source"
                      >
                        <ExternalLink className="w-4 h-4" />
                        {t('settings.about.viewSource')}
                      </GlassButton>
                    </div>

                    {/* The address itself, so the button is never the only way
                        out: it can be read, selected or copied even when the
                        opener is unavailable. */}
                    <div className="space-y-1.5">
                      <p className="text-xs text-text-secondary dark:text-gray-400">
                        {t('settings.about.repository')}
                      </p>
                      <div className="flex items-center gap-2">
                        <a
                          href={REPO_URL}
                          onClick={(event) => {
                            // The WebView must not navigate to it: opening is
                            // the opener's job, and that keeps the app window.
                            event.preventDefault();
                            void handleOpenRepoUrl();
                          }}
                          className="min-w-0 flex-1 select-text break-all text-xs text-primary hover:underline"
                          data-testid="settings-repo-url"
                        >
                          {REPO_URL}
                        </a>
                        <GlassButton
                          variant="outline"
                          size="sm"
                          onClick={() => void handleCopyRepoUrl()}
                          data-testid="settings-repo-copy"
                        >
                          {repoCopied ? <Check className="w-4 h-4" /> : <Copy className="w-4 h-4" />}
                          {repoCopied ? t('settings.about.copied') : t('settings.about.copy')}
                        </GlassButton>
                      </div>
                    </div>

                    {repoError && (
                      <p className="text-xs text-danger break-all">{repoError}</p>
                    )}

                    <p className="text-xs text-text-secondary dark:text-gray-400">
                      {t('settings.about.hint')}
                    </p>
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

      {/* Outside the panel so the portal is not nested in the settings
          animation. `model={null}` is the closed state. */}
      <ModelLicensePrompt
        model={pendingLicenseModel}
        onCancel={() => setPendingLicenseModel(null)}
        onConfirm={() => {
          const id = pendingLicenseModel?.id;
          setPendingLicenseModel(null);
          if (id) void startDownloadModel(id);
        }}
      />
    </div>
  );
}
