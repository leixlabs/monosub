//! 16 kHz mono s16le WAV reading and sample-accurate slicing (`hound`).
//!
//! Task: P1-7. See docs/tech-spec/refactor-audiocpp.md §5.3 step 3.
//!
//! Pinned baseline — keep these signatures stable for P1-9.

use std::path::{Path, PathBuf};

/// Decoded 16 kHz mono s16le PCM.
#[derive(Debug, Clone)]
pub struct WavData {
    pub sample_rate: u32,
    pub channels: u16,
    /// Interleaved s16le samples; mono input so one sample per frame.
    pub samples: Vec<i16>,
}

impl WavData {
    pub fn frames(&self) -> usize {
        self.samples.len() / self.channels.max(1) as usize
    }

    pub fn duration_s(&self) -> f64 {
        self.frames() as f64 / self.sample_rate.max(1) as f64
    }
}

/// Read a WAV file, rejecting anything that is not 16 kHz mono s16le
/// (audio.cpp only accepts that layout).
pub fn read(path: &Path) -> Result<WavData, crate::engine::EngineError> {
    let _ = path;
    unimplemented!("P1-7")
}

/// Write `start_sample..end_sample` of `data` to `out_path` as 16 kHz mono s16le.
///
/// The range is clamped to the available frames; a fully out-of-range request
/// yields an empty (header-only) WAV rather than an error.
pub fn slice(
    data: &WavData,
    start_sample: u64,
    end_sample: u64,
    out_path: &Path,
) -> Result<(), crate::engine::EngineError> {
    let _ = (data, start_sample, end_sample, out_path);
    unimplemented!("P1-7")
}

/// Convenience: read then slice, returning the number of frames written.
pub fn slice_file(
    src: &Path,
    start_sample: u64,
    end_sample: u64,
    out_path: &Path,
) -> Result<usize, crate::engine::EngineError> {
    let _ = (src, start_sample, end_sample, out_path);
    unimplemented!("P1-7")
}

/// Name convention for sliced chunks inside the task temp dir.
pub fn chunk_file_name(index: usize) -> String {
    format!("chunk_{:04}.wav", index)
}

/// Path of a sliced chunk inside `work_dir`.
pub fn chunk_path(work_dir: &Path, index: usize) -> PathBuf {
    work_dir.join(chunk_file_name(index))
}
