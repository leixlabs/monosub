//! Entitlement providers.
//!
//! Today there are two real answers and one development-only answer:
//!
//! * `AppStoreProvider` — the MAS build, implemented in `storekit.rs`.
//! * `DirectProvider` — Windows and the macOS DMG build, which are unlimited
//!   until the self-hosted checkout service lands.
//! * `FakeProvider` — `#[cfg(debug_assertions)]` only, driven by
//!   `pro_dev_set_state`; never compiled into a release.
//!
//! The trait exists so adding the checkout provider later does not have to
//! touch the queue gate or the UI. It is intentionally tiny: the provider owns
//! *how* the entitlement is obtained, while `ProStatus` (in the parent module)
//! is the transport shape shared by every provider.

use super::ProStatus;

#[allow(dead_code)] // implemented by DirectProvider; the future checkout provider will use it too
pub trait EntitlementProvider: Send + Sync {
    fn status(&self) -> ProStatus;
}

/// Direct-download / Windows provider. The user's own checkout service will
/// replace this, but today these builds keep the pre-IAP unlimited behaviour.
pub struct DirectProvider;

impl DirectProvider {
    pub fn status(&self) -> ProStatus {
        ProStatus::direct()
    }
}

impl EntitlementProvider for DirectProvider {
    fn status(&self) -> ProStatus {
        DirectProvider::status(self)
    }
}
