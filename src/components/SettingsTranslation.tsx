import { Languages, Download, Trash2, X } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { motion } from 'framer-motion';

import { useStore, TRANSLATE_TARGET_LANGUAGES, type TranslateTargetLanguage } from '@/store/useStore';
import { CustomSelect, type SelectOption } from '@/components/CustomSelect';
import { GlassButton } from '@/components/ui/glass-button';
import { GlassBadge } from '@/components/ui/glass-badge';
import { GlassSwitch } from '@/components/ui/glass-switch';
import {
  formatModelSize,
  modelManifestSize,
  translationModels,
  type ModelStatus,
} from '@/lib/models';

/**
 * Settings ▸ Translation (translation-design §7).
 *
 * Translation is an enhancement: when a run does not ask for it, nothing here
 * reaches the engine and the run behaves exactly as before. When it is asked
 * for, a missing translation model is reported *before* recognition starts, so
 * this pane also has to make the model downloadable.
 *
 * The three controls here are not the same kind of setting:
 *
 * * the switch is the **default** for a newly queued file — whether a given run
 *   translates is asked in the task area (queue toolbar + each row), where the
 *   user can still change it;
 * * the target language and the model are **configure-once**: they are
 *   properties of the setup, so they are set here and never asked again.
 *
 * The download state machine lives in `Settings` (one set of listeners, one
 * `downloadingId`), so this component receives the models and the actions as
 * props instead of opening its own event subscriptions.
 */
interface SettingsTranslationProps {
  models: ModelStatus[];
  downloadingId: string | null;
  downloadPercentById: Record<string, number>;
  onDownload: (id: string) => void;
  onCancelDownload: (id: string) => void;
  onRemove: (id: string) => void;
}

export function SettingsTranslation({
  models,
  downloadingId,
  downloadPercentById,
  onDownload,
  onCancelDownload,
  onRemove,
}: SettingsTranslationProps) {
  const { t } = useTranslation();
  const settings = useStore((state) => state.settings);
  const updateSettings = useStore((state) => state.updateSettings);

  const available = translationModels(models);

  // Endonyms, so the list is readable whatever the UI language is. Ordered like
  // `TRANSLATE_TARGET_LANGUAGES`, and exhaustive by type: adding a target without
  // a label fails `tsc` rather than rendering a bare tag.
  const languageLabels: Record<TranslateTargetLanguage, string> = {
    en: 'English',
    zh: '简体中文',
    'zh-tw': '繁體中文',
    ja: '日本語',
    ko: '한국어',
    es: 'Español',
    fr: 'Français',
    de: 'Deutsch',
    pt: 'Português',
    ar: 'العربية',
    bn: 'বাংলা',
    bo: 'བོད་ཡིག',
    cs: 'Čeština',
    fa: 'فارسی',
    gu: 'ગુજરાતી',
    he: 'עברית',
    hi: 'हिन्दी',
    id: 'Bahasa Indonesia',
    it: 'Italiano',
    kk: 'Қазақша',
    km: 'ខ្មែរ',
    mn: 'Монгол',
    mr: 'मराठी',
    ms: 'Bahasa Melayu',
    my: 'မြန်မာ',
    nl: 'Nederlands',
    pl: 'Polski',
    ru: 'Русский',
    ta: 'தமிழ்',
    te: 'తెలుగు',
    th: 'ไทย',
    tl: 'Filipino',
    tr: 'Türkçe',
    ug: 'ئۇيغۇرچە',
    uk: 'Українська',
    ur: 'اردو',
    vi: 'Tiếng Việt',
    yue: '粵語',
  };

  const targetOptions: SelectOption<TranslateTargetLanguage>[] = TRANSLATE_TARGET_LANGUAGES.map(
    (code) => ({ value: code, label: languageLabels[code] }),
  );

  // `''` means "let the backend pick the catalogue default"; the label says so
  // rather than naming a model this component cannot know is the default one.
  const modelOptions: SelectOption<string>[] = [
    { value: '', label: t('settings.translate.modelDefault') },
    ...available.map((model) => ({
      value: model.id,
      label: model.installed
        ? model.display_name
        : `${model.display_name} · ${t('models.notInstalled')}`,
    })),
  ];

  return (
    <section className="space-y-4">
      <div className="p-4 rounded-2xl border border-divider dark:border-zinc-700 bg-bg-secondary/70 dark:bg-zinc-900/50 space-y-4">
        <div className="flex items-center justify-between gap-4">
          <div className="min-w-0">
            <div className="flex items-center gap-2">
              <Languages className="w-4 h-4 text-text-primary dark:text-gray-300" />
              <label className="text-sm font-semibold text-text-primary dark:text-gray-200">
                {t('settings.translate.enable')}
              </label>
            </div>
            <p className="text-xs text-text-secondary dark:text-gray-400 mt-0.5">
              {t('settings.translate.enableDesc')}
            </p>
          </div>
          <GlassSwitch
            checked={settings.translateEnabled}
            onCheckedChange={(checked) => updateSettings({ translateEnabled: checked })}
            aria-label={t('settings.translate.enable')}
            data-testid="translation-enable"
          />
        </div>

        <div className="flex items-center justify-between gap-4">
          <label className="text-sm font-semibold text-text-primary dark:text-gray-200">
            {t('settings.translate.targetLanguage')}
          </label>
          <CustomSelect
            value={settings.translateTargetLanguage}
            onChange={(value) => updateSettings({ translateTargetLanguage: value })}
            options={targetOptions}
            className="w-[220px]"
            // Not gated on the default switch: the target is configure-once, and
            // a user who wants to pick it before turning anything on must be
            // able to. The switch above only decides what a *new file* starts
            // with (see `AppSettings.translateEnabled`).
          />
        </div>

        <div className="flex items-center justify-between gap-4">
          <label className="text-sm font-semibold text-text-primary dark:text-gray-200">
            {t('settings.translate.model')}
          </label>
          <CustomSelect
            value={settings.translationModelId}
            onChange={(value) => updateSettings({ translationModelId: value })}
            options={modelOptions}
            className="w-[220px]"
            // Same reason as the target picker: a configure-once choice must not
            // be blocked by the default-on switch.
            searchable
            testId="translation-model"
          />
        </div>

        <p className="text-xs text-text-secondary dark:text-gray-400 leading-relaxed">
          {t('settings.translate.modelHint')}
        </p>
      </div>

      {/* The translation models, with the same install / download / remove
          actions as the Models pane. Kept here so a user who just turned
          translation on can install the model without leaving the pane. */}
      <div className="p-4 rounded-2xl border border-divider dark:border-zinc-700 bg-bg-secondary/70 dark:bg-zinc-900/50 space-y-3">
        <h3 className="text-xs font-semibold text-text-secondary dark:text-gray-400 uppercase tracking-wide">
          {t('models.translationTitle')}
        </h3>

        {available.length === 0 && (
          <p className="text-xs text-text-secondary dark:text-gray-400">
            {t('settings.translate.noModels')}
          </p>
        )}

        {available.map((model) => {
          const isDownloading = downloadingId === model.id;
          const percent = downloadPercentById[model.id] ?? 0;
          const displaySize = model.installed && model.size_on_disk > 0
            ? model.size_on_disk
            : modelManifestSize(model);

          return (
            <div
              key={model.id}
              className="p-3 rounded-2xl border border-divider dark:border-zinc-700 bg-bg-secondary dark:bg-zinc-800/50 space-y-3"
            >
              <div className="flex items-start justify-between gap-3">
                <div className="min-w-0">
                  <div className="flex items-center gap-2 flex-wrap">
                    <p className="text-sm font-semibold text-text-primary dark:text-white truncate">
                      {model.display_name || model.id}
                    </p>
                    <GlassBadge variant={model.installed ? 'success' : 'warning'} size="sm">
                      {model.installed ? t('models.installed') : t('models.notInstalled')}
                    </GlassBadge>
                  </div>
                  <p className="text-xs text-text-secondary dark:text-gray-400 mt-1">
                    {t('models.size')}: {formatModelSize(displaySize)}
                  </p>
                </div>

                <div className="flex items-center gap-2 shrink-0">
                  {!model.installed && !isDownloading && (
                    <GlassButton
                      variant="primary"
                      size="sm"
                      onClick={() => onDownload(model.id)}
                    >
                      <Download className="w-4 h-4" />
                      {t('models.download')}
                    </GlassButton>
                  )}
                  {isDownloading && (
                    <GlassButton
                      variant="outline"
                      size="sm"
                      onClick={() => onCancelDownload(model.id)}
                    >
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
        })}
      </div>
    </section>
  );
}
