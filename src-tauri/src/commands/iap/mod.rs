//! In-App Purchase / Pro entitlement.
//!
//! Both store builds are free to download; a single non-consumable purchase
//! ("MonoSub License" / "MonoSub Pro") removes the one-file queue limit enforced
//! by the frontend and by `start_transcription`:
//!
//! * **Mac App Store** — StoreKit 2, see `storekit.rs`. Grandfathered users stay
//!   Pro (`grandfather.rs`).
//! * **Microsoft Store** — a Durable, non-consumable add-on, see `msstore.rs`.
//!   No grandfathering: nothing had sold before the paid capability shipped.
//!
//! Design constraints that shape this module:
//!
//! * **The store is the authority.** We never write an `is_pro` boolean into
//!   `settings.json` and trust it later; entitlements are queried on launch and
//!   after every purchase/restore (`currentEntitlements` on Apple,
//!   `AddOnLicenses` / `IsInUserCollection` on Windows).
//! * **Only Store builds pay.** `appstore` (macOS) and `msstore` (Windows) are
//!   Cargo features enabled by their respective release scripts only. Every other
//!   build (DMG, NSIS self-install, CLI) returns `Direct` and is unlimited.
//! * **Debug builds have a fake provider.** `pro_dev_set_state` changes state
//!   only under `debug_assertions`; a release binary refuses it and cannot grant
//!   Pro.

#[cfg(all(target_os = "macos", feature = "appstore"))]
mod grandfather;
#[cfg(all(target_os = "windows", feature = "msstore"))]
mod msstore;
mod provider;
#[cfg(all(target_os = "macos", feature = "appstore"))]
mod storekit;

use std::sync::{OnceLock, RwLock};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};

/// Apple non-consumable product id (Mac App Store).
pub const PRO_PRODUCT_ID: &str = "leixlabs.monosub.pro";
/// The Partner Center "Product ID" of the Microsoft Store durable add-on,
/// surfaced to code as `StoreProduct::InAppOfferToken` (Microsoft Store).
pub const PRO_ADDON_OFFER_TOKEN: &str = "monosub_pro";
/// The Store-assigned id of that add-on, used by `RequestPurchaseAsync`.
pub const PRO_ADDON_STORE_ID: &str = "9PG23VS4QX3G";
/// The version that introduces the paid batch capability. Users whose original
/// App Store download is older than this are grandfathered into Pro (macOS only).
pub const PAID_VERSION: &str = "2.1.0";

/// How many unfinished files a build may have in its queue.
///
/// `Some(1)` is the free tier; `None` is unlimited (Pro, grandfathered, or a
/// non-Store build today). Keeping this in one type is what lets the frontend
/// and the Rust command agree without duplicating a boolean.
pub const FREE_MAX_QUEUE_FILES: usize = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProProduct {
    pub id: String,
    pub display_name: String,
    pub description: String,
    pub display_price: String,
    pub currency_code: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProStatus {
    /// Whether this build can sell anything at all. `false` on DMG / NSIS /
    /// other non-Store builds.
    pub supported: bool,
    /// `app_store` | `app_store_grandfathered` | `microsoft_store` | `direct` | `checkout` | `fake`
    pub provider: String,
    /// `unknown` | `checking` | `free` | `pro` | `pending` | `error`
    pub status: String,
    pub is_pro: bool,
    /// `Some(1)` for free; `None` for Pro / unlimited.
    pub max_queue_files: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub product: Option<ProProduct>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// The provider string for whichever store build this is; `"direct"` when the
/// build sells nothing. Lets `unknown()` / `free()` stay single definitions that
/// still name the right storefront.
fn store_provider() -> &'static str {
    #[cfg(all(target_os = "macos", feature = "appstore"))]
    {
        "app_store"
    }
    #[cfg(all(target_os = "windows", feature = "msstore"))]
    {
        "microsoft_store"
    }
    #[cfg(not(any(
        all(target_os = "macos", feature = "appstore"),
        all(target_os = "windows", feature = "msstore")
    )))]
    {
        "direct"
    }
}

impl ProStatus {
    fn unknown() -> Self {
        Self {
            supported: true,
            provider: store_provider().to_string(),
            status: "unknown".to_string(),
            is_pro: false,
            max_queue_files: Some(FREE_MAX_QUEUE_FILES),
            product: None,
            error: None,
        }
    }

    fn direct() -> Self {
        Self {
            supported: false,
            provider: "direct".to_string(),
            status: "pro".to_string(),
            is_pro: true,
            max_queue_files: None,
            product: None,
            error: None,
        }
    }

    fn free() -> Self {
        Self {
            supported: true,
            provider: store_provider().to_string(),
            status: "free".to_string(),
            is_pro: false,
            max_queue_files: Some(FREE_MAX_QUEUE_FILES),
            product: None,
            error: None,
        }
    }

    fn pro(provider: impl Into<String>) -> Self {
        Self {
            supported: true,
            provider: provider.into(),
            status: "pro".to_string(),
            is_pro: true,
            max_queue_files: None,
            product: None,
            error: None,
        }
    }

    #[cfg(debug_assertions)]
    fn fake(status: &str, is_pro: bool, max_queue_files: Option<usize>) -> Self {
        Self {
            supported: true,
            provider: "fake".to_string(),
            status: status.to_string(),
            is_pro,
            max_queue_files,
            product: None,
            error: None,
        }
    }
}

fn default_status() -> ProStatus {
    #[cfg(any(
        all(target_os = "macos", feature = "appstore"),
        all(target_os = "windows", feature = "msstore")
    ))]
    {
        ProStatus::unknown()
    }
    #[cfg(not(any(
        all(target_os = "macos", feature = "appstore"),
        all(target_os = "windows", feature = "msstore")
    )))]
    {
        provider::DirectProvider.status()
    }
}

static STATUS: OnceLock<RwLock<ProStatus>> = OnceLock::new();

fn status_lock() -> &'static RwLock<ProStatus> {
    STATUS.get_or_init(|| RwLock::new(default_status()))
}

/// Latest snapshot. Cheap: clones a handful of strings; called by the queue
/// gate on every file start, so it must not touch the store synchronously.
pub fn current() -> ProStatus {
    status_lock()
        .read()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .clone()
}

fn replace(status: ProStatus) -> ProStatus {
    *status_lock()
        .write()
        .unwrap_or_else(|poisoned| poisoned.into_inner()) = status.clone();
    status
}

pub fn is_pro() -> bool {
    current().is_pro
}

pub fn max_queue_files() -> Option<usize> {
    current().max_queue_files
}

fn emit(app: &AppHandle, status: &ProStatus) {
    if let Err(error) = app.emit("pro-status-changed", status) {
        tracing::warn!(error = %error, "failed to emit pro-status-changed");
    }
}

/// Install the initial state and, on a store build, refresh entitlements off the
/// main thread (and, on macOS, start StoreKit's transaction listener).
pub fn init(app: &AppHandle) {
    #[cfg(all(target_os = "macos", feature = "appstore"))]
    {
        let app_for_refresh = app.clone();
        tauri::async_runtime::spawn(async move {
            if let Err(error) = refresh_and_emit(&app_for_refresh).await {
                tracing::warn!(error = %error, "initial IAP refresh failed");
            }
        });

        storekit::start_update_listener(app.clone());
    }

    #[cfg(all(target_os = "windows", feature = "msstore"))]
    {
        let app_for_refresh = app.clone();
        tauri::async_runtime::spawn(async move {
            if let Err(error) = refresh_and_emit(&app_for_refresh).await {
                tracing::warn!(error = %error, "initial Microsoft Store IAP refresh failed");
            }
        });
        // No push channel like StoreKit's `Transaction.updates`: entitlement is
        // re-queried on demand (`pro_refresh`) and after purchase / restore.
    }

    #[cfg(not(any(
        all(target_os = "macos", feature = "appstore"),
        all(target_os = "windows", feature = "msstore")
    )))]
    {
        let _ = app;
    }
}

async fn refresh_and_emit(app: &AppHandle) -> Result<ProStatus, String> {
    if current().provider == "fake" {
        let status = current();
        emit(app, &status);
        return Ok(status);
    }

    #[cfg(all(target_os = "macos", feature = "appstore"))]
    {
        let previous = current();
        let mut checking = previous.clone();
        checking.status = "checking".to_string();
        let checking = replace(checking);
        emit(app, &checking);

        let app_for_refresh = app.clone();

        let result = tauri::async_runtime::spawn_blocking(move || {
            storekit::refresh_status(&app_for_refresh)
        })
        .await
        .map_err(|error| format!("IAP refresh task failed: {error}"))?;

        let status = match result {
            Ok(status) => status,
            Err(error) => {
                tracing::warn!(error = %error, "StoreKit refresh failed");
                let mut status = previous;
                status.status = "error".to_string();
                status.error = Some(error);
                status
            }
        };

        let status = replace(status);
        emit(app, &status);
        Ok(status)
    }

    #[cfg(all(target_os = "windows", feature = "msstore"))]
    {
        let previous = current();
        let mut checking = previous.clone();
        checking.status = "checking".to_string();
        let checking = replace(checking);
        emit(app, &checking);

        let result = tauri::async_runtime::spawn_blocking(msstore::refresh_status)
            .await
            .map_err(|error| format!("IAP refresh task failed: {error}"))?;

        let status = match result {
            Ok(status) => status,
            Err(error) => {
                tracing::warn!(error = %error, "Microsoft Store IAP refresh failed");
                let mut status = previous;
                status.status = "error".to_string();
                status.error = Some(error);
                status
            }
        };

        let status = replace(status);
        emit(app, &status);
        Ok(status)
    }

    #[cfg(not(any(
        all(target_os = "macos", feature = "appstore"),
        all(target_os = "windows", feature = "msstore")
    )))]
    {
        let status = current();
        emit(app, &status);
        Ok(status)
    }
}

#[tauri::command]
pub fn pro_status() -> ProStatus {
    current()
}

#[tauri::command]
pub async fn pro_refresh(app: AppHandle) -> Result<ProStatus, String> {
    refresh_and_emit(&app).await
}

#[tauri::command]
pub async fn pro_product() -> Result<Option<ProProduct>, String> {
    #[cfg(all(target_os = "macos", feature = "appstore"))]
    {
        tauri::async_runtime::spawn_blocking(storekit::product_info)
            .await
            .map_err(|error| format!("product lookup task failed: {error}"))?
    }

    #[cfg(all(target_os = "windows", feature = "msstore"))]
    {
        tauri::async_runtime::spawn_blocking(msstore::product_info)
            .await
            .map_err(|error| format!("product lookup task failed: {error}"))?
    }

    #[cfg(not(any(
        all(target_os = "macos", feature = "appstore"),
        all(target_os = "windows", feature = "msstore")
    )))]
    {
        Ok(None)
    }
}

#[tauri::command]
pub async fn pro_purchase(app: AppHandle) -> Result<ProStatus, String> {
    if current().provider == "fake" {
        let status = replace(ProStatus::pro("fake"));
        emit(&app, &status);
        return Ok(status);
    }

    #[cfg(all(target_os = "macos", feature = "appstore"))]
    {
        let previous = current();
        if previous.provider == "app_store_grandfathered" {
            return Ok(previous);
        }

        let result = tauri::async_runtime::spawn_blocking(storekit::purchase)
            .await
            .map_err(|error| format!("purchase task failed: {error}"))?;

        let status = match result {
            Ok(status) => status,
            Err(error) => {
                let mut failed = current();
                failed.status = "error".to_string();
                failed.error = Some(error);
                failed
            }
        };
        let status = replace(status);
        emit(&app, &status);
        Ok(status)
    }

    #[cfg(all(target_os = "windows", feature = "msstore"))]
    {
        let app_for_purchase = app.clone();
        let result =
            tauri::async_runtime::spawn_blocking(move || msstore::purchase(&app_for_purchase))
                .await
                .map_err(|error| format!("purchase task failed: {error}"))?;
        let status = match result {
            Ok(status) => status,
            Err(error) => {
                let mut failed = current();
                failed.status = "error".to_string();
                failed.error = Some(error);
                failed
            }
        };
        let status = replace(status);
        emit(&app, &status);
        Ok(status)
    }

    #[cfg(not(any(
        all(target_os = "macos", feature = "appstore"),
        all(target_os = "windows", feature = "msstore")
    )))]
    {
        let _ = app;
        Ok(current())
    }
}

#[tauri::command]
pub async fn pro_restore(app: AppHandle) -> Result<ProStatus, String> {
    if current().provider == "fake" {
        let status = replace(ProStatus::pro("fake"));
        emit(&app, &status);
        return Ok(status);
    }

    #[cfg(all(target_os = "macos", feature = "appstore"))]
    {
        let app_for_restore = app.clone();
        let result =
            tauri::async_runtime::spawn_blocking(move || storekit::restore(&app_for_restore))
                .await
                .map_err(|error| format!("restore task failed: {error}"))?;

        let status = match result {
            Ok(status) => status,
            Err(error) => {
                let mut failed = current();
                failed.status = "error".to_string();
                failed.error = Some(error);
                failed
            }
        };
        let status = replace(status);
        emit(&app, &status);
        Ok(status)
    }

    #[cfg(all(target_os = "windows", feature = "msstore"))]
    {
        // The Microsoft Store has no explicit "sync" call; re-reading the license
        // and the user collection *is* the restore.
        let result = tauri::async_runtime::spawn_blocking(msstore::refresh_status)
            .await
            .map_err(|error| format!("restore task failed: {error}"))?;
        let status = match result {
            Ok(status) => status,
            Err(error) => {
                let mut failed = current();
                failed.status = "error".to_string();
                failed.error = Some(error);
                failed
            }
        };
        let status = replace(status);
        emit(&app, &status);
        Ok(status)
    }

    #[cfg(not(any(
        all(target_os = "macos", feature = "appstore"),
        all(target_os = "windows", feature = "msstore")
    )))]
    {
        let _ = app;
        Ok(current())
    }
}

/// Debug-only switch used by Settings ▸ Developer and tauri-pilot scenarios.
///
/// The state-changing branch is compiled only under `debug_assertions`. A
/// runtime env var would be a one-line bypass in the shipped app; the release
/// binary keeps the command signature but answers with an error, so it cannot
/// grant Pro.
#[tauri::command]
pub fn pro_dev_set_state(app: AppHandle, state: String) -> Result<ProStatus, String> {
    #[cfg(debug_assertions)]
    {
        let status = match state.as_str() {
            "free" => ProStatus::fake("free", false, Some(FREE_MAX_QUEUE_FILES)),
            "pro" => ProStatus::fake("pro", true, None),
            "grandfathered" => ProStatus::pro("app_store_grandfathered"),
            "pending" => ProStatus::fake("pending", false, Some(FREE_MAX_QUEUE_FILES)),
            "error" => {
                let mut status = ProStatus::fake("error", false, Some(FREE_MAX_QUEUE_FILES));
                status.error = Some("fake IAP error for development".to_string());
                status
            }
            other => return Err(format!("unknown fake IAP state: {other}")),
        };

        let status = replace(status);
        emit(&app, &status);
        Ok(status)
    }

    #[cfg(not(debug_assertions))]
    {
        let _ = (app, state);
        Err("pro_dev_set_state is only available in development builds".to_string())
    }
}

/// Undo [`pro_dev_set_state`], returning to the provider the build normally
/// uses. The caller can then invoke `pro_refresh` for a real store query.
#[tauri::command]
pub fn pro_dev_clear_state(app: AppHandle) -> Result<ProStatus, String> {
    #[cfg(debug_assertions)]
    {
        let status = replace(default_status());
        emit(&app, &status);
        Ok(status)
    }

    #[cfg(not(debug_assertions))]
    {
        let _ = app;
        Err("pro_dev_clear_state is only available in development builds".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn free_status_has_one_file_limit() {
        let status = ProStatus::free();
        assert!(!status.is_pro);
        assert_eq!(status.max_queue_files, Some(1));
    }

    #[test]
    fn pro_status_is_unlimited() {
        let status = ProStatus::pro("app_store");
        assert!(status.is_pro);
        assert_eq!(status.max_queue_files, None);
    }

    #[test]
    fn direct_build_is_unlimited_today() {
        let status = ProStatus::direct();
        assert!(status.is_pro);
        assert_eq!(status.max_queue_files, None);
        assert!(!status.supported);
    }
}
