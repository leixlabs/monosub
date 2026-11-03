//! The `llama-server` child process: spawn, readiness probe, one completion
//! request, and teardown.
//!
//! Task: T3-1. See docs/tech-spec/translation-design.md §3.2 (a server
//! subprocess started per translation stage) and §8 (cancellation reclaims the
//! child).
//!
//! **The process boundary is not an optimisation, it is the design** (§11):
//! audio.cpp and llama.cpp each embed their own ggml, so linking llama.cpp into
//! this process would collide with the ASR engine. Everything here talks to the
//! child over HTTP instead.
//!
//! ## Why the transport is a Unix domain socket on macOS
//!
//! The child is spawned by a sandboxed app, so it inherits the app's sandbox
//! (`com.apple.security.inherit`, see `entitlements/MonoSubHelper.mas.entitlements`).
//! Accepting an incoming connection on a TCP socket — even on `127.0.0.1` —
//! needs `com.apple.security.network.server` on the **app**, and App Review
//! rejects that entitlement when its static scan cannot see the listener. The
//! listener is in a child process that only exists while a translation is
//! running, so the scan never sees it: a loopback port costs a review round
//! trip on every submission.
//!
//! A Unix domain socket is a file, not a network endpoint: it needs no network
//! entitlement, and it cannot be reached from another machine even in
//! principle. `llama-server --host <path>.sock` binds one, and the HTTP API is
//! unchanged — only the transport is. That is also why this module speaks
//! HTTP/1.1 over `UnixStream` itself rather than using `reqwest`: no HTTP client
//! in the tree can address a socket file.
//!
//! This is not wishful thinking about the sandbox — it is how the shipped
//! profile is written. `application.sb` contains
//! `(allow network-bind network-outbound (resolving-subpath (param "application_container")))`,
//! i.e. sockets under the container are allowed unconditionally, and verified on
//! a signed `app-sandbox` binary holding only `network.client`: bind, listen,
//! accept and a full round trip under `Data/tmp` all succeeded, while the same
//! bind under `/tmp` failed with EPERM. [`socket_path`] therefore has to stay on
//! `TMPDIR`; moving it to `/tmp` would break translation inside the sandbox.
//!
//! Windows keeps loopback TCP: llama.cpp's `--host` accepts only IP addresses and
//! `*.sock` paths, its AF_UNIX support is POSIX-only, and there is no App
//! Sandbox there, so nothing would be gained.
//!
//! Measured behaviour this module is written against (Apple M4, macOS arm64):
//! cold start 0.5–5 s, RSS ~1.4 GB, Metal confirmed active, `GET /health`
//! answering `{"status":"ok"}`, and `POST /v1/chat/completions` with the
//! sampling parameters below returning the text under
//! `.choices[0].message.content`. Re-measured over a Unix socket: identical
//! responses, no request-path changes.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::process::Stdio;
#[cfg(unix)]
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, BufReader};

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

/// How long the parent may spend connecting before declaring the child absent.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);

/// Context window passed as `-c`. The batched prompt is a few hundred tokens and
/// the answer at most a few thousand, so 4096 is ample and keeps the KV cache
/// small (translation-design §3.2).
const CONTEXT_LENGTH: u32 = 4096;

/// `sockaddr_un.sun_path` is 104 bytes on macOS and Linux alike, including the
/// terminating NUL, so 103 is the longest path that can be bound. The container
/// `TMPDIR` inside the sandbox leaves room, but the check is here rather than in
/// a comment because the failure mode (a truncated socket path binding
/// somewhere unexpected) is miserable to diagnose.
#[cfg(unix)]
const MAX_SOCKET_PATH: usize = 103;

/// Suffix llama.cpp requires: `--host` treats an argument as a socket only when
/// it ends in `.sock`.
#[cfg(unix)]
const SOCKET_SUFFIX: &str = ".sock";

/// Makes each run's socket name unique inside one process. Two translations are
/// not expected to overlap (the queue runs them one at a time), but a shared
/// name would turn an overlap into a confusing bind failure instead of a second
/// socket.
#[cfg(unix)]
static SOCKET_SEQ: AtomicU64 = AtomicU64::new(0);

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

/// Where the child listens, and therefore what the parent dials.
///
/// Deliberately an explicit type rather than a URL string: the transport decides
/// which sandbox capability is needed, so that must be visible in the code
/// rather than implied by the shape of a string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Endpoint {
    /// Unix domain socket. No network entitlement is involved: the socket is a
    /// file in the app container's temporary directory.
    #[cfg(unix)]
    Socket(PathBuf),
    /// Loopback TCP. Windows only, where llama.cpp has no AF_UNIX support.
    Tcp { port: u16 },
}

impl std::fmt::Display for Endpoint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            #[cfg(unix)]
            Endpoint::Socket(path) => write!(f, "unix:{}", path.display()),
            Endpoint::Tcp { port } => write!(f, "tcp:127.0.0.1:{}", port),
        }
    }
}

impl Endpoint {
    /// The name the child should bind, and the parent will dial.
    ///
    /// Chosen here and only here, so the log line, the argv and every request
    /// all agree on it.
    fn for_child() -> Result<Self, TranslateError> {
        #[cfg(unix)]
        {
            let path = socket_path()?;
            // A leftover file from a crashed run would make bind() fail with
            // EADDRINUSE. Names are unique per process, so this only cleans up
            // the same-process restart case, but it is one line.
            let _ = std::fs::remove_file(&path);
            Ok(Endpoint::Socket(path))
        }
        #[cfg(windows)]
        {
            Ok(Endpoint::Tcp {
                port: reserve_port()?,
            })
        }
    }

    /// One HTTP/1.1 request over this endpoint.
    ///
    /// `Connection: close` is sent and at most one request is made per
    /// connection: the translation stage issues one health probe per 250 ms and
    /// one completion per 20 cues, so connection reuse would buy nothing and
    /// would cost response-framing code that has to handle keep-alive.
    async fn request(
        &self,
        method: &str,
        path: &str,
        body: Option<&str>,
        timeout: Duration,
    ) -> Result<(u16, String), TranslateError> {
        let payload = build_request(method, path, body);
        let raw = self.exchange(payload.as_bytes(), timeout).await?;
        parse_response(&raw)
    }

    async fn exchange(&self, payload: &[u8], timeout: Duration) -> Result<Vec<u8>, TranslateError> {
        match self {
            #[cfg(unix)]
            Endpoint::Socket(path) => {
                let stream = tokio::time::timeout(CONNECT_TIMEOUT, connect_socket(path))
                    .await
                    .map_err(|_| {
                        TranslateError::Request(format!(
                            "connecting to {} timed out",
                            path.display()
                        ))
                    })??;
                read_response(stream, payload, timeout).await
            }
            Endpoint::Tcp { port } => {
                let stream = tokio::time::timeout(
                    CONNECT_TIMEOUT,
                    tokio::net::TcpStream::connect(("127.0.0.1", *port)),
                )
                .await
                .map_err(|_| {
                    TranslateError::Request(format!("connecting to 127.0.0.1:{} timed out", port))
                })?
                .map_err(|error| {
                    TranslateError::Request(format!("connecting to 127.0.0.1:{}: {}", port, error))
                })?;
                read_response(stream, payload, timeout).await
            }
        }
    }

    /// One streaming POST: `on_body` sees response-body bytes as they arrive.
    ///
    /// Same transports, same framing, one difference — it hands over the body
    /// while it is still growing instead of returning it whole.
    async fn post_stream(
        &self,
        path: &str,
        body: &str,
        timeout: Duration,
        on_body: &mut (dyn FnMut(&[u8]) + Send),
    ) -> Result<u16, TranslateError> {
        let payload = build_request("POST", path, Some(body));
        match self {
            #[cfg(unix)]
            Endpoint::Socket(socket) => {
                let stream = tokio::time::timeout(CONNECT_TIMEOUT, connect_socket(socket))
                    .await
                    .map_err(|_| {
                        TranslateError::Request(format!(
                            "connecting to {} timed out",
                            socket.display()
                        ))
                    })??;
                read_stream(stream, payload.as_bytes(), timeout, on_body).await
            }
            Endpoint::Tcp { port } => {
                let stream = tokio::time::timeout(
                    CONNECT_TIMEOUT,
                    tokio::net::TcpStream::connect(("127.0.0.1", *port)),
                )
                .await
                .map_err(|_| {
                    TranslateError::Request(format!("connecting to 127.0.0.1:{} timed out", port))
                })?
                .map_err(|error| {
                    TranslateError::Request(format!("connecting to 127.0.0.1:{}: {}", port, error))
                })?;
                read_stream(stream, payload.as_bytes(), timeout, on_body).await
            }
        }
    }
}

#[cfg(unix)]
async fn connect_socket(path: &Path) -> Result<tokio::net::UnixStream, TranslateError> {
    tokio::net::UnixStream::connect(path).await.map_err(|error| {
        TranslateError::Request(format!("connecting to {}: {}", path.display(), error))
    })
}

/// The socket name for this run: unique per process, inside `TMPDIR`.
///
/// **The location is load-bearing, not cosmetic.** `application.sb` (the App
/// Sandbox profile) grants exactly this:
///
/// ```text
/// (allow network-bind network-outbound (resolving-subpath (param "application_container")))
/// ```
///
/// so a bind under the container is allowed unconditionally, while a bind
/// anywhere else is denied — measured on a signed `app-sandbox` binary with only
/// `network.client`: binding `$HOME/Library/Containers/<id>/Data/tmp/x.sock`
/// worked (bind, listen, accept and a full round trip), binding
/// `/tmp/x.sock` failed with EPERM. Inside the sandbox `TMPDIR` *is*
/// `Data/tmp` of the container, which is what makes this work; a socket in
/// `/tmp` would break the feature.
#[cfg(unix)]
fn socket_path() -> Result<PathBuf, TranslateError> {
    use std::os::unix::ffi::OsStrExt;

    let sequence = SOCKET_SEQ.fetch_add(1, Ordering::Relaxed);
    let directory = std::env::temp_dir();

    // Long user names plus a long container identifier can push a descriptive
    // name over `sun_path`; a shorter one is better than a failure, so the
    // candidates are tried in order of decreasing verbosity.
    let candidates = [
        format!("monosub-llama-{}-{}{}", std::process::id(), sequence, SOCKET_SUFFIX),
        format!("ms-{}-{}{}", std::process::id(), sequence, SOCKET_SUFFIX),
    ];

    let mut longest = 0;
    for name in candidates {
        let path = directory.join(name);
        let length = path.as_os_str().as_bytes().len();
        if length <= MAX_SOCKET_PATH {
            return Ok(path);
        }
        longest = longest.max(length);
    }

    Err(TranslateError::ServerStart(format!(
        "socket path is {} bytes, over the {} byte sun_path limit in {}",
        longest,
        MAX_SOCKET_PATH,
        directory.display()
    )))
}

/// Bind `127.0.0.1:0` to let the OS pick a free port, then release it.
///
/// Windows only (see the module docs). There is an unavoidable race between the
/// release and the child binding it, which is why the readiness probe (not the
/// bind) decides success: if another process steals the port the server never
/// becomes healthy and startup fails with a clear error instead of talking to a
/// stranger.
#[cfg(windows)]
fn reserve_port() -> Result<u16, TranslateError> {
    let listener = std::net::TcpListener::bind("127.0.0.1:0")
        .map_err(|e| TranslateError::ServerStart(format!("could not reserve a port: {}", e)))?;
    listener
        .local_addr()
        .map(|addr| addr.port())
        .map_err(|e| TranslateError::ServerStart(format!("could not read the reserved port: {}", e)))
}

/// argv after the executable name.
///
/// Pure and public so the command line can be pinned by a test instead of only
/// being describable in a comment. The socket form deliberately omits `--port`:
/// llama.cpp reads `--host` as a path when it ends in `.sock` and then ignores
/// the port entirely, so passing both would only invite the assumption that the
/// port matters.
pub fn server_args(model_path: &Path, endpoint: &Endpoint) -> Vec<String> {
    let mut argv = vec![
        "--model".to_string(),
        model_path.to_string_lossy().into_owned(),
        "--host".to_string(),
    ];

    match endpoint {
        #[cfg(unix)]
        Endpoint::Socket(path) => argv.push(path.to_string_lossy().into_owned()),
        Endpoint::Tcp { port } => {
            argv.push("127.0.0.1".to_string());
            argv.push("--port".to_string());
            argv.push(port.to_string());
        }
    }

    argv.extend([
        "-c".to_string(),
        CONTEXT_LENGTH.to_string(),
        "-ngl".to_string(),
        GPU_LAYERS.to_string(),
        "--no-webui".to_string(),
        "-np".to_string(),
        PARALLEL_SLOTS.to_string(),
        "-lv".to_string(),
        LOG_VERBOSITY.to_string(),
    ]);

    argv
}

// --- HTTP/1.1, small enough to own ----------------------------------------

fn build_request(method: &str, path: &str, body: Option<&str>) -> String {
    let mut request = format!(
        "{} {} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n",
        method, path
    );
    if let Some(body) = body {
        request.push_str("Content-Type: application/json\r\n");
        request.push_str(&format!("Content-Length: {}\r\n", body.len()));
    }
    request.push_str("\r\n");
    if let Some(body) = body {
        request.push_str(body);
    }
    request
}

/// Write the request and read the response, stopping as soon as the response is
/// complete instead of waiting for the peer to close.
///
/// Generic over the stream so the socket and TCP transports share every line of
/// the framing logic — the transport is the only thing that differs.
async fn read_response<S>(
    mut stream: S,
    payload: &[u8],
    timeout: Duration,
) -> Result<Vec<u8>, TranslateError>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let exchange = async {
        stream.write_all(payload).await?;
        stream.flush().await?;

        let mut raw = Vec::with_capacity(2048);
        let mut chunk = [0u8; 4096];
        loop {
            let read = stream.read(&mut chunk).await?;
            if read == 0 {
                break;
            }
            raw.extend_from_slice(&chunk[..read]);
            if response_complete(&raw) {
                break;
            }
        }
        Ok::<Vec<u8>, std::io::Error>(raw)
    };

    tokio::time::timeout(timeout, exchange)
        .await
        .map_err(|_| TranslateError::Request(format!("no complete response within {:?}", timeout)))?
        .map_err(|error| TranslateError::Request(format!("socket i/o: {}", error)))
}

/// Index just past the blank line that ends the header block.
fn head_end(raw: &[u8]) -> Option<usize> {
    raw.windows(4).position(|w| w == b"\r\n\r\n").map(|i| i + 4)
}

/// The streaming twin of [`read_response`]: same request, but every body byte is
/// handed to `on_body` as it arrives and the call returns once the body ends.
///
/// A non-2xx answer is not worth streaming — it is read (bounded) and turned
/// into the same `HTTP <status>: <body>` error the whole-response path produces,
/// so a caller cannot tell the two apart by its error text.
async fn read_stream<S>(
    mut stream: S,
    payload: &[u8],
    timeout: Duration,
    on_body: &mut (dyn FnMut(&[u8]) + Send),
) -> Result<u16, TranslateError>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let exchange = async {
        stream
            .write_all(payload)
            .await
            .map_err(|error| TranslateError::Request(format!("socket i/o: {}", error)))?;
        stream
            .flush()
            .await
            .map_err(|error| TranslateError::Request(format!("socket i/o: {}", error)))?;

        let mut raw = Vec::with_capacity(2048);
        let mut chunk = [0u8; 4096];
        let split = loop {
            if let Some(split) = head_end(&raw) {
                break split;
            }
            let read = stream
                .read(&mut chunk)
                .await
                .map_err(|error| TranslateError::Request(format!("socket i/o: {}", error)))?;
            if read == 0 {
                return Err(TranslateError::Request(
                    "connection closed before the response headers".to_string(),
                ));
            }
            raw.extend_from_slice(&chunk[..read]);
        };

        let head = String::from_utf8_lossy(&raw[..split]).into_owned();
        let status = head
            .lines()
            .next()
            .and_then(|line| line.split_whitespace().nth(1))
            .and_then(|code| code.parse::<u16>().ok())
            .ok_or_else(|| {
                TranslateError::Request(format!(
                    "response has no status code: {:?}",
                    head.lines().next().unwrap_or_default()
                ))
            })?;

        if !(200..300).contains(&status) {
            let mut text = raw[split..].to_vec();
            while text.len() < 512 {
                match stream.read(&mut chunk).await {
                    Ok(0) | Err(_) => break,
                    Ok(read) => text.extend_from_slice(&chunk[..read]),
                }
            }
            return Err(TranslateError::Request(format!(
                "HTTP {}: {}",
                status,
                String::from_utf8_lossy(&text)
                    .chars()
                    .take(200)
                    .collect::<String>()
            )));
        }

        if is_chunked(&head) {
            let mut body = ChunkedStream::default();
            body.push(&raw[split..]);
            loop {
                let data = body
                    .drain()
                    .map_err(|error| TranslateError::Request(format!("chunked response: {}", error)))?;
                if !data.is_empty() {
                    on_body(&data);
                }
                if body.finished {
                    break;
                }
                let read = stream
                    .read(&mut chunk)
                    .await
                    .map_err(|error| TranslateError::Request(format!("socket i/o: {}", error)))?;
                if read == 0 {
                    break;
                }
                body.push(&chunk[..read]);
            }
        } else {
            // No framing to undo: the body runs to EOF (or Content-Length, which
            // a streamed answer never sends).
            if raw.len() > split {
                on_body(&raw[split..]);
            }
            loop {
                let read = stream
                    .read(&mut chunk)
                    .await
                    .map_err(|error| TranslateError::Request(format!("socket i/o: {}", error)))?;
                if read == 0 {
                    break;
                }
                on_body(&chunk[..read]);
            }
        }

        Ok(status)
    };

    tokio::time::timeout(timeout, exchange)
        .await
        .map_err(|_| TranslateError::Request(format!("no complete response within {:?}", timeout)))?
}

fn header_value<'a>(head: &'a str, name: &str) -> Option<&'a str> {
    head.lines().skip(1).find_map(|line| {
        let (key, value) = line.split_once(':')?;
        key.trim().eq_ignore_ascii_case(name).then(|| value.trim())
    })
}

fn is_chunked(head: &str) -> bool {
    header_value(head, "transfer-encoding")
        .map(|value| value.to_ascii_lowercase().contains("chunked"))
        .unwrap_or(false)
}

/// True once `raw` holds a whole response, so the read loop can stop without
/// waiting for the peer to close the connection.
fn response_complete(raw: &[u8]) -> bool {
    let Some(split) = head_end(raw) else {
        return false;
    };
    let (head, body) = raw.split_at(split);
    let head = String::from_utf8_lossy(head);

    if is_chunked(&head) {
        // The terminating zero-size chunk is the only unambiguous end marker.
        return body.ends_with(b"0\r\n\r\n");
    }
    match header_value(&head, "content-length").and_then(|value| value.parse::<usize>().ok()) {
        Some(length) => body.len() >= length,
        // No length and no chunking: the body runs to EOF.
        None => false,
    }
}

/// Status code and body of one raw HTTP/1.1 response.
fn parse_response(raw: &[u8]) -> Result<(u16, String), TranslateError> {
    let Some(split) = head_end(raw) else {
        return Err(TranslateError::Request(format!(
            "response has no complete header block: {:?}",
            String::from_utf8_lossy(&raw[..raw.len().min(120)])
        )));
    };
    let (head, body) = raw.split_at(split);
    let head = String::from_utf8_lossy(head);

    let status_line = head.lines().next().unwrap_or_default();
    let status = status_line
        .split_whitespace()
        .nth(1)
        .and_then(|code| code.parse::<u16>().ok())
        .ok_or_else(|| {
            TranslateError::Request(format!("response has no status code: {:?}", status_line))
        })?;

    let body = if is_chunked(&head) {
        decode_chunked(body)
            .map_err(|error| TranslateError::Request(format!("chunked response: {}", error)))?
    } else if let Some(length) =
        header_value(&head, "content-length").and_then(|value| value.parse::<usize>().ok())
    {
        body[..body.len().min(length)].to_vec()
    } else {
        body.to_vec()
    };

    Ok((status, String::from_utf8_lossy(&body).into_owned()))
}

fn decode_chunked(mut body: &[u8]) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    loop {
        let line_end = body
            .windows(2)
            .position(|window| window == b"\r\n")
            .ok_or_else(|| "truncated chunk size".to_string())?;
        let size_line = String::from_utf8_lossy(&body[..line_end]);
        // Chunk extensions (`;name=value`) are legal and irrelevant here.
        let size_text = size_line.split(';').next().unwrap_or("").trim();
        let size = usize::from_str_radix(size_text, 16)
            .map_err(|_| format!("bad chunk size {:?}", size_text))?;

        body = &body[line_end + 2..];
        if size == 0 {
            return Ok(out);
        }
        if body.len() < size + 2 {
            return Err("truncated chunk body".to_string());
        }
        out.extend_from_slice(&body[..size]);
        body = &body[size + 2..];
    }
}

// --- Streaming a completion -----------------------------------------------

/// A `Transfer-Encoding: chunked` body decoded as it arrives.
///
/// [`decode_chunked`] can assume a complete body; a streamed one cannot, so this
/// keeps the partial tail and only emits a chunk once its own trailing CRLF has
/// also been buffered — a half-arrived chunk is not data yet.
#[derive(Default)]
struct ChunkedStream {
    pending: Vec<u8>,
    finished: bool,
}

impl ChunkedStream {
    fn push(&mut self, bytes: &[u8]) {
        self.pending.extend_from_slice(bytes);
    }

    /// Everything that is complete now; `finished` flips on the zero-size chunk.
    fn drain(&mut self) -> Result<Vec<u8>, String> {
        let mut out = Vec::new();
        loop {
            let Some(line_end) = self.pending.windows(2).position(|window| window == b"\r\n") else {
                return Ok(out);
            };
            let size_text = String::from_utf8_lossy(&self.pending[..line_end]);
            let size_text = size_text.split(';').next().unwrap_or("").trim().to_string();
            let size = usize::from_str_radix(&size_text, 16)
                .map_err(|_| format!("bad chunk size {:?}", size_text))?;
            let start = line_end + 2;
            if self.pending.len() < start + size + 2 {
                return Ok(out);
            }
            if size == 0 {
                self.finished = true;
                self.pending.clear();
                return Ok(out);
            }
            out.extend_from_slice(&self.pending[start..start + size]);
            self.pending.drain(..start + size + 2);
        }
    }
}

/// Splits a live `text/event-stream` body into `data:` payloads.
///
/// The stream is cut wherever the socket feels like it, so a `data:` line can
/// arrive in two pieces; the partial tail stays buffered here. `event:` / `id:`
/// / comment lines are not payloads and are dropped.
#[derive(Default)]
struct SseBody {
    pending: Vec<u8>,
}

impl SseBody {
    fn feed(&mut self, bytes: &[u8]) -> Vec<String> {
        self.pending.extend_from_slice(bytes);
        let mut payloads = Vec::new();
        while let Some(position) = self.pending.iter().position(|byte| *byte == b'\n') {
            let line: Vec<u8> = self.pending.drain(..=position).collect();
            let line = String::from_utf8_lossy(&line);
            let line = line.trim_end_matches(['\r', '\n']);
            if let Some(payload) = line.strip_prefix("data:") {
                payloads.push(payload.trim().to_string());
            }
        }
        payloads
    }
}

/// `{"choices":[{"delta":{"content":"…"}}]}` → `Some("…")`.
///
/// `[DONE]`, a status-only chunk and the final `finish_reason` chunk carry no
/// content, and all of them are normal mid-stream.
fn delta_content(payload: &str) -> Option<String> {
    let parsed: serde_json::Value = serde_json::from_str(payload).ok()?;
    parsed
        .get("choices")?
        .get(0)?
        .get("delta")?
        .get("content")?
        .as_str()
        .filter(|text| !text.is_empty())
        .map(str::to_string)
}

/// The request body of one completion, streamed or not.
///
/// Structured output (`response_format` / GBNF) is deliberately NOT used:
/// measured against this model it made the JSON-schema variant echo the source
/// untranslated and the grammar variant return `[]`. The numbered-line contract
/// plus `parse` is the measured-reliable path.
fn completion_request(prompt: &str, max_tokens: u32, stream: bool) -> serde_json::Value {
    let mut body = serde_json::json!({
        "messages": [{ "role": "user", "content": prompt }],
        "temperature": TEMPERATURE,
        "top_k": TOP_K,
        "top_p": TOP_P,
        "repeat_penalty": REPEAT_PENALTY,
        "n_predict": max_tokens,
        "cache_prompt": true,
    });
    if stream {
        body["stream"] = serde_json::Value::Bool(true);
    }
    body
}

// --- The child process ----------------------------------------------------

/// A boxed future, so [`Completer`] can be used as a trait object without an
/// async-trait dependency.
pub type BoxFuture<'a, T> = std::pin::Pin<Box<dyn std::future::Future<Output = T> + Send + 'a>>;

/// One chat completion against a translation backend.
///
/// The batching/validation/degradation logic in [`super::translate_with`] is
/// written against this trait rather than against [`LlamaServer`] directly, so
/// it can be exercised end to end against a mock listener without the bundled
/// binary or the 1.1 GB model.
pub trait Completer: Send + Sync {
    fn complete<'a>(&'a self, prompt: &'a str, max_tokens: u32) -> BoxFuture<'a, Result<String, TranslateError>>;

    /// [`Completer::complete`], with the answer handed over in pieces.
    ///
    /// The full text is still what gets parsed — deltas are a progress signal,
    /// never an input to validation — so the batch contract is identical on both
    /// paths. The default runs [`Completer::complete`] and delivers the whole
    /// answer in one piece, which is exactly right for a mock and for any
    /// transport that cannot stream; [`LlamaServer`] overrides it.
    fn complete_streaming<'a>(
        &'a self,
        prompt: &'a str,
        max_tokens: u32,
        on_delta: Box<dyn FnMut(&str) + Send + 'a>,
    ) -> BoxFuture<'a, Result<String, TranslateError>> {
        Box::pin(async move {
            let answer = self.complete(prompt, max_tokens).await?;
            let mut on_delta = on_delta;
            on_delta(&answer);
            Ok(answer)
        })
    }
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
    endpoint: Endpoint,
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

        let endpoint = Endpoint::for_child()?;
        let argv = server_args(model_path, &endpoint);

        tracing::info!(
            binary = %binary.display(),
            model = %model_path.display(),
            endpoint = %endpoint,
            argv = ?argv,
            "Starting llama-server for subtitle translation"
        );

        let mut command = tokio::process::Command::new(binary);
        #[cfg(target_os = "windows")]
        command.creation_flags(0x0800_0000); // CREATE_NO_WINDOW

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

        let mut server = Self {
            child: Mutex::new(Some(child)),
            endpoint,
            stderr_tail,
            model_path: model_path.to_path_buf(),
        };

        match server.await_healthy().await {
            Ok(()) => {
                tracing::info!(
                    endpoint = %server.endpoint,
                    model = %model_path.display(),
                    "llama-server is ready"
                );
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

        loop {
            if let Some(status) = self.exited_status() {
                return Err(TranslateError::ServerStart(format!(
                    "llama-server exited ({}) during startup — stderr: {}",
                    status,
                    self.stderr_text()
                )));
            }

            if self.health_ok().await {
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
    ///
    /// Before the child has bound the socket this fails with ENOENT, and after
    /// it has bound it but not loaded the model it answers 503 — both are the
    /// same "keep waiting" to this loop.
    async fn health_ok(&self) -> bool {
        let Ok((status, body)) = self
            .endpoint
            .request("GET", "/health", None, HEALTH_REQUEST_TIMEOUT)
            .await
        else {
            return false;
        };
        if status != 200 {
            return false;
        }
        body.contains("\"status\":\"ok\"") || body.contains("\"status\": \"ok\"")
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
        post_completion(&self.endpoint, prompt, max_tokens).await
    }

    /// The same request with `"stream": true`, calling `on_delta` per piece.
    ///
    /// Measured on the bundled model: a 20-cue batch takes ~12 s to generate, so
    /// waiting for the whole answer leaves a progress bar frozen for that long.
    /// The deltas arrive per token, so the caller can count finished lines while
    /// the batch is still being written.
    pub async fn complete_streaming(
        &self,
        prompt: &str,
        max_tokens: u32,
        on_delta: &mut (dyn FnMut(&str) + Send),
    ) -> Result<String, TranslateError> {
        if self.exited_status().is_some() {
            return Err(TranslateError::Cancelled);
        }
        post_completion_streaming(&self.endpoint, prompt, max_tokens, on_delta).await
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

    fn complete_streaming<'a>(
        &'a self,
        prompt: &'a str,
        max_tokens: u32,
        on_delta: Box<dyn FnMut(&str) + Send + 'a>,
    ) -> BoxFuture<'a, Result<String, TranslateError>> {
        Box::pin(async move {
            let mut on_delta = on_delta;
            let mut forward = |delta: &str| on_delta(delta);
            LlamaServer::complete_streaming(self, prompt, max_tokens, &mut forward).await
        })
    }
}

impl Drop for LlamaServer {
    /// Cancellation reclaims the server: aborting the transcription task drops
    /// this value, which kills the child (translation-design §8). `kill_on_drop`
    /// on the `Command` is the belt to this brace — an abort between `spawn` and
    /// the `Child` being stored still kills the process.
    ///
    /// The socket file is removed here too. Unlinking a bound socket is safe and
    /// does not disturb the child: an already-accepted connection keeps working,
    /// and no new client can reach the (now gone) name.
    fn drop(&mut self) {
        if self
            .child
            .lock()
            .map(|slot| slot.is_some())
            .unwrap_or(false)
        {
            tracing::info!(
                endpoint = %self.endpoint,
                model = %self.model_path.display(),
                "Stopping llama-server"
            );
        }
        self.kill();

        #[cfg(unix)]
        if let Endpoint::Socket(path) = &self.endpoint {
            let _ = std::fs::remove_file(path);
        }
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

/// POST one chat completion and return the assistant text.
///
/// Shared with the mock in `translate`'s tests so the request body, the status
/// handling and the response shape are covered by the same code the real server
/// path uses — only the model is fake.
pub async fn post_completion(
    endpoint: &Endpoint,
    prompt: &str,
    max_tokens: u32,
) -> Result<String, TranslateError> {
    let body = completion_request(prompt, max_tokens, false).to_string();

    let (status, text) = endpoint
        .request(
            "POST",
            "/v1/chat/completions",
            Some(&body),
            COMPLETION_TIMEOUT,
        )
        .await?;

    if !(200..300).contains(&status) {
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

/// POST one completion with `"stream": true` and return the joined answer.
///
/// The assembled text is what the caller parses — exactly as if it had waited
/// for [`post_completion`] — while `on_delta` sees the pieces in order. An
/// empty stream is a parse error, not an empty answer: a silent success would
/// look to the caller like a model that translated nothing.
pub async fn post_completion_streaming(
    endpoint: &Endpoint,
    prompt: &str,
    max_tokens: u32,
    on_delta: &mut (dyn FnMut(&str) + Send),
) -> Result<String, TranslateError> {
    let body = completion_request(prompt, max_tokens, true).to_string();
    let mut sse = SseBody::default();
    let mut answer = String::new();

    {
        let mut on_body = |bytes: &[u8]| {
            for payload in sse.feed(bytes) {
                if payload == "[DONE]" {
                    continue;
                }
                if let Some(delta) = delta_content(&payload) {
                    answer.push_str(&delta);
                    on_delta(&delta);
                }
            }
        };
        endpoint
            .post_stream("/v1/chat/completions", &body, COMPLETION_TIMEOUT, &mut on_body)
            .await?;
    }

    if answer.is_empty() {
        return Err(TranslateError::Parse(
            "streamed response carried no content".to_string(),
        ));
    }
    Ok(answer)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn model() -> PathBuf {
        PathBuf::from("/models/HY-MT/HY-MT1.5-1.8B-Q4_K_M.gguf")
    }

    #[cfg(unix)]
    #[test]
    fn server_args_for_a_socket_pass_the_path_and_no_port() {
        let args = server_args(&model(), &Endpoint::Socket(PathBuf::from("/tmp/ms-1.sock")));

        assert_eq!(
            args,
            vec![
                "--model",
                "/models/HY-MT/HY-MT1.5-1.8B-Q4_K_M.gguf",
                "--host",
                "/tmp/ms-1.sock",
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
        assert!(
            !args.iter().any(|arg| arg == "--port"),
            "a socket endpoint must not carry --port: {args:?}"
        );
    }

    #[test]
    fn server_args_for_loopback_tcp_keep_host_and_port() {
        let args = server_args(&model(), &Endpoint::Tcp { port: 51234 });

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

    /// The sandbox cannot bind a socket whose path exceeds `sun_path`, and the
    /// failure would surface as a mystifying bind error from the child.
    #[cfg(unix)]
    #[test]
    fn socket_paths_are_unique_short_and_suffixed() {
        let first = socket_path().expect("first path");
        let second = socket_path().expect("second path");

        assert_ne!(first, second, "each run needs its own socket");
        assert!(
            first.to_string_lossy().ends_with(".sock"),
            "llama.cpp only treats --host as a socket for *.sock: {}",
            first.display()
        );
        assert_eq!(first.parent(), Some(std::env::temp_dir().as_path()));
        let length = first.as_os_str().as_encoded_bytes().len();
        assert!(
            length <= MAX_SOCKET_PATH,
            "path is {length} bytes: {}",
            first.display()
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn a_generated_socket_path_can_be_bound() {
        let path = socket_path().expect("path");
        let listener = tokio::net::UnixListener::bind(&path).expect("bind the generated path");
        drop(listener);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn parse_reads_a_content_length_body() {
        let raw = b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nContent-Type: application/json\r\n\r\n{}";
        assert_eq!(parse_response(raw).unwrap(), (200, "{}".to_string()));
    }

    #[test]
    fn parse_decodes_a_chunked_body() {
        let raw =
            b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n4\r\n{\"a\"\r\n3\r\n:1}\r\n0\r\n\r\n";
        assert_eq!(parse_response(raw).unwrap(), (200, "{\"a\":1}".to_string()));
    }

    #[test]
    fn parse_reports_the_error_status_and_body() {
        let raw = b"HTTP/1.1 503 Service Unavailable\r\nContent-Length: 11\r\n\r\n{\"status\":0";
        let (status, body) = parse_response(raw).unwrap();
        assert_eq!(status, 503);
        assert!(body.starts_with("{\"status\""));
    }

    #[test]
    fn parse_rejects_a_response_without_headers() {
        assert!(parse_response(b"garbage").is_err());
    }

    #[test]
    fn completeness_follows_the_framing() {
        assert!(!response_complete(b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\n\r\nab"));
        assert!(response_complete(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nab"));
        // No length: only EOF ends it, so the loop must not stop early.
        assert!(!response_complete(b"HTTP/1.1 200 OK\r\n\r\n{}"));
        assert!(response_complete(
            b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n2\r\n{}\r\n0\r\n\r\n"
        ));
    }

    #[test]
    fn request_framing_closes_the_connection_and_declares_a_json_body() {
        let request = build_request("POST", "/v1/chat/completions", Some("{}"));

        assert!(request.starts_with("POST /v1/chat/completions HTTP/1.1\r\n"));
        assert!(request.contains("Connection: close\r\n"));
        assert!(request.contains("Content-Length: 2\r\n"));
        assert!(request.ends_with("\r\n\r\n{}"));

        let get = build_request("GET", "/health", None);
        assert!(!get.contains("Content-Length"), "a GET has no body: {get:?}");
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
                // the one that is normally present. Storage is
                // `<models>/<task>/<family>/<file>` and this one translates.
                // Default location is the repo's `models/`, i.e. one level above
                // the crate root — not `resources/..`, which is the crate
                // itself and made this test skip silently.
                let model_dir = std::env::var_os("MONOSUB_MODELS_DIR")
                    .map(PathBuf::from)
                    .unwrap_or_else(|| {
                        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                            .join("..")
                            .join("models")
                    });
                let model = model_dir
                    .join("trans")
                    .join("hy_mt")
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

    /// Live-server smoke test: the child starts, binds its socket, answers
    /// `/health`, completes one request, and leaves nothing behind when the
    /// guard drops.
    ///
    /// This is the test that proves the transport: if `--host <path>.sock` were
    /// ignored, or the framing were wrong, startup would time out here.
    #[tokio::test]
    async fn live_server_binds_answers_health_and_completes() {
        let Some((binary, model)) = bundled_server_and_model() else {
            return;
        };

        let server = LlamaServer::start(&binary, &model)
            .await
            .expect("bundled llama-server starts");

        #[cfg(unix)]
        let socket = match &server.endpoint {
            Endpoint::Socket(path) => PathBuf::from(path),
            Endpoint::Tcp { port } => panic!("expected a socket endpoint, got port {port}"),
        };
        #[cfg(unix)]
        assert!(
            // `is_file()` is false for a socket: it must be asked as metadata.
            {
                use std::os::unix::fs::FileTypeExt;
                std::fs::symlink_metadata(&socket)
                    .map(|meta| meta.file_type().is_socket())
                    .unwrap_or(false)
            },
            "the child must have bound {}",
            socket.display()
        );

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

        #[cfg(unix)]
        assert!(
            !socket.exists(),
            "the socket file must be unlinked on drop: {}",
            socket.display()
        );
    }

    fn process_alive(pid: u32) -> bool {
        std::process::Command::new("kill")
            .args(["-0", &pid.to_string()])
            .status()
            .map(|status| status.success())
            .unwrap_or(false)
    }

    // --- streaming --------------------------------------------------------

    /// A chunk cut in half by the network must not be emitted: the caller parses
    /// these bytes, so half a JSON frame would look like a broken answer.
    #[test]
    fn chunked_stream_emits_only_whole_chunks() {
        let mut body = ChunkedStream::default();
        body.push(b"5\r\nhel");
        assert!(
            body.drain().unwrap().is_empty(),
            "a half-arrived chunk is not data yet"
        );
        assert!(!body.finished);

        body.push(b"lo\r\n6\r\n world\r\n");
        assert_eq!(body.drain().unwrap(), b"hello world".to_vec());
        assert!(!body.finished);

        body.push(b"0\r\n\r\n");
        assert!(body.drain().unwrap().is_empty());
        assert!(body.finished);
    }

    /// Chunk extensions are legal and must not be read as part of the size.
    #[test]
    fn chunked_stream_ignores_chunk_extensions() {
        let mut body = ChunkedStream::default();
        body.push(b"4;name=value\r\nabcd\r\n0\r\n\r\n");
        assert_eq!(body.drain().unwrap(), b"abcd".to_vec());
        assert!(body.finished);
    }

    #[test]
    fn sse_body_reassembles_a_frame_split_across_reads() {
        let mut sse = SseBody::default();
        assert!(
            sse.feed(b"data: {\"choices\":[{\"delta\":").is_empty(),
            "an unterminated line is not a payload"
        );
        let payloads = sse.feed(b"{\"content\":\"1. a\"}}]}\n\ndata: [DONE]\n\n");
        assert_eq!(payloads.len(), 2);
        assert_eq!(delta_content(&payloads[0]).as_deref(), Some("1. a"));
        assert_eq!(payloads[1], "[DONE]");
    }

    /// Every frame that is not text: the role announcement, an empty delta, the
    /// `finish_reason` frame after the last token.
    #[test]
    fn delta_content_ignores_frames_without_text() {
        assert_eq!(
            delta_content("{\"choices\":[{\"delta\":{\"role\":\"assistant\"}}]}"),
            None
        );
        assert_eq!(
            delta_content("{\"choices\":[{\"delta\":{\"content\":\"\"}}]}"),
            None
        );
        assert_eq!(
            delta_content("{\"choices\":[{\"finish_reason\":\"stop\",\"delta\":{}}]}"),
            None
        );
        assert_eq!(delta_content("[DONE]"), None);
        assert_eq!(
            delta_content("{\"choices\":[{\"delta\":{\"content\":\"2. b\"}}]}").as_deref(),
            Some("2. b")
        );
    }

    /// Streaming changes one field of the request and nothing else, so the
    /// sampling that was measured against this model is the same on both paths.
    #[test]
    fn the_streamed_request_differs_only_by_the_stream_flag() {
        let plain = completion_request("1. Hello\n", 64, false);
        let streamed = completion_request("1. Hello\n", 64, true);

        assert!(plain.get("stream").is_none());
        assert_eq!(streamed["stream"], serde_json::Value::Bool(true));

        let mut stripped = streamed.clone();
        stripped.as_object_mut().unwrap().remove("stream");
        assert_eq!(stripped, plain);
    }

    /// The streamed answer must be the same text the whole-response path returns,
    /// and it must arrive in more than one piece (otherwise the progress signal
    /// this exists for would not exist).
    #[tokio::test]
    async fn live_streaming_delivers_the_answer_in_pieces() {
        let Some((binary, model)) = bundled_server_and_model() else {
            return;
        };
        let server = LlamaServer::start(&binary, &model)
            .await
            .expect("bundled llama-server starts");

        let mut deltas: Vec<String> = Vec::new();
        let streamed = server
            .complete_streaming("1. Hello\n", 64, &mut |delta| deltas.push(delta.to_string()))
            .await
            .expect("one streamed completion succeeds");

        assert!(!streamed.is_empty(), "the model must answer something");
        assert!(
            deltas.len() > 1,
            "the answer arrived in {} piece(s), which would not move a bar",
            deltas.len()
        );
        assert_eq!(deltas.concat(), streamed);

        drop(server);
    }
}
