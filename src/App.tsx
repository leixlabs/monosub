import { useEffect, useRef } from 'react';
import { isPermissionGranted, requestPermission, sendNotification, onAction } from '@tauri-apps/plugin-notification';
import { listen } from '@tauri-apps/api/event';
import { invoke } from '@tauri-apps/api/core';
import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow';
import { resolveResource } from '@tauri-apps/api/path';
import { useTranslation } from 'react-i18next';
import i18n from './i18n';
import { ThemeProvider } from './contexts/ThemeContext';
import { Agentation } from 'agentation';
import { Header } from './components/Header';
import { Toolbar } from './components/Toolbar';
import { FileList } from './components/FileList';
import { BottomBar } from './components/BottomBar';
import { Settings } from './components/Settings';
import { useStore } from './store/useStore';
import { processQueueBatch, isModelNotInstalledError } from './lib/transcription';
import { openModelsSettings } from './lib/models';
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

let _notificationIconPath: string | undefined;
const getNotificationIconPath = async (): Promise<string | undefined> => {
  if (_notificationIconPath !== undefined) return _notificationIconPath || undefined;
  try {
    _notificationIconPath = await resolveResource('icon.png');
  } catch {
    _notificationIconPath = '';
  }
  return _notificationIconPath || undefined;
};

function App() {
  const { t } = useTranslation();
  const updateFile = useStore((state) => state.updateFile);
  const setIsProcessing = useStore((state) => state.setIsProcessing);
  const incrementProcessedCount = useStore((state) => state.incrementProcessedCount);
  const setCurrentTaskId = useStore((state) => state.setCurrentTaskId);

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

  // Bring window to front when user clicks a system notification
  useEffect(() => {
    let listener: { unregister: () => Promise<void> } | null = null;
    onAction(() => {
      const win = getCurrentWebviewWindow();
      void win.show();
      void win.setFocus();
      void win.unminimize();
    }).then((fn) => {
      listener = fn;
    });
    return () => {
      void listener?.unregister();
    };
  }, []);

  // Sync tray menu labels with current language
  useEffect(() => {
    const syncTray = () => {
      void invoke('update_tray_menu', {
        show: t('tray.show'),
        hide: t('tray.hide'),
        quit: t('tray.quit'),
      });
    };
    syncTray();
    i18n.on('languageChanged', syncTray);
    return () => {
      i18n.off('languageChanged', syncTray);
    };
  }, [t]);

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
      if (settings.showCompletedNotification) {
        const file = files.find(f => f.id === task_id);
        void (async () => {
          let granted = await isPermissionGranted();
          if (!granted) {
            const permission = await requestPermission();
            granted = permission === 'granted';
          }
          if (granted) {
            const icon = await getNotificationIconPath();
            sendNotification({
              title: 'MonoSub',
              body: `${file?.name || 'Video'} - ${t('notification.completed')}`,
              ...(icon ? { icon } : {}),
            });
          }
        })();
      }

      if (useStore.getState().isBatchProcessing) {
        processQueueBatch();
      }
    });

    // Listen to transcription error events
    const errorUnlisten = listen<TranscribeProgressPayload>('transcribe_error', (event) => {
      const { task_id, status } = event.payload;
      console.error('[transcribe_error]', { task_id, status });
      const modelMissing = isModelNotInstalledError(status);
      // `status` is either the MODEL_NOT_INSTALLED sentinel, one of the
      // `errors.*` keys the Rust layer emits for its known failure modes, or a
      // raw message (e.g. from the subtitle writer). Resolve a key when we have
      // one and otherwise show the message as-is.
      const errorMessage =
        modelMissing || status === 'modelNotInstalled'
          ? t('errors.modelNotInstalled')
          : t(`errors.${status}`, { defaultValue: status });
      // A model that is missing (sentinel) or not in the catalogue (engine key)
      // both need the user to visit the download list.
      const needsModelDownload = modelMissing || status === 'modelNotInstalled';
      updateFile(task_id, {
        status: 'error',
        progress: 0,
        error: errorMessage,
      });
      if (needsModelDownload) {
        openModelsSettings();
      }
      setIsProcessing(false);
      setCurrentTaskId(null);

      // Show error notification if enabled
      const settings = settingsRef.current;
      const files = filesRef.current;
      if (settings.showCompletedNotification) {
        const file = files.find(f => f.id === task_id);
        void (async () => {
          const granted = await isPermissionGranted();
          if (granted) {
            const icon = await getNotificationIconPath();
            sendNotification({
              title: 'MonoSub',
              body: `${file?.name || 'Video'} - ${errorMessage}`,
              ...(icon ? { icon } : {}),
            });
          }
        })();
      }

      if (useStore.getState().isBatchProcessing) {
        processQueueBatch();
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
    <>
      <ThemeProvider>
        <div className="h-screen overflow-hidden bg-bg-secondary dark:bg-zinc-950 flex flex-col transition-colors">
          <Header />
          <Toolbar />
          <FileList />
          <BottomBar />
          <Settings />
        </div>
      </ThemeProvider>
      {process.env.NODE_ENV === "development" && <Agentation />}
    </>
  );
}

export default App;
