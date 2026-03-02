import { Folder, FolderOpen, StopCircle, HardDrive, Users, Loader2, ExternalLink } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { motion } from 'framer-motion';
import { useStore } from '@/store/useStore';
import { open } from '@tauri-apps/plugin-dialog';
import { invoke } from '@tauri-apps/api/core';
import { useState } from 'react';
import {
  ensureMultiSpeakerLicense,
  openLicenseCheckout,
  openLicenseSettings,
  promptLicenseFailureAction,
} from '@/lib/license';

export function BottomBar() {
  const { t } = useTranslation();
  const storageMode = useStore((state) => state.storageMode);
  const setStorageMode = useStore((state) => state.setStorageMode);
  const customStoragePath = useStore((state) => state.customStoragePath);
  const setCustomStoragePath = useStore((state) => state.setCustomStoragePath);
  const isProcessing = useStore((state) => state.isProcessing);
  const setIsProcessing = useStore((state) => state.setIsProcessing);
  const setCurrentTaskId = useStore((state) => state.setCurrentTaskId);
  const files = useStore((state) => state.files);
  const updateFile = useStore((state) => state.updateFile);
  const settings = useStore((state) => state.settings);
  const enableSpeakerDiarization = useStore((state) => state.enableSpeakerDiarization);
  const setEnableSpeakerDiarization = useStore((state) => state.setEnableSpeakerDiarization);
  const licenseStatus = useStore((state) => state.licenseStatus);
  const [isCheckingLicense, setIsCheckingLicense] = useState(false);

  const handleSelectCustomPath = async () => {
    try {
      const selected = await open({
        directory: true,
      });

      if (selected && typeof selected === 'string') {
        setCustomStoragePath(selected);
        setStorageMode('custom');
      }
    } catch (error) {
      console.error('Failed to select directory:', error);
    }
  };

  const truncatePath = (path: string, maxLength: number = 20) => {
    if (path.length <= maxLength) return path;
    const parts = path.split('/');
    const last = parts[parts.length - 1];
    if (last.length > maxLength) {
      return '...' + last.slice(-maxLength);
    }
    return '.../' + last;
  };

  const getLanguageLabel = () => {
    const labels: Record<string, string> = {
      'auto': t('bottomBar.auto'),
      'en': 'EN',
      'zh': '中文',
      'es': 'ES',
      'ja': '日本語',
      'de': 'DE',
      'fr': 'FR',
      'pt': 'PT',
      'zh-tw': '繁中',
      'ko': 'KR',
    };
    return labels[settings.outputLanguage] || settings.outputLanguage;
  };

  const handleStop = async () => {
    const processingFile = files.find(f => f.status === 'processing');
    if (!processingFile) return;

    try {
      await invoke('cancel_transcription', { taskId: processingFile.id });
      updateFile(processingFile.id, { status: 'pending', progress: 0 });
      setIsProcessing(false);
      setCurrentTaskId(null);
    } catch (error) {
      console.error('[cancel_transcription] Failed:', error);
    }
  };

  const handleToggleSpeakerDiarization = async () => {
    if (isProcessing || isCheckingLicense) return;

    if (enableSpeakerDiarization) {
      setEnableSpeakerDiarization(false);
      return;
    }

    setIsCheckingLicense(true);
    try {
      const licensed = await ensureMultiSpeakerLicense({
        openCheckoutOnFailure: false,
      });
      if (licensed) {
        setEnableSpeakerDiarization(true);
        return;
      }

      const latestLicenseConfig = useStore.getState().licenseConfig;
      const hasLicenseKey = typeof latestLicenseConfig?.licenseKey === 'string'
        && latestLicenseConfig.licenseKey.trim().length > 0;

      const action = await promptLicenseFailureAction({
        title: t('bottomBar.licensePromptTitle'),
        message: hasLicenseKey
          ? t('bottomBar.licensePromptWithKeyMessage')
          : t('bottomBar.licensePromptMessage'),
        openCheckoutLabel: t('bottomBar.openInBrowser'),
        openSettingsLabel: t('bottomBar.openSettings'),
        cancelLabel: t('bottomBar.later'),
      });

      if (action === 'settings') {
        openLicenseSettings();
      } else if (action === 'checkout') {
        await openLicenseCheckout();
      }
    } finally {
      setIsCheckingLicense(false);
    }
  };

  return (
    <div className="bg-bg-secondary dark:bg-zinc-950 h-14 px-6 flex items-center justify-between border-t border-divider/40 dark:border-zinc-800/40 transition-colors">
      <div className="flex items-center gap-2">
        <HardDrive className="w-4 h-4 text-text-tertiary dark:text-gray-500" />
        <span className="text-xs text-text-tertiary dark:text-gray-500 mr-2">{t('bottomBar.saveLocation')}</span>

        <div className="flex bg-bg-secondary dark:bg-zinc-800 rounded-lg p-0.5">
          <motion.button
            whileTap={{ scale: 0.97 }}
            onClick={() => setStorageMode('same')}
            className={`flex items-center gap-1.5 px-3 py-1.5 text-xs rounded-md transition-all ${storageMode === 'same'
                ? 'bg-white dark:bg-zinc-700 text-text-primary dark:text-white shadow-apple'
                : 'text-text-secondary dark:text-gray-400 hover:text-text-primary dark:hover:text-white'
              }`}
          >
            <Folder className="w-3.5 h-3.5" />
            {t('bottomBar.sameDirectory')}
          </motion.button>
          <motion.button
            whileTap={{ scale: 0.97 }}
            onClick={handleSelectCustomPath}
            className={`flex items-center gap-1.5 px-3 py-1.5 text-xs rounded-md transition-all ${storageMode === 'custom'
                ? 'bg-white dark:bg-zinc-700 text-text-primary dark:text-white shadow-apple'
                : 'text-text-secondary dark:text-gray-400 hover:text-text-primary dark:hover:text-white'
              }`}
          >
            <FolderOpen className="w-3.5 h-3.5" />
            {storageMode === 'custom' && customStoragePath
              ? truncatePath(customStoragePath)
              : t('bottomBar.selectDirectory')}
          </motion.button>
        </div>

        <span className="mx-3 w-px h-4 bg-divider dark:bg-zinc-700" />

        <span className="text-xs text-text-tertiary dark:text-gray-500">
          {t('bottomBar.format')} <span className="text-text-secondary dark:text-gray-300 font-medium">{settings.outputFormat.toUpperCase()}</span>
        </span>
        <span className="mx-2 w-px h-3 bg-divider dark:bg-zinc-700" />
        <span className="text-xs text-text-tertiary dark:text-gray-500">
          {t('bottomBar.language')} <span className="text-text-secondary dark:text-gray-300 font-medium">{getLanguageLabel()}</span>
        </span>
      </div>

      <div className="flex items-center gap-3">
        {/* Multi-speaker toggle */}
        <div className="flex items-center gap-2">
          <Users className="w-4 h-4 text-text-tertiary dark:text-gray-500" />
          <span className="text-xs text-text-tertiary dark:text-gray-500">
            {t('bottomBar.multiSpeaker')}
          </span>
          <button
            onClick={() => void handleToggleSpeakerDiarization()}
            disabled={isProcessing || isCheckingLicense}
            className={`relative w-10 h-5 rounded-full transition-colors disabled:opacity-50 disabled:cursor-not-allowed ${
              enableSpeakerDiarization
                ? 'bg-primary'
                : 'bg-bg-tertiary dark:bg-zinc-700'
            }`}
            title={t('bottomBar.multiSpeakerHint')}
          >
            {isCheckingLicense && (
              <Loader2 className="absolute inset-0 m-auto w-3 h-3 text-white animate-spin" />
            )}
            <span
              className={`absolute top-0.5 left-0.5 w-4 h-4 rounded-full bg-white shadow-sm transition-transform ${
                enableSpeakerDiarization ? 'translate-x-5' : ''
              }`}
            />
          </button>
          {licenseStatus !== 'valid' && (
            <button
              onClick={() => void openLicenseCheckout()}
              className="inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-[10px] font-semibold bg-warning/15 text-warning hover:bg-warning/20 transition-colors"
              title={t('bottomBar.proRequired')}
            >
              PRO
              <ExternalLink className="w-2.5 h-2.5" />
            </button>
          )}
        </div>

        <span className="w-px h-4 bg-divider dark:bg-zinc-700" />

        {isProcessing && (
          <motion.button
            whileTap={{ scale: 0.97 }}
            onClick={handleStop}
            className="flex items-center gap-1.5 px-4 py-1.5 bg-danger/10 dark:bg-danger/20 text-danger text-xs font-medium rounded-lg hover:bg-danger/20 dark:hover:bg-danger/30 transition-colors"
          >
            <StopCircle className="w-3.5 h-3.5" />
            {t('bottomBar.stop')}
          </motion.button>
        )}
      </div>
    </div>
  );
}
