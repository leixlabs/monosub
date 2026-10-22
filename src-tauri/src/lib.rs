//! MonoSub's shared implementation.
//!
//! Everything below used to live in `main.rs`'s module tree. It moved here so
//! that a second binary can reuse it: a `[[bin]]` is its own crate root, so
//! `src/bin/monosub-cli.rs` could not have said `use crate::engine` while the
//! modules were declared by `main.rs`. The GUI binary
//! (`src/main.rs`) is now a thin Tauri shell over this library.
//!
//! Layering, outermost first:
//!
//! | Module | Depends on Tauri? | Purpose |
//! |---|---|---|
//! | `commands`, `site`, `tray` | yes | Tauri commands, events, menu bar |
//! | `engine`, `subtitle`, `translate` | **no** | the actual work |
//! | `paths` | only via `cfg(feature = "tauri-app")` | where everything lives |
//! | `model_dir`, `logging` | settings/log files |
//!
//! That `engine`/`subtitle`/`translate` column is load-bearing: it is what lets
//! the CLI drive the same pipeline the GUI does
//! (`docs/tech-spec/cli-design.md` §3).

pub mod commands;
pub mod engine;
pub mod logging;
pub mod model_dir;
pub mod paths;
pub mod site;
pub mod subtitle;
pub mod translate;
pub mod tray;
