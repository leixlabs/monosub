import { invoke } from '@tauri-apps/api/core';
import { useStore } from '@/store/useStore';
import { ensureMultiSpeakerLicense } from '@/lib/license';

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

  if (!file || state.isProcessing || file.status === 'processing') {
    return false;
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

  const outputPath = getOutputPath(file.path, file.name);

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
