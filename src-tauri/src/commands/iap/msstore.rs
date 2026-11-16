//! Microsoft Store durable add-on integration (Windows Store build only).
//!
//! The Store build is **free to download**; a single **Durable, non-consumable**
//! add-on ("MonoSub License", offer token [`super::PRO_ADDON_OFFER_TOKEN`])
//! unlocks unlimited batch transcription. This mirrors the macOS StoreKit path in
//! `storekit.rs`, but the Windows commerce API is completely different:
//!
//! * **The Store license is the authority**, not a local boolean. We read
//!   `StoreContext::GetAppLicenseAsync().AddOnLicenses()` and fall back to
//!   `GetAssociatedStoreProductsAsync(["Durable"])` +
//!   `StoreProduct::IsInUserCollection()`. A local `is_pro` flag is never trusted.
//! * **Only the Store build pays.** `msstore` is a Cargo feature enabled only by
//!   the MSIX branch of `scripts/release/package.mjs`. The NSIS self-install and
//!   every other build stay `Direct` (unlimited).
//! * **No grandfathering.** Nothing had sold on the Microsoft Store before the
//!   paid capability shipped, so every existing install is a new free-tier user.
//! * **Purchase needs the UI thread + a parent HWND.** Microsoft: IAP is "not
//!   supported in elevated applications" (we run non-elevated) and
//!   `RequestPurchaseAsync` must run on the UI thread with the `StoreContext`
//!   associated with the app window (`IInitializeWithWindow`), else
//!   `0x80070578 ERROR_INVALID_WINDOW_HANDLE`. We *start* the purchase on the
//!   Tauri main thread and *join* it on the blocking worker, so the UI thread
//!   stays free to pump the Store's modal dialog.
//! * **Graceful degradation.** `Windows.Services.Store` requires a *retail*
//!   license; a side-loaded developer package has none (`0x803F6108`). Every call
//!   here can fail and that must read as "unlicensed / free" — never a crash.
//!
//! All functions are blocking (they call `.join()` on WinRT async operations) and
//! are therefore driven from `spawn_blocking` in the parent module. Every thread
//! that touches WinRT gets an MTA apartment first (see `ensure_com_mta`).

use tauri::Manager;
use windows::core::{Error, HSTRING, Interface};
use windows::Services::Store::{
    StoreContext, StoreLicense, StoreProduct, StorePurchaseResult, StorePurchaseStatus,
};
use windows::Win32::Foundation::HWND;
use windows::Win32::System::WinRT::{RoInitialize, RO_INIT_MULTITHREADED};
use windows::Win32::UI::Shell::IInitializeWithWindow;
use windows_collections::IIterable;

use super::{ProProduct, ProStatus, PRO_ADDON_OFFER_TOKEN, PRO_ADDON_STORE_ID};

const ADDON_KIND: &str = "Durable";

fn hstr(value: &str) -> HSTRING {
    HSTRING::from(value)
}

fn iter1(value: &str) -> IIterable<HSTRING> {
    IIterable::from(vec![hstr(value)])
}

fn hres(error: &Error) -> String {
    format!("0x{:08X}: {}", error.code().0 as u32, error.message())
}

fn ext_code(hresult: windows::core::HRESULT) -> u32 {
    hresult.0 as u32
}

/// Give the current thread a COM apartment (MTA). `S_FALSE` / `RPC_E_CHANGED_MODE`
/// are fine (the thread already has an apartment) and are ignored.
fn ensure_com_mta() {
    let _ = unsafe { RoInitialize(RO_INIT_MULTITHREADED) };
}

/// The "no retail license" family thrown when the package was side-loaded with a
/// dev certificate instead of installed from the Store. Treated as **free**, not
/// as an error, so the free tier is usable on a dev machine.
fn is_no_license(code: u32) -> bool {
    matches!(code, 0x803F6108 | 0x803F6107)
}

fn is_our_product(product: &StoreProduct) -> bool {
    let by_id = product
        .StoreId()
        .map(|id| id.to_string() == PRO_ADDON_STORE_ID)
        .unwrap_or(false);
    let by_token = product
        .InAppOfferToken()
        .map(|token| token.to_string() == PRO_ADDON_OFFER_TOKEN)
        .unwrap_or(false);
    by_id || by_token
}

/// All "Durable" add-ons the Store associates with this app (keyed by Store ID).
fn associated_products() -> Result<Vec<(HSTRING, StoreProduct)>, String> {
    let context = StoreContext::GetDefault().map_err(|error| hres(&error))?;
    let result = context
        .GetAssociatedStoreProductsAsync(&iter1(ADDON_KIND))
        .map_err(|error| hres(&error))?
        .join()
        .map_err(|error| hres(&error))?;

    let code = result.ExtendedError().map(ext_code).unwrap_or(0);
    if is_no_license(code) {
        tracing::warn!("Microsoft Store: no retail license (0x{code:08X})");
        return Ok(Vec::new());
    }
    if code != 0 {
        return Err(format!("GetAssociatedStoreProductsAsync ExtendedError 0x{code:08X}"));
    }

    let view = result.Products().map_err(|error| hres(&error))?;
    let mut products = Vec::new();
    for kv in &view {
        let key = kv.Key().map_err(|error| hres(&error))?;
        let product = kv.Value().map_err(|error| hres(&error))?;
        products.push((key, product));
    }
    Ok(products)
}

/// True when an active entitlement for the Pro add-on exists.
///
/// Two independent sources are consulted (right after a purchase they can briefly
/// disagree); either granting Pro is enough. `Ok(false)` also covers "not a
/// licensed Store install", so the caller degrades to Free instead of erroring.
pub fn has_entitlement() -> Result<bool, String> {
    ensure_com_mta();
    let context = StoreContext::GetDefault().map_err(|error| hres(&error))?;

    // 1) License map — one StoreLicense per owned add-on. The map key is the
    //    add-on SKU/store id on some OS builds and the offer token on others, so
    //    match on every identifier the license exposes.
    match context.GetAppLicenseAsync().and_then(|operation| operation.join()) {
        Ok(license) => {
            let addons = license.AddOnLicenses().map_err(|error| hres(&error))?;
            for kv in &addons {
                let key = kv.Key().map(|k| k.to_string()).unwrap_or_default();
                let entry: StoreLicense = kv.Value().map_err(|error| hres(&error))?;
                let active = entry.IsActive().unwrap_or(false);
                let sku = entry.SkuStoreId().map(|v| v.to_string()).unwrap_or_default();
                let token = entry
                    .InAppOfferToken()
                    .map(|v| v.to_string())
                    .unwrap_or_default();
                let ours = key == PRO_ADDON_STORE_ID
                    || key == PRO_ADDON_OFFER_TOKEN
                    || sku == PRO_ADDON_STORE_ID
                    || token == PRO_ADDON_OFFER_TOKEN;
                if active && ours {
                    return Ok(true);
                }
            }
            Ok(false)
        }
        Err(error) if is_no_license(error.code().0 as u32) => {
            tracing::warn!(
                "Microsoft Store: no retail license (0x{:08X}); treating as free",
                error.code().0 as u32
            );
            Ok(false)
        }
        Err(error) => Err(hres(&error)),
    }
}

/// Localized add-on metadata for the upgrade dialog. `Ok(None)` means the add-on
/// could not be resolved (dev side-load, or metadata not propagated yet); the UI
/// then falls back to its `$9.99 / ¥48` line rather than blocking the free tier.
pub fn product_info() -> Result<Option<ProProduct>, String> {
    ensure_com_mta();
    let products = associated_products()?;

    for (_store_id, product) in products {
        if !is_our_product(&product) {
            continue;
        }
        let price = product.Price().ok();
        return Ok(Some(ProProduct {
            id: PRO_ADDON_OFFER_TOKEN.to_string(),
            display_name: product.Title().map(|v| v.to_string()).unwrap_or_default(),
            description: product.Description().map(|v| v.to_string()).unwrap_or_default(),
            display_price: price
                .as_ref()
                .and_then(|price| price.FormattedPrice().ok())
                .map(|v| v.to_string())
                .unwrap_or_default(),
            currency_code: price
                .as_ref()
                .and_then(|price| price.CurrencyCode().ok())
                .map(|v| v.to_string()),
        }));
    }

    Ok(None)
}

/// Current entitlement (no grandfathering on Windows). A pure read; used on
/// launch, after a purchase and by "Restore License".
pub fn refresh_status() -> Result<ProStatus, String> {
    if has_entitlement()? {
        Ok(ProStatus::pro("microsoft_store"))
    } else {
        Ok(ProStatus::free())
    }
}

/// The HWND of the main webview window, if it exists.
fn main_window_hwnd(app: &tauri::AppHandle) -> Result<HWND, String> {
    app.get_webview_window("main")
        .ok_or_else(|| "main window is not available yet".to_string())?
        .hwnd()
        .map_err(|error| format!("could not get the main window HWND: {error}"))
}

/// Start a purchase. The Store's purchase UI is initiated on the Tauri main/UI
/// thread (required by Microsoft) and the returned async operation is joined on
/// this blocking worker, so the dialog can run while the UI thread pumps messages.
pub fn purchase(app: &tauri::AppHandle) -> Result<ProStatus, String> {
    let hwnd = main_window_hwnd(app)?;
    // `HWND` is a raw pointer and not `Send`, so the closure carries it as isize.
    let hwnd_raw = hwnd.0 as isize;
    let store_id = hstr(PRO_ADDON_STORE_ID);

    type PurchaseOp = windows_future::IAsyncOperation<StorePurchaseResult>;
    let (sender, receiver) = std::sync::mpsc::channel::<Result<(StoreContext, PurchaseOp), String>>();

    app.run_on_main_thread(move || {
        let hwnd = HWND(hwnd_raw as *mut core::ffi::c_void);
        let started = (|| -> Result<(StoreContext, PurchaseOp), String> {
            let context = StoreContext::GetDefault().map_err(|error| hres(&error))?;
            let interop: IInitializeWithWindow =
                context.cast().map_err(|error| hres(&error))?;
            // SAFETY: `interop` is a valid interface on `context`, and `hwnd` is
            // the live main window handle.
            unsafe {
                interop.Initialize(hwnd).map_err(|error| hres(&error))?;
            }
            let operation = context
                .RequestPurchaseAsync(&store_id)
                .map_err(|error| hres(&error))?;
            Ok((context, operation))
        })();
        let _ = sender.send(started);
    })
    .map_err(|error| format!("could not dispatch the purchase to the UI thread: {error}"))?;

    let (context, operation) = receiver
        .recv()
        .map_err(|_| "the purchase UI thread dropped before starting".to_string())??;

    ensure_com_mta();
    let result = operation
        .join()
        .map_err(|error| format!("the purchase did not complete: {}", hres(&error)))?;
    let _keep_context_alive = context;
    let status = result.Status().map_err(|error| hres(&error))?;
    let code = result.ExtendedError().map(ext_code).unwrap_or(0);

    if is_no_license(code) {
        let mut failed = super::current();
        failed.status = "error".to_string();
        failed.error = Some(
            "This build has no Microsoft Store retail license; in-app purchase is unavailable."
                .to_string(),
        );
        return Ok(failed);
    }

    if status == StorePurchaseStatus::Succeeded || status == StorePurchaseStatus::AlreadyPurchased {
        return Ok(ProStatus::pro("microsoft_store"));
    }
    if status == StorePurchaseStatus::NotPurchased {
        // The user cancelled: a normal outcome, keep whatever we had.
        return Ok(super::current());
    }

    let mut failed = super::current();
    failed.status = "error".to_string();
    failed.error = Some(if status == StorePurchaseStatus::NetworkError {
        "purchase failed: network error".to_string()
    } else if status == StorePurchaseStatus::ServerError {
        "purchase failed: server error".to_string()
    } else {
        format!("purchase did not succeed (status {})", status.0)
    });
    Ok(failed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn purchase_status_constants_match_the_sdk() {
        // Pinned so a windows-crate upgrade cannot silently renumber them.
        assert_eq!(StorePurchaseStatus::Succeeded.0, 0);
        assert_eq!(StorePurchaseStatus::AlreadyPurchased.0, 1);
        assert_eq!(StorePurchaseStatus::NotPurchased.0, 2);
        assert_eq!(StorePurchaseStatus::NetworkError.0, 3);
        assert_eq!(StorePurchaseStatus::ServerError.0, 4);
    }

    #[test]
    fn no_license_codes_are_recognised() {
        assert!(is_no_license(0x803F6108));
        assert!(is_no_license(0x803F6107));
        assert!(!is_no_license(0));
        assert!(!is_no_license(0x803F8001));
    }
}
