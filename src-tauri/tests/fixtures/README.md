# Subtitle writer fixtures

Golden files for `subtitle::writer` (P1-11). All non-`test_*` goldens are
produced by the **original Python writer**
(`core-engine/monosub_core/subtitle_writer.py`) from one canonical segment list,
so `cargo test subtitle` proves the Rust port byte-for-byte against it.

## Contents

| Path | Purpose |
|------|---------|
| `segments.json` | Canonical segment list. Covers a Chinese cue, an English cue, a cue with a speaker label, a punctuation-heavy cue (incl. a decimal `3.14`) and a long cue (18 s). |
| `segments.srt` | Python `SubtitleWriter.write_srt` golden. |
| `segments.vtt` | Python `SubtitleWriter.write_vtt` golden. |
| `segments.txt` | Python `SubtitleWriter.write_txt` golden. |
| `segments.md` | Python `SubtitleWriter.write_md` golden (title `# segments`, from the output file stem). |
| `segments.json.golden` | JSON golden — see the note below. |
| `generate_goldens.py` | Regenerates all of the above; committed so the goldens are reproducible. |
| `test_transcript.srt`, `test_transcript.json` | Copied verbatim from `core-engine/` (P1-12). |
| `srt-example/` | Copied verbatim from `core-engine/srt-example/` (P1-12). |

Regenerate:

```bash
core-engine/.venv/bin/python src-tauri/tests/fixtures/generate_goldens.py
```

## JSON golden is emitted manually (deliberate)

`SubtitleWriter.write_json()` only accepts a **raw word-level dict** (`--raw`
output: `{"words": [...]}`); it raises `ValueError` for anything else. The v2
segment-array JSON form required by spec §5.4
(`[{start, end, text, speaker?}]`) therefore cannot be produced by
`write_json()`.

For that reason `generate_goldens.py` emits `segments.json.golden` directly with

```python
json.dumps(segments, ensure_ascii=False, indent=2)
```

(no trailing newline). This is the exact byte sequence `writer::render_json`
must reproduce for the same input.

## Timestamp rounding divergence (documented, not a test failure)

`subtitle::format_timestamp` implements spec §5.4:
`total_ms = (seconds * 1000.0).round()`. The Python writer instead derives
milliseconds from `datetime.timedelta` and **truncates**:
`int((td.total_seconds() % 1) * 1000)`.

The two agree whenever the time is an exact multiple of 1 ms — including every
value in `segments.json` — but differ for other values, e.g.
`0.2999 s` → Rust `…,300` vs Python `…,299`, and `1.9995 s` → Rust `…,000`
(carry) vs Python `…,999`.

`writer::tests::timestamp_rounding_diverges_from_python_truncation` pins these
two examples so the divergence stays visible. All other writer tests compare
against the goldens byte-for-byte.
