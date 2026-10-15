//! 16 kHz mono s16le WAV reading and sample-accurate slicing (`hound`).
//!
//! Task: P1-7. See docs/tech-spec/refactor-audiocpp.md §5.3 step 3.
//!
//! Pinned baseline — keep these signatures stable for P1-9.

use std::path::{Path, PathBuf};

use hound::{SampleFormat, WavReader, WavSpec, WavWriter};

use crate::engine::EngineError;

/// The only layout audio.cpp accepts; [`read`] rejects everything else.
const SAMPLE_RATE: u32 = 16_000;
const CHANNELS: u16 = 1;
const BITS_PER_SAMPLE: u16 = 16;

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
    let mut reader = WavReader::open(path).map_err(|e| invalid_audio(path, e))?;
    let spec = reader.spec();

    if spec.sample_rate != SAMPLE_RATE
        || spec.channels != CHANNELS
        || spec.bits_per_sample != BITS_PER_SAMPLE
        || spec.sample_format != SampleFormat::Int
    {
        return Err(EngineError::InvalidAudio(format!(
            "{}: unsupported WAV layout: {} Hz, {} channel(s), {}-bit {:?} (audio.cpp requires {} Hz, {} channel, {}-bit int PCM)",
            path.display(),
            spec.sample_rate,
            spec.channels,
            spec.bits_per_sample,
            spec.sample_format,
            SAMPLE_RATE,
            CHANNELS,
            BITS_PER_SAMPLE,
        )));
    }

    let samples = reader
        .samples::<i16>()
        .collect::<Result<Vec<i16>, _>>()
        .map_err(|e| invalid_audio(path, e))?;

    Ok(WavData {
        sample_rate: spec.sample_rate,
        channels: spec.channels,
        samples,
    })
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
    write_slice(data, start_sample, end_sample, out_path).map(|_| ())
}

/// Convenience: read then slice, returning the number of frames written.
pub fn slice_file(
    src: &Path,
    start_sample: u64,
    end_sample: u64,
    out_path: &Path,
) -> Result<usize, crate::engine::EngineError> {
    let data = read(src)?;
    write_slice(&data, start_sample, end_sample, out_path)
}

/// Write the clamped range and return the number of frames written.
fn write_slice(
    data: &WavData,
    start_sample: u64,
    end_sample: u64,
    out_path: &Path,
) -> Result<usize, crate::engine::EngineError> {
    let channels = data.channels.max(1) as usize;
    let (start, end) = clamp_range(data.frames(), start_sample, end_sample);

    let spec = WavSpec {
        channels: CHANNELS,
        sample_rate: SAMPLE_RATE,
        bits_per_sample: BITS_PER_SAMPLE,
        sample_format: SampleFormat::Int,
    };
    let mut writer =
        WavWriter::create(out_path, spec).map_err(|e| io_error(out_path, e))?;
    for &sample in &data.samples[start * channels..end * channels] {
        writer.write_sample(sample).map_err(|e| io_error(out_path, e))?;
    }
    writer.finalize().map_err(|e| io_error(out_path, e))?;

    Ok(end - start)
}

/// Clamp `start_sample..end_sample` to `total` frames, keeping `start <= end`.
fn clamp_range(total: usize, start_sample: u64, end_sample: u64) -> (usize, usize) {
    let total = total as u64;
    let start = start_sample.min(total);
    let end = end_sample.min(total).max(start);
    (start as usize, end as usize)
}

fn invalid_audio(path: &Path, e: hound::Error) -> EngineError {
    EngineError::InvalidAudio(format!("{}: {}", path.display(), e))
}

fn io_error(path: &Path, e: hound::Error) -> EngineError {
    EngineError::Io(format!("{}: {}", path.display(), e))
}

/// Name convention for sliced chunks inside the task temp dir.
pub fn chunk_file_name(index: usize) -> String {
    format!("chunk_{:04}.wav", index)
}

/// Path of a sliced chunk inside `work_dir`.
pub fn chunk_path(work_dir: &Path, index: usize) -> PathBuf {
    work_dir.join(chunk_file_name(index))
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TempDir(PathBuf);

    impl TempDir {
        fn new(tag: &str) -> Self {
            let dir = std::env::temp_dir().join(format!(
                "monosub-wav-{}-{}",
                tag,
                uuid::Uuid::new_v4()
            ));
            std::fs::create_dir_all(&dir).expect("create temp dir");
            Self(dir)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn spec(sample_rate: u32, channels: u16, bits: u16) -> WavSpec {
        WavSpec {
            channels,
            sample_rate,
            bits_per_sample: bits,
            sample_format: SampleFormat::Int,
        }
    }

    fn write_wav(path: &Path, spec: WavSpec, samples: &[i16]) {
        let mut writer = WavWriter::create(path, spec).expect("create test wav");
        for &s in samples {
            writer.write_sample(s).expect("write sample");
        }
        writer.finalize().expect("finalize test wav");
    }

    /// 1000 mono frames with a distinctive value per frame.
    fn ramp_wav(dir: &TempDir, name: &str) -> PathBuf {
        let path = dir.path().join(name);
        let samples: Vec<i16> = (0..1000).map(|i| (i - 500) as i16).collect();
        write_wav(&path, spec(16_000, 1, 16), &samples);
        path
    }

    #[test]
    fn slice_mid_range_has_exact_frame_count_and_samples() {
        let dir = TempDir::new("mid");
        let src = ramp_wav(&dir, "src.wav");
        let out = dir.path().join("mid.wav");

        slice_file(&src, 100, 600, &out).expect("slice");

        let sliced = read(&out).expect("read slice");
        assert_eq!(sliced.frames(), 500);
        assert_eq!(sliced.sample_rate, 16_000);
        assert_eq!(sliced.channels, 1);
        let expected: Vec<i16> = (100..600).map(|i| (i - 500) as i16).collect();
        assert_eq!(sliced.samples, expected);
    }

    #[test]
    fn slice_clamps_to_available_frames_at_and_past_the_end() {
        let dir = TempDir::new("clamp");
        let src = ramp_wav(&dir, "src.wav");
        let at_end = dir.path().join("at_end.wav");
        let past_end = dir.path().join("past_end.wav");

        assert_eq!(slice_file(&src, 900, 1200, &at_end).expect("slice"), 100);
        assert_eq!(
            read(&at_end).expect("read").samples,
            (900..1000).map(|i| (i - 500) as i16).collect::<Vec<_>>()
        );

        // Entirely past the end: header-only WAV, no error.
        assert_eq!(slice_file(&src, 5000, 6000, &past_end).expect("slice"), 0);
        let empty = read(&past_end).expect("read header-only wav");
        assert_eq!(empty.frames(), 0);
        assert!(empty.samples.is_empty());
    }

    #[test]
    fn zero_length_slice_is_still_a_readable_wav() {
        let dir = TempDir::new("zero");
        let src = ramp_wav(&dir, "src.wav");
        let out = dir.path().join("zero.wav");

        assert_eq!(slice_file(&src, 500, 500, &out).expect("slice"), 0);
        let data = read(&out).expect("read zero-length slice");
        assert_eq!(data.frames(), 0);
        assert_eq!(data.duration_s(), 0.0);
        assert!(out.metadata().expect("stat").len() > 0);
    }

    #[test]
    fn slice_rejects_inverted_range_without_panicking() {
        let dir = TempDir::new("inverted");
        let src = ramp_wav(&dir, "src.wav");
        let out = dir.path().join("inverted.wav");

        assert_eq!(slice_file(&src, 600, 500, &out).expect("slice"), 0);
        assert_eq!(read(&out).expect("read").frames(), 0);
    }

    #[test]
    fn read_rejects_non_16k_sample_rate() {
        let dir = TempDir::new("rate");
        let path = dir.path().join("44k.wav");
        write_wav(&path, spec(44_100, 1, 16), &vec![0i16; 4410]);

        let err = read(&path).expect_err("44.1 kHz must be rejected");
        match err {
            EngineError::InvalidAudio(msg) => {
                assert!(msg.contains("44100"), "actual rate missing: {msg}");
                assert!(msg.contains("channel"), "channel count missing: {msg}");
                assert!(msg.contains("16-bit"), "bit depth missing: {msg}");
            }
            other => panic!("expected InvalidAudio, got {other:?}"),
        }
    }

    #[test]
    fn read_rejects_stereo_input() {
        let dir = TempDir::new("stereo");
        let path = dir.path().join("stereo.wav");
        let interleaved: Vec<i16> = (0..2000).map(|i| (i % 100) as i16).collect();
        write_wav(&path, spec(16_000, 2, 16), &interleaved);

        let err = read(&path).expect_err("stereo must be rejected");
        match err {
            EngineError::InvalidAudio(msg) => {
                assert!(msg.contains("2 channel"), "actual channels missing: {msg}");
            }
            other => panic!("expected InvalidAudio, got {other:?}"),
        }
    }

    #[test]
    fn read_rejects_non_16_bit_depth() {
        let dir = TempDir::new("bits");
        let path = dir.path().join("u8.wav");
        let samples: Vec<i16> = (0..128).map(|i| (i - 64) as i16).collect();
        write_wav(&path, spec(16_000, 1, 8), &samples);

        let err = read(&path).expect_err("8-bit must be rejected");
        match err {
            EngineError::InvalidAudio(msg) => {
                assert!(msg.contains("8-bit"), "actual bit depth missing: {msg}");
            }
            other => panic!("expected InvalidAudio, got {other:?}"),
        }
    }

    #[test]
    fn chunk_paths_are_zero_padded_inside_work_dir() {
        let work = Path::new("/tmp/work");
        assert_eq!(chunk_file_name(0), "chunk_0000.wav");
        assert_eq!(chunk_file_name(42), "chunk_0042.wav");
        assert_eq!(chunk_path(work, 42), work.join("chunk_0042.wav"));
    }
}

/// Slice invariants that P1-9's pipeline depends on.
///
/// These deliberately go beyond frame counts: the guardrail for chunked
/// transcription is that slicing is *lossless*, because the engine hands
/// `audiocpp_cli` only these cut WAVs. A silent off-by-one here would drop or
/// duplicate a sample at every VAD boundary.
#[cfg(test)]
mod slice_invariants {
    use super::*;

    fn scratch(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "monosub-wavinv-{}-{}",
            tag,
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&dir).expect("create scratch dir");
        dir
    }

    fn write(path: &Path, sample_rate: u32, channels: u16, samples: &[i16]) {
        let spec = hound::WavSpec {
            channels,
            sample_rate,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut writer = hound::WavWriter::create(path, spec).expect("create wav");
        for &sample in samples {
            writer.write_sample(sample).expect("write sample");
        }
        writer.finalize().expect("finalize wav");
    }

    /// Concatenating adjacent slices must reproduce the source PCM exactly.
    ///
    /// Boundaries are deliberately uneven and adjacent (0,1,7,100,101,500,999,1000)
    /// so an off-by-one at either end shows up as a value mismatch rather than
    /// cancelling out.
    #[test]
    fn adjacent_slices_concatenate_back_to_the_original_samples() {
        let dir = scratch("concat");
        let original: Vec<i16> = (0..1000).collect();
        let source = dir.join("source.wav");
        write(&source, 16_000, 1, &original);

        let data = read(&source).expect("read source");
        assert_eq!(data.frames(), 1000);

        let bounds = [0u64, 1, 7, 100, 101, 500, 999, 1000];
        let mut rebuilt: Vec<i16> = Vec::new();
        for (index, pair) in bounds.windows(2).enumerate() {
            let out = dir.join(format!("part-{}.wav", index));
            slice(&data, pair[0], pair[1], &out).expect("slice");

            let part = read(&out).expect("read part");
            assert_eq!(
                part.frames(),
                (pair[1] - pair[0]) as usize,
                "part {} frame count",
                index
            );
            rebuilt.extend_from_slice(&part.samples);
        }

        assert_eq!(rebuilt, original, "slices must reconstruct the source");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Every requested range lands inside `[0, frames]`, including inverted
    /// ranges and ranges entirely past the end.
    #[test]
    fn clamp_range_stays_within_the_source() {
        for (total, start, end) in [
            (100usize, 0u64, 1000u64),
            (100, 50, 200),
            (100, 200, 300),
            (100, 99, 100),
            (0, 0, 10),
            (100, 100, 100),
        ] {
            let (lo, hi) = clamp_range(total, start, end);
            assert!(lo <= hi, "({total}, {start}, {end}) -> ({lo}, {hi})");
            assert!(hi <= total, "end {hi} exceeds {total}");
            assert!(lo <= total);
        }
    }

    /// An empty range still produces a valid, re-readable WAV (P1-9 writes one
    /// per VAD chunk, and `audiocpp_cli` must accept every one of them).
    #[test]
    fn zero_length_slice_is_still_a_valid_wav() {
        let dir = scratch("zero");
        let source = dir.join("source.wav");
        write(&source, 16_000, 1, &[1, 2, 3, 4, 5]);
        let data = read(&source).expect("read source");

        let out = dir.join("empty.wav");
        slice(&data, 3, 3, &out).expect("slice");

        let empty = read(&out).expect("re-read empty slice");
        assert_eq!(empty.frames(), 0);
        assert_eq!(empty.sample_rate, 16_000);
        assert_eq!(empty.channels, 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Rejections must name the actual layout, so a user report is actionable.
    #[test]
    fn layout_rejections_name_the_actual_format() {
        let dir = scratch("reject");
        for (label, sample_rate, channels, expected) in [
            ("48k", 48_000u32, 1u16, "48000"),
            ("44k", 44_100, 1, "44100"),
            ("stereo", 16_000, 2, "2 channel"),
        ] {
            let path = dir.join(format!("{}.wav", label));
            write(&path, sample_rate, channels, &[0i16; 32]);

            let message = format!("{}", read(&path).expect_err("must reject"));
            assert!(
                message.contains(expected),
                "{label}: expected {expected:?} in {message:?}"
            );
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 8-bit input must be rejected rather than silently reinterpreted.
    #[test]
    fn rejects_eight_bit_depth() {
        let dir = scratch("depth");
        let path = dir.join("u8.wav");
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: 16_000,
            bits_per_sample: 8,
            sample_format: hound::SampleFormat::Int,
        };
        let mut writer = hound::WavWriter::create(&path, spec).expect("create wav");
        for _ in 0..32 {
            writer.write_sample(0i8).expect("sample");
        }
        writer.finalize().expect("finalize");

        let message = format!("{}", read(&path).expect_err("must reject 8-bit"));
        assert!(message.contains("8-bit"), "{message:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
