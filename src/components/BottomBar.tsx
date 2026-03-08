import { Folder, FolderOpen, HardDrive, Users, Loader2, ExternalLink } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { useStore } from '@/store/useStore';
import { open } from '@tauri-apps/plugin-dialog';
import { useState } from 'react';
import {
  ensureMultiSpeakerLicense,
  openLicenseCheckout,
  openLicenseSettings,
  promptLicenseFailureAction,
} from '@/lib/license';
import { GlassButton } from '@/components/ui/glass-button';
import { GlassSwitch } from '@/components/ui/glass-switch';

export function BottomBar() {
  const { t } = useTranslation();
  const storageMode = useStore((state) => state.storageMode);
  const setStorageMode = useStore((state) => state.setStorageMode);
  const customStoragePath = useStore((state) => state.customStoragePath);
  const setCustomStoragePath = useStore((state) => state.setCustomStoragePath);
  const isProcessing = useStore((state) => state.isProcessing);
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
    <div className="bg-bg-secondary dark:bg-zinc-950 h-14 px-6 flex-shrink-0 sticky bottom-0 z-30 flex items-center justify-between border-t border-divider/40 dark:border-zinc-800/40 transition-colors">
      <div className="flex items-center gap-2">
        <HardDrive className="w-4 h-4 text-text-tertiary dark:text-gray-500" />
        <span className="text-xs text-text-tertiary dark:text-gray-500 mr-2">{t('bottomBar.saveLocation')}</span>

        {/* 存储模式分段控件 - muted 变体支持亮暗双模式 */}
        <div className="flex bg-bg-tertiary dark:bg-zinc-800 rounded-lg p-0.5">
          <GlassButton
            variant="muted"
            size="sm"
            onClick={() => setStorageMode('same')}
            className={`h-7 px-2.5 text-xs ${storageMode === 'same'
                ? 'bg-white dark:bg-zinc-700 text-text-primary dark:text-white shadow-apple hover:scale-100'
                : ''
              }`}
          >
            <Folder className="w-3.5 h-3.5" />
            {t('bottomBar.sameDirectory')}
          </GlassButton>
          <GlassButton
            variant="muted"
            size="sm"
            onClick={() => void handleSelectCustomPath()}
            className={`h-7 px-2.5 text-xs ${storageMode === 'custom'
                ? 'bg-white dark:bg-zinc-700 text-text-primary dark:text-white shadow-apple hover:scale-100'
                : ''
              }`}
          >
            <FolderOpen className="w-3.5 h-3.5" />
            {storageMode === 'custom' && customStoragePath
              ? truncatePath(customStoragePath)
              : t('bottomBar.selectDirectory')}
          </GlassButton>
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
          <div className="relative flex items-center">
            {isCheckingLicense && (
              <Loader2 className="absolute inset-0 m-auto w-3 h-3 text-white animate-spin z-10 pointer-events-none" />
            )}
            {/* GlassSwitch 在亮色下也有足够对比 - 背景已固定 */}
            <GlassSwitch
              checked={enableSpeakerDiarization}
              onCheckedChange={() => void handleToggleSpeakerDiarization()}
              disabled={isProcessing || isCheckingLicense}
              title={t('bottomBar.multiSpeakerHint')}
              className="border border-divider dark:border-transparent"
            />
          </div>
          {licenseStatus !== 'valid' && (
            <GlassButton
              variant="ghost"
              size="sm"
              onClick={() => void openLicenseCheckout()}
              title={t('bottomBar.proRequired')}
              className="h-5 px-2 text-[10px] font-semibold text-warning hover:text-warning bg-warning/10 hover:bg-warning/20 hover:scale-100 rounded-full"
            >
              PRO
              <ExternalLink className="w-2.5 h-2.5" />
            </GlassButton>
          )}
        </div>

        <span className="w-px h-4 bg-divider dark:bg-zinc-700" />
      </div>
    </div>
  );
}
