//! Whether this machine can actually run the bundled CUDA engine.
//!
//! The Windows package ships the **CUDA** build of audio.cpp and nothing else —
//! [`crate::engine::cli::Backend::platform_default`] picks `Cuda` there and there
//! is deliberately no CPU retry, because CPU inference is too slow to be a
//! fallback. The CUDA *runtime* travels inside the package (`cudart64_13.dll`
//! next to the engine), so the one thing a user must supply is the **driver**.
//!
//! Hence the probe: `nvidia-smi`. It is installed *by* the driver, which makes
//! "no `nvidia-smi` on PATH" the honest answer to "no driver" rather than an
//! error, and its query reports the two facts that matter (which GPU, which
//! compute capability) without this app loading CUDA itself.
//!
//! The result drives one line under the empty state in the file list: ready, or
//! a short explanation plus a link to the driver download page. It is advisory
//! only — nothing here blocks a run, because the definitive answer is the
//! engine's own exit status.

use std::time::Duration;

use tokio::process::Command;

/// Oldest compute capability the bundled build supports.
///
/// The Windows engines are CUDA 13 builds, and audio.cpp pins its fatbin to
/// `75;80;86;89;90;120` — Turing (`7.5`, the RTX 20-series) is the oldest
/// architecture it carries. Pascal and older are out by construction: CUDA 13
/// removed offline compilation for everything below 7.5 (release notes §2.5.2),
/// so no 13.x fatbin can hold an sm_61 kernel. Saying this before the first file
/// fails is the whole point of the probe.
const MIN_COMPUTE_CAPABILITY: f64 = 7.5;

/// Where a user without a driver (or with one too old) should go.
const DRIVER_URL: &str = "https://www.nvidia.com/Download/index.aspx";

/// `nvidia-smi` is a driver component and can hang on a wedged driver; the card
/// must not hang with it.
const PROBE_TIMEOUT: Duration = Duration::from_secs(5);

/// One GPU as `nvidia-smi` reports it.
#[derive(Debug, Clone, PartialEq)]
pub struct Gpu {
    pub name: String,
    pub driver_version: String,
    /// `None` when the driver prints `N/A` — older drivers do not always answer
    /// this field, and an unknown capability must not be read as "too old".
    pub compute_capability: Option<f64>,
}

/// What the frontend renders. Every field is derived here, so the UI never has
/// to interpret raw `nvidia-smi` output or invent a threshold.
#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CudaStatus {
    /// `false` on macOS/Linux, where this question does not arise. The frontend
    /// renders nothing in that case.
    pub applicable: bool,
    /// Whether the CUDA engine can be expected to run.
    pub available: bool,
    /// Machine-readable reason: `ok` | `not_windows` | `no_driver` | `no_gpu`
    /// | `compute_capability_too_low` | `probe_failed`. The sentence shown to the
    /// user is picked by the frontend from this, so the copy stays translatable.
    pub reason: String,
    pub gpu_name: Option<String>,
    pub driver_version: Option<String>,
    /// The capability as the driver printed it (`"8.9"`), for display.
    pub compute_capability: Option<String>,
    pub minimum_compute_capability: f64,
    /// Raw failure text for `probe_failed`, already truncated.
    pub detail: Option<String>,
    pub driver_url: String,
}

/// Why the probe produced nothing usable.
enum ProbeError {
    /// `nvidia-smi` is not installed — on Windows that means no NVIDIA driver.
    Missing,
    /// It ran but failed, or never answered.
    Failed(String),
}

impl CudaStatus {
    fn new(applicable: bool, available: bool, reason: &str) -> Self {
        Self {
            applicable,
            available,
            reason: reason.to_string(),
            gpu_name: None,
            driver_version: None,
            compute_capability: None,
            minimum_compute_capability: MIN_COMPUTE_CAPABILITY,
            detail: None,
            driver_url: DRIVER_URL.to_string(),
        }
    }

    fn describe(mut self, gpu: &Gpu) -> Self {
        self.gpu_name = Some(gpu.name.clone());
        self.driver_version = Some(gpu.driver_version.clone());
        // One decimal, so "12" from an RTX 50-series card reads as "12.0" in the
        // same shape as "6.1" — the message compares it against `7.5`.
        self.compute_capability = gpu.compute_capability.map(|value| format!("{value:.1}"));
        self
    }

    fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(truncate(&detail.into(), 300));
        self
    }
}

/// Keep a driver's stderr from turning one sentence into a wall of text.
fn truncate(value: &str, max_chars: usize) -> String {
    let trimmed = value.trim();
    if trimmed.chars().count() <= max_chars {
        return trimmed.to_string();
    }
    let mut cut: String = trimmed.chars().take(max_chars).collect();
    cut.push('…');
    cut
}

/// Decide from the probed GPUs. Split out from the spawn so the thresholds are
/// testable without an NVIDIA machine.
fn judge(gpus: &[Gpu]) -> (bool, &'static str, Option<&Gpu>) {
    // Newest first: a laptop with switchable graphics reports both, and the
    // integrated one is not the one that will run the model.
    let Some(best) = gpus.iter().max_by(|a, b| {
        let key = |gpu: &Gpu| gpu.compute_capability.unwrap_or(f64::INFINITY);
        key(a).total_cmp(&key(b))
    }) else {
        return (false, "no_gpu", None);
    };

    match best.compute_capability {
        Some(capability) if capability < MIN_COMPUTE_CAPABILITY => {
            (false, "compute_capability_too_low", Some(best))
        }
        // An unparsable capability (`N/A`) is *not* evidence of an old card:
        // every GPU that answers `nvidia-smi` at all has a driver, and a wrong
        // "too old" sends those users to reinstall drivers for nothing.
        _ => (true, "ok", Some(best)),
    }
}

/// Parse `nvidia-smi --query-gpu=name,driver_version,compute_cap --format=csv,noheader`.
///
/// Kept pure and lenient: one line per GPU, any field may be `N/A`, and a
/// header line (someone dropped `noheader`) is skipped rather than reported as a
/// GPU called "name".
pub fn parse_nvidia_smi(stdout: &str) -> Vec<Gpu> {
    stdout.lines().filter_map(parse_line).collect()
}

fn parse_line(line: &str) -> Option<Gpu> {
    let mut fields = line.split(',').map(str::trim);
    let name = fields.next()?;
    if name.is_empty() || name.eq_ignore_ascii_case("name") {
        return None;
    }

    let driver_version = fields.next().unwrap_or_default().to_string();
    let compute_capability = fields.next().and_then(|raw| raw.parse::<f64>().ok());

    Some(Gpu {
        name: name.to_string(),
        driver_version,
        compute_capability,
    })
}

async fn probe() -> Result<Vec<Gpu>, ProbeError> {
    let args = [
        "--query-gpu=name,driver_version,compute_cap",
        "--format=csv,noheader",
    ];

    let output = match tokio::time::timeout(
        PROBE_TIMEOUT,
        Command::new("nvidia-smi").args(args).output(),
    )
    .await
    {
        Err(_) => return Err(ProbeError::Failed("nvidia-smi timed out".to_string())),
        // Windows appends `.exe` itself, so `nvidia-smi` is the right spelling.
        Ok(Err(error)) if error.kind() == std::io::ErrorKind::NotFound => {
            return Err(ProbeError::Missing)
        }
        Ok(Err(error)) => return Err(ProbeError::Failed(error.to_string())),
        Ok(Ok(output)) => output,
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    if !output.status.success() {
        // A driver with no usable device exits non-zero with this on stderr;
        // reporting it as "probe failed" would hide the actionable answer.
        if stderr.to_ascii_lowercase().contains("no devices") {
            return Ok(Vec::new());
        }
        let reason = if stderr.trim().is_empty() {
            format!("nvidia-smi exited with {}", output.status)
        } else {
            stderr.to_string()
        };
        return Err(ProbeError::Failed(reason));
    }

    Ok(parse_nvidia_smi(&stdout))
}

/// Answer the "can this machine run the engine" question.
pub async fn cuda_status_inner() -> CudaStatus {
    if !cfg!(target_os = "windows") {
        // Metal on macOS, CPU on Linux: CUDA is not what these builds use, and
        // the line this feeds is hidden anyway.
        return CudaStatus::new(false, false, "not_windows");
    }

    match probe().await {
        Err(ProbeError::Missing) => CudaStatus::new(true, false, "no_driver"),
        Err(ProbeError::Failed(detail)) => {
            CudaStatus::new(true, false, "probe_failed").with_detail(detail)
        }
        Ok(gpus) => {
            let (available, reason, best) = judge(&gpus);
            let status = CudaStatus::new(true, available, reason);
            match (reason, best) {
                // `no_gpu` has nothing to describe; a failure does have a card
                // name worth showing ("RTX 2080 · 7.5").
                ("no_gpu", _) => status,
                (_, Some(gpu)) => status.describe(gpu),
                (_, None) => status,
            }
        }
    }
}

/// Is CUDA usable here? Drives the line under the file list's empty state.
#[tauri::command]
pub async fn cuda_status() -> Result<CudaStatus, String> {
    let status = cuda_status_inner().await;
    tracing::debug!(
        available = status.available,
        reason = %status.reason,
        gpu = ?status.gpu_name,
        "CUDA probe finished"
    );
    Ok(status)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_query_output() {
        let gpus = parse_nvidia_smi(
            "NVIDIA GeForce RTX 4070, 551.23, 8.9\nNVIDIA GeForce RTX 3060, 551.23, 8.6\n",
        );

        assert_eq!(gpus.len(), 2);
        assert_eq!(gpus[0].name, "NVIDIA GeForce RTX 4070");
        assert_eq!(gpus[0].driver_version, "551.23");
        assert_eq!(gpus[0].compute_capability, Some(8.9));
    }

    #[test]
    fn skips_a_header_line_and_blank_lines() {
        // `--format=csv` without `noheader` prints this; it is not a GPU.
        let gpus = parse_nvidia_smi("\nname, driver_version, compute_cap\n\n");
        assert!(gpus.is_empty());
    }

    #[test]
    fn treats_an_unparsable_capability_as_unknown() {
        let gpus = parse_nvidia_smi("Tesla K80, 470.57.02, N/A\n");
        assert_eq!(gpus[0].compute_capability, None);
        // Unknown must not be read as "older than Turing".
        let (available, reason, _) = judge(&gpus);
        assert!(available, "an unknown capability must not be a failure: {reason}");
    }

    #[test]
    fn rejects_a_gpu_older_than_the_bundled_build() {
        // Pascal (sm_61) is below the CUDA 13 fatbin's lowest entry, and it is
        // rejected on purpose — CUDA 13 cannot compile for it at all.
        let gpus = parse_nvidia_smi("NVIDIA GeForce GTX 1080, 551.23, 6.1\n");
        let (available, reason, best) = judge(&gpus);

        assert!(!available);
        assert_eq!(reason, "compute_capability_too_low");
        assert_eq!(best.map(|gpu| gpu.name.as_str()), Some("NVIDIA GeForce GTX 1080"));
    }

    #[test]
    fn accepts_turing_and_blackwell() {
        // The two ends of what the 13.x builds actually carry.
        let turing = parse_nvidia_smi("NVIDIA GeForce RTX 2060, 551.23, 7.5\n");
        let (turing_ok, turing_reason, _) = judge(&turing);
        assert!(turing_ok);
        assert_eq!(turing_reason, "ok");

        let blackwell = parse_nvidia_smi("NVIDIA GeForce RTX 5080, 617.42, 12.0\n");
        let (blackwell_ok, blackwell_reason, _) = judge(&blackwell);
        assert!(blackwell_ok);
        assert_eq!(blackwell_reason, "ok");
    }

    #[test]
    fn picks_the_newest_of_several_gpus() {
        // Two NVIDIA GPUs (an eGPU, or a laptop that reports both): the newer
        // one must win regardless of the order the driver lists them in.
        let gpus = parse_nvidia_smi(
            "NVIDIA GeForce GTX 1050 Ti, 551.23, 6.1\nNVIDIA GeForce RTX 4060 Laptop GPU, 551.23, 8.9\n",
        );
        let (available, reason, best) = judge(&gpus);

        assert!(available);
        assert_eq!(reason, "ok");
        assert_eq!(
            best.map(|gpu| gpu.name.as_str()),
            Some("NVIDIA GeForce RTX 4060 Laptop GPU")
        );
    }

    #[test]
    fn no_gpus_is_its_own_reason() {
        let (available, reason, best) = judge(&[]);
        assert!(!available);
        assert_eq!(reason, "no_gpu");
        assert!(best.is_none());
    }

    #[tokio::test]
    async fn the_probe_is_only_applicable_on_windows() {
        let status = cuda_status_inner().await;
        assert_eq!(status.applicable, cfg!(target_os = "windows"));
        assert_eq!(status.minimum_compute_capability, MIN_COMPUTE_CAPABILITY);
        assert_eq!(status.driver_url, DRIVER_URL);
        if !cfg!(target_os = "windows") {
            assert_eq!(status.reason, "not_windows");
        }
    }
}
