//! Grandfathering for users who downloaded MonoSub from the Mac App Store
//! before the paid batch capability existed.
//!
//! These users should keep unlimited batch transcription without buying the
//! new in-app purchase. The proof is the App Store's own `AppTransaction`:
//!
//! * `originalAppVersion` is the version that was first downloaded. A value
//!   below [`PAID_VERSION`] (for example `2.0.2`) means the user was here
//!   before 2.1.0.
//! * `originalPurchaseDate` is kept in the status for support/debugging; the
//!   version comparison is the primary rule because it is stable even when the
//!   App Store's date formatting changes.
//!
//! This is a courtesy entitlement, not a security boundary: it is evaluated
//! from StoreKit's verified `AppTransaction` on first launch, and a positive
//! result is cached as `proGrandfathered` in `settings.json` so offline launches
//! keep working. Editing that file can only affect the courtesy grant, never a
//! paid StoreKit entitlement.

use storekit::AppTransaction;
use tauri::AppHandle;

use super::{ProStatus, PAID_VERSION};

use crate::model_dir;

/// Return `true` when this installation's original App Store version predates
/// the paid release. Any StoreKit/verification error is treated as "not
/// grandfathered" rather than granting access by accident.
pub fn is_grandfathered(app: &AppHandle) -> bool {
    // The cached marker exists so an App Store user who already qualified can
    // still run in batch when AppTransaction cannot reach the store (offline,
    // StoreKit daemon busy). It is only ever written after a verified result.
    let mut settings = model_dir::load_settings(app);
    if settings.pro_grandfathered {
        return true;
    }

    let Ok(result) = AppTransaction::shared() else {
        return false;
    };
    let Ok(app_transaction) = result.payload_value() else {
        return false;
    };

    if !is_version_before_paid_release(&app_transaction.original_app_version) {
        return false;
    }

    settings.pro_grandfathered = true;
    if let Err(error) = model_dir::save_settings(app, &settings) {
        tracing::warn!(error = %error, "could not cache the grandfathering marker");
    }
    true
}

/// Exposed for unit tests; `original_app_version` can be empty or a marketing
/// string on development builds, which must not grant Pro.
pub(crate) fn is_version_before_paid_release(original: &str) -> bool {
    let Ok(original) = semver::Version::parse(original.trim()) else {
        return false;
    };
    let Ok(paid) = semver::Version::parse(PAID_VERSION) else {
        return false;
    };
    original < paid
}

/// Status for a grandfathered installation. The product is deliberately absent:
/// these users never need a purchase sheet.
pub fn status() -> ProStatus {
    ProStatus::pro("app_store_grandfathered")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_below_the_paid_release_are_grandfathered() {
        assert!(is_version_before_paid_release("2.0.2"));
        assert!(is_version_before_paid_release("2.0.1"));
        assert!(is_version_before_paid_release("1.9.9"));
    }

    #[test]
    fn the_paid_release_and_later_are_not_grandfathered() {
        assert!(!is_version_before_paid_release("2.1.0"));
        assert!(!is_version_before_paid_release("2.1.1"));
        assert!(!is_version_before_paid_release("3.0.0"));
    }

    #[test]
    fn missing_or_non_semver_versions_never_grant_pro() {
        assert!(!is_version_before_paid_release(""));
        assert!(!is_version_before_paid_release("dev"));
        assert!(!is_version_before_paid_release("2.0"));
    }
}
