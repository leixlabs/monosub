# Word-level timestamps — measured behaviour

Engine: audio.cpp **0.9.1** (`git cf124a67`), `qwen3_asr` +
`qwen3_forced_aligner`, macOS arm64 / Metal. Every number below was measured on
this machine; nothing is inferred from the upstream docs.

Purpose: subtitle timing derived from **word** timings instead of estimated from
character counts, plus a `<stem>.words.json` sidecar an agent can cut video on.

---

## 1. The command that works

```bash
audiocpp_cli --task asr --family qwen3_asr \
  --model models/Qwen3-ASR-0.6B-GGUF/qwen3-asr-0.6b-q8_0.gguf \
  --backend metal --audio speech_16k.wav \
  --language Chinese --text "" \
  --words-out words.json --text-out transcript.txt \
  --request-option preserve_punctuation=true \
  --session-option qwen3_asr.forced_aligner_path=models/Qwen3-ForcedAligner-0.6B-GGUF/qwen3-forced-aligner-0.6b-q8_0.gguf \
  --session-option qwen3_asr.vad_path=<silero_vad dir>
```

Four flags are load-bearing. Dropping any one either fails the run or silently
produces unusable output:

| Flag | If omitted |
|---|---|
| `--words-out` | no word timings at all |
| `--request-option preserve_punctuation=true` | multi-chunk runs **lose all punctuation and space-join CJK**: `大 家 好 我 是 婷 婷 …` |
| `--session-option qwen3_asr.forced_aligner_path=…` | `Qwen3 ASR timestamp output requires --session-option …` |
| `--session-option qwen3_asr.vad_path=…` | `Silero VAD model path does not exist: assets/framework/models/silero_vad` |

The last one matters because `audio_chunk_mode=auto` (the default) switches to
VAD chunking **when word timestamps are requested** — its default VAD path is
relative and does not exist in an app bundle.

Note `--request-option` is legal here only because this is the single-request
path: combining it with `--request-sequence` is rejected outright (PoC §4.1).

---

## 2. `words.json` shape

```json
[{"start_sample":0,"end_sample":2560,"word":"大","confidence":0}]
```

- Times are **sample offsets** at 16 kHz → `seconds = sample / 16000`.
- Chinese splits **per character**; space-separated languages split **per word**.
- **`confidence` is always 0.0.** The forced aligner emits no score. Never gate
  on it. (This is why `engine::words` parses the field but branches on nothing.)
- **No punctuation.** `text_out` has it; the word stream does not. Cue boundaries
  therefore come from pauses, not from end-of-sentence marks.

Verified against `text_out`: the 40 words of the 10 s Chinese clip concatenate to
exactly the punctuated transcript with the punctuation removed.

---

## 3. Pause statistics — why the old thresholds do not transfer

Inter-word gaps (seconds) on three real clips:

| Clip | words | largest gaps |
|---|---|---|
| zh, 10 s | 40 | 0.40, 0.24, 0.16, 0.16, 0.08 |
| zh, 60 s | 246 | 0.40, 0.32, 0.272, 0.24, 0.24 |
| en, 8 s | 16 | 0.48, 0.16, 0.08, 0.08, 0.0 |

Two consequences:

1. **The old Python `0.5 s` spacing threshold is dead on this data.** A 0.5 s rule
   emits *zero* spacing entries on all three clips — the sidecar would carry no
   cut points at all. `SPACING_MIN_SECONDS` is therefore `0.15`.
2. **The cue split threshold has to be its own constant.** `gap_threshold = 0.5`
   is a *merge* threshold for chunk windows; word pauses are an order of magnitude
   shorter. Reusing it made a 10 s clip collapse into **one** cue.

Word-level split threshold sweep (Chinese, 10 s clip):

| pause > | cues | boundaries |
|---|---|---|
| 0.50 s | 1 | none |
| 0.25 s | 2 | 大家好 / rest |
| **0.15 s** | **5** | 大家好 / 我是婷婷 / 今天我们来讨论字幕生成的问题 / 我想问一下 / 你们现在用什么工具做字幕 |
| 0.10 s | 5 | same |

Ground truth from `text_out` punctuation: 3 sentences
(`大家好，我是婷婷。` / `今天我们来讨论字幕生成的问题。` / `首先，我想问一下，你们现在用什么工具做字幕？`).
The 0.15 s result is finer than sentence punctuation but every boundary it picks
is a real one (0.40 s at the full stop, 0.16 s at the comma-like breaks), and the
in-phrase pauses (0.08 s, 0.0 s) are correctly ignored. `0.15` is the value that
separates those two populations on this data; it is
`PostProcessConfig::word_gap_threshold`.

---

## 4. Latency

| Input | Plain ASR | With `--words-out` |
|---|---|---|
| 10 s zh | 4.5 s | 10.0 s |
| 8 s en | 4.3 s | ~10 s |
| 60 s zh | 6.9 s | 13.5 s |

The aligner roughly doubles wall time (a second 1.13 GB model runs a
non-autoregressive pass over each chunk). This is why word timing is opt-in.

---

## 5. Output artefacts

- `<stem>.srt|vtt|txt|md` — cues whose edges are **word** edges. Asserted in the
  e2e test: every cue start and end is present in the word start/end sets, so no
  cue edge can come from an estimate.
- `<stem>.words.json` — Scribe-compatible, same field names as the old Python
  `--raw` mode, so existing consumers keep working:

```json
{"words": [
  {"type": "word", "text": "大", "start": 0.0, "end": 0.16, "speaker_id": null},
  {"type": "spacing", "start": 0.4, "end": 0.8}
]}
```

Spacing counts with the 0.15 s rule: 4 (10 s zh), 22 (60 s zh), 2 (8 s en) —
versus 0 with the old 0.5 s rule.

A run that produced no words writes **no** sidecar: an empty
`{"words": []}` is indistinguishable from "the audio is silent" and would mislead
a consumer.

---

## 6. Failure modes and degradation

| Situation | Behaviour |
|---|---|
| Aligner not installed | Solver reports a `word_timestamps` gap naming the aligner; the run proceeds with estimated timing |
| Model cannot align (SenseVoice, MOSS) | Same gap path, no attempt made |
| Aligner emits nothing (silence, unalignable language) | CLI exits 0, writes `text_out` but **no** `words_out` → parsed as empty words, estimated timing |
| Model detected no language (`session.cpp:516`) | CLI errors → run logs a warning and retries with estimated timing |
| Real `run_aligned` failure + Metal unavailable | Warning + estimated timing (word timing is an enhancement, never a precondition) |

The asymmetry to remember: **a missing `words.json` is success**, not an error.
Measured: a synthetic tone exits 0 and writes only `text_out`.

---

## 7. Not covered

- **Word timing + speaker separation.** The composed plan (`asr` + `diar` +
  `turn-attribution`) attributes *chunks* to turns; with words present the cue
  builder takes a single label. Combining them would need a word-midpoint
  attribution pass. Documented limitation, deliberately out of scope here.
- **`zh-tw` / traditional output.** Recognition still emits simplified; word
  timing does not change that.
