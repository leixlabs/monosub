import { invoke } from '@tauri-apps/api/core';

/**
 * In-App Purchase status shared by the Settings card, the upgrade dialog and
 * the queue gate. The shape mirrors `commands::iap::ProStatus` in Rust; the
 * `maxQueueFiles` field is the one the queue actually reads, so the free tier
 * is not a second boolean that could drift from the backend.
 */
export type ProProvider =
  | 'app_store'
  | 'app_store_grandfathered'
  | 'direct'
  | 'checkout'
  | 'fake'
  | 'unknown';

export type ProStatusValue =
  | 'unknown'
  | 'checking'
  | 'free'
  | 'pro'
  | 'pending'
  | 'error';

export interface ProProduct {
  id: string;
  displayName: string;
  description: string;
  displayPrice: string;
  currencyCode?: string | null;
}

export interface ProStatus {
  supported: boolean;
  provider: ProProvider;
  status: ProStatusValue;
  isPro: boolean;
  /** `1` for free; `null` for unlimited. */
  maxQueueFiles: number | null;
  product?: ProProduct | null;
  error?: string | null;
}

export const PRO_STATUS_EVENT = 'pro-status-changed';

export const getProStatus = (): Promise<ProStatus> => invoke<ProStatus>('pro_status');

export const refreshProStatus = (): Promise<ProStatus> => invoke<ProStatus>('pro_refresh');

export const getProProduct = (): Promise<ProProduct | null> => invoke<ProProduct | null>('pro_product');

export const purchasePro = (): Promise<ProStatus> => invoke<ProStatus>('pro_purchase');

export const restorePro = (): Promise<ProStatus> => invoke<ProStatus>('pro_restore');

/** Debug-only in Rust; the release command returns an error and is never shown. */
export const setDevProState = (state: 'free' | 'pro' | 'grandfathered' | 'pending' | 'error') =>
  invoke<ProStatus>('pro_dev_set_state', { state });

export const clearDevProState = (): Promise<ProStatus> => invoke<ProStatus>('pro_dev_clear_state');

/** True when this page is running inside the Tauri webview (not plain Vite). */
export const isTauri = (): boolean =>
  typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;
