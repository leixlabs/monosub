use std::{env, fs, path::PathBuf, process::Command};

/// StoreKit 2 lives in Swift. `storekit-rs` compiles a Swift static library, and
/// the final Rust binary must carry an LC_RPATH for the Swift runtime or dyld
/// dies at launch with `Library not loaded: @rpath/libswift_Concurrency.dylib`.
///
/// The flag belongs in this crate's build script rather than the dependency's:
/// `cargo:rustc-link-arg` from a dependency applies to the dependency's own
/// targets, not to the final executable. `cargo:rustc-link-arg` is harmless on
/// macOS builds that don't enable the `appstore` feature.
fn add_macos_swift_rpaths() {
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("macos") {
        return;
    }

    println!("cargo:rustc-link-arg=-Wl,-rpath,/usr/lib/swift");

    if let Ok(output) = Command::new("xcode-select").arg("-p").output() {
        if output.status.success() {
            let xcode_path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !xcode_path.is_empty() {
                let swift_lib_path = format!(
                    "{xcode_path}/Toolchains/XcodeDefault.xctoolchain/usr/lib/swift/macosx"
                );
                println!("cargo:rustc-link-arg=-Wl,-rpath,{swift_lib_path}");
            }
        }
    }
}

fn cleanup_stale_resources() {
    // tauri-build copies bundle.resources into target/{triple}/{profile}/resources.
    // If a previous build copied readonly files (e.g. uv with mode 555), a later
    // overwrite can fail with EACCES during build script execution.
    if let Ok(out_dir) = env::var("OUT_DIR") {
        let mut target_dir = PathBuf::from(out_dir);
        for _ in 0..3 {
            if !target_dir.pop() {
                return;
            }
        }

        let resources_dir = target_dir.join("resources");
        if resources_dir.exists() {
            let _ = fs::remove_dir_all(resources_dir);
        }
    }
}

fn main() {
    cleanup_stale_resources();
    add_macos_swift_rpaths();
    tauri_build::build()
}
