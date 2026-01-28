import { Folder, FolderOpen, Square } from 'lucide-react';
import { useStore } from '@/store/useStore';
import { open } from '@tauri-apps/plugin-dialog';

export function BottomBar() {
  const storageMode = useStore((state) => state.storageMode);
  const setStorageMode = useStore((state) => state.setStorageMode);
  const customStoragePath = useStore((state) => state.customStoragePath);
  const setCustomStoragePath = useStore((state) => state.setCustomStoragePath);
  const isProcessing = useStore((state) => state.isProcessing);

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

  return (
    <div className="bg-bg-card border-t border-border h-16 px-6 flex items-center justify-between">
      <div className="flex items-center gap-6">
        <span className="text-sm text-text-secondary">存储到：</span>

        <label className="flex items-center gap-2 cursor-pointer">
          <input
            type="radio"
            name="storage"
            checked={storageMode === 'same'}
            onChange={() => setStorageMode('same')}
            className="w-4 h-4 text-primary"
          />
          <Folder className="w-4 h-4 text-text-secondary" />
          <span className="text-sm text-text-primary">同目录</span>
        </label>

        <label className="flex items-center gap-2 cursor-pointer">
          <input
            type="radio"
            name="storage"
            checked={storageMode === 'custom'}
            onChange={handleSelectCustomPath}
            className="w-4 h-4 text-primary"
          />
          <FolderOpen className="w-4 h-4 text-text-secondary" />
          <span className="text-sm text-text-primary">
            {storageMode === 'custom' && customStoragePath
              ? customStoragePath.split('/').pop() || '指定目录'
              : '指定目录'}
          </span>
        </label>
      </div>

      <div className="flex items-center gap-3">
        {isProcessing && (
          <button className="flex items-center gap-2 px-5 py-2.5 bg-primary text-white rounded-md hover:bg-primary-hover transition-colors">
            <Square className="w-4 h-4" />
            <span className="text-sm font-medium">暂停</span>
          </button>
        )}
      </div>
    </div>
  );
}
