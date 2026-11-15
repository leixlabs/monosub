import { create } from 'zustand';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import {
  clearDevProState,
  getProProduct,
  getProStatus,
  isTauri,
  PRO_STATUS_EVENT,
  purchasePro,
  refreshProStatus,
  restorePro,
  setDevProState,
  type ProStatus,
} from '@/lib/iap';

/**
 * The app's only entitlement store.
 *
 * It is deliberately **not** persisted: StoreKit (or the future checkout
 * service) is the source of truth, and a localStorage `isPro: true` would be a
 * one-line unlock. The store starts conservatively as "unknown / one file"; the
 * first `bootstrapEntitlement` call replaces it with the backend answer.
 */
interface EntitlementStore {
  status: ProStatus;
  initialized: boolean;
  upgradeOpen: boolean;
  busy: 'idle' | 'purchase' | 'restore';
  error: string | null;

  bootstrap: () => Promise<void>;
  refresh: () => Promise<void>;
  loadProduct: () => Promise<void>;
  purchase: () => Promise<void>;
  restore: () => Promise<void>;
  openUpgrade: () => void;
  closeUpgrade: () => void;
  setDevState: (state: 'free' | 'pro' | 'grandfathered' | 'pending' | 'error') => Promise<void>;
  clearDevState: () => Promise<void>;
}

const initialStatus: ProStatus = {
  supported: false,
  provider: 'unknown',
  status: 'unknown',
  isPro: false,
  maxQueueFiles: 1,
  product: null,
  error: null,
};

let listener: UnlistenFn | null = null;
let bootstrapping: Promise<void> | null = null;

const directFallback: ProStatus = {
  supported: false,
  provider: 'direct',
  status: 'pro',
  isPro: true,
  maxQueueFiles: null,
  product: null,
  error: null,
};

export const useEntitlementStore = create<EntitlementStore>((set, get) => ({
  status: initialStatus,
  initialized: false,
  upgradeOpen: false,
  busy: 'idle',
  error: null,

  bootstrap: async () => {
    if (!isTauri()) {
      // Vite-only browser preview: there is no StoreKit and no purchase. Keep
      // the preview usable rather than showing a fake paywall.
      set({ status: directFallback, initialized: true });
      return;
    }
    if (bootstrapping) return bootstrapping;

    bootstrapping = (async () => {
      if (!listener) {
        listener = await listen<ProStatus>(PRO_STATUS_EVENT, (event) => {
          set({ status: { ...event.payload, product: get().status.product ?? null } });
        });
      }

      try {
        const status = await getProStatus();
        set({ status, error: status.error ?? null });
      } catch (error) {
        // A dev build that has not been restarted with the new command still
        // has to remain usable; treat the failure as direct/unlimited rather
        // than blocking the UI behind a paywall that cannot be resolved.
        console.warn('[iap] pro_status failed', error);
        set({ status: directFallback });
      } finally {
        set({ initialized: true });
      }

      // The initial status is intentionally cheap (no StoreKit query on the
      // command's thread). Refresh/price load happen after the first paint.
      void get().refresh();
      if (get().status.supported) void get().loadProduct();
    })().finally(() => {
      bootstrapping = null;
    });

    return bootstrapping;
  },

  refresh: async () => {
    if (!isTauri()) return;
    try {
      const status = await refreshProStatus();
      set({ status: { ...status, product: get().status.product ?? null }, error: status.error ?? null });
    } catch (error) {
      console.warn('[iap] pro_refresh failed', error);
    }
  },

  loadProduct: async () => {
    if (!isTauri()) return;
    try {
      const product = await getProProduct();
      set((state) => ({ status: { ...state.status, product } }));
    } catch (error) {
      console.warn('[iap] pro_product failed', error);
    }
  },

  purchase: async () => {
    if (get().busy !== 'idle') return;
    set({ busy: 'purchase', error: null });
    try {
      const status = await purchasePro();
      set({ status: { ...status, product: get().status.product ?? null } });
      if (status.isPro) set({ upgradeOpen: false });
      else if (status.status === 'error') set({ error: status.error ?? 'purchase failed' });
    } catch (error) {
      console.warn('[iap] pro_purchase failed', error);
      set({ error: String(error) });
    } finally {
      set({ busy: 'idle' });
    }
  },

  restore: async () => {
    if (get().busy !== 'idle') return;
    set({ busy: 'restore', error: null });
    try {
      const status = await restorePro();
      set({ status: { ...status, product: get().status.product ?? null } });
      if (status.isPro) set({ upgradeOpen: false });
      else if (status.status === 'error') set({ error: status.error ?? 'restore failed' });
    } catch (error) {
      console.warn('[iap] pro_restore failed', error);
      set({ error: String(error) });
    } finally {
      set({ busy: 'idle' });
    }
  },

  openUpgrade: () => set({ upgradeOpen: true, error: null }),
  closeUpgrade: () => set({ upgradeOpen: false, error: null }),

  setDevState: async (state) => {
    try {
      const status = await setDevProState(state);
      set({ status: { ...status, product: get().status.product ?? null }, error: status.error ?? null });
    } catch (error) {
      console.warn('[iap] pro_dev_set_state failed', error);
    }
  },

  clearDevState: async () => {
    try {
      const status = await clearDevProState();
      set({ status: { ...status, product: get().status.product ?? null }, error: null });
      // Return to the real provider immediately; on a MAS debug build this
      // performs a StoreKit query, on a direct build it is a cheap no-op.
      void get().refresh();
    } catch (error) {
      console.warn('[iap] pro_dev_clear_state failed', error);
    }
  },
}));
