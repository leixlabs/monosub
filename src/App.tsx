import { useEffect, useRef } from 'react';
import { listen } from '@tauri-apps/api/event';
import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow';
import { useTranslation } from 'react-i18next';
import { ThemeProvider } from './contexts/ThemeContext';
import { Header } from './components/Header';
import { Toolbar } from './components/Toolbar';
import { FileList } from './components/FileList';
import { BottomBar } from './components/BottomBar';
import { Settings } from './components/Settings';
import { useStore } from './store/useStore';
import { startTranscriptionTask } from './lib/transcription';
import { addPathsToQueue } from './lib/fileImport';
import { validateLicenseOnStartup } from './lib/license';

interface StepInfoPayload {
  current_step: number;
  total_steps: number;
  step_name: string;
  step_elapsed: number;
  total_elapsed: number;
}

interface TranscribeProgressPayload {
  task_id: string;
  progress: number;
  status: string;
  step?: StepInfoPayload;
  total_time?: number;
}

function App() {
  const { t } = useTranslation();
  const updateFile = useStore((state) => state.updateFile);
  const setIsProcessing = useStore((state) => state.setIsProcessing);
  const incrementProcessedCount = useStore((state) => state.incrementProcessedCount);
  const setCurrentTaskId = useStore((state) => state.setCurrentTaskId);
  const queueStartingRef = useRef(false);

  // Use refs to avoid stale closure issues
  const filesRef = useRef(useStore.getState().files);
  const settingsRef = useRef(useStore.getState().settings);

  // Subscribe to store changes
  useEffect(() => {
    const unsubscribe = useStore.subscribe((state) => {
      filesRef.current = state.files;
      settingsRef.current = state.settings;
    });
    return unsubscribe;
  }, []);

  useEffect(() => {
    void validateLicenseOnStartup();
  }, []);

  useEffect(() => {
    // Listen to transcription progress events
    const progressUnlisten = listen<TranscribeProgressPayload>('transcribe_progress', (event) => {
      const { task_id, step } = event.payload;
      console.log('[transcribe_progress]', { task_id, step });
      
      const updates: Partial<import('./store/useStore').VideoFile> = {
        status: 'processing',
      };
      
      if (step) {
        updates.stepProgress = {
          currentStep: step.current_step,
          totalSteps: step.total_steps,
          stepName: step.step_name,
          stepElapsed: step.step_elapsed,
          totalElapsed: step.total_elapsed,
        };
      }
      
      updateFile(task_id, updates);
    });

    // Listen to transcription completion events
    const completeUnlisten = listen<TranscribeProgressPayload>('transcribe_complete', (event) => {
      const { task_id, total_time } = event.payload;
      console.log('[transcribe_complete]', { task_id, total_time });
      updateFile(task_id, {
        status: 'completed',
        progress: 100,
        stepProgress: undefined,
        totalTime: total_time,
      });
      setIsProcessing(false);
      setCurrentTaskId(null);
      incrementProcessedCount();

      // Show notification if enabled (use ref to get latest settings)
      const settings = settingsRef.current;
      const files = filesRef.current;
      if (settings.showCompletedNotification && 'Notification' in window) {
        const file = files.find(f => f.id === task_id);
        if (Notification.permission === 'granted') {
          new Notification('MonoSub', {
            body: `${file?.name || 'Video'} - ${t('notification.completed')}`,
          });
        } else if (Notification.permission !== 'denied') {
          Notification.requestPermission();
        }
      }

      // Auto process next file if enabled
      if (settings.autoProcessQueue) {
        const nextPending = files.find((f) => f.status === 'pending');
        if (nextPending && !queueStartingRef.current) {
          queueStartingRef.current = true;
          void startTranscriptionTask(nextPending.id).finally(() => {
            queueStartingRef.current = false;
          });
        }
      }
    });

    // Listen to transcription error events
    const errorUnlisten = listen<TranscribeProgressPayload>('transcribe_error', (event) => {
      const { task_id, status } = event.payload;
      console.error('[transcribe_error]', { task_id, status });
      updateFile(task_id, {
        status: 'error',
        progress: 0,
        error: status,
      });
      setIsProcessing(false);
      setCurrentTaskId(null);

      // Show error notification if enabled
      const settings = settingsRef.current;
      const files = filesRef.current;
      if (settings.showCompletedNotification && 'Notification' in window) {
        const file = files.find(f => f.id === task_id);
        if (Notification.permission === 'granted') {
          new Notification('MonoSub - Error', {
            body: `${file?.name || 'Video'} - ${status}`,
          });
        }
      }

      if (settings.autoProcessQueue) {
        const nextPending = files.find((f) => f.status === 'pending');
        if (nextPending && !queueStartingRef.current) {
          queueStartingRef.current = true;
          void startTranscriptionTask(nextPending.id).finally(() => {
            queueStartingRef.current = false;
          });
        }
      }
    });

    return () => {
      progressUnlisten.then((f) => f());
      completeUnlisten.then((f) => f());
      errorUnlisten.then((f) => f());
    };
  }, [updateFile, setIsProcessing, setCurrentTaskId, incrementProcessedCount, t]);

  useEffect(() => {
    let unlisten: (() => void) | null = null;
    let destroyed = false;

    const setupDropListener = async () => {
      const remove = await getCurrentWebviewWindow().onDragDropEvent((event) => {
        if (event.payload.type !== 'drop') return;
        const droppedPaths = event.payload.paths;
        if (!droppedPaths || droppedPaths.length === 0) return;

        void addPathsToQueue({
          paths: droppedPaths,
          existingFiles: filesRef.current,
          addFiles: useStore.getState().addFiles,
          source: 'WindowDrop',
        }).then((result) => {
          if (result.skippedDuplicates > 0) {
            console.warn(`[WindowDrop] Skipped ${result.skippedDuplicates} duplicate file(s)`);
          }
          if (result.skippedUnsupported > 0) {
            console.warn(`[WindowDrop] Skipped ${result.skippedUnsupported} unsupported file(s)`);
          }
        });
      });

      if (destroyed) {
        remove();
      } else {
        unlisten = remove;
      }
    };

    void setupDropListener();

    return () => {
      destroyed = true;
      if (unlisten) {
        unlisten();
      }
    };
  }, []);

  return (
    <ThemeProvider>
      <div className="min-h-screen bg-bg-secondary dark:bg-zinc-950 flex flex-col transition-colors">
        <Header />
        <Toolbar />
        <FileList />
        <BottomBar />
        <Settings />
      </div>
    </ThemeProvider>
  );
}

export default App;
