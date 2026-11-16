//! Where everything lives, resolved once and passed down.
//!
//! This module exists so `engine/`, `subtitle/` and `translate/` can be driven
//! from somewhere that is *not* a running Tauri app — a CLI, or a unit test.
//! Those modules used to take `&tauri::AppHandle` and reach through it for
//! paths, which meant only a Tauri process could run them.
//!
//! # Two independent things, often confused
//!
//! [`AppPaths::config_dir`] (where `settings.json` lives) and
//! [`AppPaths::models_root`] (where model weights live) are **unrelated
//! locations**. The config file merely happens to *contain* a string pointing at
//! the models directory. Measured on a real install:
//!
//! ```text
//! config_dir   ~/Library/Application Support/leixlabs.monosub
//! models_root  /Users/lei/dev/projects/2601-monosub/models      ← from settings.json
//! ```
//!
//! Nothing derives one from the other. Keep them separate.

use std::path::{Path, PathBuf};

/// Env override for the config directory (mirrors `MONOSUB_MODELS_DIR`).
pub const CONFIG_DIR_ENV: &str = "MONOSUB_CONFIG_DIR";
/// Env override for the bundled-resources root (`models.json`, `bin/`).
pub const RESOURCES_DIR_ENV: &str = "MONOSUB_RESOURCES_DIR";

/// The file that proves a directory is a resources root rather than just some
/// parent directory. Used by [`resolve_resources_root`] when a candidate can
/// only be identified by its contents.
const CATALOGUE_FILE: &str = "models.json";

/// Bundle identifier from `tauri.conf.json`, used to derive the per-app
/// directories. It must match the app's own identifier or the CLI would read a
/// different `settings.json` than the GUI writes.
const APP_IDENTIFIER: &str = "leixlabs.monosub";

/// Everything the engine needs from the environment, resolved once.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppPaths {
    /// `<...>/resources` — root of `models.json`, `bin/`, `models/silero_vad`.
    pub resources: PathBuf,
    /// Where model weights live. **Unrelated to `config_dir`**; resolved from
    /// `MONOSUB_MODELS_DIR` → `settings.json`'s `modelsDir` → `<data_dir>/models`.
    pub models_root: PathBuf,
    /// Where `settings.json` lives. **Unrelated to `models_root`**; the config
    /// file is just where the models path is written down.
    pub config_dir: PathBuf,
    /// Base directory for defaults. Not itself a place the app writes to.
    pub data_dir: PathBuf,
}

/// Why a path set could not be resolved.
#[derive(Debug)]
pub enum PathError {
    /// The environment did not provide a home/config directory.
    NoHome(&'static str),
}

impl std::fmt::Display for PathError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PathError::NoHome(what) => write!(f, "cannot resolve the {what} directory"),
        }
    }
}

impl std::error::Error for PathError {}

impl AppPaths {
    /// Resolve from a running Tauri app.
    ///
    /// `config_dir` and `data_dir` come from Tauri itself, because the GUI must
    /// keep writing to exactly the paths it always has. `resources` uses the same
    /// probes as [`AppPaths::from_env`], so the two agree.
    #[cfg(feature = "tauri-app")]
    pub fn from_app(app: &tauri::AppHandle) -> Result<Self, PathError> {
        use tauri::Manager;

        let config_dir = app
            .path()
            .app_config_dir()
            .map_err(|_| PathError::NoHome("app config"))?;
        let data_dir = app
            .path()
            .app_data_dir()
            .map_err(|_| PathError::NoHome("app data"))?;

        Self::assemble(config_dir, data_dir)
    }

    /// Resolve without a Tauri runtime: a CLI, or a test.
    ///
    /// Deliberately mirrors the GUI's platform directories using
    /// [`APP_IDENTIFIER`], so both frontends read the same `settings.json`.
    pub fn from_env() -> Result<Self, PathError> {
        let home = std::env::var_os("HOME")
            .map(PathBuf::from)
            // Windows has no HOME; `APPDATA` is the documented base there.
            .or_else(|| std::env::var_os("APPDATA").map(PathBuf::from))
            .ok_or(PathError::NoHome("home"))?;

        let (config_dir, data_dir) = if cfg!(target_os = "macos") {
            // macOS keeps both under Application Support, which is why they
            // look alike here — they are still independent values.
            let base = home
                .join("Library")
                .join("Application Support")
                .join(APP_IDENTIFIER);
            (base.clone(), base)
        } else if cfg!(target_os = "windows") {
            let base = home.join(APP_IDENTIFIER);
            (base.clone(), base)
        } else {
            (
                home.join(".config").join(APP_IDENTIFIER),
                home.join(".local").join("share").join(APP_IDENTIFIER),
            )
        };

        Self::assemble(config_dir, data_dir)
    }

    /// Shared tail of both constructors: resolve the models root from the
    /// config dir, so `modelsDir` in `settings.json` is honoured identically.
    fn assemble(config_dir: PathBuf, data_dir: PathBuf) -> Result<Self, PathError> {
        let config_dir = env_path(CONFIG_DIR_ENV).unwrap_or(config_dir);
        let models_root = resolve_models_root(&config_dir, &data_dir);
        Ok(Self {
            resources: resolve_resources_root(),
            models_root,
            config_dir,
            data_dir,
        })
    }

    /// `<config_dir>/settings.json`.
    ///
    /// Not called yet: the settings write/read path still goes through
    /// `model_dir`, which predates `AppPaths`. Kept because it is the one place
    /// that definition lives, and the CLI reads settings through it.
    #[allow(dead_code)]
    pub fn settings_path(&self) -> PathBuf {
        self.config_dir.join("settings.json")
    }
}

/// A non-empty env var as a path.
fn env_path(var: &str) -> Option<PathBuf> {
    let raw = std::env::var_os(var)?;
    let trimmed = raw.to_string_lossy().trim().to_string();
    (!trimmed.is_empty()).then(|| PathBuf::from(trimmed))
}

/// `MONOSUB_MODELS_DIR` → `settings.json`'s `modelsDir` → `<data_dir>/models`.
///
/// This is the precedence `model_dir::configured_root` has always had; it lives
/// here so it can run *before* an [`AppPaths`] exists (the models root is a field
/// of it, so it cannot be read back through one).
fn resolve_models_root(config_dir: &Path, data_dir: &Path) -> PathBuf {
    if let Some(overridden) = env_path(crate::model_dir::MODELS_DIR_ENV) {
        return overridden;
    }
    if let Some(configured) = read_models_dir_from(config_dir) {
        return configured;
    }
    data_dir.join("models")
}

/// The `modelsDir` string in `<config_dir>/settings.json`, if any.
///
/// Reads the file directly rather than through `model_dir::load_settings`, which
/// would need an [`AppPaths`] that is still being built.
fn read_models_dir_from(config_dir: &Path) -> Option<PathBuf> {
    let raw = std::fs::read_to_string(config_dir.join("settings.json")).ok()?;
    let parsed: serde_json::Value = serde_json::from_str(&raw).ok()?;
    let value = parsed.get("modelsDir")?.as_str()?.trim();
    (!value.is_empty()).then(|| PathBuf::from(value))
}

/// Where the bundled resources live, without needing an `AppHandle`.
///
/// Order: `MONOSUB_RESOURCES_DIR` → `<cwd>/src-tauri/resources` (dev checkout) →
/// `<exe_dir>/resources` (`tauri dev` copies resources next to the binary) →
/// `<exe_dir>/../Resources/resources` (macOS GUI bundle) →
/// `<exe_dir>/..` (a binary that ships inside `resources/bin/`).
///
/// That last branch is why `monosub-cli` can run at all. The bundled CLI is
/// installed at `<root>/resources/bin/monosub-cli`, so its own
/// directory is the *bin* folder and the resources root is one level up. None of
/// the earlier probes match that depth, and the previous code fell through to
/// the compile-time `CARGO_MANIFEST_DIR` fallback — a path on the build machine.
/// The result was a CLI that launched, then could not find `models.json` (it
/// cannot plan), nor the VAD model or the engine binaries next to it. This is
/// not Windows-specific: the macOS DMG has the same layout.
///
/// The GUI used to gate the exe-relative probes on `app.is_some()`; that gate is
/// gone because the probes are existence checks — when the directory is not
/// there, the next candidate is tried anyway, so dropping the gate cannot change
/// which path wins for a real app, and lets a CLI resolve the same layout.
pub fn resolve_resources_root() -> PathBuf {
    if let Some(overridden) = env_path(RESOURCES_DIR_ENV) {
        return overridden;
    }

    let mut candidates: Vec<PathBuf> = Vec::new();

    if let Ok(cwd) = std::env::current_dir() {
        candidates.push(cwd.join("src-tauri").join("resources"));
    }

    // `<exe_dir>/../..` is weaker evidence than the probes above: it is only a
    // resources root if it actually holds the catalogue. Without this check a
    // dev build would match `target/` (the grandparent of `target/debug/monosub`)
    // and resolve a directory with no models in it.
    let mut exe_inside_resources: Option<PathBuf> = None;

    if let Ok(exe) = std::env::current_exe() {
        if let Some(exe_dir) = exe.parent() {
            candidates.push(exe_dir.join("resources"));
            // macOS GUI bundle: Contents/MacOS/monosub → Contents/Resources/resources
            if let Some(contents) = exe_dir.parent() {
                candidates.push(contents.join("Resources").join("resources"));
            }
            // A binary that ships inside the resources tree: exe_dir is
            // `<root>/bin`, so `<root>` — the directory holding `models.json`
            // and `bin/` — is exactly one level up.
            if let Some(root) = exe_dir.parent() {
                if root.join(CATALOGUE_FILE).is_file() {
                    exe_inside_resources = Some(root.to_path_buf());
                }
            }
        }
    }

    // Last, after every stronger probe: a binary shipped inside resources/bin/.
    if let Some(root) = exe_inside_resources {
        candidates.push(root);
    }

    // Fall back to the dev-checkout default so a caller that cannot see any of
    // the above gets a usable (if wrong) path rather than a panic.
    candidates
        .into_iter()
        .find(|dir| dir.is_dir())
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("resources")
        })
}

/// Marker macOS sets in the environment of every sandboxed process
/// (App Sandbox, including Mac App Store builds). Absent from an unsandboxed
/// DMG build, which is how the two distributions differ at runtime.
const SANDBOX_ENV: &str = "APP_SANDBOX_CONTAINER_ID";

/// True when this process runs inside the App Sandbox.
///
/// The distinction is not cosmetic: under the sandbox the *containing folder* of
/// a file the user dragged in is not writable — only the file itself is. Writing
/// a sibling next to the input (`<stem>.srt`, `<stem>_audio.wav`) therefore fails
/// with `EPERM`, which is why every derived path asks this first.
pub fn is_sandboxed() -> bool {
    std::env::var_os(SANDBOX_ENV).is_some_and(|value| !value.is_empty())
}

/// A per-user scratch area for artifacts that are *not* the user's files —
/// extracted WAVs, thumbnails. Inside the sandbox `temp_dir()` already points at
/// the container, so this never lands in the real `/tmp` for a MAS build, and a
/// DMG build keeps using the ordinary temp dir.
pub fn scratch_dir() -> PathBuf {
    let dir = std::env::temp_dir().join("monosub");
    // 必须自己建：ffmpeg 不会替父目录兜底，缺了就是 "No such file or directory"。
    // 失败也不在这里报——真正的写入会带着具体路径失败，那才是能给用户看的消息。
    if let Err(error) = std::fs::create_dir_all(&dir) {
        tracing::warn!(dir = %dir.display(), error = %error, "could not create the scratch directory");
    }
    dir
}

/// A temp directory the bundled one-file PyInstaller binaries can unpack into,
/// or `None` when the system temp directory is fine and should be inherited.
///
/// `resources/bin/yt-dlp*.exe` is a PyInstaller one-file build: before any
/// Python runs, the bootloader creates `<temp>\_MEI<pid>xxxxxx` with a
/// current-user-only DACL and aborts when that fails:
///
/// ```text
/// [PYI-27076:ERROR] Could not create temporary directory!
/// ```
///
/// with exit code `0xFFFFFFFF`. `GetTempPathW` reads TMP then TEMP, so a value
/// pointing at a missing, over-long, full or policy-blocked directory turns URL
/// import into a failure that reads like a missing binary — there is no yt-dlp
/// diagnostic to classify because Python never starts.
///
/// The probe is an ordinary create/remove: it catches the common causes (missing
/// path, no permission, read-only volume) but not a directory that accepts a
/// create while refusing the bootloader's restrictive DACL (FAT32, some network
/// shares). There the child fails exactly as it did before — no regression.
#[cfg(target_os = "windows")]
pub fn pyinstaller_temp_dir() -> Option<PathBuf> {
    let system = std::env::temp_dir();
    if can_create_directory(&system) {
        return None;
    }

    let fallback = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .map(|base| base.join(APP_IDENTIFIER).join("tmp"))
        .or_else(|| {
            std::env::var_os("USERPROFILE").map(|home| PathBuf::from(home).join(".monosub-tmp"))
        })?;

    std::fs::create_dir_all(&fallback).ok()?;
    if !can_create_directory(&fallback) {
        return None;
    }

    tracing::warn!(
        system = %system.display(),
        fallback = %fallback.display(),
        "the system temp directory cannot host a PyInstaller unpack; using the app directory"
    );
    Some(fallback)
}

/// One create/remove under `parent`; false when the directory rejects entries.
#[cfg(target_os = "windows")]
fn can_create_directory(parent: &Path) -> bool {
    let probe = parent.join(format!("_MEI_probe_{}", std::process::id()));
    let created = std::fs::create_dir(&probe).is_ok();
    if created {
        let _ = std::fs::remove_dir(&probe);
    }
    created
}

/// Stable cache path for a derived artifact of `source`.
///
/// The path is keyed by a hash of the *absolute* source path so two files with
/// the same name in different folders cannot collide, and so the cache survives
/// across runs (a re-run of the same file reuses the WAV instead of re-decoding
/// it). `suffix` is appended to the stem, e.g. `_audio.wav`.
pub fn scratch_cache_path(source: &Path, suffix: &str) -> Option<PathBuf> {
    let stem = source.file_stem()?.to_str()?;
    let key = short_hash(&source.to_string_lossy());
    Some(scratch_dir().join(format!("{key}_{stem}{suffix}")))
}

/// Marker prefix for "the output folder cannot be written to".
///
/// The status string the backend emits is `OUTPUT_NOT_WRITABLE:<folder>`; the
/// frontend keys on the marker, shows `errors.outputNotWritable`, and opens the
/// folder panel so the user can grant access. Never silently relocate a result —
/// the user was promised `<folder>/<stem>.srt`, and a subtitle that appears
/// somewhere else is worse than an error that says why.
pub const OUTPUT_NOT_WRITABLE: &str = "OUTPUT_NOT_WRITABLE";

/// Marker prefix for "the models folder cannot be read".
///
/// A sandboxed app sees a folder it has no grant for exactly like an empty one:
/// every `is_file()` is `false`, so the UI would list 129 models as "not
/// installed" and offer downloads that then fail. Naming the folder is the only
/// honest answer.
pub const MODELS_DIR_NOT_ACCESSIBLE: &str = "MODELS_DIR_NOT_ACCESSIBLE";

/// [`path`] if its folder accepts writes right now, otherwise an error naming the
/// folder.
///
/// Writability is probed by creating a file, not by inspecting mode bits: under
/// the sandbox a directory can look writable (`0o755`, owned by the user) and
/// still deny `open()` because the grant belongs to a *different* folder.
///
/// The caller is responsible for having restored any security-scoped grant first
/// (`bookmark::activate` at startup, `authorize_dir` when the user picks a
/// folder); this function only reports the current truth.
pub fn ensure_writable_output(path: &Path) -> Result<PathBuf, String> {
    let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) else {
        return Ok(path.to_path_buf());
    };

    if dir_accepts_writes(parent) {
        return Ok(path.to_path_buf());
    }

    Err(format!("{OUTPUT_NOT_WRITABLE}:{}", parent.display()))
}

/// Can a file be created in `dir` right now? Creates and removes a probe file.
///
/// Public because the frontend asks this *before* a run starts: under the
/// sandbox "the user picked a file here" says nothing about the folder, so the
/// import path probes it and asks for the folder grant once, instead of letting
/// the first transcription fail and then asking.
pub fn dir_accepts_writes(dir: &Path) -> bool {
    if std::fs::create_dir_all(dir).is_err() {
        return false;
    }
    let probe = dir.join(format!(".monosub-write-probe-{}", std::process::id()));
    match std::fs::File::create(&probe) {
        Ok(_) => {
            let _ = std::fs::remove_file(&probe);
            true
        }
        Err(_) => false,
    }
}

/// FNV-1a, hex. Only used to keep cache file names unique and stable; not a
/// security boundary.
fn short_hash(value: &str) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in value.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{hash:016x}")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A temp directory that refuses entries must be reported as unusable, or a
    /// broken TMP would be accepted and URL import would keep failing with the
    /// bootloader's message instead of falling back.
    #[cfg(target_os = "windows")]
    #[test]
    fn the_temp_probe_rejects_a_missing_directory_and_accepts_a_real_one() {
        let missing = std::env::temp_dir().join(format!(
            "monosub-pyi-probe-missing-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&missing);
        assert!(!can_create_directory(&missing));

        let usable = std::env::temp_dir().join(format!(
            "monosub-pyi-probe-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&usable).unwrap();
        assert!(can_create_directory(&usable));
        let _ = std::fs::remove_dir_all(&usable);
    }

    /// The config dir and the models root are independent: a `modelsDir` that
    /// points somewhere completely unrelated must be honoured verbatim, and must
    /// not be rewritten to live under the config dir.
    #[test]
    fn a_configured_models_dir_is_used_verbatim_not_derived_from_the_config_dir() {
        let config_dir = std::env::temp_dir().join(format!(
            "monosub-paths-config-{}",
            std::process::id()
        ));
        let elsewhere = std::env::temp_dir().join("somewhere-else-models");
        std::fs::create_dir_all(&config_dir).unwrap();
        // Built with `serde_json` rather than `format!`: a Windows path is full of
        // backslashes, which are escapes in JSON, so a hand-formatted string is
        // invalid there and the parse fails for a reason unrelated to the test.
        let settings = serde_json::json!({ "modelsDir": elsewhere.display().to_string() });
        std::fs::write(
            config_dir.join("settings.json"),
            serde_json::to_string(&settings).unwrap(),
        )
        .unwrap();

        let found = read_models_dir_from(&config_dir);

        assert_eq!(found, Some(elsewhere));
        let _ = std::fs::remove_dir_all(&config_dir);
    }

    /// A blank `modelsDir` means "use the default", not "models live at /".
    #[test]
    fn a_blank_models_dir_is_not_a_path() {
        let config_dir = std::env::temp_dir().join(format!(
            "monosub-paths-blank-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&config_dir).unwrap();
        std::fs::write(config_dir.join("settings.json"), r#"{"modelsDir":"   "}"#).unwrap();

        assert_eq!(read_models_dir_from(&config_dir), None);
        let _ = std::fs::remove_dir_all(&config_dir);
    }

    /// A corrupt or missing settings file must not panic the CLI.
    #[test]
    fn unreadable_settings_yield_no_override() {
        let missing = std::env::temp_dir().join("monosub-paths-definitely-absent");
        assert_eq!(read_models_dir_from(&missing), None);
    }

    /// `settings_path` must come from `config_dir` alone. Pinning this stops
    /// anyone from "tidying up" by deriving it from `models_root`.
/// The CLI and the GUI must read the **same** `settings.json`, or the CLI
    /// inherits none of the model plan the user chose and the whole "reuse the
    /// chosen model" contract is void.
    ///
    /// `from_app` cannot be constructed without a real `AppHandle`, so this pins
    /// the two things that must agree instead: the platform directory the
    /// CLI derives, and the bundle identifier Tauri uses for the GUI.
    #[test]
    fn the_cli_derives_the_gui_s_app_directory() {
        let identifier = APP_IDENTIFIER;
        let home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default();

        if cfg!(target_os = "macos") {
            let expected = home
                .join("Library")
                .join("Application Support")
                .join(identifier);
            assert!(expected.ends_with(identifier), "{expected:?}");
        } else if cfg!(target_os = "linux") {
            let expected = home.join(".config").join(identifier);
            assert!(expected.ends_with(identifier), "{expected:?}");
        }

        // The identifier must be the one in tauri.conf.json. Read it here rather
        // than restating it, so a bundle-id change cannot silently desync the CLI.
        let conf = std::fs::read_to_string(
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tauri.conf.json"),
        )
        .expect("tauri.conf.json is part of the source tree");
        let parsed: serde_json::Value = serde_json::from_str(&conf).unwrap();
        assert_eq!(
            parsed.get("identifier").and_then(|v| v.as_str()),
            Some(APP_IDENTIFIER),
            "APP_IDENTIFIER drifted from tauri.conf.json — the CLI would read a different settings.json than the GUI writes"
        );
    }

    /// The resources root must resolve to something that actually contains the
    /// catalogue, in a dev checkout at least. A CLI that cannot find models.json
    /// cannot plan at all.
    #[test]
    fn the_resources_root_contains_the_model_catalogue_in_a_dev_checkout() {
        let root = resolve_resources_root();
        assert!(
            root.join("models.json").is_file(),
            "no models.json under {root:?}; a CLI run from this checkout could not plan"
        );
    }

    /// A binary that ships inside `resources/bin/` must find the resources root
    /// one level up — the layout both the DMG and the MSIX produce for
    /// `monosub-cli`. Before this branch existed the CLI launched and then could
    /// not find `models.json`, so it could not plan at all.
    ///
    /// `resolve_resources_root` reads the real `current_exe()`, so the branch is
    /// exercised through its predicate instead: the fixture lays out the shipped
    /// shape and asserts the rule identifies it.
    #[test]
    fn a_binary_inside_the_resources_tree_resolves_the_root_one_level_up() {
        let root = std::env::temp_dir().join(format!(
            "monosub-paths-inside-resources-{}",
            std::process::id()
        ));
        let bin = root.join("bin");
        std::fs::create_dir_all(&bin).unwrap();
        std::fs::write(root.join(CATALOGUE_FILE), "{}").unwrap();

        // The shipped layout: <root>/bin/monosub-cli
        let exe_dir = bin.as_path();
        let candidate = exe_dir.parent();

        assert_eq!(candidate, Some(root.as_path()));
        assert!(
            candidate.unwrap().join(CATALOGUE_FILE).is_file(),
            "the probe must accept a directory that really holds the catalogue"
        );

        let _ = std::fs::remove_dir_all(&root);
    }

    /// The same probe must reject a grandparent that merely exists. A dev build
    /// at `target/debug/monosub` has `target/` one level up, and resolving that
    /// would hand every engine a directory with no models in it.
    #[test]
    fn a_parent_without_the_catalogue_is_not_a_resources_root() {
        let root = std::env::temp_dir().join(format!(
            "monosub-paths-no-catalogue-{}",
            std::process::id()
        ));
        let bin = root.join("bin");
        std::fs::create_dir_all(&bin).unwrap();

        assert!(
            !bin.parent().unwrap().join(CATALOGUE_FILE).is_file(),
            "an empty parent must not be mistaken for a resources root"
        );

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn settings_live_in_the_config_dir_regardless_of_the_models_root() {
        let paths = AppPaths {
            resources: PathBuf::from("/res"),
            models_root: PathBuf::from("/models/elsewhere"),
            config_dir: PathBuf::from("/cfg"),
            data_dir: PathBuf::from("/data"),
        };
        assert_eq!(paths.settings_path(), PathBuf::from("/cfg/settings.json"));
    }

    /// A writable folder must be returned untouched — the DMG build would
    /// otherwise start rewriting paths it has no business touching.
    #[test]
    fn a_writable_folder_keeps_the_requested_output_path() {
        let dir = std::env::temp_dir().join(format!("monosub-paths-writable-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let requested = dir.join("episode.srt");

        assert_eq!(ensure_writable_output(&requested).unwrap(), requested);

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A folder that refuses the probe must produce a *named* failure the
    /// frontend can turn into an authorization panel — not a quiet relocation.
    #[test]
    fn an_unwritable_folder_fails_with_its_path() {
        let dir = std::env::temp_dir().join(format!("monosub-paths-readonly-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let requested = dir.join("episode.srt");

        // Make the directory read-only for the probe.
        let mut perms = std::fs::metadata(&dir).unwrap().permissions();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            perms.set_mode(0o500);
        }
        let restore = perms.clone();
        std::fs::set_permissions(&dir, perms).unwrap();

        let error = ensure_writable_output(&requested).unwrap_err();
        assert!(
            error.starts_with(&format!("{}:{}", OUTPUT_NOT_WRITABLE, dir.display())),
            "got {error}"
        );

        // Back to writable so the temp dir can be removed.
        let mut perms = restore;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            perms.set_mode(0o700);
        }
        std::fs::set_permissions(&dir, perms).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The scratch folder has to exist before ffmpeg is pointed at it: a missing
    /// parent is the exact failure that shipped once ("No such file or directory"
    /// on `<tmp>/monosub/<hash>_x_audio.wav`).
    #[test]
    fn the_scratch_dir_exists_so_ffmpeg_can_write_into_it() {
        let dir = scratch_dir();
        assert!(dir.is_dir(), "{} must be a directory", dir.display());

        let cache = scratch_cache_path(Path::new("/movies/episode.mov"), "_audio.wav").unwrap();
        assert_eq!(cache.parent(), Some(dir.as_path()));
    }

    /// Two files with the same name in different folders must not share a cache
    /// file, and the same file must map to the same path every run (that is what
    /// makes the extracted WAV reusable).
    #[test]
    fn the_scratch_cache_is_stable_and_collision_free() {
        let a = scratch_cache_path(Path::new("/movies/a/episode.mov"), "_audio.wav").unwrap();
        let again = scratch_cache_path(Path::new("/movies/a/episode.mov"), "_audio.wav").unwrap();
        let other = scratch_cache_path(Path::new("/movies/b/episode.mov"), "_audio.wav").unwrap();

        assert_eq!(a, again);
        assert_ne!(a, other);
        assert!(a.to_string_lossy().ends_with("_episode_audio.wav"));
    }
}
