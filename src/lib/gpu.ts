import { invoke } from '@tauri-apps/api/core';

/**
 * Can this machine run the bundled CUDA engine? (Windows only.)
 *
 * The Windows package ships the CUDA build of audio.cpp and nothing else, and it
 * bundles the CUDA *runtime* — so the only thing a user has to supply is the
 * driver. That is exactly what this answers, from `commands::gpu::cuda_status`,
 * which probes `nvidia-smi` rather than guessing from the OS version.
 */
export interface CudaStatus {
  /** `false` on macOS/Linux, where CUDA is not what these builds use. */
  applicable: boolean;
  available: boolean;
  reason: CudaReason;
  gpuName: string | null;
  driverVersion: string | null;
  computeCapability: string | null;
  minimumComputeCapability: number;
  /** Raw probe failure text, for `reason === 'probe_failed'`. */
  detail: string | null;
  /** NVIDIA's driver download page — what "install CUDA" actually means here. */
  driverUrl: string;
}

/**
 * Mirrors the `reason` strings in `commands/gpu.rs`.
 *
 * The Rust side names the situation; the sentence is picked here so it can be
 * translated. Keeping the wire values stable means a new reason shows up as an
 * empty line rather than as an English sentence in a Chinese UI.
 */
export type CudaReason =
  | 'ok'
  | 'not_windows'
  | 'no_driver'
  | 'no_gpu'
  | 'compute_capability_too_low'
  | 'probe_failed';

let cached: Promise<CudaStatus> | null = null;

/**
 * Probe once per session.
 *
 * The empty state remounts every time the queue empties, and spawning
 * `nvidia-smi` on each mount would make the whole panel flicker for a fact that
 * cannot change without a driver install and a restart. `refresh` is the
 * explicit "check again" button's path, and a failed call clears the cache so
 * the next mount retries instead of replaying the rejection forever.
 */
export const getCudaStatus = (refresh = false): Promise<CudaStatus> => {
  if (refresh || cached === null) {
    cached = invoke<CudaStatus>('cuda_status').catch((error: unknown) => {
      cached = null;
      throw error;
    });
  }
  return cached;
};
