//! In-App Purchase / Pro entitlement.
//!
//! The Mac App Store build is free to download; a non-consumable "MonoSub Pro"
//! purchase removes the one-file watch limit enforced by the frontend and by
//! `start_transcription`.
//!
//! Design constraints that shape this module:
//!
//! * **StoreKit 2 is the authority.** We never write an `is_pro` boolean into
//!   `settings.json` and trust it later; `Transaction.currentEntitlements`
//!   (plus `AppTransaction` for grandfathered users) is queried on launch and
//!   after every purchase/restore.
//! * **Only the MAS build pays.** `appstore` is a Cargo feature enabled only by
//!   `scripts/release/mas.mjs`. A normal DMG build returns `Direct` and never
//!   compiles the Swift bridge.
//! * **Debug builds have a fake provider.** `pro_dev_set_state` changes state
//!   only under `debug_assertions`, so UI and queue tests need neither an App
//!   Store Connect product nor a sandbox account. In a release build the
//!   command exists but returns an error and cannot grant Pro.
//! * **Grandfathered users stay Pro.** Existing App Store downloads are
//!   recognized with `AppTransaction.originalAppVersion < 2.1.0` and never see
//!   a purchase prompt.

#[cfg(all(target_os = "macos", feature = "appstore"))]
mod grandfather;
mod provider;
#[cfg(all(target_os = "macos", feature = "appstore"))]
mod storekit;

use std::sync::{OnceLock, RwLock};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};

pub const PRO_PRODUCT_ID: &str = "leixlabs.monosub.pro";
/// The version that introduces the paid batch capability. Users whose original
/// App Store download is older than this are grandfathered into Pro.
pub const PAID_VERSION: &str = "2.1.0";

/// How many unfinished files a build may have in its queue.
///
/// `Some(1)` is the free tier; `None` is unlimited (Pro, grandfathered, or a
/// non-App-Store build today). Keeping this in one type is what lets the
/// frontend and the Rust command agree without duplicating a boolean.
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
    /// Whether this build can sell anything at all. `false` on Windows and on
    /// DMG builds where there is no StoreKit integration.
    pub supported: bool,
    /// `app_store` | `app_store_grandfathered` | `direct` | `checkout`
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

impl ProStatus {
    #[cfg(all(target_os = "macos", feature = "appstore"))]
    fn unknown() -> Self {
        Self {
            supported: true,
            provider: "app_store".to_string(),
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

    #[cfg(all(target_os = "macos", feature = "appstore"))]
    fn free() -> Self {
        Self {
            supported: true,
            provider: "app_store".to_string(),
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
    #[cfg(all(target_os = "macos", feature = "appstore"))]
    {
        ProStatus::unknown()
    }
    #[cfg(not(all(target_os = "macos", feature = "appstore")))]
    {
        provider::DirectProvider.status()
    }
}

static STATUS: OnceLock<RwLock<ProStatus>> = OnceLock::new();

fn status_lock() -> &'static RwLock<ProStatus> {
    STATUS.get_or_init(|| RwLock::new(default_status()))
}

/// Latest snapshot. Cheap: clones a handful of strings; called by the queue
/// gate on every file start, so it must not touch StoreKit synchronously.
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

/// Install the initial state and, on the MAS build, start the StoreKit
/// transaction listener and refresh entitlements off the main thread.
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

    #[cfg(not(all(target_os = "macos", feature = "appstore")))]
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

    #[cfg(not(all(target_os = "macos", feature = "appstore")))]
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

    #[cfg(not(all(target_os = "macos", feature = "appstore")))]
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

    #[cfg(not(all(target_os = "macos", feature = "appstore")))]
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

    #[cfg(not(all(target_os = "macos", feature = "appstore")))]
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
/// uses. The caller can then invoke `pro_refresh` for a real StoreKit query.
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
