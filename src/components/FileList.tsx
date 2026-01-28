import { useStore } from '@/store/useStore';
import { FileItem } from './FileItem';
import { FileVideo } from 'lucide-react';

export function FileList() {
  const files = useStore((state) => state.files);

  if (files.length === 0) {
    return (
      <div className="flex-1 flex items-center justify-center">
        <div className="text-center">
          <FileVideo className="w-16 h-16 text-text-tertiary mx-auto mb-4" />
          <p className="text-text-secondary mb-2">暂无文件</p>
          <p className="text-sm text-text-tertiary">点击"添加文件"开始处理视频</p>
        </div>
      </div>
    );
  }

  return (
    <div className="flex-1 overflow-y-auto p-6 space-y-4">
      {files.map((file) => (
        <FileItem key={file.id} file={file} />
      ))}
    </div>
  );
}
