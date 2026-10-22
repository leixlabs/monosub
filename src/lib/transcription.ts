import { invoke } from '@tauri-apps/api/core';
import { useStore } from '@/store/useStore';
import { exists } from '@tauri-apps/plugin-fs';
import i18n from '@/i18n';
import { reportFrontendError } from '@/lib/logging';
import {
  MODEL_NOT_INSTALLED_ERROR,
  modelNotInstalledId,
  listModels,
  isTranslationModel,
} from '@/lib/models';

let isQueueProcessing = false;

/**
 * `start_transcription` rejects with this sentinel when the default model is
 * not installed (spec §5.2/§5.3). The Rust side embeds it in the error string.
 */
export const isModelNotInstalledError = (error: unknown): boolean =>
  String(error).includes(MODEL_NOT_INSTALLED_ERROR);

/**
 * Whether the missing model named in a `MODEL_NOT_INSTALLED:<id>` rejection is a
 * translation model.
 *
 * The recognition pre-flight runs first, so the id alone does not say whether
 * the user is missing an ASR model or the translation model they turned on
 * (translation-design §6). The catalogue is the only source of the role, so it
 * is re-read here — on the error path only, where one round trip is cheap next
 * to a wasted transcription. A lookup failure falls back to the recognition
 * wording rather than hiding the error.
 */
const isMissingTranslationModel = async (error: unknown): Promise<boolean> => {
  const missingId = modelNotInstalledId(error);
  if (!missingId) return false;
  try {
    const models = await listModels();
    const missing = models.find((model) => model.id === missingId);
    return missing ? isTranslationModel(missing) : false;
  } catch (lookupError) {
    console.warn('[transcription] could not resolve the missing model role', lookupError);
    return false;
  }
};

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

const getOutputPath = (inputPath: string, fileName: string, outputDir?: string) => {
  const state = useStore.getState();
  // For multi-format (e.g. "srt,txt"), use the first format as the primary extension
  const primaryFormat = state.settings.outputFormat.split(',')[0].trim();
  const extension = `.${primaryFormat}`;
  const outputName = fileName.replace(/\.[^/.]+$/, extension);

  // URL imports: "same as the source" would put the subtitle in the app's cache
  // directory, which the user is expected to clear. The row therefore carries its
  // own destination, resolved once when the import finished
  // (url-import-design §5.4).
  if (outputDir) {
    return `${outputDir}/${outputName}`;
  }

  if (state.storageMode === 'same') {
    return inputPath.replace(/\.[^/.]+$/, extension);
  }

  return `${state.customStoragePath}/${outputName}`;
};

export const startTranscriptionTask = async (taskId: string) => {
  const state = useStore.getState();
  const file = state.files.find((f) => f.id === taskId);

  if (!file || file.status === 'processing') {
    return false;
  }

  const outputPath = getOutputPath(file.path, file.name, file.outputDir);

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

  // Multi-speaker (diarization) is offline in v2.0 (spec §5.7) and its BottomBar
  // control is disabled, so the licence pre-flight is intentionally not routed
  // here any more. The flag still travels with the request; Rust owns the check.

  state.updateFile(file.id, {
    status: 'processing',
    progress: 0,
    error: undefined,
    // A new attempt starts from a clean slate: a stale `missingModelId` left by an
    // earlier missing-model failure would otherwise make an unrelated failure look
    // like another missing model.
    missingModelId: undefined,
    missingModel: undefined,
    downloadProgress: undefined,
  });
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
        // Per-file speaker separation: a single-narrator file can skip the
        // diar/passthrough model entirely (much faster).
        enable_diarization: file.diarization,
        // Model-plan requirements (design §9.1); all optional server-side.
        require_word_timestamps: state.settings.requireWordTimestamps,
        language_profile: state.settings.languageProfile,
        memory_budget: state.settings.memoryBudget,
        model_overrides: state.modelOverrides,
        // Translation (translation-design §7). All optional server-side: an
        // empty `translation_model` means "use the catalogue default", and a
        // disabled run translates nothing.
        translate_enabled: state.settings.translateEnabled,
        target_language: state.settings.translateTargetLanguage,
        translation_model: state.settings.translationModelId,
      },
    });
    return true;
  } catch (error) {
    if (isModelNotInstalledError(error)) {
      // The row keeps its error text AND the id of the model that is missing, so
      // the error dialog can offer the download itself. This path deliberately
      // does not open Settings any more: jumping the user to the model list made
      // them lose the file (and the queue) they were working on.
      const isTranslation = await isMissingTranslationModel(error);
      reportFrontendError(error, {
        level: 'warn',
        context: `start_transcription ${file.id} MODEL_NOT_INSTALLED`,
      });
      state.updateFile(file.id, {
        status: 'error',
        progress: 0,
        error: isTranslation
          ? i18n.t('errors.translationModelNotInstalled')
          : i18n.t('errors.modelNotInstalled'),
        // `null` (the sentinel carried no id) becomes `undefined`, which the row
        // reads as "cannot tell which model" — the prompt then only offers the
        // model list instead of guessing.
        missingModelId: modelNotInstalledId(error) ?? undefined,
        missingModel: true,
        // A previous attempt's transfer belongs to an earlier failure.
        downloadProgress: undefined,
      });
    } else {
      // The file row shows the message; the log keeps it after the row is gone.
      reportFrontendError(error, { context: `start_transcription ${file.id}` });
      state.updateFile(file.id, {
        status: 'error',
        progress: 0,
        error: String(error),
        // Any other failure must not inherit the previous attempt's model prompt.
        missingModelId: undefined,
        missingModel: undefined,
        downloadProgress: undefined,
      });
    }
    state.setIsProcessing(false);
    state.setCurrentTaskId(null);
    return false;
  }
};
