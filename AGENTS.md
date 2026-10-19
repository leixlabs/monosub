# MonoSub - Agent's Guide

> This document is intended for AI coding agents working on the MonoSub project.
> Read this file to understand the project architecture, conventions, and development workflow.

## Project Overview

**MonoSub** is a local-first desktop application for converting videos and audio files to subtitles without cloud dependencies. It provides a clean, modern UI for batch processing media files using local AI speech recognition, with optional local translation into a second language.

Speech recognition runs through **audio.cpp** (`audiocpp_cli`) and translation through
**llama.cpp** (`llama-server`), both C++ inference engines distributed as prebuilt binaries
and run as **separate child processes** — neither is linked into the app. There is
**no Python** in the runtime.

### Key Features

- 🎬 **Multi-file queue processing** - Manage multiple video/audio files simultaneously
- 🖼️ **Video thumbnails** - Automatic preview generation
- ⚡ **Local AI processing** - audio.cpp speech recognition and llama.cpp translation, both Metal accelerated
- 🧩 **Pluggable model plans** - Requirement-driven model selection (speed, word-level
  timestamps, speaker diarization, language); Tongyi/FunASR models preferred
- 🌍 **Local subtitle translation** - Optional post-recognition pass with a dedicated
  translation model (HY-MT1.5-1.8B); writes `<stem>.<lang>.<ext>` and never touches the originals
- 📦 **In-app model management** - Download/remove models on demand; nothing large is bundled
- 📝 **Multiple output formats** - SRT, VTT, TXT, MD (and JSON)
- 🌐 **Bilingual UI** - Simplified Chinese and English (`zh-CN`, `en`)
- 💾 **Flexible storage** - Save to same directory or custom location
- 🎨 **Modern UI** - Apple-inspired design system with dark/light mode

---

## Technology Stack

### Two-Layer Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│  Frontend (src/)                                                │
│  ├── React 18 + TypeScript                                      │
│  ├── TailwindCSS (Apple design system)                          │
│  ├── Zustand (state management)                                 │
│  ├── i18next (zh-CN + en)                                        │
│  └── Framer Motion (animations)                                 │
├─────────────────────────────────────────────────────────────────┤
│  App (src-tauri/)  — everything below happens in Rust           │
│  ├── Tauri 2.0, system tray                                     │
│  ├── FFmpeg/FFprobe: audio extraction, thumbnails, metadata     │
│  ├── Model download + integrity verification                    │
│  ├── Silero VAD segmentation (audiocpp_cli --task vad)          │
│  ├── ASR orchestration (audiocpp_cli --task asr)                │
│  ├── Subtitle post-processing + writers                         │
│  └── Translation orchestration (llama-server over HTTP)         │
└─────────────────────────────────────────────────────────────────┘
        │ spawns                    │ spawns
        ▼                           ▼
  audiocpp_cli                 llama-server
  (prebuilt, bundled;          (prebuilt, bundled;
   ~31 MB, no deps)             ~68 MB incl. its flat lib*.dylib)
        │                           │
   recognition only            translation only
   (its own ggml)              (its own ggml)
```

The two engines never share a process: audio.cpp and llama.cpp each embed their own
ggml, so linking both into one binary would collide. `translate/server.rs` talks to the
child over HTTP on `127.0.0.1` instead.

### Package Managers

| Component | Package Manager | File |
|-----------|----------------|------|
| Frontend | Bun | `package.json` |
| Rust Backend | Cargo | `src-tauri/Cargo.toml` |

---

## Project Structure

```
monosub/
├── src/                          # Frontend (React/TypeScript)
│   ├── components/
│   │   ├── Header.tsx
│   │   ├── Toolbar.tsx
│   │   ├── FileList.tsx
│   │   ├── FileItem.tsx
│   │   ├── BottomBar.tsx
│   │   ├── Settings.tsx         # Includes the Models + Translation sections
│   │   ├── SettingsModelPlan.tsx
│   │   ├── SettingsTranslation.tsx
│   │   └── ThemeToggle.tsx
│   ├── store/
│   │   └── useStore.ts          # Zustand state + persist v4 migration
│   ├── i18n/
│   │   ├── index.ts
│   │   └── locales/             # en.json + zh-CN.json (kept at parity)
│   ├── contexts/ThemeContext.tsx
│   ├── hooks/useTheme.ts
│   ├── lib/
│   │   ├── models.ts            # Model commands/events wrapper
│   │   ├── transcription.ts     # Start/cancel + MODEL_NOT_INSTALLED handling
│   │   ├── license.ts
│   │   ├── fileImport.ts
│   │   └── utils.ts
│   ├── App.tsx
│   └── main.tsx
│
├── src-tauri/                   # Rust: app logic + inference orchestration
│   ├── src/
│   │   ├── main.rs              # Entry point, command registration
│   │   ├── engine/              # audio.cpp integration
│   │   │   ├── mod.rs           # VAD → slice → requests.json → sequence ASR
│   │   │   ├── cli.rs           # Binary resolution, spawn, stderr ring buffer
│   │   │   ├── catalog.rs       # models.json, install checks, language mapping
│   │   │   ├── plan.rs          # Requirement → ModelPlan solver (pure, unit-tested)
│   │   │   ├── direct.rs        # Passthrough path: one unsplit call, parses turns/segments
│   │   │   ├── aligned.rs       # Word-timestamps path: one call + --words-out
│   │   │   ├── words.rs         # words.json parsing (samples → seconds)
│   │   │   ├── vad.rs           # silero_vad invocation + chunk parsing
│   │   │   └── wav.rs           # 16k mono s16le read + sample slicing
│   │   ├── translate/           # llama.cpp integration (SPAWNED, never linked)
│   │   │   ├── mod.rs           # should_translate → server → batch → validate → degrade
│   │   │   ├── server.rs        # llama-server spawn, /health probe, chat completion
│   │   │   ├── prompt.rs        # Batched numbered prompts (template from models.json)
│   │   │   ├── parse.rs         # Numbered-line / JSON response parsing (pure)
│   │   │   ├── language.rs      # Tag normalisation, skip decision, script census
│   │   │   └── error.rs         # TranslateError + user-showable status
│   │   ├── subtitle/            # Output generation
│   │   │   ├── mod.rs           # Segment, postprocess, write
│   │   │   ├── postprocess.rs   # Split / estimate / merge
│   │   │   └── writer.rs        # srt / vtt / txt / md / json
│   │   ├── commands/
│   │   │   ├── transcribe.rs    # Task registry, events, cancellation
│   │   │   ├── models.rs        # Model download/verify/remove
│   │   │   ├── video.rs         # FFmpeg/FFprobe paths, metadata, thumbnails
│   │   │   ├── license.rs       # Polar licensing + is_pro_licensed
│   │   │   ├── update.rs        # GitHub release check
│   │   │   └── tray.rs
│   │   └── logging.rs
│   ├── resources/
│   │   ├── models.json          # Model catalogue (v2: capabilities/roles/tier/engine)
│   │   ├── licenses/            # audio.cpp Apache-2.0 + llama.cpp MIT + notices
│   │   ├── bin/                 # fetched: ffmpeg, ffprobe, audiocpp_cli, lib/
│   │   │                        #   and llama-server-* + its lib*.dylib FLAT in bin/
│   │   └── models/silero_vad/   # fetched: silero_vad_16k.safetensors
│   ├── tests/fixtures/          # Python-generated subtitle goldens
│   ├── Cargo.toml
│   └── tauri.conf.json
│
├── scripts/
│   ├── build/
│   │   ├── fetch-audiocpp.sh    # Pinned download + SHA256 of CLI + VAD model
│   │   ├── fetch-llamacpp.sh    # Pinned download + SHA256 of llama-server + dylibs
│   │   ├── prepare-ffmpeg.sh
│   │   └── prepare-resources.sh # Orchestrates the above; regenerates MANIFEST
│   ├── release/
│   │   ├── build-macos-app.sh
│   │   ├── sign-app.sh          # Signs bundled binaries, then the app
│   │   ├── bump-version.sh
│   │   └── release.sh
│   └── utils/
│       ├── version-utils.sh
│       ├── check-i18n-keys.mjs  # Asserts both locales match en.json
│       └── clean-temp-files.sh
│
├── docs/tech-spec/              # Refactor spec, PoC, task list, accuracy report
├── landing_page/
├── package.json
├── tailwind.config.js
├── tsconfig.json
└── vite.config.ts
```

---

## Development Commands

### Prerequisites

- **Bun** - Frontend package manager ([Install](https://bun.sh/))
- **Rust** 1.70+ - For the Tauri backend
- **FFmpeg** - Optional; the bundled binary is preferred

Python/`uv` are **no longer required**.

### Initial Setup

```bash
# 1. Install frontend dependencies
bun install

# 2. Fetch the bundled runtimes (audiocpp_cli + silero VAD, llama-server + dylibs)
#    and ffmpeg
./scripts/build/prepare-resources.sh
```

### Development Workflow

```bash
bun run tauri dev    # Tauri app: Rust + webview (Vite is started by Tauri)
bun run dev          # Vite only (frontend in a browser; no Tauri APIs)
bun run check        # TypeScript type check
bun run lint         # ESLint
```

### Tests

```bash
bun run test:rust     # Rust unit + integration tests
bun run lint:rust     # cargo clippy
bun run check:i18n    # Locale parity
```

Tests that need the bundled binary/model print a `skip:` reason and pass when those
gitignored artifacts are absent, so a fresh clone still has a green suite.

### Interactive UI Testing (tauri-pilot)

Debug builds expose the UI over a Unix socket at
`/tmp/tauri-pilot-dev.leix.monosub.sock` via the `tauri-plugin-pilot` plugin
(`main.rs`, gated by `#[cfg(debug_assertions)]`, plus `pilot:default` in
`capabilities/default.json`). Install the matching CLI once:

```bash
cargo install tauri-pilot-cli    # pinned to 0.8.1; keep the plugin tag in sync
```

Then, with `bun run tauri dev` running:

```bash
tauri-pilot ping                                        # check the socket
tauri-pilot snapshot -i                                 # accessibility tree + @eN refs
tauri-pilot click @e5 / tauri-pilot assert text @e1 "…" # refs reset on each snapshot
tauri-pilot logs --level error                          # console/JS errors
tauri-pilot run src-tauri/tests/pilot/smoke.toml        # committed end-to-end smoke
tauri-pilot run src-tauri/tests/pilot/smoke.toml --junit /tmp/pilot.xml
```

The plugin requires Tauri ≥ 2.12 ({plugin, CLI and `@tauri-apps/*` versions all
moved together}). It is compiled out of release builds, so `tauri build` is
unaffected. `screenshot` (DOM html-to-image) is flaky on this page; prefer
`screenshot_native --window-id <id> --output <path>` when a PNG is needed.

### Production Builds

```bash
bun run build         # Frontend only (tsc -b && vite build)
bun run tauri build   # Full native bundle (runs `build` via beforeBuildCommand)
bun run build:dmg     # macOS DMG + signing (scripts/build/build-macos-app.sh)
```

There is a single build flow — the old `lite`/`full` split is gone, because there is
no Python runtime to bundle.

---

## Data Flow

```
User adds files
    ↓
Rust extracts metadata (FFprobe) + generates thumbnail (FFmpeg)
    ↓
User clicks "Start"
    ↓
Solver: Requirements → ModelPlan   (engine/plan.rs)
    ↓
Rust checks every model in the plan is installed
    (else MODEL_NOT_INSTALLED → download UI)
    ↓
FFmpeg extracts 16 kHz mono s16le WAV (cached next to the media as *_audio.wav)
    ↓
┌── plan.passthrough ──────────────────────────────────────────────┐
│ audiocpp_cli --task asr  (one unsplit call)                       │
│   --segments-out / --turns-out  → text + times + speakers         │
└───────────────────────────────────────────────────────────────────┘
   or
┌── multi-stage ───────────────────────────────────────────────────┐
│ audiocpp_cli --task vad      → vad_chunks.json (speech segments)  │
│ Rust slices the WAV per chunk, writes requests.json               │
│ audiocpp_cli --task asr --request-sequence → one text per chunk   │
│ (opt) --task diar → turns → Rust turn attribution by overlap      │
└───────────────────────────────────────────────────────────────────┘
    ↓
Rust post-processes (split/estimate/merge, speaker-aware) → writes every format
    ↓
Translation enabled AND source ≠ target?  (translate::language::should_translate)
    │ yes  → llama-server (127.0.0.1, spawned per stage) → <stem>.<lang>.<ext>
    │ no   → skipped; originals are already on disk either way
    ↓
Rust emits events → React updates the UI
```

Translation is an **enhancement, never a precondition**: the original subtitle files are
written before step 5 starts, so a translation failure is logged as a warning and the task
still completes. It never reaches `transcribe_error`.

### Model plans (capability-driven model selection)

Which model runs is **not hardcoded**. `docs/tech-spec/model-selection-design.md`
defines a **requirement → plan solver**:

```
Requirements { word_timestamps, diarization, passthrough, language, speed, backend }
        │
        ▼  engine/plan.rs  (pure functions; unit-testable without AppHandle)
 ModelPlan { steps: [Vad | Asr | Align | Diarize | Passthrough | TurnAttribution], … }
        │
        ▼  engine/mod.rs → engine/direct.rs (passthrough) or the VAD-sequence pipeline
```

Rules that follow from this:

- **Capabilities live in `models.json`, not in Rust.** Adding a model is a JSON
  change. The solver reads `capabilities.timestamps` / `capabilities.diarization` /
  `capabilities.produces_text`, plus `roles`, `languages`, `source`, `speed`, `tier`.
- **Tongyi/FunASR (`source: "tongyi"`) is preferred** when scores tie — hence
  `sense_asr` / `fun_asr_nano` are the defaults for plain ASR.
- **Every requirement must be satisfiable by an audiocpp `task`.** There is no
  `spk`/`sv` task in audio.cpp, so CAM++ (the old `core-engine/.../diarizer.py`) is
  **not reproducible** — speech separation comes from a `diar`-task model or from an
  ASR model that emits speaker labels itself.
- **Passthrough is a first-class mode.** When the plan's single step is a
  `Passthrough` model (`moss_transcribe_diarize`, which emits text + timestamps +
  speaker labels in one call), the whole VAD → slice → `requests.json` → sequence
  pipeline is **skipped** and the file is transcribed unsplit. Per-request options
  may then go on the command line, since no `--request-sequence` is involved
  (PoC §4.1) — but note `--request-option` and `--request-sequence` still must not
  be combined.
- **Diarization requires a non-Tongyi model.** Neither `sense_asr` nor `fun_asr_nano`
  exposes speaker output; FunASR's "ASR + VAD + CAM++" pipeline is a *Python*
  composition, not a checkpoint capability.
- The `[Speaker A]: ` prefix in `subtitle/writer.rs` is fed by `RawChunk.speaker`;
  `subtitle/postprocess.rs` must not merge across differing speakers.
- **Translation models are never part of a `ModelPlan`.** They are `role: "translate"`
  entries with `engine: "llamacpp"`, and `plan::solve` filters the catalogue down to
  `engine == audiocpp && !role.translate` before it looks for candidates. Which
  translation model runs is chosen **by role from `models.json`**
  (`catalog::translation_models` / `default_translation_model_id`, best `tier` first),
  independently of the recognition plan — there is no `StepKind` for translation.
  For such an entry `family` is only a prompt-template hint and never reaches audio.cpp.

### Word-level timestamps (opt-in, second timing source)

Two timing sources exist and they must not be mixed:

| Model | Timing | How cues get their times |
|---|---|---|
| `sense_asr`, `fun_asr_nano`, MOSS passthrough | none / segment | **estimated**: the VAD chunk window is shared out in proportion to character count (`postprocess::distribute_times`). The chunk boundary is the only real anchor. |
| `qwen3_asr` + its forced aligner | word | **measured**: a cue starts at its first word and ends at its last (`postprocess::group_words_into_cues`). No estimation at all. |

`qwen3_asr` runs a **different pipeline** when word timing is asked for
(`engine/aligned.rs`): one unsplit CLI call instead of VAD → slice →
`requests.json` → sequence, because the model chunks internally and shifts word
spans back onto the original timeline itself.

Four flags are load-bearing; `docs/tech-spec/word-timestamps-poc.md` records the
measurements:

- `--words-out` — the word stream itself.
- `--request-option preserve_punctuation=true` — **without it** a multi-chunk run
  loses all punctuation and space-joins CJK (`大 家 好 我 是 婷 婷 …`).
- `--session-option qwen3_asr.forced_aligner_path=…` — the aligner is a session
  option, **not** a planned step. audio.cpp's `align` task aligns an *existing*
  transcript, and MonoSub never has one, so `StepKind::Align` stays unused.
- `--session-option qwen3_asr.vad_path=…` — `audio_chunk_mode=auto` switches to
  VAD chunking when timestamps are requested, and its default path is relative.

Two numbers were tuned on measured data and are **not** the old Python values:

```rust
// subtitle/mod.rs — a cue splits on a pause longer than this
PostProcessConfig::word_gap_threshold = 0.15;  // measured: real boundaries 0.40/0.16/0.16, in-phrase 0.08/0.0
// subtitle/writer.rs — a silence worth cutting on in the sidecar
writer::SPACING_MIN_SECONDS = 0.15;            // 0.5 emitted zero spacings on every measured clip
```

`word_gap_threshold` is **separate from `gap_threshold`** on purpose:
`gap_threshold = 0.5` is a *merge* threshold for chunk windows, and reusing it for
word pauses collapsed a 10 s clip into one cue.

The capability claim is conditional, and the catalogue says so: `qwen3_asr`
declares `requires: ["qwen3_forced_aligner_0_6b_q8_0"]`, and `plan::gaps` turns an
uninstalled requirement into a downloadable gap. Its `language_tokens` map
`zh → Chinese` etc., because the aligner picks CJK tokenisation from that same
string.

Degradation is always to estimated timing, never to failure: a missing aligner, a
model that cannot align, an empty `words.json`, or an alignment error all log a
warning and continue. **A missing `words.json` is success, not an error** —
measured: an unalignable input exits 0 and writes only `text_out`.

Output is additive: `<stem>.srt|vtt|txt|md` plus `<stem>.words.json`, a
Scribe-compatible word/silence document (same field names as the old Python
`--raw` mode) that an agent can cut video on. A run with no words writes no
sidecar. Word timing **combined with speaker separation** is not implemented.

### Why segmentation is a separate stage from ASR

`sense_asr` (SenseVoice) has **no word/segment timestamps**; it emits one flat text
blob. So segmentation is done up front by **Silero VAD** — which runs *inside
`audiocpp_cli`* (`--task vad --family silero_vad`), not in Rust. Rust only builds
that invocation and parses the returned sample offsets (`engine/vad.rs`); there is
no VAD algorithm in this codebase. Timestamps are then *estimated* inside each VAD
chunk in proportion to character count. Chunk boundaries are therefore the only
real timing anchor.

Three VAD options are quality-sensitive and were tuned on measured evidence (see
`docs/tech-spec/audiocpp-accuracy-p1-23.md`). They are **CLI arguments**, declared
in Rust only so a test can pin their values:

```rust
// engine/vad.rs — forwarded to audiocpp_cli as --vad-chunk-* flags
pub const VAD_MAX_SECONDS: f64 = 15.0;
pub const VAD_MERGE_GAP_SECONDS: f64 = 0.2;
pub const VAD_PADDING_SECONDS: f64 = 0.1;
```

`padding p` widens each speech span by `p` on both sides, so a raw silence gap `g`
becomes `g - 2p`, and two spans merge when `g - 2p < merge_gap` — that merge
decision is made by the CLI, not here. Over-merging irrecoverably fuses two
utterances and misplaces their boundary; under-merging is recovered downstream by
`subtitle::postprocess`, which re-joins cues closer than its own 0.5 s
`gap_threshold`. Prefer under-merging when in doubt.

### Why translation is a separate engine, and how it is driven

Translation hangs off the **end** of the pipeline: the input is the already-timed
`Vec<Segment>` produced by `subtitle::postprocess`, and only `text` may change — times and
speaker labels are the ASR's output and are never re-derived. That is why the whole
subtitle is written first and the translated files are additive.

- **Engine**: `translate/server.rs` spawns the bundled `llama-server` on a random
  `127.0.0.1` port, one process per translation stage, and kills it on drop. It waits for
  `GET /health` to answer `{"status":"ok"}` before sending anything.
- **Output contract**: numbered lines, **not** structured output. Measured against
  HY-MT1.5-1.8B, `response_format: json_schema` echoed the source untranslated or returned
  an empty array, and a raw GBNF grammar returned `[]`. Asking for `1. …` lines and
  parsing them produced exact cue counts at batch sizes 10/20/40, so `BATCH_SIZE = 20`.
  See `docs/tech-spec/translation-poc.md` §5.
- **The prompt wording is load-bearing; do not "simplify" it** (`translate/prompt.rs`).
  Three measured traps, all of which silently produce *wrong output* rather than an error:
  1. **Never prepend the model card's single-segment template to the batch contract.** Two
     competing instructions make the model drop numbering — and at 20 cues it returns the
     **source text echoed untranslated**, which is non-empty, so a naive "did we get a
     reply?" check accepts it as success.
  2. **The count sentence must be its own paragraph and must be accompanied by a two-line
     worked example.** With the count but no example, batches of 2–4 cues failed on *every*
     attempt and 14 failed intermittently; with the example, 27/27 requests across sizes
     2–20 were exact. Folding the count into the instruction sentence also breaks numbering.
  3. **A lone cue uses a different prompt.** The numbered shape is unreliable for exactly
     one short cue, so `single_cue_prompt` uses the card's single-segment form and the
     answer is taken verbatim. For a **Chinese source the target must be named in Chinese**
     (`翻译为英语`): writing `翻译为English` made the model answer in Chinese for every probe,
     because it reads the English word as content instead of as the target language.
  These were found by running the shipped prompt against the real model; the unit tests pin
  the shapes, and the evidence is in `docs/tech-spec/translation-poc.md` §5.4.
- **Degradation**: a batch whose cue count does not match is retried at half size, down to
  a single cue; a cue that still fails keeps its original text. A translation run is
  therefore always *complete* in cue count, even when some cues are untranslated. The
  single-cue rung also rejects two measured non-answers as failures rather than shipping
  them: a reply identical to the source, and the prompt's own instruction coming back.
- **`models.json` schema for a translate entry**: `roles: ["translate"]`,
  `engine: "llamacpp"` (absent means `audiocpp`, so every recognition entry is unchanged),
  and `prompt_template` naming the prompt wording (`hy_mt`). `capabilities` is declared as
  `timestamps: none / diarization: none / produces_text: true` — it describes what the
  model can do to a subtitle, not a recognition step. `family` is a prompt-template hint
  here, and `source` is `other`, because `Source` is a closed enum and HY-MT belongs to no
  existing family.
- **Failure semantics**: a translation error is a warning, never a task failure. The
  recognised files are already on disk; `translate_and_write` logs the outcome and, when
  nothing at all was translated, deliberately writes no `<stem>.<lang>.<ext>` file rather
  than a duplicate of the original under a language it does not contain.
- **Language tags**: `RawChunk.language` carries the ASR's own `<|zh|>` tag, read by
  `subtitle::postprocess::extract_language_tag` **before** `clean_text` deletes it. The tag
  set is closed (`zh/en/yue/ja/ko`) because the same delimiters carry emotion, event and
  ITN markers. No tag means *unknown*, and unknown means **translate anyway** — a wasted
  run on a same-language input is cheaper than silently dropping a translation. When there
  is no tag and the request was `auto`, `language::detect_by_script` is the last resort and
  answers only for unambiguous scripts (kana ⇒ `ja`, hangul ⇒ `ko`, CJK ≥ latin ⇒ `zh`).

---

## IPC Communication Pattern

### Frontend → Backend (Tauri Commands)

| Command | Purpose |
|---------|---------|
| `start_transcription` | Begin transcription task |
| `cancel_transcription` | Cancel running task |
| `get_video_metadata` | Extract duration, resolution, size |
| `generate_thumbnail` | Create video preview |
| `resolve_paths` | Normalise dropped paths |
| `open_in_finder` | Reveal file in system file manager |
| `list_models` | Catalogue + per-model install state |
| `download_model` | Stream a model, verify size/SHA256, then rename |
| `cancel_model_download` | Abort a download and delete the `.part` |
| `remove_model` | Delete a model directory |
| `open_models_dir` | Reveal the models folder |
| `activate_license` / `validate_license` / `deactivate_license` | Polar licensing |
| `check_github_release_update` | Update check |
| `update_tray_menu` | Localise tray labels |
| `resolve_model_plan` | Solve `Requirements` → `ModelPlan`; drives the Settings preview |

The catalogue contains both engines, but only audio.cpp entries can fill a recognition
plan; `translate` entries are grouped separately by `role` in the UI.

### Backend → Frontend (Tauri Events)

| Event | Payload | Purpose |
|-------|---------|---------|
| `transcribe_progress` | `{task_id, progress, status, step?, total_time?}` | Progress + step updates |
| `transcribe_complete` | `{task_id, total_time?}` | Finished; all formats written |
| `transcribe_error` | `{task_id, status}` | Failed; `status` is user-showable |
| `model_download_progress` | `{id, downloaded, total}` | Throttled to ≥200 ms |
| `model_download_complete` | `{id}` | Model ready |
| `model_download_error` | `{id, message}` | Download failed |

`step` is `{current_step, total_steps, step_name, step_elapsed, total_elapsed}` with
`step_name` from the frozen set `Extracting audio` / `Loading model` / `Transcribing` /
`Writing subtitle` / `Translating`. `Translating` was **appended**; the first four keep
their exact spelling, because `FileItem.tsx` derives the i18n key as
`steps.<lowercase, spaces → _>` and renaming one would blank that step. `total_steps` is
**4 normally and 5 when the run translates**: `Steps::new(translating)` picks the
denominator per run, and the first step event already carries the right one.

`TranscribeRequest` carries three translation fields, all `Option` so an older frontend
that sends none of them translates nothing: `translate_enabled`, `target_language`, and
`translation_model` (the UI sends `''`/absent for "catalogue default").

There is no stdout protocol any more — the engine is called directly, not via a
line-parsing subprocess.

---

## Key Files Reference

| Path | Purpose |
|------|---------|
| `src/App.tsx` | Event listeners for progress/completion/error |
| `src/store/useStore.ts` | Zustand state, `persist` v4 + migrate |
| `src/lib/models.ts` | Model commands/events, `MODEL_NOT_INSTALLED_ERROR` |
| `src/components/Settings.tsx` | Settings incl. the Models + Translation groups |
| `src/components/SettingsModelPlan.tsx` | Model-plan requirements + live preview + manual overrides |
| `src/components/SettingsTranslation.tsx` | Translation switch, target language, translation-model download |
| `src-tauri/src/engine/mod.rs` | The transcription pipeline (dispatches on the plan) |
| `src-tauri/src/engine/plan.rs` | Requirement → ModelPlan solver |
| `src-tauri/src/engine/direct.rs` | Passthrough (one-shot) inference path |
| `src-tauri/src/engine/aligned.rs` | Word-timestamp path: one `qwen3_asr` call with `--words-out` |
| `src-tauri/src/engine/words.rs` | `words.json` parsing, sample→seconds, script-aware joining |
| `src-tauri/src/engine/cli.rs` | Binary resolution + process management |
| `src-tauri/src/engine/catalog.rs` | `models.json`, install checks, language mapping |
| `src-tauri/src/commands/transcribe.rs` | Task registry, events, cancellation, translation stage |
| `src-tauri/src/commands/models.rs` | Model download/verify/remove |
| `src-tauri/src/translate/mod.rs` | Translation stage: should-translate → batch → validate → degrade |
| `src-tauri/src/translate/server.rs` | `llama-server` spawn, `/health` probe, one chat completion |
| `src-tauri/src/translate/prompt.rs` | Batched numbered prompts; template comes from `models.json` |
| `src-tauri/src/translate/parse.rs` | Numbered-line response parsing |
| `src-tauri/src/translate/language.rs` | Tag normalisation, `zh` vs `zh-tw`, skip decision |
| `src-tauri/src/subtitle/writer.rs` | Format writers |
| `src-tauri/resources/models.json` | Model catalogue (v2 schema, incl. `engine` / `translate` role) |
| `docs/tech-spec/model-selection-design.md` | Requirement → ModelPlan design |
| `docs/tech-spec/translation-design.md` | Translation design |
| `docs/tech-spec/translation-poc.md` | Measured llama.cpp / HY-MT behaviour (pins, speed, format) |
| `src-tauri/tests/fixtures/` | Subtitle goldens (+ their generator) |
| `scripts/build/fetch-audiocpp.sh` | Pinned upstream binary + VAD download |
| `scripts/build/fetch-llamacpp.sh` | Pinned llama.cpp `llama-server` + dylib download |

---

## Code Style Guidelines

### TypeScript/React

- Functional components with hooks
- Import order: React → Third-party → Local → Types
- `@/` alias for imports from `src/`
- Zustand with persistence for settings
- Keep both locales (`en.json`, `zh-CN.json`) in sync (see `scripts/utils/check-i18n-keys.mjs`)

### Rust

- `tracing` for structured logging
- `Result<T, String>` at Tauri command boundaries; `EngineError` inside `engine/`
- Async/await with `tokio`
- Follow the existing module structure; keep `engine/` free of Tauri UI concerns

---

## State Management (Zustand)

```typescript
files: VideoFile[]              // File queue
storageMode: 'same' | 'custom'  // Output location
customStoragePath: string       // Custom output directory
settings: AppSettings           // User preferences (persisted)
isProcessing: boolean           // Global processing state
currentTaskId: string | null    // Active task
```

Persisted to `localStorage` as `monosub-storage` at `version: 4`. The `migrate` function
**drops** `enableNoiseReduction`, `enableVolumeNormalization`, `enableBilingual` and
`bilingualTargetLang`: those
settings never reached the engine in the old code either (the Python engine hardcoded
the filters), so they were dead. The equivalent ffmpeg filters are now always applied.

The v3 step **adds** model-plan defaults and must not drop anything; the v4 step adds the
translation defaults the same way:

```ts
// settings (added in v3)
requireWordTimestamps: boolean            // false
speedPreference: 'fast' | 'balanced' | 'accurate' // 'balanced'
// top level (added in v3)
modelOverrides: Partial<Record<'asr' | 'vad' | 'align' | 'diar', string>>

// settings (added in v4; translation)
translateEnabled: boolean                 // false
translateTargetLanguage: TranslateTargetLanguage // 'en' (never 'auto')
translationModelId: string                // '' means "catalogue default"
```

`settings.defaultDiarization` keeps the meaning the old `enableSpeakerDiarization` had; it
now maps to `Requirements.diarization` and seeds each newly queued file's own toggle instead
of being a disabled placeholder. `TranslateTargetLanguage`
is `Exclude<OutputLanguage, 'auto'>`: `auto` is a *source*-detection value and "translate
into auto" has no meaning, so the Rust side skips such a run rather than guessing.

---

## Important Implementation Details

### Binary Path Resolution

Bundled binaries (`ffmpeg`, `ffprobe`, `audiocpp_cli`, `llama-server`) are resolved in this
order by the single `engine/cli.rs::resolve_bundled_binary` (`pub(crate)`, shared by both
engines so the rules cannot drift):

1. `MONOSUB_FFMPEG_PATH` / `MONOSUB_FFPROBE_PATH` / `MONOSUB_AUDIOCPP_PATH` /
   `MONOSUB_LLAMACPP_PATH` override
2. Dev checkout: `src-tauri/resources/bin/`
3. Next to the executable
4. `<Contents>/Resources/resources/bin/` (app bundle)
5. System paths / `PATH` (ffmpeg only)

Names use the `<name>-<target-triple>` convention, e.g.
`audiocpp_cli-aarch64-apple-darwin` and `llama-server-aarch64-apple-darwin`.

`llama-server`'s libraries must sit **flat in `resources/bin/`**, next to the executable —
its only `LC_RPATH` is `@loader_path`. ffmpeg's dylibs live in `bin/lib/` and are resolved
by ffmpeg's own rpath; the two layouts must not be merged.

### Model Storage

Models live under `<models_dir>/<target_directory>/`, mirroring upstream's
`model_specs` layout so a user can drop files in by hand. `<models_dir>` defaults to
`<app_data_dir>/models` and is **user-configurable** (Settings → 模型 → 存储位置,
persisted to `<app_config_dir>/settings.json`, or `MONOSUB_MODELS_DIR`): models are
0.2–1.2 GB each, so pointing at an existing folder must not require re-downloading.
`model_dir::configured_root` is the single resolver used by both the catalogue and
the downloader, so they can never disagree. `is_installed` checks file
existence plus size match and deliberately **never hashes** — hashing a 250 MB file on
every startup is not acceptable. Hashing happens once, after download.

### Output Naming

The original files are derived by `output_path_for` and are **never touched by
translation**: the first format writes to `output_path` verbatim, later ones replace only
the extension. Translated files are additive and go through a separate function
`translation_output_path`, producing `<stem>.<lang>.<ext>` next to the original
(`video.srt` → `video.en.srt`). The `<lang>` is `language::file_tag`: canonicalised
(`zh-tw` → `zh-Hant`) and reduced to `[A-Za-z0-9-]`, so it can never contain a path
separator or a second dot. `derive_output_paths` returns originals first and translated
paths second, which is what makes a cancelled run clean up both.

### The `audiocpp_cli` Contract (hard-won; PoC §4.1/§4.2)

- **Per-request options must go in `requests.json`, never on the command line.**
  `--request-option` combined with `--request-sequence` makes v0.9.1 fail with
  `unknown options`. `engine/cli.rs` rejects that combination in code, and a test
  pins the absence of the flag.
- `--text-out` naming is `<stem>_<safe_id><ext>`; the CLI also prints
  `text_out[<id>]=<path>`, and we prefer parsing that line over reconstructing it.
- `metrics[<id>].…` is printed per request **as it completes**, which is what drives
  progress. Do not assume it arrives in order.
- An all-silent input returns `[]` with **exit code 0** — that is success, not an
  error. The app writes an empty subtitle.
- `--session-option sense_asr.vad_model_path=<dir>` must be passed explicitly, or the
  CLI falls back to its own relative `assets/…` path.

### Audio Extraction

Every input (video **and** audio) is converted to 16 kHz mono s16le WAV, cached next
to the media as `<stem>_audio.wav` and reused while `cache_mtime >= source_mtime`.

The filter chain is preserved from the previous engine because it changes the
recognition input:

```
highpass=f=80,lowpass=f=8000,afftdn=nf=-25:nr=12:nt=w,loudnorm=I=-16:TP=-1.5:LRA=11
```

If the filtered attempt fails, extraction is retried once **without** `-af`.

### Language Mapping

`engine/catalog.rs::map_language`. The CLI *accepts* 23 language tags but only
`{auto, zh, en, yue, ja, ko, nospeech}` map to a real token; everything else silently
becomes `auto`. The mapping therefore happens on our side: `zh-tw` → `zh`, and
`es`/`de`/`fr`/`pt` → `auto`. **Recognition** has never converted to Traditional.

Translation is the one place `zh` and traditional Chinese are distinct targets: the
translation model is prompted with the target's script (`zh` vs `zh-Hant`) and can convert
between them, so `zh-tw` → `en` and `zh` → `zh-tw` are both real translation jobs
(`translate/language.rs`).

### Cancellation

`cancel_transcription` aborts the task and then awaits it, so cleanup is
deterministic. The CLI child dies via `kill_on_drop`, and `engine::transcribe` owns a
`Drop` guard for its temp directory — aborting the task is sufficient for both.
The `llama-server` child is reclaimed the same way: the translation stage owns a
`LlamaServer` whose `Drop` kills it, so an abort mid-translation leaves no orphan.
Partially written subtitle files are removed **only if their mtime is at or after the
task start**, so a pre-existing subtitle the user already had is not destroyed. That
list is `derive_output_paths`, which includes the translated `<stem>.<lang>.<ext>` paths
when translation is on — so a cancelled run does not leave a newer translated file behind.

### Stale Closure Prevention

`App.tsx` uses a refs pattern for event listeners:

```typescript
const filesRef = useRef(useStore.getState().files);
useEffect(() => {
  const unsubscribe = useStore.subscribe((state) => {
    filesRef.current = state.files;
  });
  return unsubscribe;
}, []);
```

---

## Environment Variables

| Variable | Purpose |
|----------|---------|
| `MONOSUB_FFMPEG_PATH` | Override the FFmpeg binary path |
| `MONOSUB_FFPROBE_PATH` | Override the FFprobe binary path |
| `MONOSUB_AUDIOCPP_PATH` | Override the audio.cpp CLI path |
| `MONOSUB_LLAMACPP_PATH` | Override the llama.cpp `llama-server` path |
| `MONOSUB_MODELS_DIR` | Override the models directory (wins over the in-app setting) |
| `APPLE_SIGNING_IDENTITY` | Code signing identity (optional) |
| `APPLE_ID` / `APPLE_PASSWORD` / `APPLE_TEAM_ID` | Notarization credentials (optional) |

---

## Security Considerations

1. **Local-first**: No cloud dependencies; all media processing is local
2. **Downloaded models are verified**: byte size and, when pinned, SHA256
3. **Path safety**: model file paths are validated against directory traversal before
   any download destination or deletion is honoured
4. **File system access**: limited to user-selected directories and app data
5. **Subprocess isolation**: both inference binaries run as separate child processes with
   `stdin` closed and no inherited environment secrets; `llama-server` listens only on
   `127.0.0.1` and is killed with its task

---

## Licensing / Attribution

**MonoSub is a personal learning project and is not distributed commercially.**
Model licences are therefore recorded for *attribution only* — they are **not** a
gate on which models may be offered. There is no commercial-use red line: a model
whose upstream licence is non-commercial (e.g. CC-BY-NC) may still be listed and
downloaded.

What still matters is honesty about provenance:

- `audiocpp_cli` is **Apache-2.0**, so its LICENSE text must ship with the app —
  see `src-tauri/resources/licenses/`.
- `llama-server` (and its dylibs) is **MIT**; its LICENSE text ships with the app as
  `resources/licenses/llama.cpp-LICENSE`.
- **HY-MT1.5-1.8B** is *not* bundled: it is downloaded on demand and recorded for
  attribution only (`Tencent HY Community License Agreement`, release date 2025-12-30,
  licence text: <https://huggingface.co/tencent/HY-MT1.5-1.8B-GGUF/blob/main/License.txt>).
  It is never re-hosted.
- Each catalogue entry names its licence (`licenses[]` / `license`) and its
  `attribution`; the About page surfaces them.
- Adding a bundled component means updating
  `src-tauri/resources/licenses/THIRD-PARTY-NOTICES.md`.
- **Never re-host** third-party weights in the repo or the DMG — models are always
  downloaded on demand (this keeps the app small and the licences intact).
- Licences with an explicit *redistribution* restriction (e.g. NVIDIA's
  "local-use only, pending redistribution approval") are the one real exception:
  they may be used locally but must not be added to a downloadable catalogue until
  approval is confirmed — mark them `tier: experimental` and keep them
  non-downloadable, or omit them entirely.

> Rationale: several strong diarization models (Sortformer) are CC-BY-NC, while the
> permissive ones are not necessarily the best. A blanket "no non-commercial models"
> rule would silently remove the best tool for the job.

---

## Troubleshooting

**audiocpp_cli missing / not executable:**
```bash
./scripts/build/fetch-audiocpp.sh
```

**llama-server missing / translation reports `engineMissing`:**
```bash
./scripts/build/fetch-llamacpp.sh
```

**Model missing:** the app reports `MODEL_NOT_INSTALLED`; download it from
Settings → Models. A missing *translation* model reports the same sentinel with its id
(`MODEL_NOT_INSTALLED:<id>`) and points at Settings → Translation. Models can also be
placed manually under `<app_data_dir>/models/<target_directory>/`.

**FFmpeg not found:**
```bash
brew install ffmpeg     # macOS
```

**Rust compilation errors:**
```bash
rustup update
cd src-tauri && cargo clean && cargo build
```

---

## Contributing Guidelines

1. Follow existing code style and patterns
2. Update this file if you change architecture or build process
3. Run `bun run test:rust`, `bun run check`, `bun run lint`, and `bun run check:i18n` before committing
4. Ensure i18n strings are added to **both** locales (`en.json` and `zh-CN.json`)
5. Log important events using `tracing` (Rust) or `console.log` (Frontend)
6. Tests that need the bundled binary or model must skip with a reason, not fail

---

## Resources

- [Tauri Documentation](https://tauri.app/)
- [audio.cpp](https://github.com/0xShug0/audio.cpp) — the recognition engine
- [llama.cpp](https://github.com/ggml-org/llama.cpp) — the translation engine
- [FunASR / SenseVoice](https://github.com/modelscope/FunASR) — model family
- [Hunyuan MT](https://huggingface.co/tencent/HY-MT1.5-1.8B-GGUF) — the translation model
- [Tailwind CSS](https://tailwindcss.com/)
- [Zustand Documentation](https://github.com/pmndrs/zustand)
- `docs/tech-spec/refactor-audiocpp.md` — the refactor spec
- `docs/tech-spec/audiocpp-poc.md` — measured CLI behaviour
- `docs/tech-spec/audiocpp-accuracy-p1-23.md` — accuracy + VAD tuning evidence
- `docs/tech-spec/translation-design.md` — translation design
- `docs/tech-spec/translation-poc.md` — measured llama.cpp / HY-MT behaviour
