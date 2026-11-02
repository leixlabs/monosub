//! Security-scoped bookmarks — how a sandboxed app remembers a folder the user
//! granted it.
//!
//! Under the App Sandbox a directory grant lives in the *process*, not in a
//! path: `/Users/me/Movies` is just a string, and after a restart writing there
//! fails with `EPERM`. The only way to keep the grant is a security-scoped
//! bookmark — an opaque blob created while the grant is live, resolvable later
//! without showing the user a panel again.
//!
//! That is the contract this module exists for:
//!
//! * the user grants a folder **once** (the open panel in the frontend),
//! * [`create`] turns it into a bookmark that gets persisted in `settings.json`,
//! * every later launch [`activate`]s the bookmarks, which restores write access
//!   for the whole process,
//! * when a bookmark can no longer be resolved (folder deleted, grant revoked,
//!   container reset) the caller **fails loudly with the folder's path** rather
//!   than silently writing somewhere else.
//!
//! Unsandboxed builds (the DMG) and non-macOS builds need none of this: a plain
//! path already grants access, so [`create`] answers `Ok(None)` and callers keep
//! storing plain strings.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// A directory the user granted, plus the bookmark that makes the grant survive
/// a restart. Persisted in `settings.json` (`dirGrants`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DirGrant {
    /// The path as the user saw it — used for matching and for error messages.
    pub path: String,
    /// Hex-encoded bookmark blob. Opaque; never parsed by us.
    pub bookmark: String,
}

/// A bookmark that resolved back to a live folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resolved {
    pub path: PathBuf,
    /// The bookmark still resolved but the system says it should be recreated.
    pub stale: bool,
}

/// Does this platform need bookmarks at all?
pub fn is_supported() -> bool {
    cfg!(target_os = "macos")
}

// ---------------------------------------------------------------- hex codec
//
// Bookmarks are binary; `settings.json` is text. Hex costs two characters per
// byte but needs no dependency and stays diff-readable, which matters for a file
// users are told to delete when things go wrong.

pub(crate) fn hex_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(char::from_digit(u32::from(byte >> 4), 16).unwrap());
        out.push(char::from_digit(u32::from(byte & 0x0f), 16).unwrap());
    }
    out
}

pub(crate) fn hex_decode(text: &str) -> Option<Vec<u8>> {
    if !text.len().is_multiple_of(2) {
        return None;
    }
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len() / 2);
    for pair in bytes.chunks(2) {
        let hi = (pair[0] as char).to_digit(16)?;
        let lo = (pair[1] as char).to_digit(16)?;
        out.push(((hi << 4) | lo) as u8);
    }
    Some(out)
}

// ---------------------------------------------------------------- macOS

#[cfg(target_os = "macos")]
mod imp {
    use super::{DirGrant, Resolved};
    use objc2::runtime::Bool;
    use objc2_foundation::{
        NSData, NSString, NSURL, NSURLBookmarkCreationOptions, NSURLBookmarkResolutionOptions,
    };
    use std::path::{Path, PathBuf};
    use std::sync::Mutex;

    /// Resolved URLs are kept alive for the whole process: `startAccessing` is
    /// reference-counted per URL object, and the sandbox drops the grant when the
    /// last reference goes away. Leaking them here is exactly the intent — the
    /// process is the lifetime of the grant.
    static ACTIVE: Mutex<Vec<objc2::rc::Retained<NSURL>>> = Mutex::new(Vec::new());

    pub fn create(dir: &Path) -> Result<String, String> {
        let url = file_url(dir)?;
        let data = url
            .bookmarkDataWithOptions_includingResourceValuesForKeys_relativeToURL_error(
                NSURLBookmarkCreationOptions::WithSecurityScope,
                None,
                None,
            )
            .map_err(|error| describe(&error))?;

        // Keep access for this session too: the user just picked the folder, the
        // very next write must not need another trip through the panel.
        hold(url);

        Ok(super::hex_encode(&data.to_vec()))
    }

    pub fn resolve(bookmark: &str) -> Result<Resolved, String> {
        let bytes = super::hex_decode(bookmark)
            .ok_or_else(|| "the stored bookmark is not valid hex".to_string())?;
        let data = NSData::with_bytes(&bytes);
        let mut stale = Bool::new(false);

        let url = unsafe {
            NSURL::URLByResolvingBookmarkData_options_relativeToURL_bookmarkDataIsStale_error(
                &data,
                NSURLBookmarkResolutionOptions::WithSecurityScope,
                None,
                &mut stale,
            )
        }
        .map_err(|error| describe(&error))?;

        let path = url
            .path()
            .map(|path| PathBuf::from(path.to_string()))
            .ok_or_else(|| "the resolved bookmark has no file path".to_string())?;

        // The sandbox only honours the grant while access is held. `false` means
        // the grant is gone (revoked, folder moved, container reset) — but this is
        // not where we decide to fail: an unsandboxed build answers `false` too,
        // and for a sandboxed one the writability probe reports the folder by
        // name, which is what lets the UI ask for it again.
        let started = unsafe { url.startAccessingSecurityScopedResource() };
        if !started {
            tracing::debug!(
                path = %path.display(),
                "the resolved bookmark did not grant access; writes will be probed instead"
            );
        }
        hold(url);

        Ok(Resolved {
            path,
            stale: stale.as_bool(),
        })
    }

    fn file_url(dir: &Path) -> Result<objc2::rc::Retained<NSURL>, String> {
        let text = dir
            .to_str()
            .ok_or_else(|| format!("{} is not valid UTF-8", dir.display()))?;
        let path = NSString::from_str(text);
        Ok(NSURL::fileURLWithPath_isDirectory(&path, true))
    }

    fn hold(url: objc2::rc::Retained<NSURL>) {
        if let Ok(mut active) = ACTIVE.lock() {
            active.push(url);
        }
    }

    fn describe(error: &objc2_foundation::NSError) -> String {
        error.localizedDescription().to_string()
    }

    /// Resolve every persisted grant, keeping the live ones for the process.
    ///
    /// A failure here is not fatal — the app still runs, it just cannot write to
    /// that folder, and the write path reports it by name instead of pretending
    /// the folder is gone.
    pub fn activate(grants: &[DirGrant]) -> Vec<(String, Result<PathBuf, String>)> {
        grants
            .iter()
            .map(|grant| {
                let outcome = resolve(&grant.bookmark).map(|resolved| resolved.path);
                if let Ok(path) = &outcome {
                    tracing::info!(
                        granted = %grant.path,
                        resolved = %path.display(),
                        "restored access to a user-granted folder"
                    );
                }
                (grant.path.clone(), outcome)
            })
            .collect()
    }
}

// ---------------------------------------------------------------- everything else

#[cfg(not(target_os = "macos"))]
mod imp {
    use super::{DirGrant, Resolved};
    use std::path::{Path, PathBuf};

    pub fn create(_dir: &Path) -> Result<String, String> {
        // No sandbox, no grant to remember. The caller stores the plain path.
        Err("security-scoped bookmarks are macOS-only".to_string())
    }

    pub fn resolve(_bookmark: &str) -> Result<Resolved, String> {
        Err("security-scoped bookmarks are macOS-only".to_string())
    }

    pub fn activate(_grants: &[DirGrant]) -> Vec<(String, Result<PathBuf, String>)> {
        Vec::new()
    }
}

/// Create a bookmark for a folder the user just granted.
///
/// `Ok(None)` means "this platform does not need one" — the caller should keep
/// storing the plain path and move on.
pub fn create(dir: &Path) -> Result<Option<String>, String> {
    if !is_supported() {
        return Ok(None);
    }
    imp::create(dir).map(Some)
}

/// Resolve a stored bookmark back into a usable folder, restoring access.
pub fn resolve(bookmark: &str) -> Result<Resolved, String> {
    imp::resolve(bookmark)
}

/// Restore every persisted grant at startup. Returns one outcome per grant so the
/// caller can log which folders are unusable and why.
pub fn activate(grants: &[DirGrant]) -> Vec<(String, Result<PathBuf, String>)> {
    imp::activate(grants)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_round_trips() {
        let bytes = vec![0x00, 0x0f, 0xa5, 0xff, 0x10];
        let encoded = hex_encode(&bytes);
        assert_eq!(encoded, "000fa5ff10");
        assert_eq!(hex_decode(&encoded), Some(bytes));
    }

    #[test]
    fn malformed_hex_is_rejected_not_panicked() {
        assert_eq!(hex_decode("abc"), None, "odd length");
        assert_eq!(hex_decode("zz"), None, "not hex");
    }

    #[test]
    fn grants_serialize_the_way_the_settings_file_expects() {        let grant = DirGrant {
            path: "/Users/me/Movies".to_string(),
            bookmark: "00ff".to_string(),
        };
        let json = serde_json::to_string(&grant).unwrap();
        assert_eq!(json, r#"{"path":"/Users/me/Movies","bookmark":"00ff"}"#);
        assert_eq!(serde_json::from_str::<DirGrant>(&json).unwrap(), grant);
    }

    /// The objc2 plumbing, end to end: a folder becomes a bookmark, the bookmark
    /// becomes the folder again. Runs unsandboxed too (creating a bookmark does
    /// not need a grant), which is the only way this path gets exercised in CI.
    #[cfg(target_os = "macos")]
    #[test]
    fn a_bookmark_round_trips_to_the_same_folder() {
        let dir = std::env::temp_dir().join(format!("monosub-bookmark-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();

        let encoded = create(&dir).unwrap().expect("macOS produces a bookmark");
        assert!(super::hex_decode(&encoded).is_some(), "bookmark must be hex");

        let resolved = resolve(&encoded).unwrap();
        // /var and /private/var are the same folder; compare the real paths.
        assert_eq!(
            resolved.path.canonicalize().unwrap(),
            dir.canonicalize().unwrap()
        );

        let _ = std::fs::remove_dir_all(&dir);
    }
}
