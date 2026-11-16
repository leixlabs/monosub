import { useTranslation } from 'react-i18next';
import { Check, Loader2, Sparkles } from 'lucide-react';
import {
  GlassDialog,
  GlassDialogContent,
  GlassDialogDescription,
  GlassDialogFooter,
  GlassDialogHeader,
  GlassDialogTitle,
} from '@/components/ui/glass-dialog';
import { GlassButton } from '@/components/ui/glass-button';
import { useEntitlementStore } from '@/store/useEntitlement';

/**
 * The one upgrade surface. It is opened whenever the queue gate would refuse a
 * second file, and from the Settings license card.
 *
 * The price is never hardcoded here: `displayPrice` comes from StoreKit, so the
 * dialog shows the storefront's actual currency/price (US$9.99, ¥48, or the
 * local equivalent). The fallback only covers the short window before StoreKit
 * returns product metadata.
 */
export function ProUpgradeDialog() {
  const { t } = useTranslation();
  const open = useEntitlementStore((state) => state.upgradeOpen);
  const close = useEntitlementStore((state) => state.closeUpgrade);
  const status = useEntitlementStore((state) => state.status);
  const busy = useEntitlementStore((state) => state.busy);
  const error = useEntitlementStore((state) => state.error);
  const purchase = useEntitlementStore((state) => state.purchase);
  const restore = useEntitlementStore((state) => state.restore);

  const product = status.product;
  const price = product?.displayPrice ?? t('pro.priceFallback');
  const isGrandfathered = status.provider === 'app_store_grandfathered';

  if (status.provider === 'direct') return null;

  return (
    <GlassDialog open={open} onOpenChange={(next) => { if (!next) close(); }}>
      <GlassDialogContent
        className="max-w-md rounded-3xl"
        data-testid="pro-upgrade-dialog"
      >
        <GlassDialogHeader>
          <div className="mb-1 grid h-10 w-10 place-items-center rounded-2xl bg-primary/10 text-primary">
            <Sparkles className="h-5 w-5" />
          </div>
          <GlassDialogTitle>{t('pro.title')}</GlassDialogTitle>
          <GlassDialogDescription>{t('pro.subtitle')}</GlassDialogDescription>
        </GlassDialogHeader>

        <div className="mt-5 space-y-3">
          <div className="rounded-2xl border border-divider bg-black/[0.03] p-4 dark:border-white/10 dark:bg-white/[0.04]">
            <div className="flex items-baseline justify-between gap-3">
              <span className="text-sm font-medium text-text-primary dark:text-zinc-100">
                {t('pro.productName')}
              </span>
              <span className="text-xl font-bold tabular-nums text-text-primary dark:text-white" data-testid="pro-price">
                {price}
              </span>
            </div>
            <p className="mt-1 text-xs text-text-secondary dark:text-zinc-400">
              {t('pro.oneTime')}
            </p>

            <ul className="mt-3 space-y-2">
              <li className="flex items-start gap-2 text-xs text-text-secondary dark:text-zinc-300">
                <Check className="mt-0.5 h-3.5 w-3.5 shrink-0 text-success" />
                {t('pro.benefitBatch')}
              </li>
              {status.provider === 'app_store' && (
                <li className="flex items-start gap-2 text-xs text-text-secondary dark:text-zinc-300">
                  <Check className="mt-0.5 h-3.5 w-3.5 shrink-0 text-success" />
                  {t('pro.benefitFamily')}
                </li>
              )}
            </ul>
          </div>

          <p className="text-[11px] leading-relaxed text-text-tertiary dark:text-zinc-500">
            {t('pro.freeHint')}
          </p>

          {isGrandfathered && (
            <p className="rounded-xl bg-success/10 px-3 py-2 text-xs text-success">
              {t('pro.grandfathered')}
            </p>
          )}
          {error && <p className="text-xs text-danger" data-testid="pro-error">{error}</p>}
        </div>

        <GlassDialogFooter>
          <GlassButton variant="ghost" size="sm" onClick={close} data-testid="pro-not-now">
            {t('pro.notNow')}
          </GlassButton>
          {!isGrandfathered && (
            <>
              <GlassButton
                variant="outline"
                size="sm"
                onClick={() => void restore()}
                disabled={busy !== 'idle'}
                data-testid="pro-restore"
              >
                {busy === 'restore' && <Loader2 className="h-4 w-4 animate-spin" />}
                {t('pro.restore')}
              </GlassButton>
              <GlassButton
                variant="primary"
                size="sm"
                onClick={() => void purchase()}
                disabled={busy !== 'idle' || status.status === 'pending'}
                data-testid="pro-purchase"
              >
                {busy === 'purchase' && <Loader2 className="h-4 w-4 animate-spin" />}
                {status.status === 'pending' ? t('pro.pending') : t('pro.upgrade')}
              </GlassButton>
            </>
          )}
        </GlassDialogFooter>
      </GlassDialogContent>
    </GlassDialog>
  );
}
