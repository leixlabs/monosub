import { useEffect } from 'react';
import { listen } from '@tauri-apps/api/event';
import { Header } from './components/Header';
import { Toolbar } from './components/Toolbar';
import { FileList } from './components/FileList';
import { BottomBar } from './components/BottomBar';
import { useStore } from './store/useStore';

interface TranscribeProgressPayload {
  task_id: string;
  progress: number;
  status: string;
}

function App() {
  const updateFile = useStore((state) => state.updateFile);
  const setIsProcessing = useStore((state) => state.setIsProcessing);

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
    });

    return () => {
      progressUnlisten.then((f) => f());
      completeUnlisten.then((f) => f());
    };
  }, [updateFile, setIsProcessing]);

  return (
    <div className="min-h-screen bg-bg-secondary flex flex-col">
      <Header />
      <Toolbar />
      <FileList />
      <BottomBar />
    </div>
  );
}

export default App;
