import { useCallback, useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { invoke } from '@tauri-apps/api/core';
import { AlertTriangle, Download, RefreshCw } from 'lucide-react';

import { GlassButton } from '@/components/ui/glass-button';
import { getCudaStatus, type CudaStatus } from '@/lib/gpu';

/**
 * One line under the file list's empty state: is the CUDA engine usable here?
 *
 * It belongs to the empty state rather than to Settings because of *when* the
 * answer matters: a user with no driver learns it on their first file, after
 * waiting for the queue to fail. Saying it before anything is added costs one
 * `nvidia-smi` call and turns a mystery failure into an install link.
 *
 * Renders **nothing** unless `applicable` — macOS/Linux builds do not use CUDA,
 * and an error from the probe is not a verdict about the machine, so neither
 * gets a warning nobody can act on.
 */
export function CudaNotice() {
  const { t } = useTranslation();
  const [status, setStatus] = useState<CudaStatus | null>(null);
  const [isChecking, setIsChecking] = useState(true);

  const check = useCallback(async (refresh: boolean) => {
    setIsChecking(true);
    try {
      setStatus(await getCudaStatus(refresh));
    } catch (error) {
      // Silent on purpose: no UI is better than a scary line about a check the
      // user cannot fix. The failure is in the log for the bug report.
      console.error('[cuda_status] Failed:', error);
      setStatus(null);
    } finally {
      setIsChecking(false);
    }
  }, []);

  useEffect(() => {
    void check(false);
  }, [check]);

  const openDriverPage = async (url: string) => {
    try {
      await invoke('open_external_url', { url });
    } catch (error) {
      console.error('[open_external_url] Failed:', error);
      try {
        window.open(url, '_blank');
      } catch (fallbackError) {
        console.error('[window.open] Failed:', fallbackError);
      }
    }
  };

  if (!status?.applicable) return null;

  if (status.available) {
    return (
      <div
        className="mt-6 flex items-center justify-center gap-1.5 text-xs text-text-tertiary dark:text-gray-500"
        data-testid="cuda-ready"
        title={status.driverVersion ? `NVIDIA ${status.driverVersion}` : undefined}
      >
        <span className="w-1.5 h-1.5 rounded-full bg-success" />
        {t('fileList.cuda.ready', { gpu: status.gpuName ?? 'CUDA' })}
      </div>
    );
  }

  /**
   * A driver install only helps when the driver is the problem. On a card below
   * the build's floor (`compute_capability_too_low`) no driver can add the
   * missing kernels — sending that user to NVIDIA's download page would waste
   * their afternoon and end exactly where it started.
   */
  const driverCanFix = status.reason !== 'compute_capability_too_low';

  const reason = (() => {
    switch (status.reason) {
      case 'no_driver':
        return t('fileList.cuda.reasonNoDriver');
      case 'no_gpu':
        return t('fileList.cuda.reasonNoGpu');
      case 'compute_capability_too_low':
        return t('fileList.cuda.reasonTooOld', {
          gpu: status.gpuName ?? t('fileList.cuda.thisGpu'),
          capability: status.computeCapability ?? '?',
          minimum: status.minimumComputeCapability,
        });
      case 'probe_failed':
        return t('fileList.cuda.reasonProbeFailed', { detail: status.detail ?? 'nvidia-smi' });
      default:
        return '';
    }
  })();

  return (
    <div
      className="mt-6 rounded-2xl border border-warning/40 bg-warning/5 dark:bg-warning/10 p-3 text-left"
      data-testid="cuda-unavailable"
    >
      <div className="flex items-start gap-2">
        <AlertTriangle className="w-4 h-4 mt-0.5 shrink-0 text-warning" />
        <div className="min-w-0 space-y-1">
          <p className="text-xs font-semibold text-text-primary dark:text-gray-200">
            {t('fileList.cuda.title')}
          </p>
          <p className="text-xs text-text-secondary dark:text-gray-400 leading-relaxed">
            {reason}
          </p>
          <p className="text-xxs text-text-tertiary dark:text-gray-500 leading-relaxed">
            {driverCanFix ? t('fileList.cuda.hint') : t('fileList.cuda.tooOldHint')}
          </p>

          <div className="flex items-center gap-2 pt-1.5">
            {driverCanFix && (
              <GlassButton
                variant="primary"
                size="sm"
                onClick={() => void openDriverPage(status.driverUrl)}
                data-testid="cuda-install-driver"
              >
                <Download className="w-3.5 h-3.5" />
                {t('fileList.cuda.installDriver')}
              </GlassButton>
            )}
            <GlassButton
              variant="ghost"
              size="sm"
              onClick={() => void check(true)}
              disabled={isChecking}
              data-testid="cuda-recheck"
            >
              <RefreshCw className={`w-3.5 h-3.5 ${isChecking ? 'animate-spin' : ''}`} />
              {t('fileList.cuda.recheck')}
            </GlassButton>
          </div>
        </div>
      </div>
    </div>
  );
}
