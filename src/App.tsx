import { useEffect } from 'react';
import { listen } from '@tauri-apps/api/event';
import { Header } from './components/Header';
import { Toolbar } from './components/Toolbar';
import { FileList } from './components/FileList';
import { BottomBar } from './components/BottomBar';
import { Settings } from './components/Settings';
import { useStore } from './store/useStore';

interface TranscribeProgressPayload {
  task_id: string;
  progress: number;
  status: string;
}

function App() {
  const updateFile = useStore((state) => state.updateFile);
  const setIsProcessing = useStore((state) => state.setIsProcessing);
  const incrementProcessedCount = useStore((state) => state.incrementProcessedCount);
  const files = useStore((state) => state.files);
  const settings = useStore((state) => state.settings);

  useEffect(() => {
    // Listen to transcription progress events
    const progressUnlisten = listen<TranscribeProgressPayload>('transcribe_progress', (event) => {
      const { task_id, progress, status } = event.payload;
      console.log('[transcribe_progress]', { task_id, progress, status });
      updateFile(task_id, {
        status: 'processing',
        progress: Math.round(progress),
      });
    });

    // Listen to transcription completion events
    const completeUnlisten = listen<TranscribeProgressPayload>('transcribe_complete', (event) => {
      const { task_id } = event.payload;
      console.log('[transcribe_complete]', { task_id });
      updateFile(task_id, {
        status: 'completed',
        progress: 100,
      });
      setIsProcessing(false);
      incrementProcessedCount();

      // Show notification if enabled
      if (settings.showCompletedNotification && 'Notification' in window) {
        const file = files.find(f => f.id === task_id);
        if (Notification.permission === 'granted') {
          new Notification('MonoSub', {
            body: `${file?.name || '视频'} 字幕生成完成`,
          });
        } else if (Notification.permission !== 'denied') {
          Notification.requestPermission();
        }
      }

      // Auto process next file if enabled
      if (settings.autoProcessQueue) {
        const nextPending = files.find(f => f.status === 'pending');
        if (nextPending) {
          console.log('[autoProcessQueue] Next file:', nextPending.name);
        }
      }
    });

    return () => {
      progressUnlisten.then((f) => f());
      completeUnlisten.then((f) => f());
    };
  }, [updateFile, setIsProcessing, incrementProcessedCount, files, settings]);

  return (
    <div className="min-h-screen bg-bg-secondary flex flex-col">
      <Header />
      <Toolbar />
      <FileList />
      <BottomBar />
      <Settings />
    </div>
  );
}

export default App;
