//! The `llama-server` child process: spawn, readiness probe, one completion
//! request, and teardown.
//!
//! Task: T3-1. See docs/tech-spec/translation-design.md §3.2 (a server
//! subprocess on a random loopback port, started per translation stage) and §8
//! (cancellation reclaims the child).
//!
//! **The process boundary is not an optimisation, it is the design** (§11):
//! audio.cpp and llama.cpp each embed their own ggml, so linking llama.cpp into
//! this process would collide with the ASR engine. Everything here talks to the
//! child over HTTP instead.
//!
//! Measured behaviour this module is written against (Apple M4, macOS arm64):
//! cold start 0.5–5 s, RSS ~1.4 GB, Metal confirmed active, `GET /health`
//! answering `{"status":"ok"}`, and `POST /v1/chat/completions` with the
//! sampling parameters below returning the text under
//! `.choices[0].message.content`.

use std::collections::VecDeque;
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use tokio::io::{AsyncBufReadExt, BufReader};

use super::error::TranslateError;

/// stderr kept for error reporting; same bound as `engine::cli` (spec §5.1).
const STDERR_TAIL_LINES: usize = 50;

/// How long the server may take to answer `/health` with `ok` before it is
/// killed. Measured cold start is 0.5–5 s; the generous ceiling covers a cold
/// page cache on a first run of a 1.1 GB GGUF.
const STARTUP_TIMEOUT: Duration = Duration::from_secs(120);

/// How often `/health` is polled while starting up.
const STARTUP_POLL_INTERVAL: Duration = Duration::from_millis(250);

/// Per-request read timeout for a single health probe: it must be shorter than
/// the poll interval, or a hung probe would eat the whole startup budget.
const HEALTH_REQUEST_TIMEOUT: Duration = Duration::from_secs(2);

/// Timeout for one completion request. Measured decode is ~42–53 tok/s, so a
/// 20-cue batch (~1000–2000 output tokens) finishes in well under a minute; the
/// ceiling is for a slow machine rather than for a bigger batch.
const COMPLETION_TIMEOUT: Duration = Duration::from_secs(300);

/// Context window passed as `-c`. The batched prompt is a few hundred tokens and
/// the answer at most a few thousand, so 4096 is ample and keeps the KV cache
/// small (translation-design §3.2).
const CONTEXT_LENGTH: u32 = 4096;

// --- Measured server parameters -------------------------------------------

/// GPU layers: offload everything; Metal was confirmed active with this.
const GPU_LAYERS: &str = "99";
/// One parallel slot: the translation stage is single-threaded by design.
const PARALLEL_SLOTS: &str = "1";
/// Log verbosity 1: warnings and errors only, so stderr stays a useful tail.
const LOG_VERBOSITY: &str = "1";

/// Sampling, verbatim from the verified PoC measurement.
pub const TEMPERATURE: f64 = 0.7;
pub const TOP_K: u32 = 20;
pub const TOP_P: f64 = 0.6;
pub const REPEAT_PENALTY: f64 = 1.05;

/// argv after the executable name, exactly as measured.
///
/// Pure and public so the command line can be pinned by a test instead of only
/// being describable in a comment.
pub fn server_args(model_path: &Path, port: u16) -> Vec<String> {
    [
        "--model".to_string(),
        model_path.to_string_lossy().into_owned(),
        "--host".to_string(),
        "127.0.0.1".to_string(),
        "--port".to_string(),
        port.to_string(),
        "-c".to_string(),
        CONTEXT_LENGTH.to_string(),
        "-ngl".to_string(),
        GPU_LAYERS.to_string(),
        "--no-webui".to_string(),
        "-np".to_string(),
        PARALLEL_SLOTS.to_string(),
        "-lv".to_string(),
        LOG_VERBOSITY.to_string(),
    ]
    .to_vec()
}

/// A boxed future, so [`Completer`] can be used as a trait object without an
/// async-trait dependency.
pub type BoxFuture<'a, T> = std::pin::Pin<Box<dyn std::future::Future<Output = T> + Send + 'a>>;

/// One chat completion against a translation backend.
///
/// The batching/validation/degradation logic in [`super::translate_with`] is
/// written against this trait rather than against [`LlamaServer`] directly, so
/// it can be exercised end to end against a local HTTP listener without the
/// bundled binary or the 1.1 GB model.
pub trait Completer: Send + Sync {
    fn complete<'a>(&'a self, prompt: &'a str, max_tokens: u32) -> BoxFuture<'a, Result<String, TranslateError>>;
}

/// A running `llama-server`, killed when this value drops.
pub struct LlamaServer {
    /// `None` only after [`Drop`] took the child; kept as an `Option` so the
    /// kill is explicit rather than relying on `kill_on_drop` alone.
    ///
    /// Behind a `Mutex` so the exit probe in `complete` can take `&self` through
    /// the [`Completer`] trait object, which is what the batch loop is written
    /// against.
    child: Mutex<Option<tokio::process::Child>>,
    client: reqwest::Client,
    port: u16,
    /// Last [`STDERR_TAIL_LINES`] stderr lines, shared with the drain task.
    stderr_tail: Arc<Mutex<VecDeque<String>>>,
    /// Kept for logging and for the `Drop` message.
    model_path: PathBuf,
}

impl LlamaServer {
    /// Start a server for `model_path` and wait until it answers `/health`.
    ///
    /// `binary` is the resolved `llama-server` path; a bare name (the PATH
    /// fallback of `engine::cli`) is passed to the OS lookup and reported as
    /// [`TranslateError::BinaryMissing`] when that lookup fails.
    pub async fn start(binary: &Path, model_path: &Path) -> Result<Self, TranslateError> {
        ensure_model(model_path)?;

        if let Some(parent) = binary.parent() {
            if !parent.as_os_str().is_empty() && !binary.is_file() {
                return Err(TranslateError::BinaryMissing(format!(
                    "{} is not a file (run ./scripts/build/fetch-llamacpp.sh)",
                    binary.display()
                )));
            }
        }

        let port = reserve_port()?;
        let argv = server_args(model_path, port);

        tracing::info!(
            binary = %binary.display(),
            model = %model_path.display(),
            port,
            argv = ?argv,
            "Starting llama-server for subtitle translation"
        );

        let mut command = tokio::process::Command::new(binary);
        command
            .args(&argv)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);

        let mut child = command.spawn().map_err(|e| {
            TranslateError::BinaryMissing(format!("failed to spawn {}: {}", binary.display(), e))
        })?;

        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| TranslateError::ServerStart("no llama-server stdout".to_string()))?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| TranslateError::ServerStart("no llama-server stderr".to_string()))?;

        // Both pipes must be drained: llama-server logs on startup, and a full
        // pipe would block the child before it ever becomes healthy.
        tokio::spawn(async move {
            let mut reader = BufReader::new(stdout).lines();
            while let Ok(Some(_)) = reader.next_line().await {}
        });

        let stderr_tail: Arc<Mutex<VecDeque<String>>> = Arc::new(Mutex::new(VecDeque::new()));
        let stderr_sink = Arc::clone(&stderr_tail);
        tokio::spawn(async move {
            let mut reader = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = reader.next_line().await {
                let mut tail = stderr_sink.lock().expect("stderr ring poisoned");
                if tail.len() == STDERR_TAIL_LINES {
                    tail.pop_front();
                }
                tail.push_back(line);
            }
        });

        let client = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(5))
            .build()
            .map_err(|e| TranslateError::ServerStart(format!("http client: {}", e)))?;

        let mut server = Self {
            child: Mutex::new(Some(child)),
            client,
            port,
            stderr_tail,
            model_path: model_path.to_path_buf(),
        };

        match server.await_healthy().await {
            Ok(()) => {
                tracing::info!(port, model = %model_path.display(), "llama-server is ready");
                Ok(server)
            }
            Err(error) => {
                // Take the child out and kill it here rather than waiting for
                // the caller to drop the value: a startup failure must not leave
                // a model-loading process behind.
                server.kill();
                Err(error)
            }
        }
    }

    /// Poll `/health` until it reports `ok`, the deadline passes, or the child
    /// dies. A dead child is reported immediately with its stderr tail, so a
    /// bad model file does not cost the whole startup budget.
    async fn await_healthy(&self) -> Result<(), TranslateError> {
        let deadline = Instant::now() + STARTUP_TIMEOUT;
        let url = format!("http://127.0.0.1:{}/health", self.port);

        loop {
            if let Some(status) = self.exited_status() {
                return Err(TranslateError::ServerStart(format!(
                    "llama-server exited ({}) during startup — stderr: {}",
                    status,
                    self.stderr_text()
                )));
            }

            if self.health_ok(&url).await {
                return Ok(());
            }

            if Instant::now() >= deadline {
                return Err(TranslateError::ServerStart(format!(
                    "llama-server did not become healthy within {} s — stderr: {}",
                    STARTUP_TIMEOUT.as_secs(),
                    self.stderr_text()
                )));
            }

            tokio::time::sleep(STARTUP_POLL_INTERVAL).await;
        }
    }

    /// One `/health` probe; any transport error is simply "not ready yet".
    async fn health_ok(&self, url: &str) -> bool {
        let Ok(response) = self
            .client
            .get(url)
            .timeout(HEALTH_REQUEST_TIMEOUT)
            .send()
            .await
        else {
            return false;
        };
        if !response.status().is_success() {
            return false;
        }
        match response.text().await {
            Ok(body) => body.contains("\"status\":\"ok\"") || body.contains("\"status\": \"ok\""),
            Err(_) => false,
        }
    }

    /// The child's exit status when it has already finished.
    fn exited_status(&self) -> Option<String> {
        let mut child = self.child.lock().expect("server child mutex poisoned");
        let child = child.as_mut()?;
        match child.try_wait() {
            Ok(Some(status)) => Some(format!("{:?}", status.code())),
            _ => None,
        }
    }

    /// Ring-buffer contents, oldest first, joined for one error line.
    fn stderr_text(&self) -> String {
        self.stderr_tail
            .lock()
            .map(|tail| tail.iter().cloned().collect::<Vec<_>>().join("\n"))
            .unwrap_or_default()
    }

    /// Kill the child if it is still alive. Idempotent.
    fn kill(&mut self) {
        let mut slot = self.child.lock().expect("server child mutex poisoned");
        if let Some(child) = slot.as_mut() {
            // `start_kill` only signals; the process is reaped by `kill_on_drop`
            // when the `Child` is dropped with this struct, which is also what
            // makes an aborted task reclaim the server.
            let _ = child.start_kill();
        }
        *slot = None;
    }

    /// One completion through the running server.
    ///
    /// A child that is no longer running reports [`TranslateError::Cancelled`]
    /// rather than a transport error: nothing more can be asked of it, and the
    /// batch loop stops instead of failing the same way for every remaining
    /// batch. That is the shape a cancelled task leaves behind (§8).
    pub async fn complete(&self, prompt: &str, max_tokens: u32) -> Result<String, TranslateError> {
        if self.exited_status().is_some() {
            return Err(TranslateError::Cancelled);
        }
        let url = format!("http://127.0.0.1:{}/v1/chat/completions", self.port);
        post_completion(&self.client, &url, prompt, max_tokens).await
    }
}

impl Completer for LlamaServer {
    fn complete<'a>(
        &'a self,
        prompt: &'a str,
        max_tokens: u32,
    ) -> BoxFuture<'a, Result<String, TranslateError>> {
        Box::pin(LlamaServer::complete(self, prompt, max_tokens))
    }
}

impl Drop for LlamaServer {
    /// Cancellation reclaims the server: aborting the transcription task drops
    /// this value, which kills the child (translation-design §8). `kill_on_drop`
    /// on the `Command` is the belt to this brace — an abort between `spawn` and
    /// the `Child` being stored still kills the process.
    fn drop(&mut self) {
        if self
            .child
            .lock()
            .map(|slot| slot.is_some())
            .unwrap_or(false)
        {
            tracing::info!(
                port = self.port,
                model = %self.model_path.display(),
                "Stopping llama-server"
            );
        }
        self.kill();
    }
}

/// The bundled binary must exist before a model is loaded, but a missing model
/// is the more common case, so it is checked first (it is also the cheaper
/// error to act on: download the model rather than reinstall the engine).
fn ensure_model(model_path: &Path) -> Result<(), TranslateError> {
    if model_path.is_file() {
        return Ok(());
    }
    Err(TranslateError::ModelMissing(format!(
        "{} is not on disk",
        model_path.display()
    )))
}

/// Bind `127.0.0.1:0` to let the OS pick a free port, then release it.
///
/// There is an unavoidable race between the release and the child binding it,
/// which is why the readiness probe (not the bind) decides success: if another
/// process steals the port the server never becomes healthy and startup fails
/// with a clear error instead of talking to a stranger.
fn reserve_port() -> Result<u16, TranslateError> {
    let listener = TcpListener::bind("127.0.0.1:0")
        .map_err(|e| TranslateError::ServerStart(format!("could not reserve a port: {}", e)))?;
    listener
        .local_addr()
        .map(|addr| addr.port())
        .map_err(|e| TranslateError::ServerStart(format!("could not read the reserved port: {}", e)))
}

/// POST one chat completion and return the assistant text.
///
/// Shared with the test mock so the request body and the response shape are
/// covered by the same code the real server path uses.
pub async fn post_completion(
    client: &reqwest::Client,
    url: &str,
    prompt: &str,
    max_tokens: u32,
) -> Result<String, TranslateError> {
    // Structured output (`response_format` / GBNF) is deliberately NOT used:
    // measured against this model it made the JSON-schema variant echo the
    // source untranslated and the grammar variant return `[]`. The numbered-line
    // contract plus `parse` is the measured-reliable path.
    let body = serde_json::json!({
        "messages": [{ "role": "user", "content": prompt }],
        "temperature": TEMPERATURE,
        "top_k": TOP_K,
        "top_p": TOP_P,
        "repeat_penalty": REPEAT_PENALTY,
        "n_predict": max_tokens,
        "cache_prompt": true,
    });

    let response = client
        .post(url)
        .timeout(COMPLETION_TIMEOUT)
        .json(&body)
        .send()
        .await
        .map_err(|e| TranslateError::Request(format!("POST {}: {}", url, e)))?;

    let status = response.status();
    let text = response
        .text()
        .await
        .map_err(|e| TranslateError::Request(format!("reading the response: {}", e)))?;

    if !status.is_success() {
        return Err(TranslateError::Request(format!(
            "HTTP {}: {}",
            status,
            text.chars().take(200).collect::<String>()
        )));
    }

    let parsed: serde_json::Value = serde_json::from_str(&text)
        .map_err(|e| TranslateError::Parse(format!("response is not JSON: {}", e)))?;

    parsed
        .get("choices")
        .and_then(|choices| choices.get(0))
        .and_then(|choice| choice.get("message"))
        .and_then(|message| message.get("content"))
        .and_then(|content| content.as_str())
        .map(|content| content.to_string())
        .ok_or_else(|| {
            TranslateError::Parse(format!(
                "no choices[0].message.content in the response: {}",
                text.chars().take(200).collect::<String>()
            ))
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn server_args_match_the_measured_command_line() {
        let args = server_args(Path::new("/models/HY-MT/HY-MT1.5-1.8B-Q4_K_M.gguf"), 51234);

        assert_eq!(
            args,
            vec![
                "--model",
                "/models/HY-MT/HY-MT1.5-1.8B-Q4_K_M.gguf",
                "--host",
                "127.0.0.1",
                "--port",
                "51234",
                "-c",
                "4096",
                "-ngl",
                "99",
                "--no-webui",
                "-np",
                "1",
                "-lv",
                "1",
            ]
        );
    }

    #[test]
    fn reserving_a_port_yields_a_bindable_loopback_port() {
        let port = reserve_port().expect("reserve");
        assert_ne!(port, 0);
        // The reservation is released, so the child (or this test) can rebind it.
        let listener = TcpListener::bind(("127.0.0.1", port)).expect("rebind the reserved port");
        assert_eq!(listener.local_addr().unwrap().port(), port);
    }

    #[test]
    fn missing_model_is_reported_before_anything_is_spawned() {
        let error = ensure_model(Path::new("/nowhere/HY-MT1.5-1.8B-Q4_K_M.gguf")).unwrap_err();
        assert!(matches!(error, TranslateError::ModelMissing(_)), "{error:?}");
        assert_eq!(error.status(), "modelNotInstalled");
        assert!(ensure_model(Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml"))).is_ok());
    }

    /// A bundled path that does not exist is a missing binary; a bare name is
    /// handed to the OS lookup and only fails at spawn time.
    #[tokio::test]
    async fn missing_binary_is_reported_as_binary_missing() {
        let error = match LlamaServer::start(
            Path::new("/nowhere/llama-server"),
            Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml")),
        )
        .await
        {
            Err(error) => error,
            Ok(_) => panic!("a missing binary must not start a server"),
        };

        assert!(matches!(error, TranslateError::BinaryMissing(_)), "{error:?}");
        assert_eq!(error.status(), "engineMissing");
    }

    /// A bare name (`engine::cli`'s PATH fallback) that PATH cannot resolve must
    /// also come back as `BinaryMissing`, not as a panic.
    #[tokio::test]
    async fn unresolvable_bare_name_is_binary_missing() {
        let error = match LlamaServer::start(
            Path::new("monosub-definitely-not-a-real-binary"),
            Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml")),
        )
        .await
        {
            Err(error) => error,
            Ok(_) => panic!("an unresolvable binary must not start a server"),
        };

        assert!(matches!(error, TranslateError::BinaryMissing(_)), "{error:?}");
    }

    /// The bundled server is gitignored, so the live-server tests skip on a
    /// fresh clone rather than failing.
    fn bundled_server_and_model() -> Option<(PathBuf, PathBuf)> {
        let resources = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/resources"));
        let triple = "aarch64-apple-darwin";
        for candidate in [
            resources.join("bin").join(format!("llama-server-{triple}")),
            resources.join("bin").join("llama-server"),
        ] {
            if candidate.is_file() {
                // Any GGUF will do for a smoke check; the translation model is
                // the one that is normally present.
                let model_dir = std::env::var_os("MONOSUB_MODELS_DIR")
                    .map(PathBuf::from)
                    .unwrap_or_else(|| resources.join("..").join("models"));
                let model = model_dir
                    .join("HY-MT1.5-1.8B-GGUF")
                    .join("HY-MT1.5-1.8B-Q4_K_M.gguf");
                if model.is_file() {
                    return Some((candidate, model));
                }
                eprintln!("skip: translation model not present at {}", model.display());
                return None;
            }
        }
        eprintln!("skip: bundled llama-server not present (run ./scripts/build/fetch-llamacpp.sh)");
        None
    }

    /// Live-server smoke test: the child starts, answers `/health`, completes one
    /// request, and is gone when the guard drops.
    #[tokio::test]
    async fn live_server_starts_answers_health_and_completes() {
        let Some((binary, model)) = bundled_server_and_model() else {
            return;
        };

        let server = LlamaServer::start(&binary, &model)
            .await
            .expect("bundled llama-server starts");
        assert_ne!(server.port, 0);

        let answer = server
            .complete("1. Hello\n", 64)
            .await
            .expect("one completion succeeds");
        assert!(!answer.is_empty(), "the model must answer something");

        let pid = server
            .child
            .lock()
            .unwrap()
            .as_ref()
            .and_then(|child| child.id());
        drop(server);

        if let Some(pid) = pid {
            // The child is signalled on drop; give the OS a moment to reap it.
            tokio::time::sleep(Duration::from_millis(300)).await;
            assert!(
                !process_alive(pid),
                "llama-server {pid} survived the guard being dropped"
            );
        }
    }

    fn process_alive(pid: u32) -> bool {
        std::process::Command::new("kill")
            .args(["-0", &pid.to_string()])
            .status()
            .map(|status| status.success())
            .unwrap_or(false)
    }
}
