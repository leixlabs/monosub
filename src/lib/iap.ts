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
  | 'microsoft_store'
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

/** 离线授权信息（checkout 构建）。 */
export interface LicenseInfo {
  product: string;
  version: string;
  email: string;
  name: string | null;
  issuedAt: number;
  expiresAt: number | null;
  orderId: string | null;
  licensedTo: string;
}

/** 粘贴授权码激活（离线 Ed25519 验签）。 */
export const activateLicense = (license: string): Promise<ProStatus> =>
  invoke<ProStatus>('pro_activate_license', { license });

/** 当前授权信息，用于「关于 / Licensed to」。 */
export const getLicenseInfo = (): Promise<LicenseInfo | null> =>
  invoke<LicenseInfo | null>('license_info');

/** 打开内嵌购买窗口（checkout.leix.dev 商品页）。 */
export const openCheckoutWindow = (url: string): Promise<void> =>
  invoke<void>('open_checkout_window', { url });

/** True when this page is running inside the Tauri webview (not plain Vite). */
export const isTauri = (): boolean =>
  typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;
