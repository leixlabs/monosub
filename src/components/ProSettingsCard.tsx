import { useTranslation } from 'react-i18next';
import { Sparkles, Loader2, RotateCw } from 'lucide-react';
import { GlassButton } from '@/components/ui/glass-button';
import { GlassBadge } from '@/components/ui/glass-badge';
import { useEntitlementStore } from '@/store/useEntitlement';

/**
 * Settings ▸ System ▸ MonoSub 授权.
 *
 * It also hosts the debug-only fake-state buttons. `import.meta.env.DEV` is
 * false in the Vite production build, and the Rust commands themselves refuse
 * to change state in a release binary, so this is not a shipping bypass.
 */
export function ProSettingsCard() {
  const { t } = useTranslation();
  const status = useEntitlementStore((state) => state.status);
  const busy = useEntitlementStore((state) => state.busy);
  const openUpgrade = useEntitlementStore((state) => state.openUpgrade);
  const restore = useEntitlementStore((state) => state.restore);
  const setDevState = useEntitlementStore((state) => state.setDevState);
  const clearDevState = useEntitlementStore((state) => state.clearDevState);
  const isDev = import.meta.env.DEV || import.meta.env.VITE_IAP_DEV === '1';

  // Release direct builds have nothing to sell; in development the card stays
  // visible so the fake-state buttons below can exercise the free tier.
  if (status.provider === 'direct' && !isDev) return null;

  const price = status.product?.displayPrice ?? t('pro.priceFallback');
  const isGrandfathered = status.provider === 'app_store_grandfathered';

  return (
    <div className="p-4 rounded-2xl border border-divider dark:border-zinc-700 bg-bg-secondary/70 dark:bg-zinc-900/50 space-y-3" data-testid="settings-pro-card">
      <div className="flex items-start justify-between gap-4">
        <div className="min-w-0">
          <div className="flex items-center gap-2 flex-wrap">
            <Sparkles className="w-4 h-4 text-primary" />
            <span className="text-sm font-semibold text-text-primary dark:text-gray-200">
              {t('pro.productName')}
            </span>
            {status.status === 'pro' && (
              <GlassBadge variant="success" size="sm">{t('pro.activeBadge')}</GlassBadge>
            )}
            {status.status === 'pending' && (
              <GlassBadge variant="warning" size="sm">{t('pro.pendingBadge')}</GlassBadge>
            )}
          </div>

          <p className="mt-1 text-xs text-text-secondary dark:text-gray-400 leading-relaxed">
            {status.status === 'unknown' || status.status === 'checking'
              ? t('pro.checking')
              : isGrandfathered
                ? t('pro.grandfathered')
                : status.isPro
                  ? t('pro.active')
                  : t('pro.freeLimit')}
          </p>
          {status.error && <p className="mt-1 text-xs text-danger">{status.error}</p>}
        </div>

        <div className="flex shrink-0 items-center gap-2">
          {!status.isPro && status.status !== 'pending' && (
            <GlassButton variant="primary" size="sm" onClick={openUpgrade} data-testid="settings-pro-upgrade">
              {t('pro.upgrade')}
            </GlassButton>
          )}
          {!isGrandfathered && (
            <GlassButton
              variant="outline"
              size="sm"
              onClick={() => void restore()}
              disabled={busy !== 'idle'}
              data-testid="settings-pro-restore"
            >
              {busy === 'restore' ? <Loader2 className="w-4 h-4 animate-spin" /> : <RotateCw className="w-4 h-4" />}
              {t('pro.restore')}
            </GlassButton>
          )}
        </div>
      </div>

      {!status.isPro && (
        <p className="text-xs text-text-tertiary dark:text-gray-500">
          {t('pro.priceLine', { price })}
        </p>
      )}

      {isDev && (
        <div className="border-t border-divider dark:border-zinc-700 pt-3 space-y-2">
          <p className="text-[11px] font-semibold uppercase tracking-wide text-text-tertiary dark:text-gray-500">
            {t('pro.devTitle')}
          </p>
          <div className="flex flex-wrap gap-2">
            <GlassButton size="sm" variant="outline" onClick={() => void setDevState('free')} data-testid="pro-dev-free">
              {t('pro.devFree')}
            </GlassButton>
            <GlassButton size="sm" variant="outline" onClick={() => void setDevState('pro')} data-testid="pro-dev-pro">
              {t('pro.devPro')}
            </GlassButton>
            <GlassButton size="sm" variant="outline" onClick={() => void setDevState('pending')} data-testid="pro-dev-pending">
              {t('pro.devPending')}
            </GlassButton>
            <GlassButton size="sm" variant="outline" onClick={() => void setDevState('error')} data-testid="pro-dev-error">
              {t('pro.devError')}
            </GlassButton>
            <GlassButton size="sm" variant="ghost" onClick={() => void clearDevState()} data-testid="pro-dev-clear">
              {t('pro.devClear')}
            </GlassButton>
          </div>
        </div>
      )}
    </div>
  );
}
