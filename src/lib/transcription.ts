import { invoke } from '@tauri-apps/api/core';
import { useStore } from '@/store/useStore';
import { ensureMultiSpeakerLicense } from '@/lib/license';
import { exists } from '@tauri-apps/plugin-fs';

let isQueueProcessing = false;

export const processQueueBatch = () => {
  const state = useStore.getState();
  if (!state.isBatchProcessing) return;

  const currentFiles = state.files;
  const pendingFiles = currentFiles.filter((f) => f.status === 'pending');
  const processingFiles = currentFiles.filter((f) => f.status === 'processing');

  const MAX_CONCURRENT_TASKS = 3;
  const availableSlots = Math.max(0, MAX_CONCURRENT_TASKS - processingFiles.length);

  if (availableSlots > 0 && pendingFiles.length > 0 && !isQueueProcessing) {
    isQueueProcessing = true;

    const tasksToStart = pendingFiles.slice(0, availableSlots);
    const promises = tasksToStart.map((f) => startTranscriptionTask(f.id));

    Promise.all(promises).finally(() => {
      isQueueProcessing = false;
      // We explicitly try triggering another batch check after current spins finish
      setTimeout(() => processQueueBatch(), 100);
    });
  }

  if (processingFiles.length === 0 && pendingFiles.length === 0) {
    state.setIsBatchProcessing(false);
    state.setIsProcessing(false);
  }
};

const getOutputPath = (inputPath: string, fileName: string) => {
  const state = useStore.getState();
  const extension = `.${state.settings.outputFormat}`;

  if (state.storageMode === 'same') {
    return inputPath.replace(/\.[^/.]+$/, extension);
  }

  return `${state.customStoragePath}/${fileName.replace(/\.[^/.]+$/, extension)}`;
};

export const startTranscriptionTask = async (taskId: string) => {
  const state = useStore.getState();
  const file = state.files.find((f) => f.id === taskId);

  if (!file || file.status === 'processing') {
    return false;
  }

  const outputPath = getOutputPath(file.path, file.name);

  if (state.settings.skipExistingSubtitle) {
    try {
      const fileExists = await exists(outputPath);
      if (fileExists) {
        state.updateFile(file.id, {
          status: 'skipped',
          progress: 100,
          error: undefined,
          stepProgress: undefined,
          totalTime: undefined
        });
        state.incrementProcessedCount();
        console.log(`[transcription] Skipping task ${taskId}, subtitle already exists: ${outputPath}`);

        // Trigger queue processor so batch loop continues for skipped files
        setTimeout(() => processQueueBatch(), 100);
        return true;
      }
    } catch (err) {
      console.warn('Failed to check if output file exists', err);
    }
  }

  if (state.enableSpeakerDiarization) {
    const licensed = await ensureMultiSpeakerLicense({
      openCheckoutOnFailure: false,
    });

    if (!licensed) {
      const message =
        useStore.getState().licenseMessage ||
        'Multi-speaker recognition requires a valid Polar license.';
      state.updateFile(file.id, {
        status: 'error',
        progress: 0,
        error: message,
      });
      return false;
    }
  }

  state.updateFile(file.id, { status: 'processing', progress: 0, error: undefined });
  state.setIsProcessing(true);
  state.setCurrentTaskId(file.id);

  try {
    await invoke('start_transcription', {
      request: {
        id: file.id,
        path: file.path,
        output_path: outputPath,
        format: state.settings.outputFormat,
        language: state.settings.outputLanguage,
        enable_diarization: state.enableSpeakerDiarization,
      },
    });
    return true;
  } catch (error) {
    state.updateFile(file.id, { status: 'error', progress: 0, error: String(error) });
    state.setIsProcessing(false);
    state.setCurrentTaskId(null);
    return false;
  }
};
