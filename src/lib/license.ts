import { invoke } from '@tauri-apps/api/core';
import { confirm } from '@tauri-apps/plugin-dialog';
import { message } from '@tauri-apps/plugin-dialog';
import { useStore } from '@/store/useStore';

interface ActivateLicenseResponse {
  activated: boolean;
  activationId?: string | null;
  status: string;
  message: string;
  expiresAt?: string | null;
}

interface ValidateLicenseResponse {
  valid: boolean;
  status: string;
  message: string;
  expiresAt?: string | null;
  activationRequired?: boolean;
}

interface DeactivateLicenseResponse {
  deactivated: boolean;
  status: string;
  message: string;
}

interface EnsureLicenseOptions {
  forceRefresh?: boolean;
  openCheckoutOnFailure?: boolean;
}

interface LicenseCheckoutPromptOptions {
  title: string;
  message: string;
  openActionLabel: string;
  cancelLabel: string;
}

interface LicenseFailureActionPromptOptions {
  title: string;
  message: string;
  openCheckoutLabel: string;
  openSettingsLabel: string;
  cancelLabel: string;
}

type LicenseFailureAction = 'checkout' | 'settings' | 'cancel';

const VALID_STATUS_CACHE_MS = 5 * 60 * 1000;
const POLAR_BASE_URL = 'https://polar.leix.dev';
const POLAR_ORGANIZATION_ID = '6c7d88dd-4cae-4491-85c9-4dbb65170b12';
const POLAR_BENEFIT_ID = 'f288c189-f31b-4e0a-b849-1a0308420110';
const POLAR_CHECKOUT_URL =
  'https://polar.leix.dev/v1/checkout-links/polar_cl_JgYlmsUnlbej0vpnuI197nQltlEsUxg1hZd0r3unixT/redirect';
export const SETTINGS_OPEN_CATEGORY_STORAGE_KEY = 'monosub-settings-category';

export const polarLicenseConstants = {
  baseUrl: POLAR_BASE_URL,
  organizationId: POLAR_ORGANIZATION_ID,
  benefitId: POLAR_BENEFIT_ID,
  checkoutUrl: POLAR_CHECKOUT_URL,
};

const createDeviceId = () => {
  if (typeof crypto !== 'undefined' && typeof crypto.randomUUID === 'function') {
    return crypto.randomUUID();
  }
  return `monosub-${Date.now()}-${Math.random().toString(36).slice(2, 10)}`;
};

const safeTrim = (value: unknown): string => {
  return typeof value === 'string' ? value.trim() : '';
};

const ensureDeviceId = () => {
  const state = useStore.getState();
  const current = safeTrim(state.licenseConfig?.licenseDeviceId);
  if (current) return current;

  const generated = createDeviceId();
  state.setLicenseDeviceId(generated);
  return generated;
};

const hasRecentValidCache = () => {
  const { licenseStatus, licenseLastCheckedAt } = useStore.getState();
  if (licenseStatus !== 'valid' || !licenseLastCheckedAt) {
    return false;
  }

  const checkedAtMs = Date.parse(licenseLastCheckedAt);
  if (Number.isNaN(checkedAtMs)) {
    return false;
  }

  return Date.now() - checkedAtMs <= VALID_STATUS_CACHE_MS;
};

let _validatePromise: Promise<ValidateLicenseResponse> | null = null;

export const validatePolarLicense = async (): Promise<ValidateLicenseResponse> => {
  if (_validatePromise) return _validatePromise;

  _validatePromise = _doValidatePolarLicense().finally(() => {
    _validatePromise = null;
  });
  return _validatePromise;
};

const _doValidatePolarLicense = async (): Promise<ValidateLicenseResponse> => {
  const state = useStore.getState();
  const { licenseConfig } = state;
  const key = safeTrim(licenseConfig?.licenseKey);
  const activationId = safeTrim(licenseConfig?.licenseActivationId) || undefined;
  const deviceId = ensureDeviceId();

  if (!key) {
    const response: ValidateLicenseResponse = {
      valid: false,
      status: 'invalid',
      message: 'License key is required.',
      expiresAt: null,
    };
    state.setLicenseValidation({
      status: 'invalid',
      message: response.message,
      expiresAt: null,
    });
    return response;
  }

  state.setLicenseStatus('checking');
  try {
    const response = await invoke<ValidateLicenseResponse>('validate_license', {
      request: {
        key,
        organizationId: POLAR_ORGANIZATION_ID,
        benefitId: POLAR_BENEFIT_ID,
        activationId,
        deviceId,
      },
    });

    state.setLicenseValidation({
      status: response.valid ? 'valid' : response.status === 'error' ? 'error' : 'invalid',
      message: response.message,
      expiresAt: response.expiresAt ?? null,
    });

    return response;
  } catch (error) {
    const message = `License validation failed: ${String(error)}`;
    state.setLicenseValidation({
      status: 'error',
      message,
      expiresAt: null,
    });
    return {
      valid: false,
      status: 'error',
      message,
      expiresAt: null,
    };
  }
};

export const activatePolarLicense = async (): Promise<ActivateLicenseResponse> => {
  const state = useStore.getState();
  const key = safeTrim(state.licenseConfig?.licenseKey);
  const deviceId = ensureDeviceId();

  if (!key) {
    const response: ActivateLicenseResponse = {
      activated: false,
      status: 'invalid',
      message: 'License key is required.',
      activationId: null,
      expiresAt: null,
    };
    state.setLicenseValidation({
      status: 'invalid',
      message: response.message,
      expiresAt: null,
    });
    return response;
  }

  state.setLicenseStatus('checking');
  try {
    const response = await invoke<ActivateLicenseResponse>('activate_license', {
      request: {
        key,
        organizationId: POLAR_ORGANIZATION_ID,
        benefitId: POLAR_BENEFIT_ID,
        deviceId,
      },
    });

    if (response.activated && response.activationId) {
      state.setLicenseActivationId(response.activationId);
    }

    state.setLicenseValidation({
      status: response.activated ? 'valid' : response.status === 'error' ? 'error' : 'invalid',
      message: response.message,
      expiresAt: response.expiresAt ?? null,
    });

    return response;
  } catch (error) {
    const message = `License activation failed: ${String(error)}`;
    state.setLicenseValidation({
      status: 'error',
      message,
      expiresAt: null,
    });
    return {
      activated: false,
      status: 'error',
      message,
      activationId: null,
      expiresAt: null,
    };
  }
};

export const deactivatePolarLicense = async (): Promise<DeactivateLicenseResponse> => {
  const state = useStore.getState();
  const key = safeTrim(state.licenseConfig?.licenseKey);
  const activationId = safeTrim(state.licenseConfig?.licenseActivationId);

  if (!key || !activationId) {
    return {
      deactivated: false,
      status: 'invalid',
      message: 'No activation found on this device.',
    };
  }

  try {
    const response = await invoke<DeactivateLicenseResponse>('deactivate_license', {
      request: {
        key,
        organizationId: POLAR_ORGANIZATION_ID,
        activationId,
      },
    });

    if (response.deactivated) {
      state.setLicenseActivationId('');
      state.clearLicenseValidation();
    } else {
      state.setLicenseValidation({
        status: response.status === 'error' ? 'error' : 'invalid',
        message: response.message,
      });
    }
    return response;
  } catch (error) {
    return {
      deactivated: false,
      status: 'error',
      message: `License deactivation failed: ${String(error)}`,
    };
  }
};

export const openLicenseCheckout = async (): Promise<boolean> => {
  const url = POLAR_CHECKOUT_URL;

  try {
    await invoke('open_external_url', { url });
    return true;
  } catch (error) {
    console.error('[open_external_url] Failed:', error);
    try {
      window.open(url, '_blank');
      return true;
    } catch (fallbackError) {
      console.error('[window.open] Failed:', fallbackError);
      return false;
    }
  }
};

export const openLicenseSettings = (): void => {
  try {
    localStorage.setItem(SETTINGS_OPEN_CATEGORY_STORAGE_KEY, 'license');
  } catch (error) {
    console.warn('[openLicenseSettings] Failed to persist target category:', error);
  }
  useStore.getState().setIsSettingsOpen(true);
};

export const promptOpenLicenseCheckout = async (
  options: LicenseCheckoutPromptOptions
): Promise<boolean> => {
  try {
    return await confirm(options.message, {
      title: options.title,
      kind: 'info',
      okLabel: options.openActionLabel,
      cancelLabel: options.cancelLabel,
    });
  } catch (error) {
    console.error('[promptOpenLicenseCheckout] Failed:', error);
    return false;
  }
};

export const promptLicenseFailureAction = async (
  options: LicenseFailureActionPromptOptions
): Promise<LicenseFailureAction> => {
  try {
    const result = await message(options.message, {
      title: options.title,
      kind: 'info',
      buttons: {
        yes: options.openSettingsLabel,
        no: options.openCheckoutLabel,
        cancel: options.cancelLabel,
      },
    });

    if (result === options.openSettingsLabel || result === 'Yes') {
      return 'settings';
    }
    if (result === options.openCheckoutLabel || result === 'No') {
      return 'checkout';
    }
    return 'cancel';
  } catch (error) {
    console.error('[promptLicenseFailureAction] Failed:', error);
    return 'cancel';
  }
};

export const ensureMultiSpeakerLicense = async (
  options: EnsureLicenseOptions = {}
): Promise<boolean> => {
  const { forceRefresh = false, openCheckoutOnFailure = false } = options;
  const state = useStore.getState();
  const { licenseConfig } = state;
  const hasConfig = Boolean(safeTrim(licenseConfig?.licenseKey));

  if (!hasConfig) {
    state.setLicenseValidation({
      status: 'invalid',
      message: 'Please enter your Polar license key first.',
      expiresAt: null,
    });
    if (openCheckoutOnFailure) {
      await openLicenseCheckout();
    }
    return false;
  }

  if (!forceRefresh && hasRecentValidCache()) {
    return true;
  }

  if (!safeTrim(licenseConfig?.licenseActivationId)) {
    const activation = await activatePolarLicense();
    if (!activation.activated) {
      if (openCheckoutOnFailure) {
        await openLicenseCheckout();
      }
      return false;
    }
  }

  const result = await validatePolarLicense();
  if (!result.valid && result.activationRequired) {
    const activation = await activatePolarLicense();
    if (activation.activated) {
      const revalidate = await validatePolarLicense();
      if (revalidate.valid) return true;
    }
  }

  if (!result.valid && openCheckoutOnFailure) {
    await openLicenseCheckout();
  }

  return result.valid;
};

let _startupValidationPromise: Promise<void> | null = null;

export const validateLicenseOnStartup = async () => {
  const { licenseConfig } = useStore.getState();
  if (!safeTrim(licenseConfig?.licenseKey)) return;

  if (_startupValidationPromise) {
    return _startupValidationPromise;
  }

  _startupValidationPromise = ensureMultiSpeakerLicense({
    forceRefresh: true,
    openCheckoutOnFailure: false,
  })
    .then(() => undefined)
    .finally(() => {
      _startupValidationPromise = null;
    });

  await _startupValidationPromise;
};
