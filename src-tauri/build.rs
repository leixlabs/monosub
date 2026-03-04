use std::{env, fs, path::PathBuf};

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
    tauri_build::build()
}
