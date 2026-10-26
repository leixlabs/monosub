import { invoke } from '@tauri-apps/api/core';
import { ExternalLink, ShieldAlert } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { GlassBadge } from '@/components/ui/glass-badge';
import { GlassButton } from '@/components/ui/glass-button';
import {
  GlassDialog,
  GlassDialogContent,
  GlassDialogDescription,
  GlassDialogFooter,
  GlassDialogHeader,
  GlassDialogTitle,
} from '@/components/ui/glass-dialog';
import type { ModelCommercialUse, ModelStatus } from '@/lib/models';

export interface ModelLicensePromptProps {
  /**
   * The catalogue entry the user asked to download.
   *
   * `null` closes the dialog, which is why the caller passes its pending state
   * straight in: there is no separate `open` flag to get out of sync with which
   * model the confirmation is about.
   */
  model: ModelStatus | null;
  onConfirm: () => void;
  onCancel: () => void;
}

/** Badge colour per commercial-use verdict. `allowed` never reaches this dialog. */
const BADGE_VARIANT: Record<ModelCommercialUse, 'warning' | 'destructive'> = {
  allowed: 'warning',
  restricted: 'warning',
  noncommercial: 'destructive',
  unknown: 'warning',
};

/** Open a licence page in the user's browser, through the shell command. */
function openExternal(url: string) {
  void invoke('open_external_url', { url }).catch((error) => {
    console.error('[open_external_url] Failed:', error);
    try {
      window.open(url, '_blank');
    } catch (fallbackError) {
      console.error('[window.open] Failed:', fallbackError);
    }
  });
}

/**
 * The one thing standing between a click and a model download whose licence may
 * not permit what the user is about to do with it.
 *
 * Shown for every entry whose upstream terms are not plainly permissive
 * (`commercial_use !== 'allowed'`), which covers 51 of the 129 catalogue entries:
 * 27 conditional, 16 non-commercial, 8 with no published licence.
 *
 * What this dialog is and is not:
 *
 * * **It states the upstream terms, it does not interpret them.** The licence
 *   name comes from the catalogue, the link goes to the publisher's own text,
 *   and nothing here paraphrases a clause into a legal conclusion. The paragraph
 *   per verdict says what kind of restriction exists, not whether a particular
 *   use is fine.
 * * **It moves the decision to the user, explicitly.** MonoSub hosts no
 *   weights — every download goes to the publisher's URL — so nothing is being
 *   redistributed here. That is what makes "you check the terms" an honest
 *   division of labour rather than a disclaimer pasted over a risk the app
 *   created.
 * * **It records a click, not a consent.** `onConfirm` starts the download; it
 *   does not store a per-model acceptance anywhere. Re-prompting on the next
 *   download is deliberate: a licence can be read once and forgotten, and the
 *   second download is a second decision about a model that may be used
 *   differently.
 */
export function ModelLicensePrompt({ model, onConfirm, onCancel }: ModelLicensePromptProps) {
  const { t } = useTranslation();

  const licenseUrl = model?.licenses.find((entry) => entry.url)?.url ?? null;

  return (
    <GlassDialog open={model !== null} onOpenChange={(open) => !open && onCancel()}>
      <GlassDialogContent
        className="glass-medium dark:glass-dark-strong max-w-lg overflow-hidden rounded-3xl"
        closeClassName="text-text-secondary hover:text-text-primary hover:bg-black/5 dark:text-white/60 dark:hover:text-white dark:hover:bg-white/10"
        data-testid="model-license-dialog"
      >
        <GlassDialogHeader className="shrink-0">
          <GlassDialogTitle className="flex items-center gap-2 text-text-primary dark:text-white">
            <ShieldAlert className="w-4 h-4 text-warning" />
            {t('models.license.title')}
          </GlassDialogTitle>
          <GlassDialogDescription className="text-text-secondary dark:text-white/60">
            {t('models.license.subtitle')}
          </GlassDialogDescription>
        </GlassDialogHeader>

        {model && (
          <div className="mt-4 min-h-0 flex-1 space-y-3 overflow-y-auto pr-1">
            <div className="space-y-1">
              <p className="text-xxs font-medium uppercase tracking-wide text-text-tertiary dark:text-white/40">
                {t('models.license.modelLabel')}
              </p>
              <p
                className="select-text break-words text-sm font-medium text-text-primary dark:text-gray-100"
                data-testid="model-license-model"
              >
                {model.display_name || model.id}
              </p>
            </div>

            <div className="space-y-1">
              <p className="text-xxs font-medium uppercase tracking-wide text-text-tertiary dark:text-white/40">
                {t('models.license.licenseLabel')}
              </p>
              <div className="flex flex-wrap items-center gap-2">
                <p
                  className="select-text break-words text-sm text-text-primary dark:text-gray-100"
                  data-testid="model-license-name"
                >
                  {model.license}
                </p>
                <GlassBadge
                  variant={BADGE_VARIANT[model.commercial_use]}
                  size="sm"
                  data-testid="model-license-badge"
                >
                  {t(`models.commercialUse.${model.commercial_use}`)}
                </GlassBadge>
              </div>
              {licenseUrl && (
                <GlassButton
                  variant="ghost"
                  size="sm"
                  className="!px-1"
                  onClick={() => openExternal(licenseUrl)}
                  data-testid="model-license-link"
                >
                  <ExternalLink className="w-3.5 h-3.5" />
                  {t('models.license.open')}
                </GlassButton>
              )}
            </div>

            {/* One paragraph per verdict — what *kind* of restriction this is.
                Deliberately not a per-model verdict: the app does not know what
                the user is about to do with the transcript. */}
            <p
              className="rounded-xl border border-divider bg-bg-secondary/60 px-3 py-2 text-xs leading-relaxed text-text-secondary dark:border-white/10 dark:bg-white/5 dark:text-white/70"
              data-testid="model-license-terms"
            >
              {t(`models.license.terms.${model.commercial_use}`)}
            </p>

            <p className="text-xs leading-relaxed text-text-secondary dark:text-white/70">
              {t('models.license.disclaimer')}
            </p>
          </div>
        )}

        <GlassDialogFooter className="shrink-0">
          <GlassButton variant="ghost" size="sm" onClick={onCancel} data-testid="model-license-cancel">
            {t('models.cancel')}
          </GlassButton>
          <GlassButton
            variant="primary"
            size="sm"
            onClick={onConfirm}
            data-testid="model-license-confirm"
          >
            {t('models.license.confirm')}
          </GlassButton>
        </GlassDialogFooter>
      </GlassDialogContent>
    </GlassDialog>
  );
}
