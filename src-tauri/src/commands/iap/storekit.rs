//! StoreKit 2 implementation for the Mac App Store build.
//!
//! `storekit-rs` exposes the Swift `StoreKit` APIs synchronously; every public
//! function here is therefore called from a blocking worker (`spawn_blocking`
//! in the parent module or a dedicated listener thread), never from Tauri's
//! main thread. `Product.purchase` in particular presents Apple's purchase
//! sheet and returns `NotSupported` if invoked on the main thread.

use std::time::Duration;

use storekit::{AppStore, Product, PurchaseResult, StoreKitError, Transaction};
use tauri::AppHandle;

use super::{grandfather, ProProduct, ProStatus, PRO_PRODUCT_ID};

fn error_to_string(error: StoreKitError) -> String {
    error.to_string()
}

/// True when StoreKit reports a verified, non-revoked transaction for
/// `PRO_PRODUCT_ID`. This is the only source of paid Pro entitlement.
pub fn has_entitlement() -> Result<bool, String> {
    let mut stream =
        Transaction::current_entitlements_for(PRO_PRODUCT_ID).map_err(error_to_string)?;

    loop {
        match stream.next_timeout(Duration::from_millis(1_500)) {
            Ok(Some(result)) => {
                // A signed-in customer can have an old revoked transaction;
                // `currentEntitlements` should omit it, but checking the field
                // makes the rule explicit and testable.
                if let Ok(transaction) = result.payload_value() {
                    let data = transaction.data();
                    if data.product_id == PRO_PRODUCT_ID && data.revocation_date.is_none() {
                        return Ok(true);
                    }
                }
            }
            Ok(None) => return Ok(false),
            // The stream is finite for `currentEntitlements`; a timeout means
            // StoreKit has no more values to hand us right now.
            Err(StoreKitError::TimedOut(_)) => return Ok(false),
            Err(error) => return Err(error_to_string(error)),
        }
    }
}

/// The current entitlement, including the grandfathering rule. Called on
/// launch, after a purchase and after a restore.
pub fn refresh_status(app: &AppHandle) -> Result<ProStatus, String> {
    if has_entitlement()? {
        return Ok(ProStatus::pro("app_store"));
    }

    if grandfather::is_grandfathered(app) {
        return Ok(grandfather::status());
    }

    Ok(ProStatus::free())
}

/// The localized product metadata used by the upgrade dialog. Product lookup
/// is allowed to fail while App Store Connect metadata is still being filled
/// in; the caller reports that as "pricing unavailable" rather than as a
/// reason not to use the free tier.
pub fn product_info() -> Result<Option<ProProduct>, String> {
    let products = Product::products_for([PRO_PRODUCT_ID]).map_err(error_to_string)?;
    let Some(product) = products.into_iter().next() else {
        return Ok(None);
    };

    Ok(Some(ProProduct {
        id: product.id,
        display_name: product.display_name,
        description: product.description,
        display_price: product.display_price,
        currency_code: product.currency_code,
    }))
}

/// Start a purchase. `UserCancelled` is a normal outcome, not an error; a
/// pending Ask-to-Buy transaction leaves the app in the pending state until
/// `Transaction.updates` delivers the approved transaction.
pub fn purchase() -> Result<ProStatus, String> {
    let products = Product::products_for([PRO_PRODUCT_ID]).map_err(error_to_string)?;
    let Some(product) = products.into_iter().next() else {
        return Err("PRODUCT_UNAVAILABLE".to_string());
    };

    match product.purchase(&[]).map_err(error_to_string)? {
        PurchaseResult::Success(verification) => {
            let transaction = verification
                .payload_value()
                .map_err(|error| format!("purchase verification failed: {error}"))?;

            if transaction.data().product_id != PRO_PRODUCT_ID {
                return Err("unexpected product in purchase result".to_string());
            }

            // Grant before finishing: if the process dies after `finish` but
            // before the in-memory state is updated, the next launch still sees
            // the transaction through `currentEntitlements`.
            let status = ProStatus::pro("app_store");
            if let Err(error) = transaction.finish() {
                // The purchase already succeeded; leaving the transaction
                // unfinished only makes StoreKit deliver it again next launch.
                tracing::warn!(error = %error, "could not finish StoreKit transaction");
            }
            Ok(status)
        }
        PurchaseResult::UserCancelled => Ok(super::current()),
        PurchaseResult::Pending => Ok(ProStatus {
            status: "pending".to_string(),
            is_pro: false,
            max_queue_files: Some(super::FREE_MAX_QUEUE_FILES),
            ..ProStatus::free()
        }),
    }
}

/// "Restore Purchases": force StoreKit to sync with the App Store, then read
/// current entitlements again. This is the path Apple requires for a
/// non-consumable purchase on a new Mac or after reinstalling.
pub fn restore(app: &AppHandle) -> Result<ProStatus, String> {
    AppStore::sync().map_err(error_to_string)?;
    refresh_status(app)
}

/// Long-lived `Transaction.updates` listener.
///
/// It catches purchases completed outside this process (Ask to Buy approval,
/// Family Sharing, a purchase interrupted by app termination) and refunds /
/// revocations. It must be started once at launch and kept alive for the whole
/// process; dropping the stream cancels the listener.
pub fn start_update_listener(app: AppHandle) {
    std::thread::Builder::new()
        .name("monosub-iap-updates".to_string())
        .spawn(move || {
            let mut stream = match Transaction::updates() {
                Ok(stream) => stream,
                Err(error) => {
                    tracing::warn!(error = %error, "could not start StoreKit transaction listener");
                    return;
                }
            };

            loop {
                match stream.next() {
                    Ok(Some(result)) => {
                        match result.payload_value() {
                            Ok(transaction) => {
                                let data = transaction.data();
                                let status = if data.product_id == PRO_PRODUCT_ID {
                                    if data.revocation_date.is_none() {
                                        ProStatus::pro("app_store")
                                    } else {
                                        // A refund/revocation must downgrade even
                                        // if the in-memory snapshot still says Pro.
                                        ProStatus::free()
                                    }
                                } else {
                                    super::current()
                                };

                                let status = super::replace(status);
                                super::emit(&app, &status);

                                // Finish both purchases and revoked transactions:
                                // the latter must not stay in the queue forever.
                                if let Err(error) = transaction.finish() {
                                    tracing::warn!(error = %error, "could not finish StoreKit transaction");
                                }
                            }
                            Err(error) => {
                                tracing::warn!(error = %error, "ignoring an unverified StoreKit transaction");
                            }
                        }
                    }
                    Ok(None) => {
                        tracing::info!("StoreKit transaction listener ended");
                        break;
                    }
                    Err(error) => {
                        tracing::warn!(error = %error, "StoreKit transaction listener stopped");
                        break;
                    }
                }
            }
        })
        .expect("failed to spawn StoreKit transaction listener");
}
