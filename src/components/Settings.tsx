import { X, RotateCcw } from 'lucide-react';
import { useStore, SubtitleFormat, OutputLanguage } from '@/store/useStore';

export function Settings() {
  const isOpen = useStore((state) => state.isSettingsOpen);
  const setIsOpen = useStore((state) => state.setIsSettingsOpen);
  const settings = useStore((state) => state.settings);
  const updateSettings = useStore((state) => state.updateSettings);
  const resetSettings = useStore((state) => state.resetSettings);

  if (!isOpen) return null;

  const formatOptions: { value: SubtitleFormat; label: string }[] = [
    { value: 'srt', label: 'SRT' },
    { value: 'vtt', label: 'VTT' },
    { value: 'txt', label: 'TXT' },
  ];

  const languageOptions: { value: OutputLanguage; label: string }[] = [
    { value: 'auto', label: '自动检测' },
    { value: 'zh', label: '中文' },
    { value: 'en', label: 'English' },
  ];

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center">
      {/* Backdrop */}
      <div
        className="absolute inset-0 bg-black/30 backdrop-blur-sm animate-fade-in"
        onClick={() => setIsOpen(false)}
      />

      {/* Panel */}
      <div className="relative bg-bg-card rounded-xl shadow-apple-lg w-[480px] max-h-[80vh] overflow-hidden animate-slide-up">
        {/* Header */}
        <div className="flex items-center justify-between px-6 py-4 border-b border-divider">
          <h2 className="text-lg font-semibold text-text-primary">设置</h2>
          <button
            onClick={() => setIsOpen(false)}
            className="p-1.5 rounded-md hover:bg-bg-secondary transition-colors"
          >
            <X className="w-5 h-5 text-text-secondary" />
          </button>
        </div>

        {/* Content */}
        <div className="p-6 space-y-6 overflow-y-auto max-h-[calc(80vh-120px)]">
          {/* Output Settings */}
          <section>
            <h3 className="text-sm font-medium text-text-primary mb-4">输出设置</h3>
            <div className="space-y-4">
              {/* Format */}
              <div className="flex items-center justify-between">
                <label className="text-sm text-text-secondary">字幕格式</label>
                <div className="flex gap-1 bg-bg-secondary rounded-lg p-1">
                  {formatOptions.map((opt) => (
                    <button
                      key={opt.value}
                      onClick={() => updateSettings({ outputFormat: opt.value })}
                      className={`px-4 py-1.5 text-sm rounded-md transition-all ${
                        settings.outputFormat === opt.value
                          ? 'bg-white text-text-primary shadow-apple'
                          : 'text-text-secondary hover:text-text-primary'
                      }`}
                    >
                      {opt.label}
                    </button>
                  ))}
                </div>
              </div>

              {/* Language */}
              <div className="flex items-center justify-between">
                <label className="text-sm text-text-secondary">识别语言</label>
                <select
                  value={settings.outputLanguage}
                  onChange={(e) => updateSettings({ outputLanguage: e.target.value as OutputLanguage })}
                  className="px-3 py-1.5 text-sm bg-bg-secondary border border-border rounded-lg text-text-primary focus:outline-none focus:ring-2 focus:ring-primary/20"
                >
                  {languageOptions.map((opt) => (
                    <option key={opt.value} value={opt.value}>
                      {opt.label}
                    </option>
                  ))}
                </select>
              </div>

              {/* Bilingual */}
              <div className="flex items-center justify-between">
                <div>
                  <label className="text-sm text-text-secondary">双语字幕</label>
                  <p className="text-xs text-text-tertiary mt-0.5">同时生成翻译字幕</p>
                </div>
                <label className="relative inline-flex items-center cursor-pointer">
                  <input
                    type="checkbox"
                    checked={settings.enableBilingual}
                    onChange={(e) => updateSettings({ enableBilingual: e.target.checked })}
                    className="sr-only peer"
                  />
                  <div className="w-11 h-6 bg-bg-secondary rounded-full peer peer-checked:bg-primary transition-colors after:content-[''] after:absolute after:top-0.5 after:left-[2px] after:bg-white after:rounded-full after:h-5 after:w-5 after:transition-all peer-checked:after:translate-x-full after:shadow-sm" />
                </label>
              </div>

              {settings.enableBilingual && (
                <div className="flex items-center justify-between pl-4 border-l-2 border-primary/20">
                  <label className="text-sm text-text-secondary">翻译目标语言</label>
                  <select
                    value={settings.bilingualTargetLang}
                    onChange={(e) => updateSettings({ bilingualTargetLang: e.target.value as 'en' | 'zh' })}
                    className="px-3 py-1.5 text-sm bg-bg-secondary border border-border rounded-lg text-text-primary focus:outline-none focus:ring-2 focus:ring-primary/20"
                  >
                    <option value="en">English</option>
                    <option value="zh">中文</option>
                  </select>
                </div>
              )}
            </div>
          </section>

          {/* Processing Settings */}
          <section>
            <h3 className="text-sm font-medium text-text-primary mb-4">处理设置</h3>
            <div className="space-y-4">
              {/* Noise Reduction */}
              <div className="flex items-center justify-between">
                <div>
                  <label className="text-sm text-text-secondary">音频降噪</label>
                  <p className="text-xs text-text-tertiary mt-0.5">减少背景噪音干扰</p>
                </div>
                <label className="relative inline-flex items-center cursor-pointer">
                  <input
                    type="checkbox"
                    checked={settings.enableNoiseReduction}
                    onChange={(e) => updateSettings({ enableNoiseReduction: e.target.checked })}
                    className="sr-only peer"
                  />
                  <div className="w-11 h-6 bg-bg-secondary rounded-full peer peer-checked:bg-primary transition-colors after:content-[''] after:absolute after:top-0.5 after:left-[2px] after:bg-white after:rounded-full after:h-5 after:w-5 after:transition-all peer-checked:after:translate-x-full after:shadow-sm" />
                </label>
              </div>

              {/* Volume Normalization */}
              <div className="flex items-center justify-between">
                <div>
                  <label className="text-sm text-text-secondary">音量标准化</label>
                  <p className="text-xs text-text-tertiary mt-0.5">统一音量水平</p>
                </div>
                <label className="relative inline-flex items-center cursor-pointer">
                  <input
                    type="checkbox"
                    checked={settings.enableVolumeNormalization}
                    onChange={(e) => updateSettings({ enableVolumeNormalization: e.target.checked })}
                    className="sr-only peer"
                  />
                  <div className="w-11 h-6 bg-bg-secondary rounded-full peer peer-checked:bg-primary transition-colors after:content-[''] after:absolute after:top-0.5 after:left-[2px] after:bg-white after:rounded-full after:h-5 after:w-5 after:transition-all peer-checked:after:translate-x-full after:shadow-sm" />
                </label>
              </div>

              {/* Auto Process Queue */}
              <div className="flex items-center justify-between">
                <div>
                  <label className="text-sm text-text-secondary">自动处理队列</label>
                  <p className="text-xs text-text-tertiary mt-0.5">完成后自动处理下一个</p>
                </div>
                <label className="relative inline-flex items-center cursor-pointer">
                  <input
                    type="checkbox"
                    checked={settings.autoProcessQueue}
                    onChange={(e) => updateSettings({ autoProcessQueue: e.target.checked })}
                    className="sr-only peer"
                  />
                  <div className="w-11 h-6 bg-bg-secondary rounded-full peer peer-checked:bg-primary transition-colors after:content-[''] after:absolute after:top-0.5 after:left-[2px] after:bg-white after:rounded-full after:h-5 after:w-5 after:transition-all peer-checked:after:translate-x-full after:shadow-sm" />
                </label>
              </div>
            </div>
          </section>

          {/* Notification Settings */}
          <section>
            <h3 className="text-sm font-medium text-text-primary mb-4">通知设置</h3>
            <div className="space-y-4">
              <div className="flex items-center justify-between">
                <div>
                  <label className="text-sm text-text-secondary">完成通知</label>
                  <p className="text-xs text-text-tertiary mt-0.5">转换完成时显示系统通知</p>
                </div>
                <label className="relative inline-flex items-center cursor-pointer">
                  <input
                    type="checkbox"
                    checked={settings.showCompletedNotification}
                    onChange={(e) => updateSettings({ showCompletedNotification: e.target.checked })}
                    className="sr-only peer"
                  />
                  <div className="w-11 h-6 bg-bg-secondary rounded-full peer peer-checked:bg-primary transition-colors after:content-[''] after:absolute after:top-0.5 after:left-[2px] after:bg-white after:rounded-full after:h-5 after:w-5 after:transition-all peer-checked:after:translate-x-full after:shadow-sm" />
                </label>
              </div>
            </div>
          </section>
        </div>

        {/* Footer */}
        <div className="flex items-center justify-between px-6 py-4 border-t border-divider bg-bg-tertiary">
          <button
            onClick={resetSettings}
            className="flex items-center gap-2 px-4 py-2 text-sm text-text-secondary hover:text-text-primary transition-colors"
          >
            <RotateCcw className="w-4 h-4" />
            重置默认
          </button>
          <button
            onClick={() => setIsOpen(false)}
            className="px-5 py-2 text-sm font-medium text-white bg-primary rounded-lg hover:bg-primary-hover transition-colors"
          >
            完成
          </button>
        </div>
      </div>
    </div>
  );
}
