# Microsoft Store Durable Add-on IAP — Rust/Tauri 2.12 API Research

Target: MonoSub, Tauri **2.12.4**, Rust + React, shipped as an MSIX **Packaged Win32** app
(package identity, non-elevated) on Windows 11 x64.

Goal: a free download whose durable add-on unlocks a Pro capability. We need (a) read ownership,
(b) read display name / price, (c) launch the purchase UI, (d) handle status. Hand-rolled with the
`windows` crate, gated behind Cargo feature `msstore` and
`#[cfg(all(target_os = "windows", feature = "msstore"))]`.

> This report mixes **Microsoft Learn prose** with **facts read from the exact crate source of the
> pinned `windows` 0.62.2 / `windows-collections` 0.3.2 / `windows-future` 0.3.2** that the project
> already resolves, and with a **compile harness** I built and ran (see §9). Where something could not
> be verified it is marked **UNVERIFIED**. Two of the user's assumptions turned out to be wrong and are
> corrected below (§4.3 and §4.4).

---

## 0. TL;DR — the decisions that matter

1. **Use `windows = "=0.62.2"`** (plus `windows-collections = "=0.3.2"` and `windows-future = "=0.3.2"`).
   Tauri 2.12.4 depends on `windows = "0.62"` → resolves to **0.62.2**, and
   `tauri::WebviewWindow::hwnd()` returns `windows::Win32::Foundation::HWND` **from windows 0.62.2**.
   Using 0.61.3 would give a *different nominal type* → type error when passing Tauri's HWND. (§1)
2. `IInitializeWithWindow` lives at **`windows::Win32::UI::Shell::IInitializeWithWindow`** (feature
   **`Win32_UI_Shell`**) — *not* `windows::UI::Shell`. (§2)
3. **`IAsyncOperation` has NO `.get()` in windows 0.62 / windows-future 0.3.2.** The blocking wait is
   **`.join()`**; the async form is `.await` (`IntoFuture`). `.get()` belongs to an older windows-future.
   This is the single biggest API correction. (§4.3)
4. **You cannot pass `&[HSTRING]` / `&Vec<HSTRING>`** to `GetUserCollectionAsync` / `GetStoreProductsAsync`.
   The parameter is `Param<IIterable<HSTRING>>`; build
   `windows_collections::IIterable::from(vec![HSTRING::from("Durable")])` and pass `&it`.
   (Verified: `&[HSTRING]` fails to compile.) (§4.4)
5. **`0x803F6107` is NOT the "no license" code.** It is the generic *"Something unexpected happened"*.
   The **sideloaded / dev-cert / no-retail-license** code is **`0x803F6108`** ("app license is missing
   information… uninstall and reinstall from the Store"). **`0x80070578`** (`ERROR_INVALID_WINDOW_HANDLE`)
   means *you did not call `RequestPurchaseAsync` on the UI thread / did not set the owner HWND*.
   **`0x803F8001`** means *no entitlement for that product* (genuinely not owned). (§7)

---

## 1. Which `windows` crate version — and why it must be 0.62.2

### Evidence

`src-tauri/Cargo.lock` (read directly):

- `tauri` **2.12.4** (`Cargo.lock:3889`) depends on `"windows 0.62.2"` (`Cargo.lock:3936`).
- `windows` **0.62.2** (`Cargo.lock:5271`) and `windows` **0.61.3** (`Cargo.lock:5258`) both exist.
- `windows 0.61.3` is pulled in **only** by `enigo 0.6.1` (`Cargo.lock:907-930`, used by the
  debug-only `tauri-pilot` test plugin). It is **not** in Tauri's runtime path.
- `tauri-runtime-wry`, `wry`, `webview2-com 0.39.1`, `webview2-com-sys` all depend on
  `"windows 0.62.2"` (`Cargo.lock:5163,5185,4179,5761`).

`tauri` 2.12.4 crate source (`~/.cargo/registry/src/.../tauri-2.12.4`):

- `Cargo.toml:420-428`: `[target."cfg(windows)".dependencies.windows] version = "0.62"` with features
  `Win32_Foundation, Win32_UI, Win32_UI_WindowsAndMessaging, Win32_System_LibraryLoader, Win32_Graphics_Gdi`.
- `src/webview/webview_window.rs:43`: `use windows::Win32::Foundation::HWND;`
- `src/webview/webview_window.rs:1964-1968`:
  ```rust
  /// Returns the native handle that is used by this window.
  #[cfg(windows)]
  pub fn hwnd(&self) -> crate::Result<HWND> { self.window.hwnd() }
  ```

So the HWND type returned by `window.hwnd()` is **`windows 0.62.2`'s `Win32::Foundation::HWND`**.

### Implication

Cargo compiles each `windows` major/minor as a **separate crate instance**, so `0.61.3::HWND` and
`0.62.2::HWND` are distinct nominal types even though both are `#[repr(transparent)] struct HWND(pub *mut c_void)`.
If our module used 0.61.3, `IInitializeWithWindow::Initialize(hwnd_from_tauri)` would not typecheck
(it needs `0.62.2::HWND`). **Pin `=0.62.2`; Cargo unifies it with Tauri's copy → one `HWND` type,
zero conversion code.** (Both versions define `HWND(pub *mut core::ffi::c_void)` — the user's note is
correct — but the nominal identity is what matters.)

---

## 2. Cargo.toml dependency snippet

```toml
# src-tauri/Cargo.toml
[target.'cfg(target_os = "windows")'.dependencies]
# MUST equal Tauri 2.12's windows (0.62.2) so its HWND is bit-for-bit the same type.
windows = { version = "=0.62.2", optional = true, features = [
    "Services_Store",              # Windows.Services.Store.* (StoreContext, StoreProduct, …)
    "Foundation",                  # IAsyncOperation / DateTime / collections glue
    "Foundation_Collections",      # IMapView / IIterable usage in Store signatures
    "Win32_Foundation",            # HWND
    "Win32_UI_Shell",              # IInitializeWithWindow  <-- the shell COM interface
    "Win32_System_WinRT",          # RoInitialize (only if you call Store from a non-UI thread)
    # "Win32_UI_WindowsAndMessaging" is NOT needed for these calls; harmless if you add it.
] }
# Direct deps so we can *name* the collection / async types the Store APIs return:
windows-collections = { version = "=0.3.2", optional = true }   # IIterable, IMapView, IKeyValuePair
windows-future      = { version = "=0.3.2", optional = true }   # IAsyncOperation

[features]
msstore = ["dep:windows", "dep:windows-collections", "dep:windows-future"]
default = ["tauri-app"]           # keep existing; msstore is opt-in
```

Notes:

- **`Services_Store = ["Services"]`** and **`Services = ["Foundation"]`** in windows 0.62.2's
  `Cargo.toml` (lines 276 / verified), so `Services_Store` transitively enables `Foundation`.
  `Foundation_Collections` is listed explicitly for clarity.
- **`IInitializeWithWindow` is a shell COM interface** and is exposed only with feature
  **`Win32_UI_Shell`**. Module path in windows 0.62.2:
  **`windows::Win32::UI::Shell::IInitializeWithWindow`**. It is *not* under `windows::UI::Shell`
  (that path does not exist in this crate). Source:
  `windows-0.62.2/src/Windows/Win32/UI/Shell/mod.rs:27249` `define_interface!(IInitializeWithWindow, …)`.
- `HWND` type: `windows::Win32::Foundation::HWND` (feature `Win32_Foundation`).
- Do **not** put `optional = true` on the deps if you instead gate with `[target.'cfg(windows)'.dependencies]`
  and `#[cfg(feature="msstore")]` only in code — either style works; the snippet above uses optional deps
  so `msstore` fully controls compilation.

---

## 3. Exact type paths & method signatures (windows 0.62.2 — read from source)

All under `windows::Services::Store` unless noted. Signatures below are copied verbatim from
`windows-0.62.2/src/Windows/Services/Store/mod.rs` (line numbers in parentheses).

| API | Exact Rust signature (0.62.2) |
|---|---|
| `StoreContext::GetDefault` | `pub fn GetDefault() -> windows_core::Result<StoreContext>` (1351) — **no args** |
| Cast to shell interop | `let interop: IInitializeWithWindow = ctx.cast()?;` (trait `windows::core::Interface`) |
| `IInitializeWithWindow::Initialize` | `pub unsafe fn Initialize(&self, hwnd: windows::Win32::Foundation::HWND) -> windows_core::Result<()>` (Win32/UI/Shell/mod.rs:27252) |
| `GetUserCollectionAsync` | `pub fn GetUserCollectionAsync<P0>(&self, productkinds: P0) -> Result<windows_future::IAsyncOperation<StoreProductQueryResult>> where P0: Param<windows_collections::IIterable<HSTRING>>` (1065) |
| `GetStoreProductsAsync` | `pub fn GetStoreProductsAsync<P0,P1>(&self, productkinds: P0, storeids: P1) -> Result<IAsyncOperation<StoreProductQueryResult>> where P0: Param<IIterable<HSTRING>>, P1: Param<IIterable<HSTRING>>` (1034) |
| `StoreContext::RequestPurchaseAsync` | `pub fn RequestPurchaseAsync(&self, storeid: &HSTRING) -> Result<IAsyncOperation<StorePurchaseResult>>` (1110) |
| `StoreProductQueryResult::Products` | `pub fn Products(&self) -> Result<IMapView<HSTRING, StoreProduct>>` (2018) |
| `StoreProductQueryResult::ExtendedError` | `pub fn ExtendedError(&self) -> Result<windows_core::HRESULT>` (2025) |
| `StoreProduct::StoreId` | `pub fn StoreId(&self) -> Result<HSTRING>` (1793) |
| `StoreProduct::Title` | `pub fn Title(&self) -> Result<HSTRING>` (1807) |
| `StoreProduct::Description` | `pub fn Description(&self) -> Result<HSTRING>` (1814) |
| `StoreProduct::ProductKind` | `pub fn ProductKind(&self) -> Result<HSTRING>` (1821) |
| `StoreProduct::IsInUserCollection` | `pub fn IsInUserCollection(&self) -> Result<bool>` (1863) |
| `StoreProduct::Price` | `pub fn Price(&self) -> Result<StorePrice>` (1870) |
| `StoreProduct::InAppOfferToken` | `pub fn InAppOfferToken(&self) -> Result<HSTRING>` (1915) |
| `StorePrice::FormattedPrice` | `pub fn FormattedPrice(&self) -> Result<HSTRING>` (1719) |
| `StorePrice::CurrencyCode` | `pub fn CurrencyCode(&self) -> Result<HSTRING>` (1740) |
| `StorePurchaseResult::Status` | `pub fn Status(&self) -> Result<StorePurchaseStatus>` (2139) |
| `StorePurchaseResult::ExtendedError` | `pub fn ExtendedError(&self) -> Result<HRESULT>` (2146) |
| `StoreAppLicense::AddOnLicenses` | `pub fn AddOnLicenses(&self) -> Result<IMapView<HSTRING, StoreLicense>>` (678) |
| `StoreLicense::SkuStoreId` / `IsActive` | `Result<HSTRING>` (1459) / `Result<bool>` (1466) |
| `StoreContext::GetAppLicenseAsync` | `pub fn GetAppLicenseAsync(&self) -> Result<IAsyncOperation<StoreAppLicense>>` (1020) |

`StorePurchaseStatus` is a **newtype over i32 with associated consts** (NOT a Rust `enum`), so you
compare with `==`, not `match`-by-variant (source 2166-2175; learn docs table below):

```rust
pub struct StorePurchaseStatus(pub i32);
impl StorePurchaseStatus {
    pub const Succeeded: Self = Self(0);        // purchase succeeded
    pub const AlreadyPurchased: Self = Self(1); // user already owns it
    pub const NotPurchased: Self = Self(2);     // did not succeed (user cancelled / failed)
    pub const NetworkError: Self = Self(3);     // network connectivity
    pub const ServerError: Self = Self(4);      // Store server error
}
```

`IMapView<K,V>` (from `windows_collections`) supports `Lookup(key) -> Result<V>`, `HasKey`, `Size`,
`First()`, and **`IntoIterator`** for both owned and `&` refs, yielding `IKeyValuePair<K,V>` which has
`Key() -> Result<K>` and `Value() -> Result<V>`. So you iterate with
`for pair in &products { pair.Key()?; pair.Value()?; }`.

---

## 4. Reading ownership + product info — the important gotchas

### 4.1 Ownership

Two supported ways. **Recommended for a durable add-on: `GetAppLicenseAsync().AddOnLicenses()`**
(one round trip, gives an explicit `IsActive` flag per add-on, keyed by the add-on's *Store ID / SKU*).
The method the user asked for, **`GetUserCollectionAsync([\"Durable\"])`**, also works — it returns only
products the user has an entitlement to, so *presence ⇒ owned, absence ⇒ not owned*.

### 4.2 Display name / price

`GetStoreProductsAsync(&[kinds], &[storeIds])` → `StoreProductQueryResult`. Iterate `Products()` or
`Lookup(storeId)`, then `Title()`, `Description()`, and `Price()` → `StorePrice` with
`FormattedPrice()` (localised, e.g. `"¥48.00"`) and `CurrencyCode()` (e.g. `"CNY"`). `FormattedPrice`
is already localised to the user's market; you normally display it verbatim.

### 4.3 CORRECTION — there is no `.get()` on `IAsyncOperation` in this version

In **windows-future 0.3.2** (what windows 0.62.2 uses) the async types implement `IntoFuture` and
provide **`join()`**, but **no `get()`**:

- `windows-future-0.3.2/src/join.rs`: `impl<T: RuntimeType> IAsyncOperation<T> { pub fn join(&self) -> Result<T> }`
- `windows-future-0.3.2/src/future.rs`: `impl<T> IntoFuture for IAsyncOperation<T> { … }` (so `.await` works)
- `bindings.rs`: `pub fn GetResults(&self) -> Result<TResult>` (low-level; you don't call this directly)

So: **blocking = `.join()`**; **async = `.await`**. (`.get()` was the name in an earlier windows-future;
**UNVERIFIED** for 0.2.1 specifically, but irrelevant — we pin 0.62.2 where the name is `join()`.)

### 4.4 CORRECTION — you must build an `IIterable`, not a slice

`GetUserCollectionAsync`/`GetStoreProductsAsync` take `P0: Param<IIterable<HSTRING>>`. In windows 0.62.2
**neither `&[HSTRING]` nor `&Vec<HSTRING>` implements that bound** (I compiled both: they fail with
`&[HSTRING]: Param<IIterable<HSTRING>, InterfaceType> is not satisfied`). The correct call is:

```rust
use windows_collections::IIterable;
let kinds: IIterable<windows::core::HSTRING> =
    IIterable::from(vec![windows::core::HSTRING::from("Durable")]);
let result = ctx.GetUserCollectionAsync(&kinds)?.join()?;
```

`IIterable<T>` has `impl<T: RuntimeType> From<Vec<T::Default>>`, and `HSTRING::Default == HSTRING`, so
`IIterable::from(vec![HSTRING])` is the canonical builder (`windows-collections-0.3.2/src/iterable.rs:86`).
Product-kind strings recognised by the Store are `"Durable"` for a durable add-on (`"Consumable"`,
`"UnmanagedConsumable"`, `"Application"` also exist).

---

## 5. The UI-thread problem — the concrete, safe pattern

### What Microsoft requires

From `StoreContext.RequestPurchaseAsync` (learn, both overloads):

> **Important** This method must be called on the UI thread.
> **Important** In-app purchase functionality is not supported in elevated applications.
> … if the **ErrorCode** is **0x80070578 (ERROR_INVALID_WINDOW_HANDLE)**, this indicates the method was
> not called on the UI thread. If you are calling this in a desktop application that uses the Desktop
> Bridge, this can also indicate that you did not configure the object to specify which application
> window is the owner window for modal dialogs shown by this method.

And from `Windows.Services.Store` namespace page:

> APIs in this namespace require a valid retail license issued by the Microsoft Store. A developer
> license is not a retail license and is not supported; using one may result in errors or crashes.
> Apps that declare the `appLicensing` restricted capability may launch without a license present
> locally; the system acquires one automatically when StoreContext is initialized.

So a desktop (Packaged Win32) app must:
1. get the real `HWND` of its window,
2. `ctx.cast::<IInitializeWithWindow>()?.Initialize(hwnd)?` **before** any call that shows Store UI, and
3. call `RequestPurchaseAsync` on the **UI thread**.

### Getting the HWND in Tauri

```rust
// tauri::WebviewWindow (or Window) — windows-only method
let hwnd: windows::Win32::Foundation::HWND = window.hwnd()?;  // exact 0.62.2 HWND, no conversion
```
`window.hwnd()` returns `crate::Result<HWND>` (tauri-2.12.4 `webview_window.rs:1966`). Because we pin
`windows = "=0.62.2"`, this is **the same type** our module imports — pass it straight in.

### Option A — do the whole call on the UI thread, blocking with `.join()`

`app_handle.run_on_main_thread(move || { … ctx.RequestPurchaseAsync(&id)?.join()? … })`.
This satisfies "UI thread", **but `.join()` parks the main thread** (the Tauri/Win32 event loop) for the
entire time the purchase dialog is open. The Store's payment UI is a separate process, so it may still
complete, but MonoSub's own window freezes and cannot pump messages for the modal's **owner** window —
fragile, and Microsoft's guidance is not to block the UI thread. **Not recommended for production.**

### Option B — initiate on the UI thread, `join()` on a background thread (SAFE — recommended)

`RequestPurchaseAsync` itself returns **immediately** with a `Send + Sync` `IAsyncOperation` (the
generated type is `unsafe impl Send/Sync`, `windows-future` `bindings.rs:1753`). So:

1. On the UI thread (`run_on_main_thread`): `GetDefault` → `IInitializeWithWindow::Initialize(hwnd)` →
   `RequestPurchaseAsync(&id)` → send `(StoreContext, IAsyncOperation<StorePurchaseResult>)` back over a
   channel. This closure does **no blocking** — it returns in microseconds.
2. On the command's (worker) thread: `recv()` the pair, **`RoInitialize(RO_INIT_MULTITHREADED)`** to give
   the worker thread a COM apartment, then `op.join()?`. The UI thread is free to pump messages the whole
   time.

Why B is safe: WinRT async completion for agile operations is delivered on a thread-pool thread, and
`Async::join` just sets a `Completed` handler and waits on an event — so it may block *any* thread that
has an apartment. Initiation still happens on the UI thread with the owner HWND attached, which is
exactly what the ownership/modal requirement demands.

**Apartment note (important):** The Tauri main thread is already COM-initialized (STA) because WebView2
owns it. **Any other thread that touches WinRT** — including the thread that calls `join()` and the
`Completed` handler setup — must first initialize COM. Use
`unsafe { windows::Win32::System::WinRT::RoInitialize(RO_INIT_MULTITHREADED) }` (feature
`Win32_System_WinRT`); `CO_E_NOTINITIALIZED`/`RPC_E_*` failures otherwise are the symptom. (`RoInitialize`
on an already-initialized MTA returns `S_FALSE`/`RPC_E_CHANGED_MODE`; tolerate/ignore that.)

Read-only calls (`GetDefault`, `GetUserCollectionAsync`, `GetStoreProductsAsync`, `GetAppLicenseAsync`)
do **not** show UI and do **not** require the UI thread — run them on any COM-initialized thread
(initialise MTA there).

---

## 6. Two constraints confirmed

| Constraint | Status for MonoSub | Implication |
|---|---|---|
| "In-app purchase functionality is not supported in elevated applications." (Learn: RequestPurchaseAsync, In-app purchases and trials, StoreContext) | MonoSub runs **non-elevated** (MSIX packaged Win32) → OK | Never elevate the process for IAP. If a future flow needs admin, do it in a separate elevated step, never around `RequestPurchaseAsync`. |
| Requires **package identity** (Learn: namespace + StoreContext remarks; "desktop apps that use the Desktop Bridge") | MSIX gives identity → OK | `StoreContext::GetDefault()` needs package identity; NSIS `release:exe` build (no identity) will fail these calls — so gate IAP on `msstore`, not on the plain `.exe` build. |
| "APIs require a valid **retail** license; a developer license is not a retail license and is not supported." | Dev/side-loaded MSIX with a test cert has **no retail license** → these calls error | Must degrade gracefully (see §7). |

---

## 7. Error handling for non-Store / unlicensed contexts (and how to tell it apart)

### Error codes (Learn: "Error codes for Store operations")

| HRESULT | Meaning (verbatim) |
|---|---|
| `0x80070578` | `ERROR_INVALID_WINDOW_HANDLE` — *not called on UI thread* **or** *no owner HWND set on StoreContext* (from RequestPurchaseAsync remarks). |
| `0x803F6107` | **"Something unexpected happened while processing the current operation."** — generic failure (NOT the license code). |
| `0x803F6108` | **"The in-app purchase could not be completed because the app license is missing information. This error can occur when you side-load your app. To resolve this, uninstall the app and then reinstall it from the Store to refresh the app license."** ← this is the **dev-cert / no retail license** case. |
| `0x803F8001` | "You do not have an entitlement for the product. You might be using a different Microsoft account than the one that was used to purchase the product." ← **genuinely not owned**. |
| `0x803F6101/6102/6103` | app / add-on / product **not found** (wrong or not-yet-published Store ID). |
| `0x803F700C` | device offline (offline permissions not enabled). |

`GetUserCollectionAsync`'s own value when there is no license to read is the `0x803F6108` family; when
the user simply doesn't own the durable add-on, the call **succeeds** and simply returns an **empty map**
(or an absent key when you `Lookup`).

### How to distinguish the three states

1. **Not a licensed Store install / side-loaded dev cert** → `StoreContext::GetDefault()` or the first
   async call fails, or `StoreProductQueryResult::ExtendedError()` is non-zero (`0x803F6108`,
   `0x803F6107`, `0x803F6101/2/3`). Treat as **Free / unlicensed**, log a warning, **never crash / never
   block the app**.
2. **Licensed install, user does NOT own the add-on** → `GetDefault` + query succeed, `ExtendedError ==
   0`, collection/`AddOnLicenses` is **empty** (or key absent). Treat as **Free** — this is the normal
   "show the Buy button" state.
3. **Owned** → product present / `StoreLicense::IsActive() == true` → **Pro**.

### Robust predicate

```rust
fn classify(err_code: u32) -> &'static str {
    match err_code {
        0 => "ok",
        0x803F6108 | 0x803F6107 => "no-retail-license",   // side-loaded / dev cert -> treat as Free
        0x803F6101 | 0x803F6102 | 0x803F6103 => "product-not-found", // add-on not published / wrong id
        0x803F8001 => "not-entitled",                    // genuinely not owned
        0x80070578 => "wrong-thread-or-hwnd",            // bug in our plumbing, not a user state
        _ => "other",
    }
}
```

Design rule for MonoSub: `has_entitlement()` returns `Ok(false)` (not `Err`) for the **no-retail-license**
family, so the app simply shows the free tier and the Buy button; it returns `Err(String)` only for bugs
like `0x80070578` that we want surfaced in logs. `product_info()` returns `Ok(None)` when the add-on
can't be resolved (don't hard-code a price fallback that lies). `purchase()` maps `0x803F6108` to a
user-facing "This build isn't a Store install; purchases are unavailable" message.

---

## 8. Verification harness (what I actually compiled and ran)

Toolchain: `rustc 1.99.1`, `cargo 1.99.1`, offline build against the exact crates already in the
project's registry cache. Harness crate at `%TEMP%\winiap_test`, deps
`windows = "=0.62.2"` (features above) + `windows-collections = "=0.3.2"` + `windows-future = "=0.3.2"`.

Confirmed compiling (`cargo build --offline` → exit 0) for:

- `StoreContext::GetDefault()`, `.cast::<IInitializeWithWindow>()`, `unsafe { interop.Initialize(hwnd) }`
- `IIterable::from(vec![HSTRING])` + `ctx.GetUserCollectionAsync(&it)?.join()?`
- `r.Products()?` iterate via `for pair in &products { pair.Key()?, pair.Value()? }`
- `products.Lookup(&store_id)?`; `p.Price()?.FormattedPrice()?`, `.CurrencyCode()?`, `p.Title()?`,
  `p.Description()?`, `p.IsInUserCollection()?`, `p.InAppOfferToken()?`, `p.StoreId()?`
- `ctx.GetStoreProductsAsync(&kinds, &ids)?.join()?`
- `ctx.RequestPurchaseAsync(&id)?.join()?`, `res.Status()?`, `res.ExtendedError()`
- `windows::Win32::System::WinRT::RoInitialize(RO_INIT_MULTITHREADED)` (feature `Win32_System_WinRT`)
- `Error::code().0 as u32` / `Error::message()` for printf-style HRESULT logging

Confirmed **failing** (so do not write it this way): `ctx.GetUserCollectionAsync(&[HSTRING])` /
`&Vec<HSTRING>` → `E0277: &[HSTRING]: Param<IIterable<HSTRING>, InterfaceType> is not satisfied`.

Not tested here (environment-limited): the Tauri `run_on_main_thread` glue and an end-to-end purchase —
those need a Store-signed MSIX with a retail license and Partner-Center-published add-on (§7).

---

## 9. Rust module skeleton (matches windows 0.62.2)

```rust
// src-tauri/src/iap/msstore.rs  (compiled only for this target+feature)
#![cfg(all(target_os = "windows", feature = "msstore"))]

use windows::core::{Error, HSTRING, Interface};
use windows::Services::Store::{
    StoreContext, StoreLicense, StoreProduct, StorePurchaseResult, StorePurchaseStatus,
};
use windows::Win32::Foundation::HWND;
use windows::Win32::System::WinRT::{RoInitialize, RO_INIT_MULTITHREADED};
use windows::Win32::UI::Shell::IInitializeWithWindow;
use windows_collections::IIterable;

// --- Partner Center identifiers ---------------------------------------------
// ADDON_STORE_ID = the add-on's "Store ID" (12-char, e.g. 9P…): what RequestPurchaseAsync wants.
// OFFER_TOKEN    = the in-app offer token you typed in Partner Center (e.g. "monosub_pro"):
//                  an extra stable matcher for the same product, exposed by StoreProduct::InAppOfferToken().
const ADDON_STORE_ID: &str = "9PG23VS4QX3G"; // <-- put the real add-on Store ID here
const OFFER_TOKEN:    &str = "monosub_pro";  // <-- the add-on's in-app offer token
const ADDON_KIND:     &str = "Durable";

#[derive(Debug, Clone)]
pub struct ProductInfo {
    pub store_id: String,
    pub title: String,
    pub description: String,
    pub price: String,     // StorePrice::FormattedPrice(), already localised: "¥48.00"
    pub currency: String,  // StorePrice::CurrencyCode(): "CNY"
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PurchaseOutcome {
    Purchased,
    AlreadyPurchased,
    NotPurchased,
    NetworkError,
    ServerError,
    NotAvailable(String), // e.g. this process has no retail Store license
}

// --- helpers ----------------------------------------------------------------
fn hstr(s: &str) -> HSTRING { HSTRING::from(s) }
fn iter1(s: &str) -> IIterable<HSTRING> { IIterable::from(vec![hstr(s)]) }
fn hres(e: &Error) -> String { format!("0x{:08X}: {}", e.code().0 as u32, e.message()) }
fn ext_code(h: windows::core::HRESULT) -> u32 { h.0 as u32 }

/// Give the current thread a COM apartment (MTA). No-op if already initialized.
fn ensure_com_mta() {
    // S_FALSE / RPC_E_CHANGED_MODE are fine: another apartment already exists.
    let _ = unsafe { RoInitialize(RO_INIT_MULTITHREADED) };
}

fn is_no_license(code: u32) -> bool {
    matches!(code, 0x803F6108 | 0x803F6107)
}

// --- (a) ownership ----------------------------------------------------------
/// True iff the durable add-on is owned. `Ok(false)` also covers "not a licensed Store install"
/// so the app degrades to Free instead of erroring.
pub fn has_entitlement() -> Result<bool, String> {
    ensure_com_mta();
    let ctx = StoreContext::GetDefault().map_err(|e| hres(&e))?;

    // Preferred: one call, explicit IsActive flag per add-on (keyed by add-on Store ID).
    match ctx.GetAppLicenseAsync().and_then(|op| op.join()) {
        Ok(lic) => {
            let addons = lic.AddOnLicenses().map_err(|e| hres(&e))?;
            for kv in &addons {
                let sku = kv.Key().map(|k| k.to_string()).unwrap_or_default();
                let l: StoreLicense = kv.Value().map_err(|e| hres(&e))?;
                if sku == ADDON_STORE_ID && l.IsActive().unwrap_or(false) {
                    return Ok(true);
                }
            }
            Ok(false)
        }
        Err(e) if is_no_license(e.code().0 as u32) => {
            tracing::warn!("Store: no retail license (0x{:08X}); treating as Free", e.code().0 as u32);
            Ok(false)
        }
        Err(e) => Err(hres(&e)),
    }
}

/// Optional: the GetUserCollectionAsync flavour the ticket asked about (equivalent truth value).
pub fn has_entitlement_via_collection() -> Result<bool, String> {
    ensure_com_mta();
    let ctx = StoreContext::GetDefault().map_err(|e| hres(&e))?;
    let r = ctx.GetUserCollectionAsync(&iter1(ADDON_KIND))
        .map_err(|e| hres(&e))?
        .join()
        .map_err(|e| hres(&e))?;
    let code = r.ExtendedError().map(ext_code).unwrap_or(0);
    if is_no_license(code) { tracing::warn!("Store: no retail license 0x{code:08X}"); return Ok(false); }
    if code != 0 { return Err(format!("GetUserCollectionAsync ExtendedError 0x{code:08X}")); }
    let products = r.Products().map_err(|e| hres(&e))?;
    for kv in &products {
        let p: StoreProduct = kv.Value().map_err(|e| hres(&e))?;
        let owned_by_id  = p.StoreId().map(|s| s.to_string()).unwrap_or_default() == ADDON_STORE_ID;
        let owned_by_tok = p.InAppOfferToken().map(|t| t.to_string()).unwrap_or_default() == OFFER_TOKEN;
        if (owned_by_id || owned_by_tok) && p.IsInUserCollection().unwrap_or(false) {
            return Ok(true);
        }
    }
    Ok(false)
}

// --- (b) display name / price ----------------------------------------------
pub fn product_info() -> Result<Option<ProductInfo>, String> {
    ensure_com_mta();
    let ctx = StoreContext::GetDefault().map_err(|e| hres(&e))?;
    let r = ctx.GetStoreProductsAsync(&iter1(ADDON_KIND), &iter1(ADDON_STORE_ID))
        .map_err(|e| hres(&e))?
        .join()
        .map_err(|e| hres(&e))?;
    let code = r.ExtendedError().map(ext_code).unwrap_or(0);
    if is_no_license(code) { tracing::warn!("Store: no retail license 0x{code:08X}"); return Ok(None); }
    if code != 0 { return Err(format!("GetStoreProductsAsync ExtendedError 0x{code:08X}")); }

    let products = r.Products().map_err(|e| hres(&e))?;
    let mut found: Option<StoreProduct> = None;
    for kv in &products {
        if kv.Key().map(|k| k.to_string()).unwrap_or_default() == ADDON_STORE_ID {
            found = Some(kv.Value().map_err(|e| hres(&e))?);
            break;
        }
    }
    let Some(p) = found else { return Ok(None) }; // add-on not resolvable (not published / wrong id)

    let price = p.Price().map_err(|e| hres(&e))?;
    Ok(Some(ProductInfo {
        store_id:    ADDON_STORE_ID.to_string(),
        title:       p.Title().map(|h| h.to_string()).unwrap_or_default(),
        description: p.Description().map(|h| h.to_string()).unwrap_or_default(),
        price:       price.FormattedPrice().map(|h| h.to_string()).unwrap_or_default(),
        currency:    price.CurrencyCode().map(|h| h.to_string()).unwrap_or_default(),
    }))
}

// --- (c) purchase ----------------------------------------------------------
/// Initiate on the Tauri UI thread (owner HWND attached), await on a worker thread.
/// `app`/`window` are the Tauri handles; HWND comes from `window.hwnd()`.
pub fn purchase(
    app: &tauri::AppHandle,
    window: &tauri::WebviewWindow,
) -> Result<PurchaseOutcome, String> {
    let hwnd: HWND = window.hwnd().map_err(|e| e.to_string())?;
    let store_id = hstr(ADDON_STORE_ID);

    type Op = windows_future::IAsyncOperation<StorePurchaseResult>;
    let (tx, rx) = std::sync::mpsc::channel::<Result<(StoreContext, Op), String>>();

    // (1) UI thread: create + attach HWND + start the purchase. Non-blocking.
    app.run_on_main_thread(move || {
        let started = (|| -> Result<(StoreContext, Op), String> {
            let ctx = StoreContext::GetDefault().map_err(|e| hres(&e))?;
            let interop: IInitializeWithWindow = ctx.cast().map_err(|e| hres(&e))?;
            unsafe { interop.Initialize(hwnd).map_err(|e| hres(&e))?; }
            let op = ctx.RequestPurchaseAsync(&store_id).map_err(|e| hres(&e))?;
            Ok((ctx, op))
        })();
        let _ = tx.send(started);
    })
    .map_err(|e| e.to_string())?;

    // (2) worker thread: wait for the operation. UI stays responsive.
    let (ctx, op) = rx.recv().map_err(|e| e.to_string())??;
    ensure_com_mta();
    let res = op.join().map_err(|e| hres(&e))?; // <-- .join(), NOT .get()
    let _keep_alive = ctx;                       // store context lives until join completes
    let status = res.Status().map_err(|e| hres(&e))?;
    let code = res.ExtendedError().map(ext_code).unwrap_or(0);

    if is_no_license(code) {
        return Ok(PurchaseOutcome::NotAvailable(
            "This build has no Microsoft Store retail license; in-app purchase is unavailable.".into(),
        ));
    }
    if status == StorePurchaseStatus::Succeeded { return Ok(PurchaseOutcome::Purchased); }
    if status == StorePurchaseStatus::AlreadyPurchased { return Ok(PurchaseOutcome::AlreadyPurchased); }
    if status == StorePurchaseStatus::NetworkError { return Ok(PurchaseOutcome::NetworkError); }
    if status == StorePurchaseStatus::ServerError { return Ok(PurchaseOutcome::ServerError); }
    Ok(PurchaseOutcome::NotPurchased)
}
```

Where to get the HWND: pass the `tauri::WebviewWindow` (or `Window`) whose parent you want the purchase
dialog centred over; call `window.hwnd()` **on the same window the user is interacting with**. Because
the crate version is pinned to 0.62.2 the returned `HWND` needs **no** cast/transmute.

---

## 10. Is there a simpler supported route?

- **Hand-roll with `windows` + `IInitializeWithWindow`** — what this report documents. Full control,
  no extra dependency, matches Tauri's crate version. Cost: you write ~200 lines and own the
  UI-thread/HWND plumbing and the graceful-degradation policy. **Recommended for MonoSub**, especially
  since it already has a Mac `storekit-rs` IAP module and wants a symmetric Windows one.

- **`tauri-plugin-iap` (Choochmeque)** — a real Tauri v2 plugin whose **Windows backend uses
  the Microsoft Store API**; claims consumables, durables and subscriptions, "Uses SKUs for
  subscription offer variations", "Automatic acknowledgment handled by the Store" on Windows.
  (https://github.com/Choochmeque/tauri-plugin-iap, npm/crates `tauri-plugin-iap`, current ~0.10.)
  *Pros:* cross-platform (matches MonoSub's iOS/Android/macOS future), JS + Rust surface, capability
  permissions wired. *Cons:* adds a plugin + capabilities + a JS bridge you may not want; its Windows
  codegen and API shape are third-party (need to read its source to confirm it does
  `IInitializeWithWindow` with **your** window HWND and that it pins a compatible `windows` version);
  heavier than four functions. **UNVERIFIED** whether its Windows path is battle-tested and whether it
  exposes `ProductInfo` price the way you need — evaluate its source before adopting.

- **`windows-store`-style helper crates** — I did **not** find a maintained, purpose-built crate for
  *durable* Microsoft Store add-ons that is clearly better than 4 thin `windows` calls. `winrt` / `windows`
  are the bindings; there is no widely-used "StoreContext convenience" crate. **UNVERIFIED / none found.**

- **Tauri's own official plugin list** has `tauri-plugin-iap` as a community plugin (not one of the
  core CrabNebula plugins); no first-party Store-IAP plugin exists. So "hand-roll with `windows`" is the
  mainstream path for this exact requirement.

---

## 11. Sources

Microsoft Learn (API semantics, constraints, error codes):
- `StoreContext.RequestPurchaseAsync` — UI-thread + elevated + `0x80070578` remarks:
  https://learn.microsoft.com/en-us/uwp/api/windows.services.store.storecontext.requestpurchaseasync
- `Windows.Services.Store` namespace — retail-license / developer-license / elevated notes:
  https://learn.microsoft.com/en-us/uwp/api/windows.services.store
- `StoreContext` class — retail-license remarks + `appLicensing`:
  https://learn.microsoft.com/en-us/uwp/api/windows.services.store.storecontext
- `StoreContext.GetUserCollectionAsync`:
  https://learn.microsoft.com/en-us/uwp/api/windows.services.store.storecontext.getusercollectionasync
- `StoreContext.GetStoreProductsAsync`:
  https://learn.microsoft.com/en-us/uwp/api/windows.services.store.storecontext.getstoreproductsasync
- `StoreContext.GetAppLicenseAsync`:
  https://learn.microsoft.com/en-us/uwp/api/windows.services.store.storecontext.getapplicenseasync
- `StoreProductQueryResult` (Products / ExtendedError):
  https://learn.microsoft.com/en-us/uwp/api/windows.services.store.storeproductqueryresult
- `StoreProduct.IsInUserCollection`:
  https://learn.microsoft.com/en-us/uwp/api/windows.services.store.storeproduct.isinusercollection
- `StoreProduct.Price` / `StorePrice.FormattedPrice` / `StorePrice.CurrencyCode`:
  https://learn.microsoft.com/en-us/uwp/api/windows.services.store.storeproduct.price ,
  https://learn.microsoft.com/en-us/uwp/api/windows.services.store.storeprice.formattedprice ,
  https://learn.microsoft.com/en-us/uwp/api/windows.services.store.storeprice.currencycode
- `StorePurchaseStatus` (field values):
  https://learn.microsoft.com/en-us/uwp/api/windows.services.store.storepurchasestatus
- `StorePurchaseResult.Status` / `.ExtendedError`:
  https://learn.microsoft.com/en-us/uwp/api/windows.services.store.storepurchaseresult.status ,
  https://learn.microsoft.com/en-us/uwp/api/windows.services.store.storepurchaseresult.extendederror
- Error codes for Store operations (`0x803F6107`, `0x803F6108`, `0x803F8001`, …):
  https://learn.microsoft.com/en-us/windows/uwp/monetize/error-codes-for-store-operations
- In-app purchases and trials (elevated not supported; add-on types):
  https://learn.microsoft.com/en-us/windows/uwp/monetize/in-app-purchases-and-trials
- Enable in-app purchases of apps and add-ons (end-to-end sample flow):
  https://learn.microsoft.com/en-us/windows/uwp/monetize/enable-in-app-purchases-of-apps-and-add-ons
- Rust `windows` crate docs (generated API): https://microsoft.github.io/windows-docs-rs/
  (`windows::Services::Store`, `windows::Win32::UI::Shell::IInitializeWithWindow`)

Crate source read locally (authoritative for this project's exact versions), under
`~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/`:
- `windows-0.62.2/src/Windows/Services/Store/mod.rs` (all Store signatures/line numbers above)
- `windows-0.62.2/src/Windows/Win32/UI/Shell/mod.rs:27249` (`IInitializeWithWindow`)
- `windows-collections-0.3.2/src/{bindings.rs,iterable.rs,map_view.rs}` (`IIterable`, `IMapView`, `From<Vec>`)
- `windows-future-0.3.2/src/{join.rs,future.rs,bindings.rs}` (`join()`, `IntoFuture`, `Send/Sync`)
- `tauri-2.12.4/{Cargo.toml:420-428, src/webview/webview_window.rs:43,1966}` (`windows = "0.62"`, `hwnd()`)
- `src-tauri/Cargo.lock` (windows 0.61.3 vs 0.62.2 dependency graph)

---

## 12. Explicitly UNVERIFIED / gaps

- Whether `tauri-plugin-iap`'s Windows backend correctly performs `IInitializeWithWindow` with the
  caller's real HWND and pins a `windows` version compatible with Tauri 2.12.4 — **UNVERIFIED**; read
  its source before adopting.
- The exact value of `0x803F6108` returned by *this* app's dev-cert MSIX on *this* machine — the code
  is documented, but the live HRESULT must be observed on an actual sideloaded build. Add
  `tracing::warn!` of `0x{:08X}` on every failure and confirm.
- End-to-end purchase completion requires a **Store-signed** MSIX with a retail license and a
  **published** add-on in Partner Center; it cannot be exercised from a dev build.
- Whether `GetUserCollectionAsync([\"Durable\"])` returns the durable add-on for *all* SKU/subscription
  shapes vs `GetAppLicenseAsync().AddOnLicenses()` — both are documented; the license path is the one
  the Microsoft "durable add-on" sample uses. Prefer it and keep the collection path as a fallback.
