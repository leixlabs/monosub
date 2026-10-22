//! Which `--request-option` values a model actually accepts.
//!
//! `audiocpp_cli` validates per-request options against the selected model's
//! own family spec and fails the whole run on an unknown one:
//!
//! ```text
//! audiocpp_cli failed: unknown MOSS-Transcribe-Diarize request option: language
//! ```
//!
//! The VAD → `requests.json` → `--request-sequence` pipeline used to write
//! `language` and `audio_chunk_mode` into every request unconditionally. That
//! only works for the families that declare both — 7 of the 30 ASR families in
//! v0.9.1 declare neither, so selecting any of them failed immediately.
//!
//! # Why this reads the GGUF
//!
//! The authoritative list is the family spec, and every GGUF carries a copy of
//! it in the `audiocpp.model_spec.json` KV pair. That means the answer travels
//! with the model file: a model downloaded next year needs no code change here.
//!
//! The CLI's own introspection does *not* expose this. `--list-loaders --json`
//! reports only `api_endpoints` / `instructions_policy` / `tasks`, and
//! `--inspect --model` reports task/mode/language/config information but no
//! options.
//!
//! # Why there is a second layer
//!
//! Not every GGUF embeds a full spec. Some are *minimal* — two keys, `family`
//! and `sources`:
//!
//! | model | embedded keys |
//! |---|---|
//! | `sense_asr`, `fun_asr_nano`, `moss_transcribe_diarize`, `sortformer_diar` | 16–17 (full) |
//! | `qwen3_asr`, `qwen3_forced_aligner` | **2 (minimal)** |
//!
//! A minimal spec must not be read as "accepts nothing": `qwen3_asr` really does
//! take `language` and `audio_chunk_mode`. So a minimal or unreadable spec falls
//! through to [`REJECTS_LANGUAGE`], a negative list of the families measured to
//! reject the two options this pipeline sends.
//!
//! A negative list rather than a positive one is deliberate: the default for an
//! unknown family is "send the options", which is what every model did before
//! this module existed, so an unrecognised future family behaves exactly as it
//! does today instead of silently losing its language hint.

use std::io::{BufReader, Read, Seek, SeekFrom};
use std::path::Path;

use crate::engine::catalog::ModelSpec;

/// GGUF magic, little-endian `GGUF`.
const GGUF_MAGIC: [u8; 4] = [0x47, 0x47, 0x55, 0x46];

/// The metadata key holding the embedded family spec.
const SPEC_KEY: &str = "audiocpp.model_spec.json";

/// Upper bound on a metadata string we are willing to read.
///
/// A GGUF header cannot be trusted to be well-formed just because the magic
/// matched; a corrupt length field would otherwise ask for a multi-gigabyte
/// allocation. The largest real spec is a few tens of KB.
const MAX_STRING_BYTES: u64 = 4 * 1024 * 1024;

/// Metadata array elements we are willing to walk.
///
/// Tensor metadata uses arrays (shapes, names) and the header precedes the
/// weights, so this only bounds a corrupt file, not a real one.
const MAX_ARRAY_ITEMS: u64 = 1 << 20;

/// Families measured to reject `language` **and** `audio_chunk_mode`.
///
/// From `audiocpp_cli`'s embedded family table (v0.9.1), cross-checked by running
/// each one through `--request-sequence`:
///
/// ```text
/// unknown MOSS-Transcribe-Diarize request option: language
/// unknown MOSS-Transcribe-Diarize request option: audio_chunk_mode
/// ```
///
/// `moss_transcribe_diarize` also embeds a *full* spec, so for it the GGUF layer
/// answers first; the entry is kept so a minimal-spec MOSS build still works.
const REJECTS_LANGUAGE: &[&str] = &[
    "moss_transcribe_diarize",
    "citrinet_asr",
    "confucius4_r2t2",
    "niagara_asr",
    "vibeasr",
    "voxtral_realtime",
    "samsone",
];

/// The per-request options this pipeline may send.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RequestOptions {
    /// Send the top-level `"language"` field.
    pub language: bool,
    /// Send `options.audio_chunk_mode`.
    pub audio_chunk_mode: bool,
}

impl RequestOptions {
    /// Everything on — the pre-`spec.rs` behaviour, and the safe answer when the
    /// model cannot be inspected.
    pub const ALL: Self = Self {
        language: true,
        audio_chunk_mode: true,
    };

    /// Nothing but `id` and `audio`.
    pub const NONE: Self = Self {
        language: false,
        audio_chunk_mode: false,
    };

    /// Resolve from the option names a family declares.
    fn from_declared(declared: &[String]) -> Self {
        Self {
            language: declared.iter().any(|name| name == "language"),
            audio_chunk_mode: declared.iter().any(|name| name == "audio_chunk_mode"),
        }
    }
}

/// What a model accepts, resolved from its GGUF and its family.
///
/// `spec` is the model's path on disk, which is why this takes a [`ModelSpec`]
/// rather than a family name: the answer lives in the file, and the file is
/// found through the catalogue (see [`crate::engine::catalog::model_path`]).
pub fn request_options(family: &str, model_path: Option<&Path>) -> RequestOptions {
    // Layer 1: the embedded spec, when it is a full one.
    if let Some(path) = model_path {
        match declared_request_options(path) {
            // A non-empty declaration is authoritative.
            Ok(Some(declared)) if !declared.is_empty() => {
                return RequestOptions::from_declared(&declared);
            }
            // Minimal spec (2 keys) or a full spec with no request options at
            // all: fall through rather than concluding "accepts nothing". A
            // genuinely option-less family is covered by the negative list.
            Ok(_) => {}
            Err(error) => {
                tracing::warn!(
                    family = %family,
                    path = %path.display(),
                    error = %error,
                    "could not read the embedded model spec; falling back to the family table"
                );
            }
        }
    }

    // Layer 2: the measured negative list.
    if REJECTS_LANGUAGE.contains(&family) {
        return RequestOptions::NONE;
    }

    // Layer 3: unknown family behaves as it always did.
    RequestOptions::ALL
}

/// Resolve for a catalogue entry, using the same `AppPaths` resolution the
/// transcription pipeline uses to find the weights.
pub fn request_options_for(
    paths: &crate::paths::AppPaths,
    spec: &ModelSpec,
) -> RequestOptions {
    let model_path = crate::engine::catalog::model_path(paths, &spec.id)
        .ok()
        .flatten();
    request_options(&spec.family, model_path.as_deref())
}

/// The `options.request[].name` list embedded in `gguf_path`.
///
/// `Ok(None)` means the file carries no spec at all; `Ok(Some(vec![]))` means it
/// carries one with no request options. The caller treats both as "ask the next
/// layer" — the distinction is kept for tests and logs.
pub fn declared_request_options(gguf_path: &Path) -> Result<Option<Vec<String>>, String> {
    let file = std::fs::File::open(gguf_path)
        .map_err(|e| format!("open {}: {}", gguf_path.display(), e))?;
    let mut reader = BufReader::new(file);
    read_spec(&mut reader)
}

/// Walk a GGUF header looking for [`SPEC_KEY`].
fn read_spec<R: Read + Seek>(reader: &mut R) -> Result<Option<Vec<String>>, String> {
    let mut magic = [0u8; 4];
    reader
        .read_exact(&mut magic)
        .map_err(|e| format!("read magic: {}", e))?;
    if magic != GGUF_MAGIC {
        return Ok(None);
    }

    let version = read_u32(reader)?;
    // v1/v2 have a different header layout; only v3 is emitted by the pinned
    // audio.cpp packages, and guessing at older ones is not worth the risk.
    if version != 3 {
        return Ok(None);
    }

    let _tensor_count = read_u64(reader)?;
    let kv_count = read_u64(reader)?;

    for _ in 0..kv_count {
        let key = read_string(reader)?;
        let value_type = read_u32(reader)?;

        if key == SPEC_KEY {
            let json = read_string(reader)?;
            return Ok(Some(request_option_names(&json)?));
        }
        skip_value(reader, value_type)?;
    }

    Ok(None)
}

/// Pull `options.request[].name` out of an embedded spec document.
///
/// A spec whose top-level keys are only `family`/`sources` is the *minimal*
/// variant and yields an empty list.
fn request_option_names(json: &str) -> Result<Vec<String>, String> {
    let document: serde_json::Value =
        serde_json::from_str(json).map_err(|e| format!("parse embedded spec: {}", e))?;

    let Some(request) = document
        .get("options")
        .and_then(|options| options.get("request"))
        .and_then(|request| request.as_array())
    else {
        return Ok(Vec::new());
    };

    Ok(request
        .iter()
        .filter_map(|entry| entry.get("name").and_then(|name| name.as_str()))
        .map(str::to_string)
        .collect())
}

fn read_u32<R: Read>(reader: &mut R) -> Result<u32, String> {
    let mut buf = [0u8; 4];
    reader
        .read_exact(&mut buf)
        .map_err(|e| format!("read u32: {}", e))?;
    Ok(u32::from_le_bytes(buf))
}

fn read_u64<R: Read>(reader: &mut R) -> Result<u64, String> {
    let mut buf = [0u8; 8];
    reader
        .read_exact(&mut buf)
        .map_err(|e| format!("read u64: {}", e))?;
    Ok(u64::from_le_bytes(buf))
}

/// GGUF strings are a `u64` byte length followed by UTF-8 bytes.
fn read_string<R: Read>(reader: &mut R) -> Result<String, String> {
    let length = read_u64(reader)?;
    if length > MAX_STRING_BYTES {
        return Err(format!("string length {} exceeds the sane bound", length));
    }
    let mut bytes = vec![0u8; length as usize];
    reader
        .read_exact(&mut bytes)
        .map_err(|e| format!("read string body: {}", e))?;
    String::from_utf8(bytes).map_err(|e| format!("string is not utf-8: {}", e))
}

/// Byte size of a fixed-width GGUF scalar type, or `None` for string/array.
fn scalar_size(value_type: u32) -> Option<u64> {
    Some(match value_type {
        0 | 1 | 7 => 1,  // u8, i8, bool
        2 | 3 => 2,      // u16, i16
        4 | 5 | 6 => 4,  // u32, i32, f32
        10 | 11 | 12 => 8, // u64, i64, f64
        _ => return None,
    })
}

/// Advance past one metadata value we do not care about.
fn skip_value<R: Read + Seek>(reader: &mut R, value_type: u32) -> Result<(), String> {
    if let Some(size) = scalar_size(value_type) {
        reader
            .seek(SeekFrom::Current(size as i64))
            .map_err(|e| format!("skip scalar: {}", e))?;
        return Ok(());
    }

    match value_type {
        // string
        8 => {
            let length = read_u64(reader)?;
            if length > MAX_STRING_BYTES {
                return Err(format!("string length {} exceeds the sane bound", length));
            }
            reader
                .seek(SeekFrom::Current(length as i64))
                .map_err(|e| format!("skip string: {}", e))?;
            Ok(())
        }
        // array: element type, then count, then count elements
        9 => {
            let element_type = read_u32(reader)?;
            let count = read_u64(reader)?;
            if count > MAX_ARRAY_ITEMS {
                return Err(format!("array count {} exceeds the sane bound", count));
            }
            if let Some(size) = scalar_size(element_type) {
                reader
                    .seek(SeekFrom::Current((size * count) as i64))
                    .map_err(|e| format!("skip array body: {}", e))?;
                return Ok(());
            }
            // An array of strings is the only variable-width element type GGUF
            // defines; walk it element by element.
            if element_type == 8 {
                for _ in 0..count {
                    let length = read_u64(reader)?;
                    if length > MAX_STRING_BYTES {
                        return Err(format!("string length {} exceeds the sane bound", length));
                    }
                    reader
                        .seek(SeekFrom::Current(length as i64))
                        .map_err(|e| format!("skip array string: {}", e))?;
                }
                return Ok(());
            }
            Err(format!("unsupported array element type {}", element_type))
        }
        other => Err(format!("unsupported metadata value type {}", other)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    /// Build a v3 GGUF header with the given metadata entries.
    fn gguf_with(entries: &[(String, Option<String>)]) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&GGUF_MAGIC);
        out.extend_from_slice(&3u32.to_le_bytes());
        out.extend_from_slice(&0u64.to_le_bytes()); // tensor count
        out.extend_from_slice(&(entries.len() as u64).to_le_bytes());

        for (key, spec) in entries {
            out.extend_from_slice(&(key.len() as u64).to_le_bytes());
            out.extend_from_slice(key.as_bytes());
            match spec {
                Some(json) => {
                    out.extend_from_slice(&8u32.to_le_bytes()); // string
                    out.extend_from_slice(&(json.len() as u64).to_le_bytes());
                    out.extend_from_slice(json.as_bytes());
                }
                None => {
                    out.extend_from_slice(&4u32.to_le_bytes()); // u32
                    out.extend_from_slice(&7u32.to_le_bytes());
                }
            }
        }
        out
    }

    fn options_of(bytes: Vec<u8>) -> Option<Vec<String>> {
        read_spec(&mut Cursor::new(bytes)).expect("parse")
    }

    #[test]
    fn a_full_spec_yields_its_request_options() {
        let spec = r#"{"family":"moss_transcribe_diarize","options":{"request":[
            {"name":"max_tokens"},{"name":"instruct"}]}}"#;
        let names = options_of(gguf_with(&[(SPEC_KEY.to_string(), Some(spec.to_string()))]))
            .expect("spec present");
        assert_eq!(names, vec!["max_tokens", "instruct"]);
    }

    #[test]
    fn a_minimal_spec_yields_no_options_rather_than_an_error() {
        // Measured shape of the shipped qwen3_asr GGUF: two keys, no `options`.
        let spec = r#"{"family":"qwen3_asr","sources":[]}"#;
        let names = options_of(gguf_with(&[(SPEC_KEY.to_string(), Some(spec.to_string()))]))
            .expect("spec present");
        assert!(names.is_empty());
    }

    #[test]
    fn unrelated_metadata_is_skipped_without_losing_the_spec() {
        // A scalar before the spec, and a string array before it, exercise both
        // skip paths.
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&GGUF_MAGIC);
        bytes.extend_from_slice(&3u32.to_le_bytes());
        bytes.extend_from_slice(&0u64.to_le_bytes());
        bytes.extend_from_slice(&3u64.to_le_bytes());

        // scalar u32
        bytes.extend_from_slice(&(4u64.to_le_bytes()));
        bytes.extend_from_slice(b"test");
        bytes.extend_from_slice(&4u32.to_le_bytes());
        bytes.extend_from_slice(&42u32.to_le_bytes());

        // array of strings
        bytes.extend_from_slice(&(5u64.to_le_bytes()));
        bytes.extend_from_slice(b"names");
        bytes.extend_from_slice(&9u32.to_le_bytes()); // array
        bytes.extend_from_slice(&8u32.to_le_bytes()); // of string
        bytes.extend_from_slice(&2u64.to_le_bytes()); // count
        for item in ["ab", "cde"] {
            bytes.extend_from_slice(&(item.len() as u64).to_le_bytes());
            bytes.extend_from_slice(item.as_bytes());
        }

        // the spec
        let spec = r#"{"family":"sense_asr","options":{"request":[
            {"name":"language"},{"name":"audio_chunk_mode"}]}}"#;
        bytes.extend_from_slice(&(SPEC_KEY.len() as u64).to_le_bytes());
        bytes.extend_from_slice(SPEC_KEY.as_bytes());
        bytes.extend_from_slice(&8u32.to_le_bytes());
        bytes.extend_from_slice(&(spec.len() as u64).to_le_bytes());
        bytes.extend_from_slice(spec.as_bytes());

        let names = options_of(bytes).expect("spec present");
        assert_eq!(names, vec!["language", "audio_chunk_mode"]);
    }

    #[test]
    fn a_file_without_the_spec_is_not_an_error() {
        assert!(options_of(gguf_with(&[("other".to_string(), None)])).is_none());
    }

    #[test]
    fn a_non_gguf_file_is_not_an_error() {
        assert!(options_of(b"not a gguf at all".to_vec()).is_none());
    }

    #[test]
    fn the_measured_negative_list_disables_both_options() {
        for family in REJECTS_LANGUAGE {
            assert_eq!(
                request_options(family, None),
                RequestOptions::NONE,
                "{family} must not receive language/audio_chunk_mode"
            );
        }
    }

    #[test]
    fn an_unknown_family_keeps_the_historic_behaviour() {
        // Sending the options is what every model got before spec.rs existed, so
        // an unrecognised family must not silently lose its language hint.
        assert_eq!(request_options("some_future_asr", None), RequestOptions::ALL);
    }

    #[test]
    fn the_embedded_spec_outranks_the_negative_list() {
        // A full spec that *does* declare both wins over the list, so a rebuilt
        // MOSS that gains the options starts working with no code change.
        let spec = r#"{"family":"moss_transcribe_diarize","options":{"request":[
            {"name":"language"},{"name":"audio_chunk_mode"}]}}"#;
        let dir = std::env::temp_dir().join(format!("monosub-spec-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("m.gguf");
        std::fs::write(&path, gguf_with(&[(SPEC_KEY.to_string(), Some(spec.to_string()))])).unwrap();

        assert_eq!(
            request_options("moss_transcribe_diarize", Some(&path)),
            RequestOptions::ALL
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_minimal_spec_falls_through_to_the_family_table() {
        // qwen3_asr ships a 2-key spec but really accepts both options; reading
        // "no options declared" as "accepts nothing" would break it.
        let spec = r#"{"family":"qwen3_asr","sources":[]}"#;
        let dir = std::env::temp_dir().join(format!("monosub-spec-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("q.gguf");
        std::fs::write(&path, gguf_with(&[(SPEC_KEY.to_string(), Some(spec.to_string()))])).unwrap();

        assert_eq!(request_options("qwen3_asr", Some(&path)), RequestOptions::ALL);
        // ...and a minimal-spec family that IS on the list still gets trimmed.
        assert_eq!(
            request_options("moss_transcribe_diarize", Some(&path)),
            RequestOptions::NONE
        );

        let _ = std::fs::remove_dir_all(&dir);
    }
}
