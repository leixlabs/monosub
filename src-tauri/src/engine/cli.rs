//! audio.cpp CLI process management: path resolution, spawn, line callbacks,
//! stderr ring buffer, exit-code mapping.
//!
//! Task: P1-6. See docs/tech-spec/refactor-audiocpp.md §5.1.
//!
//! Path resolution must reuse the ffmpeg rule in `commands::video`
//! (dev: `src-tauri/resources/bin/`; prod: `Resources/resources/bin/`).
//!
//! Pinned baseline — keep these signatures stable for P1-8/P1-9/P1-15.

use std::path::PathBuf;

/// Where a `--backend` value came from; P1-9 retries `metal` failures on `cpu`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Backend {
    Metal,
    Cpu,
}

/// A finished or running CLI invocation request.
#[derive(Debug, Clone)]
pub struct CliInvocation {
    pub task: String,
    pub family: String,
    pub model: PathBuf,
    pub audio: Option<PathBuf>,
    pub backend: Backend,
    pub args: Vec<String>,
}

/// Captured result of a completed CLI run.
#[derive(Debug, Clone)]
pub struct CliOutput {
    pub status: std::process::ExitStatus,
    pub stdout: String,
    /// Last 50 stderr lines, joined with `\n` (spec §5.1).
    pub stderr_tail: String,
}

/// Errors surfaced to `transcribe_error` (spec §5.3 error table).
#[derive(Debug, Clone)]
pub enum EngineError {
    /// Binary missing or not executable → `errors.engineMissing`.
    Missing(String),
    /// Non-zero exit → `errors.engineFailed`.
    Failed { message: String, stderr_tail: String },
    /// ffmpeg non-zero → `errors.audioExtractFailed`.
    AudioExtractFailed(String),
    /// Spawn/IO failure.
    Io(String),
    /// Malformed WAV input.
    InvalidAudio(String),
    /// CLI produced unparseable output.
    Protocol(String),
}

impl std::fmt::Display for EngineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EngineError::Missing(m) => write!(f, "engine missing: {}", m),
            EngineError::Failed { message, .. } => write!(f, "engine failed: {}", message),
            EngineError::AudioExtractFailed(m) => write!(f, "audio extract failed: {}", m),
            EngineError::Io(m) => write!(f, "io error: {}", m),
            EngineError::InvalidAudio(m) => write!(f, "invalid audio: {}", m),
            EngineError::Protocol(m) => write!(f, "protocol error: {}", m),
        }
    }
}

impl std::error::Error for EngineError {}

/// Resolve the bundled `audiocpp_cli-<target-triple>` binary.
pub fn resolve_cli_path(app: Option<&tauri::AppHandle>) -> Result<PathBuf, EngineError> {
    unimplemented!("P1-6")
}

/// Resolve the bundled silero VAD directory
/// (`resources/models/silero_vad`, containing `silero_vad_16k.safetensors`).
pub fn resolve_vad_path(app: Option<&tauri::AppHandle>) -> Result<PathBuf, EngineError> {
    unimplemented!("P1-6")
}

/// Resolve the bundled `models.json` path.
pub fn resolve_catalog_path(app: Option<&tauri::AppHandle>) -> Result<PathBuf, EngineError> {
    unimplemented!("P1-6")
}

/// Spawn the CLI and call `on_line` for each stdout line as it arrives.
///
/// `stderr` is drained concurrently into a bounded 50-line ring buffer and
/// returned in [`CliOutput::stderr_tail`]. Non-zero exit maps to
/// [`EngineError::Failed`].
pub async fn spawn(
    app: Option<&tauri::AppHandle>,
    invocation: CliInvocation,
    on_line: impl FnMut(&str) + Send,
) -> Result<CliOutput, EngineError> {
    let _ = (app, invocation, on_line);
    unimplemented!("P1-6")
}
