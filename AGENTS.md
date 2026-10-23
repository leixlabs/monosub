# MonoSub - Agent's Guide

> This document is intended for AI coding agents working on the MonoSub project.
> Read this file to understand the project architecture, conventions, and development workflow.

## Project Overview

**MonoSub** is a local-first desktop application for converting videos and audio files to subtitles without cloud dependencies. It provides a clean, modern UI for batch processing media files using local AI speech recognition, with optional local translation into a second language.

Speech recognition runs through **audio.cpp** (`audiocpp_cli`) and translation through
**llama.cpp** (`llama-server`), both C++ inference engines distributed as prebuilt binaries
and run as **separate child processes** — neither is linked into the app. A third bundled
binary, **yt-dlp**, downloads Bilibili subtitles or audio for URL import. There is
**no Python** in the runtime.

### Key Features

- 🎬 **Multi-file queue processing** - Manage multiple video/audio files simultaneously
- 🖼️ **Video thumbnails** - Automatic preview generation
- ⚡ **Local AI processing** - audio.cpp speech recognition and llama.cpp translation, both Metal accelerated
- 🧩 **Pluggable model plans** - Requirement-driven model selection (speed, word-level
  timestamps, speaker diarization, language); Tongyi/FunASR models preferred
- 🌍 **Local subtitle translation** - Optional post-recognition pass with a dedicated
  translation model (HY-MT1.5-1.8B); writes `<stem>.<lang>.<ext>` and never touches the originals
- 🔗 **URL import (Bilibili)** - Paste links to fetch the platform subtitle directly, or fall
  back to downloading the audio and transcribing it locally
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
│  ├── Translation orchestration (llama-server over HTTP)         │
│  └── URL import: yt-dlp subtitle fetch / audio download         │
└─────────────────────────────────────────────────────────────────┘
        │ spawns                    │ spawns
        ▼                           ▼
  audiocpp_cli                 llama-server
  (prebuilt, bundled;          (prebuilt, bundled;
   ~31 MB, no deps)             ~68 MB incl. its flat lib*.dylib)
        │                           │
   recognition only            translation only
   (its own ggml)              (its own ggml)

  … and a third child, `yt-dlp-<triple>` (~35 MB, Unlicense), for URL import only:
  it fetches a platform subtitle or downloads the audio, and never runs inference.
```

The inference engines never share a process: audio.cpp and llama.cpp each embed their own
ggml, so linking both into one binary would collide. `translate/server.rs` talks to the
child over HTTP on `127.0.0.1` instead. `yt-dlp` is spawned per URL-import stage and is a
downloader, not an engine — it is listed here only so nobody assumes there are exactly two
bundled binaries.

A second entry point runs over the same pipeline: `monosub-cli` links the same crate
(`src-tauri/src/lib.rs`) instead of the Tauri shell, so it re-implements nothing — see
命令行入口（`monosub-cli`） below.

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
│   │   ├── Settings.tsx         # Settings shell + 5 sidebar groups
│   │   ├── SettingsModelPlan.tsx
│   │   ├── SettingsTranslation.tsx
│   │   ├── SettingsUrlImport.tsx # urlImport group: dir, cookies, prefer-local
│   │   ├── UrlImportDialog.tsx  # Paste links; per-URL errors; mirrors Settings
│   │   ├── UrlImportItem.tsx    # One import row above FileList (not in files)
│   │   └── ThemeToggle.tsx
│   ├── store/
│   │   └── useStore.ts          # Zustand state + persist v6 migration
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
│   │   ├── urlImport.ts         # parse/start/cancel + the url_import_* listeners
│   │   └── utils.ts
│   ├── App.tsx
│   └── main.tsx
│
├── src-tauri/                   # Rust: app logic + inference orchestration
│   ├── src/
│   │   ├── lib.rs               # The crate: 9 pub mods, shared by both binaries
│   │   ├── main.rs              # Tauri shell: plugins, command registration, tray
│   │   ├── bin/monosub-cli.rs   # The CLI (see 命令行入口（`monosub-cli`）)
│   │   ├── engine/              # audio.cpp integration
│   │   │   ├── mod.rs           # VAD → slice → requests.json → sequence ASR
│   │   │   ├── cli.rs           # Binary resolution, spawn, stderr ring buffer
│   │   │   ├── catalog.rs       # models.json, install checks, language mapping
│   │   │   ├── plan.rs          # Requirement → ModelPlan solver (pure, unit-tested)
│   │   │   ├── spec.rs          # GGUF spec → which --request-option a family accepts
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
│   │   │   ├── stage.rs         # Tauri-free half: gate, <stem>.<lang>.<ext> writes
│   │   │   └── error.rs         # TranslateError + user-showable status
│   │   ├── site/                # URL import (Bilibili): yt-dlp + subtitle hit/fallback
│   │   │   ├── mod.rs           # Commands, IMPORT_TASKS, Semaphore(3), events, job stages
│   │   │   ├── pipeline.rs      # The decision path: select → fetch → write → or fall back
│   │   │   ├── deps.rs          # The seam pipeline runs against (RealDeps / FakeDeps)
│   │   │   ├── url.rs           # parse_input: split/normalise/de-dupe/whitelist (pure)
│   │   │   ├── ytdlp.rs         # Binary path, constant argv (pure), spawn, stderr ring
│   │   │   ├── resolve.rs       # info dict → SiteMedia; multi-P verdict (pure)
│   │   │   ├── lang.rs          # select(OutputLanguage, &SiteMedia) → Option<Pick>
│   │   │   ├── srt.rs           # SRT text → Vec<Segment>, timeline untouched (pure)
│   │   │   ├── naming.rs        # Title sanitising + `<title> [<id>].<ext>` (pure)
│   │   │   ├── progress.rs      # `PROG` download-template line parsing (pure)
│   │   │   └── error.rs         # ImportError + stderr classification → status
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
│   │   │   └── tray.rs          # Menu-bar item: renders labels + progress rows
│   │   └── logging.rs
│   ├── resources/
│   │   ├── models.json          # Model catalogue (v2: capabilities/roles/tier/engine)
│   │   ├── licenses/            # audio.cpp, llama.cpp, yt-dlp + notices
│   │   ├── bin/                 # fetched: ffmpeg, ffprobe, audiocpp_cli, yt-dlp, lib/
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
│   │   ├── fetch-ytdlp.sh       # Pinned download + SHA256 of yt-dlp + its licences
│   │   ├── prepare-ffmpeg.sh
│   │   └── prepare-resources.sh # Runs the fetches, then regenerates MANIFEST
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

# 2. Fetch the bundled runtimes: ffmpeg, audiocpp_cli + silero VAD, llama-server
#    + dylibs, and yt-dlp (URL import). One command, all of them.
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
tauri-pilot run src-tauri/tests/pilot/model-plan.toml   # the plan's four questions
tauri-pilot run src-tauri/tests/pilot/smoke.toml --junit /tmp/pilot.xml
```

The plugin requires Tauri ≥ 2.12 ({plugin, CLI and `@tauri-apps/*` versions all
moved together}). It is compiled out of release builds, so `tauri build` is
unaffected. `screenshot` (DOM html-to-image) is flaky on this page; prefer
`screenshot_native --window-id <id> --output <path>` when a PNG is needed.

Three traps when writing or running a scenario:

- **A changed Rust file needs the dev binary rebuilt, not just Vite's HMR.** Vite
  reloads the webview, so the UI looks fresh while the *backend* still answers
  from the previous build — a scenario then fails on behaviour that is correct on
  disk. Restart `bun run tauri dev` after touching `src-tauri/`.
- **Assert on attributes, not on rendered text.** Resolution is an async IPC
  round trip, so a text assertion straight after a click races the solver (a bare
  `assert-contains` on the plan panel passes or fails on timing). `SettingsModelPlan`
  publishes `data-plan-id` / `data-plan-models` / `data-plan-memory` /
  `data-plan-timestamps`, which a `wait` step targets deterministically.
- **Message strings are localised.** The smoke scenario asserts `模型`, so it only
  passes with the UI in Chinese; set `monosub-language` (`zh-CN`) before running it
  on a machine whose session is in English.

### Production Builds

```bash
bun run build         # Frontend only (tsc -b && vite build)
bun run tauri build   # Full native bundle (runs `build` via beforeBuildCommand)
bun run build:dmg     # macOS DMG + signing (scripts/build/build-macos-app.sh)
bun run build:msix    # Windows MSIX for the Microsoft Store (Windows 11 only)
```

There is a single build flow — the old `lite`/`full` split is gone, because there is
no Python runtime to bundle.

#### Windows / Microsoft Store

The Store package is built on Windows 11 x64 only: Tauri cannot cross-compile a
Windows target from macOS (MSVC link + WebView2 SDK), and `winapp` requires Windows.
See `docs/tech-spec/microsoft-store-publish-spec.md` for the submission flow.

Windows scripts live in `scripts/windows/`, kept separate from the macOS flows in
`scripts/build/` and `scripts/release/`. Do not merge them: the toolchains, the
artifact formats (DMG vs MSIX) and the signing models all differ, so a change made
for one platform in a shared file can silently break the other. `scripts/utils/`
is the only shared code.

```bash
./scripts/windows/prepare-resources.sh   # Windows binaries into resources/bin
./scripts/windows/build-msix.sh          # → dist/monosub_<version>.0_x64.msix
./scripts/windows/release-msix.sh --tag v2.0.0 # upload to a GitHub release
```

Four things about the Windows path that are easy to get wrong:

- The bundled binaries are named `<name>-x86_64-pc-windows-msvc.exe` — the target
  triple comes **before** the extension. `engine::cli::binary_candidates` appends
  those `.exe` forms on Windows; `commands::video` had always done so separately,
  which is why only the ffmpeg path resolved before.
- `resources/bin/` holds the macOS and Windows binaries side by side; the triple in
  the name keeps them apart. They are gitignored, so each platform's resource
  script fetches only its own set.
- The MSIX is unsigned by default because the Store re-signs every package, and a
  self-signed one is rejected at submission. `--sign` produces a second, dev-signed
  copy for local `Add-AppxPackage` verification only.
- The golden subtitle fixtures are byte-compared, so `.gitattributes` marks them
  `-text`. Without that, `core.autocrlf=true` checks them out as CRLF on Windows and
  every writer test fails on a machine where nothing is wrong.

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
┌── every model, one pipeline ─────────────────────────────────────┐
│ audiocpp_cli --task vad      → vad_chunks.json (speech segments)  │
│ Rust slices the WAV per chunk, writes requests.json               │
│   (fields trimmed to what the family declares — engine/spec.rs)   │
│ audiocpp_cli --task asr --request-sequence → one text per chunk   │
│   --segments-out → real cue boundaries when the model emits them  │
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

A file that came from a **URL import** is an ordinary file by this point: the fallback path
downloads its audio before the row is queued, so the flow above is entered at the top with
a real local path. A **subtitle hit** never enters this flow at all — no WAV, no plan, no
model (see [URL Import](#url-import-bilibili)).

### Model plans (capability-driven model selection)

Which model runs is **not hardcoded**. `docs/tech-spec/model-selection-design.md`
defines a **requirement → plan solver**, and the user answers exactly **four
questions** — everything else is derived:

```
Requirements { language_profile (语种场景), diarization, word_timestamps,
               memory (显存占用), language, backend }
        │
        ▼  engine/plan.rs  (pure functions; unit-testable without AppHandle)
 ModelPlan { steps: [Vad | Asr | Align | Diarize | TurnAttribution], … }
        │
        ▼  engine/mod.rs → the VAD → slice → requests.json → sequence pipeline
```

| # | Setting | Values | What it decides |
|---|---|---|---|
| 1 | `languageProfile` | `mandarin` / `english` / `mixed` / `multilingual` | the family bias (§5.4). `english` biases to the English-first families (`parakeet_tdt` first), which `mandarin` + `auto` would not reach. `mixed`/`multilingual` re-rank the table toward the models that follow a mid-sentence language switch, so `sense_asr` (one token language per session) stops being the pick for Chinese. |
| 2 | `defaultDiarization` | bool | whether the `Asr+Diarize+TurnAttribution` template is enumerated |
| 3 | `requireWordTimestamps` | bool | `word_shortfall_rank` — a ranking term, not a filter, so a catalogue without a word-capable ASR still plans one and reports a gap |
| 4 | `memoryBudget` | `small` / `medium` / `large` | `footprint(spec)` (weight-file bytes) → `over_budget_rank` + the `memory_match` score term |

`best_model_for_role` orders candidates as: word-timing grade → 语种场景 family bias →
显存 fit → score. **A capability always outranks a budget**: `中英混合` + 显存小 still
plans the 1 GB multilingual model, because the cheaper alternative drops half the
sentence. Ranking the budget higher was a measured bug (it picked the 254 MB
`sense_asr` for mixed-language audio); the plan preview reports the footprint it
actually ended up at, so an overshoot is visible instead of silent.

**There is no speed preference any more.** In this catalogue a model's weight size
decides both its footprint and its throughput (SenseVoice Q8 is 254 MB and the
fastest thing on offer; the 1.0–1.2 GB models are the slow ones), so asking for
"fast" and "small" separately would be one question too many. v6 of the store
migrates a v5 `fast`/`balanced`/`accurate` onto `small`/`medium`/`large` rather
than dropping it.

Rules that follow from this:

- **Capabilities live in `models.json`, not in Rust.** Adding a model is a JSON
  change. The solver reads `capabilities.timestamps` / `capabilities.diarization` /
  `capabilities.produces_text`, plus `roles`, `languages`, `source`, `speed`, `tier`.
- **A declared capability is not automatically a deliverable one.** The catalogue
  now carries the whole `docs/../模型调查` survey (129 entries, 89 recognition across
  31 families + 40 translation models), and a dozen families declare
  `timestamps: word`. Only `qwen3_asr` has a word path here — `engine/aligned.rs`
  passes `qwen3_asr.forced_aligner_path` / `vad_path` as family-scoped session
  options and needs a companion `align` entry. So `ModelSpec::supports_word_timestamps()`
  (declares `word` **and** `family == WORD_ALIGNED_FAMILY`) is what the solver ranks
  on; ranking on the declaration alone made `parakeet_tdt` get planned for a word
  request and then silently fall back to estimated timing.
- **Tongyi/FunASR (`source: "tongyi"`) is preferred** when scores tie — hence
  `sense_asr` / `fun_asr_nano` are the defaults for plain ASR.
- **显存预算 is a rank, never a filter.** `footprint` classifies a model by its
  download bytes (< 512 MB small, < 1.5 GB medium, else large) — quantisation is
  *not* consulted, because a q4 file that still weighs 1.5 GB needs the same
  1.5 GB. `over_budget_rank` orders candidates by fit; when nothing fits, the
  closest model is still planned and **no gap is reported**, because an oversized
  footprint is a cost, not a missing capability. `MEMORY_OVERSHOOT_PENALTY` (150)
  exceeds `RECOMMENDED_TIER_BONUS + TONGYI_SOURCE_BONUS` (160 combined), which is
  what makes 小 actually beat a recommended 1 GB Tongyi model.
- **语种场景 is also a rank, not a filter.** The catalogue does not declare which
  models truly switch language mid-sentence, so inventing that flag would be a
  claim the app cannot verify; the profile re-orders `bias_families` instead.
- **Every requirement must be satisfiable by an audiocpp `task`.** There is no
  `spk`/`sv` task in audio.cpp, so CAM++ (the old `core-engine/.../diarizer.py`) is
  **not reproducible** — speech separation comes from a `diar`-task model or from an
  ASR model that emits speaker labels itself.
- **There is no one-shot / "passthrough" path any more.** It existed for exactly
  one model (`moss_transcribe_diarize`) and was removed: that call decoded the
  whole file under a single `--max-tokens` budget, so long audio truncated with
  `MOSS-Transcribe-Diarize reached max_tokens before EOS` (measured: a 5-minute
  clip already failed, a 17-minute one needed 6928 tokens against a 6223 budget).
  Every model now runs per VAD chunk, which bounds the decode budget by the chunk
  length (≤15 s) instead of the file length — a 4-hour input is as safe as a
  30-second one. Speaker labels come from a `diar` model plus Rust turn
  attribution, never from the ASR model's own output, because per-chunk speaker
  ids are not comparable across chunks.
- **Diarization requires a non-Tongyi model.** Neither `sense_asr` nor `fun_asr_nano`
  exposes speaker output; FunASR's "ASR + VAD + CAM++" pipeline is a *Python*
  composition, not a checkpoint capability.
- **`status` is one three-state field, not two booleans.** `models.json` says
  `status: "supported" | "unverified" | "unsupported"` per entry, and the enum
  (`catalog::SupportStatus`) is the single answer to *how far do we stand behind
  this model?*:
  `supported` = run end to end here and known to work; `unverified` = offered but
  never exercised here; `unsupported` = deliberately not offered. The two booleans
  this replaced (`supported` / `verified`) answered overlapping halves of one
  question and could express a state that never existed (`supported: false` +
  `verified: true` = measured *and* never offered). An absent field reads as
  `unverified`: not `unsupported`, which would empty every plan, and not
  `supported`, which would claim 124 unrun entries are proven.
- **`unsupported` retires an entry without deleting it.** A retired entry stays in
  `models.json` so the survey still reads as one document and the measurement
  stays next to the model, but four places refuse it: `plan::solve` filters it
  out, `plan::apply_model_overrides` drops a pin to it (a `localStorage` override
  from before the retirement is a real path), `download_model` rejects it, and
  `commands::transcribe` / `monosub-cli` refuse a *frozen* plan that names one
  (`modelPlan.unsupportedModel` / exit 2) rather than running it. Its first use is
  `sortformer_diar` v1.
- **`unverified` is offered, just ranked lower.** It stays selectable and
  downloadable; it only loses `VERIFIED_BONUS` in the solver. `is_offered()` and
  `is_verified()` are the two questions callers actually ask, and the distinction
  matters at every gate: the downloader and the solver ask the first, the score
  asks the second. Filtering `unverified` out would hide most of the 129-entry
  catalogue behind a manual override.
- **The diarization default is `nemotron_3_diar`, not `sortformer_diar`.** v1's
  ceiling is architectural, not a budget: its encoder graph is sized from
  `tf_encoder.max_source_positions=1500` at 12.5 fps, so **120 s** is the most it
  can ever see in one call, and `session_len_sec` above that fails outright
  (`session_len_sec exceeds tf_encoder.max_source_positions`, measured). Long
  audio therefore only worked through `diar::diarize_long`'s window stitching,
  and any regression there silently dropped speaker labels past 120 s — which is
  the bug that motivated the switch. `nemotron_3_diar` has 8 speakers instead of
  4, no fixed position table (RoPE, so no ceiling), a permissive licence
  (OpenMDW-1.1 vs CC-BY-NC-4.0), and **no `session_len_sec` session option at
  all**. That last point is why `diar::session_args` gates the flag on the
  model's own GGUF spec (`engine/spec.rs::session_options`): `audiocpp_cli` fails
  the whole run on an undeclared session option rather than ignoring it.
  `sortformer_diar_v2` is **not** a candidate — audio.cpp ships no GGUF for it
  (redistribution approval pending), and NVIDIA's own
  `diar_streaming_sortformer_4spk-v2.q8_0.gguf` is a NeMo-Speech.cpp artifact
  (`general.architecture = sortformer`), which `audiocpp_cli` cannot load: it
  requires `general.architecture = audiocpp` plus `audiocpp.model_spec.json`.
- The `[Speaker A]: ` prefix in `subtitle/writer.rs` is fed by `RawChunk.speaker`;
  `subtitle/postprocess.rs` must not merge across differing speakers.
- **Speaker labels are decided per cue, not per VAD chunk.** `diar::attribute_turns`
  labels a whole chunk, and a chunk is a VAD window of up to
  `VAD_MAX_SECONDS` (15 s) — so a window that spans a speaker change gives both
  sides one label. Measured on a 4-hour two-person interview: 535 of 552 speaker
  changes did land on a chunk boundary, but **20 % of the cues sat in a chunk
  containing both a question and its answer**, which is how the host's
  「这对于其他模型公司很难吗？」 came out as the guest's `Speaker B`. So
  `diar::attribute_cues` re-checks every finished cue against the same turns,
  where a cue (one sentence, 1–3 s) makes the overlap decisive.
  `subtitle::postprocess` therefore takes the turns and applies them **after**
  the cues exist but **before** `merge_short` — merging is speaker-aware, so a cue
  still carrying the coarse chunk label would be fused with its neighbour first
  and the diarizer's boundary would be lost. A cue that overlaps no turn keeps
  the chunk-level answer (the diarizer drops turns under `MIN_TURN_SECONDS`, and
  a gap is not evidence of a new speaker). This is the design's "以 diar 为准",
  and it is why the turns travel out of the recognition block in
  `commands/transcribe.rs` instead of dying with the chunk labels.
  **This is now measured, not inferred** — `docs/eval/diarization-quality-eval.md`
  re-scored the same 4-hour interview before and after the change against an
  independent audio ground truth (frame f0, frame-level speaker composition,
  unsupervised ECAPA embeddings, and text-content markers): label accuracy
  **93.90 % → 99.85 %**, and the host's questions mislabelled as the guest
  **30.77 % → 0.67 %**. The gain is concentrated on the host (25.65 % → 0.40 %
  wrong) exactly as predicted, because he owns only 15.6 % of the audio and so
  loses every chunk that spans a question/answer boundary. The remaining 5 errors
  are not attribution bugs: one is a genuine mislabel, one follows from an
  estimated-timing boundary that overlaps the previous speaker's tail, one is a
  0.6 s filler, one is the documented "no overlap keeps the old label" choice
  (`None`), and one is a 4.7 s cue that genuinely holds both speakers. Note that evaluation doc also records a caveat this codebase must not
  forget — the **timeline** is unchanged by this fix, and on the estimated-timing
  path a cue's text and its time span can disagree (the motivating cue
  「这对于其他模型公司很难吗？」 now has the right *label* but its span still sits
  on the guest's audio). Label quality and timeline quality are separate axes.
- **The catalogue is all-or-nothing, so its enums are open.** `load_catalog` fails
  the whole file on one bad value, which would take every working model down. The
  survey introduced `speed: "slow"` (7–14B translation models), third-party
  `source` values, a `gen` role and Python engines, so: `Source` and `Role`/`Engine`
  carry `#[serde(other)]` catch-alls, `Speed` gained `Slow` (ranked above
  `Accurate` — different meaning), and `Engine::is_runnable()` gates downloads so a
  `diffusers`/`transformers` entry is listed but never offered. Tests pin both
  directions (`unknown_enum_values_degrade_instead_of_failing_the_catalogue`,
  `the_committed_catalogue_carries_slow_and_survey_vendors`).
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
| `sense_asr`, `fun_asr_nano` | none | **estimated**: the VAD chunk window is shared out in proportion to character count (`postprocess::distribute_times`). The chunk boundary is the only real anchor. |
| `moss_transcribe_diarize` | segment | **measured** where the model emits `--segments-out` (one `RawChunk` per segment, offsets shifted by the chunk start); estimation is the fallback when a chunk wrote none. |
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

**Three translation settings, two kinds.** `translateTargetLanguage` and
`translationModelId` are **configure-once** — which language and which model are properties
of the user's setup, so they live only in Settings and a run never asks again.
`settings.translateEnabled` is only the **default** a newly queued file starts from: the
switch that decides whether *this* run translates sits in the task area (the queue toolbar's
`BatchTranslationToggle`, and each row's own `file.translate`), the way speaker separation
does. A Settings-only switch made the app translate files the user had not asked about in
that moment — the user's words: "把这个开关作为默认值的设置，运行在任务中进行关闭或开启".
`TranscribeRequest.translate_enabled` therefore carries `file.translate`, not the setting;
URL import has no queue row to toggle before the job starts, so it still reads the default.

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
- **The count is per window, not per answer.** `run_batch` counts only the numbered lines
  whose index falls inside `[start+1, end]`. The prompt's own worked example
  (`1. First.` / `2. Second.`) is numbered too, and the model sometimes restates it before
  its real lines: for a window starting at 1 those two numbers collide with real cue numbers
  and collapse in the parse map, so the file's first batch always passed, while every later
  window saw them as two extra distinct indices. Measured on a 4884-cue file, a complete
  two-cue answer read as `expected 2 numbered cue(s), got 4` and was walked down the retry
  ladder to a single cue — and the single-cue rung rejects a source echo, which is what the
  model tends to return for exactly those short tail fragments. That chain, not the model
  alone, is why 302 cues shipped in the source language and the row said 部分未翻译.
  Counting only the window changes no output (an out-of-window index never maps to a cue);
  it only stops discarding complete answers. A batch answered in a foreign numbering still
  counts 0 and still retries.
- **"Partly untranslated" carries its own number.** `TranslationReport` also carries
  `translated` / `kept_original`, and the row's badge reads `未翻译 3/4884` with a tooltip
  explaining the shortfall. `degraded` is one boolean for both "one stray line" and "a third
  of the file", and the pipeline already computed the counts — hiding them made the state
  unanswerable at a glance.
- **`models.json` schema for a translate entry**: `roles: ["translate"]`,
  `engine: "llamacpp"` (absent means `audiocpp`, so every recognition entry is unchanged),
  and `prompt_template` naming the prompt wording (`hy_mt`, or `generic`/`chatml`/
  `gemma`/`cohere`/`llama3` for the survey models — an unknown name falls back to
  `generic`, so a typo degrades the prompt instead of failing the run).
  `hy_mt_1_8b_q4_k_m` is `tier: "default"`, which is what keeps it the catalogue
  default now that 39 other translate models exist; without that tier the picker
  would pick whichever `recommended` entry happened to come first. A translate
  entry must **not** list `"auto"` in `languages` — that is a source-detection
  value, and it failed the "every committed translate language has a display
  name" guard. `capabilities` is declared as
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

## URL Import (Bilibili)

Pasting a link is **not** a transcription job with a different input. It is a subtitle
lookup that may degrade into one, and that shape is why `src-tauri/src/site/` exists
beside (not inside) `engine/` and `translate/`. Design:
`docs/tech-spec/url-import-design.md`; task list: `docs/tech-spec/url-import-tasks.md`.

### The two paths

- **Subtitle hit.** `site/pipeline.rs::run` resolves the video, `site/lang.rs::select`
  picks exactly one track, and the SRT is written straight through `subtitle::write`. This
  path **never enters `start_transcription`**: that command takes an audio path, needs a
  model plan and reports `transcribe_*`, none of which a platform subtitle has. It reports
  `url_import_*` instead.
- **Audio fallback.** No usable track → `-f ba[ext=m4a]/ba/b` downloads the audio to
  `<app_cache_dir>/url-import/<id>/`, then the job ends with `Queued`. The frontend queues
  that path through the existing `fileImport` flow, so `extract_wav`, `get_video_metadata`,
  `generate_thumbnail` and `TranscribeRequest` need no change — the path always exists by
  the time it is queued. A previously downloaded file is reused rather than fetched again.
- **The decision path runs against a seam** (`site/deps.rs::Deps`: resolve / fetch /
  download / translate / progress; `RealDeps` shells out, `FakeDeps` records). Every
  branch — hit, fetch from the info dict, skip, translate, fall back, fail — is a unit test
  with no process and no network. New branching belongs in `pipeline.rs`; `mod.rs` stays
  commands, `IMPORT_TASKS`, the `Semaphore(3)` and the event payloads.
- **`parse_url_input` lives in Rust** (`site/url.rs`): that is the only way the
  split / normalise / de-duplicate / whitelist rules get tests, and `start_url_import`
  trusts its input. Tracking parameters are dropped, `p` is kept (it selects a part), and
  `/bangumi/`, `/cheese/`, `/space/`, `/favlist` are rejected as `unsupported_kind`.
- **The subtitle fetch re-serialises the info dict** instead of re-running Resolve, and the
  verdict is the file on disk: an unmatched `--sub-langs` exits **0** and writes nothing, so
  `RunOutput::success()` must never be used as the "did we get a subtitle?" test.

### The module split

`site/` is a peer of `engine/` and `translate/`, and the split is by testability:

| File | Owns |
|---|---|
| `mod.rs` | the four commands, `UrlImportRequest`, `IMPORT_TASKS`, `Semaphore(3)`, cancel + `Drop` cleanup, the three event payloads, audio download and its cache |
| `pipeline.rs` | the decision path (select → fetch → write → translate, else `Queued`), written against `Deps` |
| `deps.rs` | that seam: `RealDeps` (yt-dlp + events) and `FakeDeps` (tests) |
| `url.rs` | `parse_input`: split, normalise, de-duplicate, whitelist |
| `ytdlp.rs` | binary resolution, constant argv, spawn, per-line stdout, stderr ring buffer |
| `resolve.rs` | info dict → `SiteMedia`, multi-P verdict |
| `lang.rs` | `select` → at most one `Pick` |
| `srt.rs` | SRT text → `Vec<Segment>` |
| `naming.rs` | title sanitising and the output path |
| `progress.rs` | the `PROG` line parser |
| `error.rs` | `ImportError`, stderr classification, the login hint |

The parts that can silently deliver the wrong file — language choice (`lang`), the info-dict
parse (`resolve`), the timeline (`srt`), the file name (`naming`), the `PROG` line
(`progress`) — are pure functions with fixture tests rather than integration runs against a
real video. Only `ytdlp::run` and `resolve::run` spawn a process, and only `mod.rs`,
`deps.rs`, `resolve.rs` and `ytdlp.rs` mention `AppHandle` at all.

### yt-dlp: bundled, pinned, and what the invocation may not do

- **Bundled and pinned: 2026.08.19 + SHA256, no self-update.**
  `scripts/build/fetch-ytdlp.sh` downloads the official platform executable, verifies it and
  installs it as `resources/bin/yt-dlp-<target-triple>` (macOS: `yt-dlp_macos`, one
  Universal file serving both triples). Bilibili's extractor changes quickly, so a
  user-installed yt-dlp produces "the version is wrong" reports nobody can diagnose.
  `MONOSUB_YTDLP_PATH` overrides the path for debugging and hot-fixing only. Like every
  bundled binary it is signed by `sign-app.sh`, and it runs as a `kill_on_drop` child with
  `stdin` closed.
- **Constant argv, pinned by tests** (`site/ytdlp.rs::build_argv`): `--ignore-config` (a
  user's yt-dlp config can change the output template, format or subtitle options, and every
  judgement here is based on files on disk), `--no-playlist`, `--newline` (progress is read
  line by line), and `--ffmpeg-location <bundled>` (needed only for a merge, but it must
  never pick up a stray system ffmpeg). `--no-progress` is NOT part of that always-on set —
  see the measured trap below. **`--no-warnings` is never passed**: 412, "needs login" and
  region notices arrive as warnings, which is exactly what `site/error.rs` classifies. The
  URL is one argv element, never a shell string.
- **Measured facts from U0-9 that are load-bearing** (re-checked against the bundled
  2026.08.19 binary except where a bullet says otherwise):
  - **`--list-subs` is required.** yt-dlp extracts subtitles lazily; with `--print-json`
    alone Bilibili reports `subtitles: {}`. With it, stdout is a human-readable table
    followed by the info dict, so the parser takes the first line that is a JSON **object**
    with a string `id` — a table cell such as `123` is valid JSON on its own.
  - **`subtitles[].data` is inlined SRT.** Bilibili's extractor runs `json2srt` and puts the
    text in `data` with no `url`, so a hit costs one network call. (This is the design's
    U0-9 finding: the anonymous probe here returned only a `danmaku` track, so the inlined
    shape was not re-verified end to end.) `--load-info-json … --write-subs --sub-langs
    <escaped tag> --sub-format srt/best` is only the fallback for a track without `data`.
    `--convert-subs` is never used: Bilibili already serves SRT.
  - **`--progress-template` must not be combined with `--no-progress`.** Measured with the
    bundled 2026.08.19 binary: the pair emits **zero** `PROG` lines — the template is dropped
    silently rather than rejected, so the download bar would stay indeterminate forever. The
    flags are therefore split: `ALWAYS_ARGS` (`--ignore-config`, `--no-playlist`,
    `--newline`) plus `NO_PROGRESS_ARG`, which `build_argv` adds for `Resolve` and
    `FetchSubtitles` but **not** for `DownloadAudio`. Two tests pin the split
    (`the_audio_stage_never_passes_no_progress`, `the_quiet_stages_still_suppress_progress`),
    because the failure is silent: nothing errors, progress simply never arrives.
  - **An unmatched `--sub-langs` exits 0**, printing only "There are no subtitles for the
    requested languages" and writing no subtitle file. The verdict is therefore the
    non-empty `<id>.<tag>.srt` on disk — never the exit code, never the message.
  - **A URL without `?p=` silently yields part 1**: the id becomes `<BV>_p1` and the title
    gains ` p01 <part name>`, even with `--no-playlist`. Converting P1 and calling it the
    video is silently wrong and expanding to every part would create dozens of jobs, so
    `resolve::check_multipart` fails with `urlImport.multiPart` unless the URL carries `p=`.

### Selection, output, translation and UI

Language choice, naming and translation are the parts a user would notice going wrong, so each
rule below is a product promise rather than an implementation detail.

- **Language selection is strict** (`site/lang.rs`) because the wrong track delivers a file
  in the wrong language that the user cannot notice before reading it. `zh` and `zh-tw`
  **never** fall back to each other; `ai-zh` is accepted for `zh` only (it is Bilibili's own
  ASR of the original audio, while the other `ai-*` tracks are machine translations *of* it);
  human tracks outrank AI ones; `auto` selects nothing and goes straight to recognition
  ("let the engine decide" cannot answer which subtitle language was asked for); `danmaku`
  is XML bullet comments and is never a subtitle. `--sub-langs` is a regex, so the tag is
  escaped. The result is exactly one `Pick { tag, kind, source_lang }`, and `source_lang`
  is what makes the translation skip exact instead of script-guessed.
- **Login**: stderr is classified, and `HTTP Error 412` maps to the
  `URL_IMPORT_NEEDS_COOKIES` sentinel, which the row turns into a browser picker
  (Chrome / Edge / Firefox / Safari) that writes `settings.bilibiliCookiesBrowser` and
  retries; Settings → URL import holds the same three controls persistently. The softer
  "Subtitles are only available when logged in" **warning** is not an error: the run
  falls back to recognition as usual and only sets `login_hint: true`, so the row can say
  that logging in would have skipped recognition entirely. Matching stderr prose is the one
  exception to "never parse text", and it is unavoidable because yt-dlp is not localised;
  every matched string is a constant with a fixture test.
- **Naming**: `<sanitized title> [<id>].<ext>`. The id is what makes the name unique and
  idempotent — two videos cannot collide, re-importing the same URL produces the same path,
  and `skipExistingSubtitle` therefore behaves exactly as it does for local files. Titles
  are sanitised (`/ \ : * ? " < > |` and control characters → `_`, whitespace collapsed,
  leading/trailing dots and spaces trimmed, truncated to 120 **characters**); an empty
  result becomes `[<id>]`. Subtitles are written to `urlImportOutputDir`
  (`~/Downloads/MonoSub` by default, `customStoragePath` in `custom` mode) and **never** into
  the cache directory, which holds only intermediate audio under
  `<app_cache_dir>/url-import/<id>/`.
- **No post-processing and no `words.json`**: platform cues are already split by the
  uploader, so `site/srt.rs` only converts the format and keeps the timeline verbatim;
  re-splitting at 50 chars / 10 s would destroy the author's cue boundaries. A subtitle hit
  writes no word sidecar.
- **Translation is the same stage on both paths.** The hit path calls
  `translate::stage::translate_and_write` — the function `commands/transcribe.rs` also
  calls — with `Pick.source_lang` and the primary output path, so `<stem>.<lang>.<ext>`
  naming and "a translation failure is a warning, never a task failure" cannot drift.
  `translate::stage` also owns the pre-flight gate, which the import runs **before** Resolve
  so a missing model reports `MODEL_NOT_INSTALLED:<id>` without a network call.
- **The UI side**: `Toolbar.tsx` adds the entry (`data-testid="toolbar-import-url"`),
  `UrlImportDialog.tsx` is the paste panel, `UrlImportItem.tsx` renders one row above
  `FileList`, and `SettingsUrlImport.tsx` is the fifth Settings group (sidebar id
  `urlImport`, between `translate` and `system` — the sidebar button's test id is generated
  from the id, so it renders as `settings-category-urlImport`). Language and output directory
  are mirrored from Settings rather than editable in the dialog, because a second place to
  set them would be a second source of truth. The offline pilot scenario for this panel is
  `src-tauri/tests/pilot/url-import.toml` (22 steps, no network): it asserts per-link
  rejection, that the two rejection kinds use different wording, that the panel stays open,
  and that a rejected link creates no import row. The online cases live in
  `docs/tech-spec/url-import-verification.md`.
- **Scope**: only Bilibili is supported — `bilibili.com` (with or without `www.`/`m.`)
  `/video/BV…|av…`, a bare `BV…`/`av…` id, and `b23.tv` short links. The three video hosts
  are rewritten to `www.bilibili.com` before de-duplication, so the mobile and desktop links
  to one video collapse into one row; a `b23.tv` link is passed through untouched, because
  only the redirect knows which video it is. YouTube is deferred, not half-supported: it needs
  a bundled JS runtime, `-orig` track rules to avoid machine-translated captions, and proxy
  support (`docs/tech-spec/url-import-design.md` §10).

### Clipboard watch: one-click import

Copying a Bilibili link in a browser and switching to MonoSub should not require opening the
paste dialog. The app therefore polls the clipboard and, when it holds **exactly one**
supported video, shows a dismissible pill with one import button.

- **All the judgement is in Rust** (`site/clipboard.rs::classify`, unit-tested), because the
  frontend has no test runner and this rule has edges. The IPC command
  `read_clipboard_video` reads the clipboard (via `tauri-plugin-clipboard-manager`, the
  `clipboard-manager:allow-read-text` capability, and nothing else) and returns
  `Option<ClipboardHit { url, id, text }>`. It never writes to the clipboard.
- **High precision is the whole design.** A prompt that appears for the wrong reason teaches
  the user to dismiss it unread. So: zero or **two or more** URLs → no prompt (a batch belongs
  in the paste dialog); an unsupported site, `bangumi`/`cheese`, or a non-video page → no
  prompt; more than `MAX_CLIPBOARD_CHARS` (500) → no prompt, because a copied article that
  happens to mention a video is not a copied link.
- **It must poll while unfocused.** The user copies the link in their *browser*, so the app is
  in the background at the moment of the copy. The first implementation gated on
  `document.hasFocus()` and therefore never fired in the case the feature exists for —
  measured in the running app, then fixed to poll in both states (1.5 s focused, 2.5 s
  background, and immediately on focus or visibility change).
- **`id` is `Option`.** A `b23.tv` short link's path segment is an opaque code (`xyz789`), not
  a video id; presenting it as one is a lie the user cannot check until the import finishes.
  The pill shows the link itself in that case.
- **Dismissal is remembered per clipboard text**, so ignoring one link does not suppress the
  next, and the same ignored link does not return. A hit is also suppressed once its URL is
  already in `files[].sourceUrl` (queued) or in the `urlImports` slice (in flight).
- **The pill is suppressed while a modal is open** (the paste dialog or Settings), because it
  would sit on top of the panel.
- **It is a setting, on by default** (`settings.clipboardWatchEnabled`), because reading the
  clipboard is the kind of thing a user must be able to switch off.
- Like every other transfer, the pill hands its URL to `startUrlImport`, so an accepted prompt
  behaves exactly like a pasted link: same output directory, formats, translation settings and
  `url_import_*` events.

---

## IPC Communication Pattern

### Frontend → Backend (Tauri Commands)

| Command | Purpose |
|---------|---------|
| `start_transcription` | Begin transcription task |
| `cancel_transcription` | Cancel running task |
| `set_transcription_concurrency` | `capacity` (1–3); sizes the Rust semaphore from the user's 并发数 setting |
| `get_video_metadata` | Extract duration, resolution, size |
| `generate_thumbnail` | Create video preview |
| `resolve_paths` | Normalise dropped paths |
| `find_existing_path` | `paths → Option<String>`: the first subtitle already on disk (drives `skipExistingSubtitle`; deliberately not the fs plugin — see *Skipping existing subtitles*) |
| `open_in_finder` | Reveal a file / open a directory in the system file manager (`commands::opener`) |
| `open_file` | Open a written subtitle with its default app (`commands::opener::open_file`); what a finished row's subtitle name calls — *revealing* leaves the user one double-click short of reading it |
| `list_models` | Catalogue + per-model install state |
| `download_model` | Stream a model, verify size/SHA256, then rename |
| `cancel_model_download` | Abort a download and delete the `.part` |
| `remove_model` | Delete a model directory |
| `open_models_dir` | Create + **open** the models folder itself (not `-R` in its parent) |
| `activate_license` / `validate_license` / `deactivate_license` | Polar licensing |
| `check_github_release_update` | Update check |
| `update_tray_menu` | Localise the tray's Show / Hide / Quit labels (progress rows are preserved) |
| `update_tray_progress` | `progress: {title?, rows[]}`; the menu-bar text + the greyed progress rows a left click shows |
| `resolve_model_plan` | Solve `Requirements` → `ModelPlan`; drives the Settings preview |
| `parse_url_input` | `text → Vec<ParsedUrl>`: split, normalise, de-dupe, whitelist (pure Rust) |
| `start_url_import` | `request: UrlImportRequest`; registers the job and returns immediately |
| `cancel_url_import` | `task_id`; aborts the import stage only (not a queued file) |
| `read_clipboard_video` | — ; returns `Option<ClipboardHit{url,id,text}>` for a clipboard Bilibili link |
| `default_url_import_dir` | `~/Downloads/MonoSub`, resolved from Rust (Tauri knows the home dir) |
| `log_directory` | Absolute path of the rolling log directory (Settings ▸ 系统 shows it) |
| `open_logs_dir` | Create the log directory and open it in the system file manager |
| `log_frontend_error` | `level`, `message`, `context?`; writes a core-flow frontend failure into the same log file |

The catalogue contains both engines, but only audio.cpp entries can fill a recognition
plan; `translate` entries are grouped separately by `role` in the UI.

### Backend → Frontend (Tauri Events)

| Event | Payload | Purpose |
|-------|---------|---------|
| `transcribe_progress` | `{task_id, progress, overall?, status, step?, total_time?}` | Progress + step updates |
| `transcribe_complete` | `{task_id, total_time?, translation?}` | Finished; all formats written. `translation` = `{written[], skipped?, degraded, translated?, kept_original?}` and is present only when the run had a translation plan — it is how the row says "translated to English" or *why* no `<stem>.<lang>.<ext>` appeared (`skipped: sameLanguage` is the common, correct case), and the counts let a degraded row state the shortfall (`未翻译 3/4884`) instead of a bare "partly" |
| `transcribe_error` | `{task_id, status}` | Failed; `status` is user-showable |
| `model_download_progress` | `{id, downloaded, total}` | Throttled to ≥200 ms |
| `model_download_complete` | `{id}` | Model ready |
| `model_download_error` | `{id, message}` | Download failed |
| `url_import_progress` | `{task_id, stage, progress?}` | Parallel to `transcribe_progress`, but its own namespace |
| `url_import_complete` | `{task_id, outcome}` | One URL converged: subtitle written, or audio ready to queue |
| `url_import_error` | `{task_id, status}` | One URL failed; `status` is a sentinel, i18n key or message |

`step` is `{current_step, total_steps, step_name, phase?, step_elapsed, total_elapsed}`
with `step_name` from the frozen set `Extracting audio` / `Loading model` / `Transcribing`
/ `Writing subtitle` / `Translating`. `Translating` was **appended**; the first four keep
their exact spelling, because `FileItem.tsx` derives the i18n key as
`steps.<lowercase, spaces → _>` and renaming one would blank that step. `total_steps` is
**4 normally and 5 when the run translates**: `Steps::new(translating)` picks the
denominator per run, and the first step event already carries the right one.

**`phase` names what inside the recognition step is running** — `vad` / `asr` / `diar` —
because that step is the one that takes minutes and "转录中" alone cannot distinguish
"finding the speech" from "reading it" from "separating the speakers". It is absent on
every other step and on the word-aligned path. The engine's `ProgressFn` used to carry a
free-form `&str` message ("Detecting speech", "Segmenting audio"); the GUI needed to
*compare* it, and prose comparisons are exactly what this codebase refuses to do, so it
became `engine::ProgressPhase` — a closed enum whose `as_str()` is both the payload value
and the i18n key. The old message was never displayed anywhere anyway.

**Inside the recognition step the phase replaces the step name in the badge**, rather
than sitting beside it: "转录中 / 识别中" says 转录 twice, so the badge reads
`[icon] 识别中 · 42%`. Every other step keeps its own name, because no phase is running.
The icon changes with the step (`AudioLines` / `Cpu` / `FileText` / `Languages`), keyed by
the frozen step name so a rename shows the spinner instead of the wrong icon.

**`GlassBadge` must forward `size` into the variant call.** It spread the prop onto the
`div` instead, where `size="sm"` is a valid HTML attribute with no styling effect — so
every badge in the app silently rendered at `md` (`px-3 py-1`, `text-sm`) and each
caller's `size="sm"` was a no-op. That, not the padding, was most of the file row's
height. `GlassButton` already forwarded it correctly; the two components had drifted.

**"Not working" has three distinct states, and the row must not conflate them.**
`pending` + `isBatchProcessing` means "behind the concurrency limit, will start on
its own" (badge 排队中); `pending` alone means "waiting for you to press 开始" (等待处理);
and `processing` + `waiting: true` means "dispatched, parked in the backend's
semaphore queue" (等待空位, no bar, no spinner). The third state is new with the
concurrency setting: `ProgressSink::waiting` fires *before* `transcribe_permit()`
awaits, and `TranscribeProgress.waiting` is a flag rather than a fourth `status`
value so the UI compares an identifier instead of prose. The tray averages only
admitted files (`buildTitle` skips `waiting`), because folding a parked 0 % into
the average makes one running task look half-speed. Without this, a queued file
showed "启动中…" with a spinner — claiming work that had not begun — which is the
normal state for every file after the first at the default concurrency of 1.

**Two percentages, and only one is shown.** `progress` is the fraction *within the
current step* and restarts at 0 whenever a step begins; `overall` is the run-wide figure
and is what `FileItem`'s bar, its badge, the tray title and the CLI's `percent` all read.
Before `overall` existed the frontend mapped `progress` onto `file.progress`, so the bar
and the menu-bar percentage fell back to 0 % at every step change. `overall` is absent on
`queued` / `error` events, where there is no position in the run to report.

The per-step figure is carried on the event but **not rendered**: an earlier cut printed
it inside the badge next to the run-wide percentage, which was two numbers for one
question (the user's words: "这个数字去掉和上面重复了"). The row now states the number
once, in the badge, and the bar draws the same value.

**The steps are not equal shares of the bar.** `overall` sums a weight table
(`STEP_WEIGHTS_WITHOUT_TRANSLATION` = 0.02 / 0.03 / 0.85 / 0.10; the translating variant
splits the last two into 0.10 writing + 0.15 translating and drops transcription to 0.70).
Equal weights made extracting a WAV and starting the model worth 25 % each, so the bar
jumped to 50 % before a single word was recognised and then sat still. The weights are a
*rendering* choice — the bar still reaches 100 % exactly when the last step does — so a
machine that extracts unusually slowly is slow in the bar, not wrong about it.

**The ASR stage reports progress in audio seconds, not chunk counts.**
`ProgressFn`'s `(done, total)` are weighted units summed from each VAD chunk's sample
length, so a 15 s sentence advances the bar 30× as far as a 0.5 s interjection. Counting
chunks made the percentage describe the bookkeeping — a 1 % step for "嗯", then seconds
of stillness on a long utterance — and the VAD segmentation already carries the lengths,
so weighting them costs nothing.

`TranscribeRequest` carries three translation fields, all `Option` so an older frontend
that sends none of them translates nothing: `translate_enabled`, `target_language`, and
`translation_model` (the UI sends `''`/absent for "catalogue default").

**URL import is a separate namespace.** The `url_import_*` events are constructed and
emitted only in `site/`, and the four URL-import commands (`parse_url_input`,
`start_url_import`, `cancel_url_import`, `default_url_import_dir`, `read_clipboard_video`)
live there too;
`transcribe_*`, `TranscribeRequest` and `STEP_NAMES` are untouched by them (design §5.9
forbids adding a URL stage to `STEP_NAMES` — a platform subtitle has no engine step to
report). The payload shapes are the ones `site/mod.rs` serialises:

```ts
// url_import_progress — `stage` is a frozen set, snake_case
{ task_id, stage: 'resolving' | 'fetching_subtitles' | 'downloading_audio'
              | 'writing' | 'translating',
  progress?: number }   // 0…100; the key is OMITTED (not null) when indeterminate

// url_import_complete — `Outcome` is an internally tagged enum, tag name `outcome`
{ task_id, outcome:
  | { outcome: 'subtitle', paths: string[], translated_paths: string[], tag: string,
      kind: 'human' | 'ai', source_lang: string, skipped: boolean }
  | { outcome: 'queued', path: string, title: string, name: string,
      duration: number | null, login_hint: boolean } }

// url_import_error
{ task_id, status: string }
```

Two naming traps, both pinned by a serialisation test in `site/mod.rs`: the enum tag is
`outcome` (not `kind`), and `kind` is the **`SubKind` field** of the subtitle arm — so the two
coexist in one payload (`{outcome: "subtitle", kind: "human"}`). `progress` is omitted rather
than serialised as `null` when absent.

`status` is a sentinel the UI routes on (`URL_IMPORT_NEEDS_COOKIES`,
`MODEL_NOT_INSTALLED:<id>`), an i18n key (`urlImport.multiPart`), or `key:detail`. A
`Queued` outcome is not a transcription task: the frontend queues `path` through the
existing `fileImport` flow, so the follow-up run reports `transcribe_*` as usual.

There is no stdout protocol any more — the engine is called directly, not via a
line-parsing subprocess.

---

## 命令行入口（`monosub-cli`）

> **Status: implemented.** Design: `docs/tech-spec/cli-design.md`; §11.7 records the measured
> real transcription. The binary is `src-tauri/src/bin/monosub-cli.rs`.

`monosub-cli` is the second frontend over the same pipeline. It shares the whole crate with
the GUI rather than re-deriving it: `src-tauri/src/lib.rs` declares the 9 `pub mod`s,
`src/main.rs` is the thin Tauri shell over that library, and `Cargo.toml` carries
`[[bin]] monosub` / `[[bin]] monosub-cli` / `[lib] monosub`. It calls the same
`commands::transcribe::transcribe_file` the GUI calls, with a `ProgressSink` that writes to
stderr instead of emitting `transcribe_progress`. **It re-implements nothing**, so "the GUI's
output is good but the CLI's differs" cannot become a bug nobody can chase.

That guarantee is what keeps `engine/` / `subtitle/` / `translate/` free of `tauri`: they
take `&AppPaths` (`from_app` for the GUI, `from_env` for the CLI and tests), which is also why
they run under `cargo test`.

```bash
monosub-cli video.mp4
monosub-cli video.mp4 --output-dir ~/subs --format srt,vtt
monosub-cli video.mp4 --translate --target en
monosub-cli *.mp4 --json
```

Rules the CLI is built around:

- **No model flag.** The four questions are answered in the GUI: `resolve_model_plan` writes
  the solved `recognitionPlan` into `<app_config_dir>/settings.json`, and the CLI reads that
  instead of asking again. `--model` is deliberately not offered — pinning a model is the
  GUI's "advanced" action, and a second place to choose one is a second source of truth. The
  CLI still re-runs `plan::solve` over those cached answers and **trusts the solver**, warning
  on stderr when the two disagree (the user may have deleted a model since).
- **What it controls** is `-o/--output-dir` / `-f/--format` / `-l/--language` /
  `--translate` / `--target` / `--json` / `-q/--quiet` / `-h/--help`. Those are the
  per-invocation choices; the model comes from the settings the GUI wrote. Multiple inputs are
  processed **in order**: one failing file does not stop the rest, and the process exits with
  the **first** failure's code once all are done.
- **Translation is off by default**, even when `settings.json` carries
  `translation.enabled: true` — the GUI's switch is deliberately not consulted, because it is
  an extra 1.8B model load a batch script should opt into per run. The gate is
  `translate::stage::resolve`, the same function the GUI and URL import call. Two wiring
  quirks to know about: the CLI passes `model_id: None`, so it always takes the catalogue
  default and never reads `settings.json`'s `translation.modelId`; and `--target` has no
  default in code, so `--translate` **without** `--target` resolves to
  `SkipReason::NoTarget` and only prints "continuing without translation". **Pass
  `--translate --target <lang>`** to get a translated file.
- **Two streams, two audiences.** Human progress goes to **stderr**; `--json` (JSON Lines,
  `queued` / `progress` / `done`) goes to **stdout**, so `… --json | jq` works and a script
  never has to grep the wording a screenshot shows. The JSON field set is **experimental**
  (may change in a minor version); the stable contract is the **exit code** plus the **output
  file paths** the run reports.
- **No inter-process lock.** Running the CLI and the app at once makes both share one GPU,
  which on Metal slows both down. That is documented, not locked (the app's semaphore is
  process-local).

The step denominator follows the run: **4** steps normally, **5** when it translates — both
measured, and the same `STEPS_WITHOUT_TRANSLATION` / `STEPS_WITH_TRANSLATION` constants the
GUI renders.

| Exit code | Meaning |
|---|---|
| 0 | every input succeeded — an all-silent file is success and writes an empty subtitle, with or without `--translate` |
| 1 | usage error: bad argument, input file does not exist |
| 2 | model missing (`MODEL_NOT_INSTALLED`, with or without an `:<id>` suffix); stderr names it and points at the App to install |
| 3 | transcription failed — engine or ffmpeg non-zero exit |
| 4 | the output could not be written: the CLI's own `create_dir_all`, or a `TaskFailure` whose context is `writing the subtitle failed` |
| 5 | the subtitles are valid but the translation was **attempted and produced nothing**; stderr adds "the subtitles are valid; only the translated copy failed." |

Exit 5 is strictly "attempted and produced nothing". A **skip** is not a failure and stays exit
0: `SkipReason::NoSegments` (an all-silent input) and `SkipReason::SameLanguage` (a detected
source equal to the target) both legitimately write no translated file. `StageOutcome` carries
`skipped: Option<SkipReason>` so the call site can tell the two apart — before that field
existed, a silent WAV with `--translate` reported exit 5 (measured), which was wrong.

`-f/--format` defaults to `srt` and `-l/--language` to the persisted plan's language, else
`auto`. Note `--help` claims both default to "the app setting": that is only true of the
language. Output *format* is a browser-side preference and is not in `settings.json`, so the
CLI falls back to `srt` rather than guessing.

§11.7 holds the measured evidence for the recognition path (one real transcription, plus the
exit-1 and exit-2 paths). Translation was measured separately: a 2.2 s Chinese clip
(`say -v Tingting`) with `--translate --target en` wrote `speech.srt` + `speech.en.srt`
("大家好，我是一个测试。" → "Hello everyone, I'm a tester.") and exited 0, with the step
denominator at 5.

**The config directory and the models directory are two unrelated concepts.** Neither
implies nor is derived from the other, and the CLI must not invent a second rule for
either:

| Concept | What it is | Default | Override |
|---|---|---|---|
| config dir | where `settings.json` lives, and nothing else | `<app_config_dir>`: macOS `~/Library/Application Support/dev.leix.monosub`, Windows `%APPDATA%/dev.leix.monosub`, Linux `~/.config/dev.leix.monosub` | `MONOSUB_CONFIG_DIR` |
| models dir | where model weights live | `<app_data_dir>/models` | `MONOSUB_MODELS_DIR`, or the `modelsDir` string stored *inside* that `settings.json` |

The installed app already looks like this: `settings.json` sits under the config dir while
its `modelsDir` points somewhere else entirely (here, into the repo). So "the models are
next to the config file" is wrong, and hard-coding `~/.monosub` would be a second settings
location; `MONOSUB_CONFIG_DIR=~/.monosub` is the supported way to get that literal path.

---

## Key Files Reference

| Path | Purpose |
|------|---------|
| `src/App.tsx` | Event listeners for progress/completion/error |
| `src/store/useStore.ts` | Zustand state, `persist` v6 + migrate (v6 maps the retired `speedPreference` onto `memoryBudget`); `urlImports` is not persisted |
| `src/lib/models.ts` | Model commands/events, `MODEL_NOT_INSTALLED_ERROR` |
| `src/components/Settings.tsx` | Settings shell + its five sidebar groups: `files` / `models` / `translate` / `urlImport` / `system` |
| `src/components/SettingsModelPlan.tsx` | The plan's four questions (语种场景 / 说话人分离 / 词级时间戳 / 显存占用) + live preview + manual overrides; publishes `data-plan-*` attributes for the pilot scenario |
| `src/components/CustomSelect.tsx` | The app's only dropdown. `searchable` adds a filter box and is on for the **model** pickers (82 ASR / 40 translate entries), off for the short lists. Keeps the empty-value option (`Auto`) only when something else matches, so a no-hit search shows "no matching models" instead of a lone "Auto" |
| `src/components/SettingsModelsDir.tsx` | The models storage-location card: path, change / reset, and the **only** "open models folder" button (`open_models_dir`) |
| `src/components/SettingsTranslation.tsx` | Translation **default** switch, target language, translation-model download; the target and the model are configure-once |
| `src/components/BatchTranslationToggle.tsx` | Queue-toolbar translation switch: applies one value to every pending file, the way `BatchDiarizationToggle` does for speaker separation |
| `src/components/BatchDiarizationToggle.tsx` | Queue-toolbar speaker-separation switch (all pending files at once) |
| `src/components/SettingsUrlImport.tsx` | The `urlImport` Settings group (output dir, Bilibili cookie browser, prefer-local) |
| `src/components/UrlImportDialog.tsx` | "From a link" panel: paste, per-row errors, language/output dir mirrored from Settings |
| `src/components/UrlImportItem.tsx` | One import row rendered above `FileList`; never a member of `files` |
| `src/lib/urlImport.ts` | `parse_url_input` / `start_url_import` wrappers + the `url_import_*` listeners |
| `src-tauri/src/engine/mod.rs` | The transcription pipeline (dispatches on the plan) |
| `src-tauri/src/engine/plan.rs` | Requirement → ModelPlan solver |
| `src-tauri/src/engine/spec.rs` | Reads `audiocpp.model_spec.json` out of a GGUF to learn which `--request-option`s a family accepts (3 layers: GGUF → measured negative list → safe default) |
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
| `src-tauri/src/translate/stage.rs` | Tauri-free translation stage: `resolve`/`will_translate` gate, `translate_and_write`, `<stem>.<lang>.<ext>` naming — shared by transcription and URL import |
| `src-tauri/src/site/mod.rs` | URL-import commands, `IMPORT_TASKS`, `Semaphore(3)`, `url_import_*` payloads |
| `src-tauri/src/site/pipeline.rs` | Import decision path (select → fetch → write → translate, else fall back) over `site/deps.rs` |
| `src-tauri/src/site/ytdlp.rs` | yt-dlp resolution, constant argv, spawn + line reading, stderr ring buffer |
| `src-tauri/src/site/url.rs` | `parse_input`: split, normalise (`p` kept), de-duplicate, Bilibili whitelist |
| `src-tauri/src/site/lang.rs` | `select`: strict language → one `Pick { tag, kind, source_lang }` |
| `src-tauri/src/subtitle/writer.rs` | Format writers |
| `src-tauri/resources/models.json` | Model catalogue (v2 schema, incl. `engine` / `translate` role; 129 entries from the 模型调查 survey — 89 recognition across 31 families + 40 translation models) |
| `docs/tech-spec/model-selection-design.md` | Requirement → ModelPlan design |
| `docs/tech-spec/translation-design.md` | Translation design |
| `docs/tech-spec/translation-poc.md` | Measured llama.cpp / HY-MT behaviour (pins, speed, format) |
| `src-tauri/tests/fixtures/` | Subtitle goldens (+ their generator) |
| `scripts/build/fetch-audiocpp.sh` | Pinned upstream binary + VAD download |
| `scripts/build/fetch-llamacpp.sh` | Pinned llama.cpp `llama-server` + dylib download |
| `scripts/build/fetch-ytdlp.sh` | Pinned yt-dlp download (+ its LICENSE / THIRD_PARTY_LICENSES.txt) |
| `src-tauri/src/site/clipboard.rs` | `classify`: clipboard text → one `ClipboardHit`, or nothing (pure) |
| `src/lib/clipboardWatch.ts` | Clipboard polling hook: focus-aware cadence, per-text dismissal |
| `src/lib/tray.ts` | Menu-bar mirror: `buildStatusRows` / `buildTitle` from `files`, coalesced pushes |
| `src-tauri/src/commands/logs.rs` | `log_directory` / `open_logs_dir` / `log_frontend_error` |
| `src-tauri/src/commands/opener.rs` | The **only** way to open a path: `open_directory` (`open <dir>`) vs `reveal_in_file_manager` (file ⇒ `open -R`), awaited + exit-status checked |
| `src/lib/logging.ts` | `reportFrontendError` (5 s per-message dedupe) + the `window.onerror` / `unhandledrejection` hooks |
| `src-tauri/src/commands/tray.rs` | Menu-bar state + `layout`: status rows, separator, localised actions |
| `src-tauri/src/tray.rs` | Builds the icon; `show_menu_on_left_click(true)` |
| `src/components/ClipboardPrompt.tsx` | The one-click import pill |
| `docs/tech-spec/url-import-design.md` | URL import design (flow, language rules, IPC, packaging) |
| `docs/tech-spec/url-import-tasks.md` | URL import task list (U0–U5) |
| `docs/tech-spec/cli-design.md` | `monosub-cli` design (implemented, §11.7): library split, `AppPaths`, settings hand-off, exit codes |
| `src-tauri/src/lib.rs` | The shared crate: 9 `pub mod`s, so `main.rs` and `bin/monosub-cli.rs` both `use monosub::…` |
| `src-tauri/src/bin/monosub-cli.rs` | The CLI: hand-rolled argument parsing, `--json` JSON Lines, multi-file loop over `transcribe_file` |

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
settings: AppSettings           // User preferences (persisted)
modelOverrides: Partial<Record<'asr' | 'vad' | 'align' | 'diar', string>>  // persisted
storageMode: 'same' | 'custom'  // Output location (persisted)
customStoragePath: string       // Custom output directory (persisted)
isProcessing: boolean           // Global processing state
currentTaskId: string | null    // Active task
urlImports: Record<string, UrlImportState>  // In-flight URL imports (NOT persisted)
autoStartImports: boolean       // Dialog checkbox; auto-start is pure frontend (NOT persisted)
```

Persisted to `localStorage` as `monosub-storage` at `version: 7`. The `migrate` function
**drops** `enableNoiseReduction`, `enableVolumeNormalization`, `enableBilingual` and
`bilingualTargetLang`: those
settings never reached the engine in the old code either (the Python engine hardcoded
the filters), so they were dead. The equivalent ffmpeg filters are now always applied.

The v3–v7 steps **add** fields and must not drop anything; v6 **replaces** one
field and migrates its value instead of discarding it:

```ts
// settings (added in v3)
requireWordTimestamps: boolean            // false
// top level (added in v3)
modelOverrides: Partial<Record<'asr' | 'vad' | 'align' | 'diar', string>>

// settings (added in v4; translation)
translateEnabled: boolean                 // false — the DEFAULT for a new file, not the per-run switch
translateTargetLanguage: TranslateTargetLanguage // 'en' (never 'auto') — configure once
translationModelId: string                // '' means "catalogue default" — configure once

// settings (added in v5; URL import)
urlImportOutputDir: string                // '' means ~/Downloads/MonoSub
bilibiliCookiesBrowser: CookiesBrowser    // '' = not configured → no cookie flag at all
preferLocalRecognition: boolean           // false: take the platform subtitle when offered

// settings (v6; the recognition plan's four questions)
languageProfile: 'mandarin' | 'english' | 'mixed' | 'multilingual'  // 'mandarin'
memoryBudget: 'small' | 'medium' | 'large'              // 'medium'
// `speedPreference` was REMOVED in v6: `memoryBudgetFromSpeed` maps
// fast→small, accurate→large, balanced→medium, so a v5 blob keeps its intent.

// settings (v7; concurrency)
concurrency: 1 | 2 | 3                    // 1 — see the two-consumers note below
```

**`settings.concurrency` has two consumers and they must agree.** The frontend queue uses
it as the number of tasks it dispatches (`processQueueBatch`), and the Rust semaphore is
sized from it through the `set_transcription_concurrency` command. Before the setting
existed the queue hard-coded 3 while `TRANSCRIBE_SEMAPHORE` admitted **1**, so two of the
three "running" rows sat at 0 % inside the backend — which is also why the per-row start
button had been disabled on the global `isProcessing`: nothing could have run anyway.
A new start is queued, not refused: the app is built to accept it and wait for a slot
(`开始任务之后自动排队就好了`).

Resizing replaces the semaphore rather than mutating it (`tokio`'s capacity is fixed at
creation). Stages already running hold permits from the old one, so they finish
undisturbed; only newly spawned tasks see the new figure. That is what a user expects from
lowering a limit: "no more than N from now on", not "kill what is running".

**Run mode is deliberately *not* in this store.** It is per-model and must reach the CLI,
which is a separate process with no WebView, so it lives in `settings.json`
(`modelModes`) alongside the frozen plan. A `localStorage` copy would be a second source of
truth the CLI cannot read — the same reason `recognitionPlan` is persisted by Rust. The
Settings row only *reads* it (`effective_mode` from `list_models`) and holds no state of
its own, because there is nothing to set: the app streams wherever it can.

`urlImports` is the one deliberately **non-persisted** slice: an import is an in-flight
network job, so a reload must start with nothing rather than with rows whose task ids no
longer exist. Its rows feed `UrlImportItem` and never `files`, so an import in progress
is not counted by "start all" or by the batch counters.

`settings.defaultDiarization` keeps the meaning the old `enableSpeakerDiarization` had; it
now maps to `Requirements.diarization` and seeds each newly queued file's own toggle instead
of being a disabled placeholder. `TranslateTargetLanguage`
is `Exclude<OutputLanguage, 'auto'>`: `auto` is a *source*-detection value and "translate
into auto" has no meaning, so the Rust side skips such a run rather than guessing.

---

## Important Implementation Details

### Binary Path Resolution

Bundled binaries (`ffmpeg`, `ffprobe`, `audiocpp_cli`, `llama-server`, `yt-dlp`) are
resolved in this order by the single `engine/cli.rs::resolve_bundled_binary`
(`pub(crate)`, shared by every bundled binary so the rules cannot drift):

1. `MONOSUB_FFMPEG_PATH` / `MONOSUB_FFPROBE_PATH` / `MONOSUB_AUDIOCPP_PATH` /
   `MONOSUB_LLAMACPP_PATH` / `MONOSUB_YTDLP_PATH` override
2. Dev checkout: `src-tauri/resources/bin/`
3. Next to the executable
4. `<Contents>/Resources/resources/bin/` (app bundle)
5. System paths / `PATH` (ffmpeg only)

Names use the `<name>-<target-triple>` convention, e.g.
`audiocpp_cli-aarch64-apple-darwin`, `llama-server-aarch64-apple-darwin`, and
`yt-dlp-aarch64-apple-darwin`. `site/ytdlp.rs::resolve_path` only supplies the base name
and env var to that shared resolver — it deliberately does not re-implement the chain.

`llama-server`'s libraries must sit **flat in `resources/bin/`**, next to the executable —
its only `LC_RPATH` is `@loader_path`. ffmpeg's dylibs live in `bin/lib/` and are resolved
by ffmpeg's own rpath; the two layouts must not be merged.

### Opening a path in the file manager

`commands/opener.rs` is the **only** implementation, and there are three entry points because
there are three user intents:

- `open_directory(dir)` — "show me what is in here" → macOS `open <dir>`.
- `reveal_in_file_manager(path)` — "show me this thing" → a file is `open -R <file>`, but a
  **directory is opened**, not revealed.
- `open_file(path)` — "open this file" → `open <file>`, i.e. the default app. This is what a
  finished row's subtitle name calls: the user asked to read the subtitle, and revealing it
  in Finder leaves them one double-click short. Windows needs `cmd /C start "" <path>` — the
  empty title is required, because `start` treats its first quoted argument as the window
  title and would otherwise eat the path.

`-R` means "select it in the parent window" (`man open`: *Reveals the file(s) in the Finder
instead of opening them*), so applying it to a directory shows the **parent** folder —
measured: `open -R <models>` lands on `<repo>/` with `models` selected, while `open <models>`
makes `models/` the window's target. That difference is why the "打开模型目录" button looked
dead. Two further defects were stacked on top: the old code used `.spawn()` (no wait, stderr
discarded), so the failure was silent, and the frontend called `void openModelsDir()` without
`await`/`catch`, so the `Err` was dropped.

Rules that follow:

- Every child is `await`ed with `.output()` and a non-zero exit becomes a readable `Err`
  (`<cmd> <args> failed (exit N): <stderr>`). Never `spawn()`-and-forget.
- Paths go in as their own `argv` element. No shell string, so a folder named `模型 目录`
  needs no escaping and cannot be split.
- A path that does not exist is classified as a **file**, so the tool itself reports
  `The file … does not exist.` instead of this module guessing a reason.
- `explorer.exe` exits 1 even on success, so on Windows the exit status cannot decide
  success (`status_decides_success()`); only a launch failure is reported there.
- Callers must surface the error. `SettingsModelsDir.tsx` shows it in the card and logs it
  through `reportFrontendError`; a silent `void openX()` is what made this bug invisible.

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

The original files are derived by `subtitle::output_path_for` (moved out of
`commands/transcribe.rs` so the URL import's per-format loop cannot re-implement the rule)
and are **never touched by translation**: the first format writes to `output_path` verbatim,
later ones replace only the extension. Translated files are additive and go through a
separate function `translation_output_path`, producing `<stem>.<lang>.<ext>` next to the
original (`video.srt` → `video.en.srt`). The `<lang>` is `language::file_tag`: canonicalised
(`zh-tw` → `zh-Hant`) and reduced to `[A-Za-z0-9-]`, so it can never contain a path
separator or a second dot. `derive_output_paths` returns originals first and translated
paths second, which is what makes a cancelled run clean up both.

URL imports have their own stem (`site::naming`, `<title> [<id>]`) but the same per-format
extension swap.

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
- `--session-option {family}.vad_model_path=<dir>` must be passed explicitly, or the
  CLI falls back to its own relative `assets/…` path — **but only to a model that
  declares the option**. `spec::sends_vad_model_path` reads the model's own GGUF first
  and falls back to a family list (`sense_asr` only); a family-level table was wrong:
  measured on the shipped Fun-ASR-Nano 2512 GGUF, whose spec declares exactly one
  session option (`weight_type`), sending `fun_asr_nano.vad_model_path` failed the
  whole run with `unknown Fun-ASR-Nano session option`.

### Which `--request-option`s a model accepts

`audiocpp_cli` validates every per-request option against the selected model's
family spec and **fails the whole run** on an unknown one:

```text
audiocpp_cli failed: unknown MOSS-Transcribe-Diarize request option: language
```

`engine/mod.rs::write_requests` used to write `language` and `audio_chunk_mode`
into every request unconditionally. That only works for families that declare
both — 7 of the 30 ASR families declare neither, so selecting any of them failed
immediately. `engine/spec.rs` resolves the answer in three layers:

1. **The GGUF itself.** Every model carries its family spec in the
   `audiocpp.model_spec.json` metadata KV, so the answer travels with the file
   and a model downloaded next year needs no code change. The CLI cannot be asked:
   `--list-loaders --json` reports only `api_endpoints` / `instructions_policy` /
   `tasks`, and `--inspect --model` reports task/mode/language/config but no
   options.
2. **A measured negative list**, because not every GGUF embeds a *full* spec.
   `qwen3_asr` and `qwen3_forced_aligner` ship two keys (`family`, `sources`) —
   yet `qwen3_asr` really does accept `language`. A minimal spec must therefore
   fall through, never be read as "accepts nothing".
3. **A safe default**: an unknown family keeps the historic behaviour (send the
   options), so a future family degrades to what it does today instead of
   silently losing its language hint.

A negative list rather than a positive one is deliberate — the default for an
unrecognised family is "send", which is what every model got before.

`--segments-out` needs no such table: it is a global CLI flag. A model without
segment support accepts it and simply writes no file (measured with `sense_asr`),
and the read side treats a missing file as "fall back to estimated timing".

### Model status: supported / unverified / unsupported

`models.json` carries one `status` per entry, orthogonal to `tier`: `tier` says how
much a model is *recommended* within the offered set, `status` says whether the
model is offered at all and whether anyone has run it. A model that loads is not
necessarily a model that works — a GGUF can declare a family and still fail on the
CLI. (This replaced the `verified` / `supported` boolean pair; see the model-plan
rules above for why one axis is the honest shape.)

| status | solver | download | UI badge |
|---|---|---|---|
| `supported` | plans it, +`VERIFIED_BONUS` | yes | `models.supported` (success) |
| `unverified` | plans it, no bonus | yes | `models.unverified` (neutral) |
| `unsupported` | never plans it | refused | `models.unsupported` (destructive) |

All three are labelled, including `supported`. Leaving the good state blank made
an absent badge ambiguous — it could mean "verified and fine" or "the field did
not arrive", and the second is a real case: an older backend sends nothing, which
the frontend reads as `unverified`. A badge on every row removes that ambiguity
at the cost of one word.

Unverified models stay **selectable**. They lose `VERIFIED_BONUS` (45) in the
solver's score, which is larger than `INSTALLED_BONUS` (30) but smaller than
`RECOMMENDED_TIER_BONUS` (100) — so a verified model that still needs a download
beats an unverified one already on disk, while 显存预算 and language fit still
outrank verification. The UI labels them instead of hiding them; filtering them
out would hide most of the catalogue behind a manual override.

The Settings row shows exactly one status badge — always exactly one, never
zero — and a retired entry has no download button because the backend would
refuse the request. The manual model
pickers in `SettingsModelPlan` filter `unsupported` out entirely and clear a pin
that points at one, so a stale `localStorage` override cannot freeze a plan the
run then rejects.

### Run mode: streaming wherever it is possible

`models.json` carries a `modes` array per entry (`offline` / `streaming`), taken from
the family spec embedded in the GGUF (`audiocpp.model_spec.json` → `modes`) and
cross-checked against `audiocpp_cli --list-loaders --json`. Absent means offline only,
which keeps every pre-existing entry on exactly the argv it had.

**A declaration is not an entitlement to be used.** Streaming runs only where the
pipeline can actually drive it, and that is a strictly smaller set:

- **Recognition cannot stream in this CLI version.** The ASR path is
  `--request-sequence`, which `audiocpp_cli` treats as a *batch input* and rejects in
  any mode but offline (`app/cli/main.cpp`: `batch inputs require offline mode`). So
  `sense_asr`, `qwen3_asr` and `moss_transcribe_diarize` all declare `streaming` and
  none of them streams here. `cli::build_argv` enforces this in code
  (`reject_streaming_with_batch_input`), so it is not a comment someone can forget.
- **Diarization is the one stage that can stream.** It is a single whole-file call, and
  `nemotron_3_diar`'s `finalize()` returns the complete `speaker_turns`, so
  `--turns-out` has the same shape in both modes.

`ModelSpec::streaming_selectable()` is the single predicate (`declares streaming` **and**
`has role diar` **and** `engine == audiocpp`), and `ModelSpec::default_mode()` derives
from it. The pipeline picks `--mode` from it, so a model can never be run in a mode the
predicate denies.

**There is no user-facing switch, and this is deliberate.** Streaming is what the app
does wherever it can, so a control would be asking a question the app has already
answered — the GUI is not where a user should have to learn that offline diarization
renumbers speakers per window. `list_models` therefore returns exactly one mode field,
`effective_mode`, and `Settings.tsx` renders it as a badge (only for streaming; the
absence of a badge says offline). It is a statement of what will run, not an input.

Streaming is the default wherever it is available for two reasons — and the second one is
the one that matters:

1. a local-first app runs beside whatever else the machine is doing, so the lower peak
   footprint is worth having;
2. **speaker identity is a correctness issue, not a memory one.** The offline path splits
   audio over `max_session_seconds` into overlapping windows; each window numbers its
   speakers from zero and `chain_labels` joins them by an overlap vote. Streaming never
   restarts the session, so its ids are consistent by construction. A streaming session
   therefore always takes the single-call path, even when `max_session_seconds` is set —
   windowing it would reintroduce exactly the renumbering streaming avoids.

**An override exists as a data escape hatch, not as UI.** `<app_config_dir>/settings.json`
accepts `modelModes: { "<id>": "offline" | "streaming" }` — one entry per model, because
whether a session can be fed in chunks is a property of the checkpoint.
`model_dir::chosen_mode` is the only resolver: no entry means
`ModelSpec::default_mode()`, an unreadable value means the same, and a stored `streaming`
for a model that cannot stream is **dropped with a log line** rather than obeyed — a
settings file outlives a catalogue, and a plan change must not turn into a spawn-time
error. There is no command that writes this map: it is read-only from the app's point of
view, so a hand edit is the only way to pin a mode, and `merge_model_mode` exists only so
that shape stays tested (and so a future UI, if one is ever justified, has one writer).

### Picking files: cache-name exclusions apply to directory scans only

`resolve_paths` (`commands/video.rs`) is what turns a dialog selection or a drop into queue
rows. It filters out this app's own artefacts — `<stem>_audio.wav`, `<stem>_thumb.jpg`,
dotfiles — because a directory expansion would otherwise queue the WAV the app just wrote
next to the video.

That filter was applied to **explicitly picked** paths too, and a file named
`EP494_audio.wav` is an ordinary audio input: choosing it added no row and printed nothing,
so it looked like the dialog was broken. `is_media_file` is therefore extension-only, and
the exclusions live in `is_cache_artefact`, which only `collect_media_files` consults. "The
user picked this file" outranks "this looks like our cache".

### Skipping existing subtitles (`skipExistingSubtitle`)

The decision is made in the **frontend** (`src/lib/transcription.ts`) before
`start_transcription` is invoked, because the backend command has no notion of
"the user already has this file". Three things there are load-bearing:

- **The check must not use `@tauri-apps/plugin-fs`.** That plugin's `exists` is
  gated by its path *scope*, which covers the app's own directories plus whatever
  the user picked in a **dialog**. A file added by drag-and-drop, by a URL import's
  cache directory, or simply a subtitle sitting next to a video in `~/Movies` is
  outside that scope, and the rejection was swallowed by a try/catch +
  `console.warn`, so **the setting looked on and did nothing** — measured in the
  running app:

  ```text
  warn Failed to check if output file exists forbidden path:
       /Users/…/x.srt, maybe it is not allowed on the scope for
       `allow-exists` permission in your capability file
  ```

  The Rust command `find_existing_path` (`commands/video.rs`) answers the same
  question with `std::fs`, which the backend already uses on those exact paths.
  The plugin is therefore **no longer a dependency** (removed from `Cargo.toml`,
  `package.json` and the capability file) — re-adding it for something else means
  re-reading this note.
- **Every format in the run is checked, not just the primary one.** With
  `srt,vtt` and only a `.vtt` on disk, checking `output_path` alone would rewrite
  half the set. `outputPathsToCheck` mirrors Rust's `subtitle::output_path_for`
  (first format keeps the path verbatim, later ones swap only the extension).
- **A failed check means "nothing found".** A broken existence probe must never
  block a transcription the user asked for.

The row then shows *which* file caused the skip (`skippedOutputPath`), because
"Skipped" alone reads as "nothing to do" when the real answer is usually "your
subtitle is already in the custom output folder, in another format".

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

`cancel_url_import` is the same shape in a **separate registry**: it aborts, awaits, then
removes the job's scratch directory (the task's `WorkDirGuard` would have done it anyway) and,
separately, the `.part` / `.ytdl` / `.temp` files under the **per-video cache directory**
(`<cache>/url-import/<id>/`). Both locations are swept because a download in progress writes
its `.part` into the cache, not into the scratch directory — sweeping only the latter was the
first cut's bug. `ImportEntry` therefore records both `work_dir` and `cache_dir`, and the
cache path comes from a sanitised video-id hint (so a percent-encoded `..%2F..%2Fetc` in the
URL cannot escape it). It acts on the import stage alone — once a `Queued` outcome has been
emitted the job is finished and cancelling means `cancel_transcription` or removing the queue
row — and it deliberately keeps an already downloaded audio file so re-importing the same URL
reuses it: only the partial extensions are removed, never the finished media.

### The menu-bar item shows transcription progress

A **left click on the menu-bar icon opens the menu**, and the menu leads with what the app
is doing: up to three greyed progress rows (`<file> — <step> (<n>/<total>)`, then
`<n> more queued`), a separator, then Show / Hide / Quit. While a run is in flight the
tray title next to the icon carries the percentage (`42%`); when idle there is **no
title at all**, because a permanent `0%` is noise in the menu bar. Right click shows the
same menu (platform default), so progress is always one click away on either button.

Left click used to **toggle the window**; it must not, because the question a left click
asks is "how is it going?", and answering by hiding an already-visible window is the
wrong answer.

Two pieces, split by what each side knows:

- **The wording** is the frontend's (`src/lib/tray.ts`), because it is the only side that
  knows the active locale. It builds the rows from the same `steps.*` keys the window
  uses and pushes `{ title?, rows[] }` through `update_tray_progress`.
- **The layout** is Rust (`commands/tray.rs`) and is a pure `layout` function over one
  snapshot: status rows first, a separator *only when there is something above it*, then
  the three actions in a fixed order (`quit` stays the bottom row). At most
  `MAX_STATUS_ROWS` rows, blank rows dropped, rows truncated to `MAX_ROW_CHARS`
  **characters** so a Chinese file name cannot panic or stretch the menu.

`update_tray_menu` (labels) and `update_tray_progress` (rows + title) write into **one**
`TrayState` and every write re-renders the whole menu from it. Without that, a language
change mid-run would erase the progress rows and a progress push would revert the labels
to English. `src/lib/tray.ts::startTraySync` subscribes to the **store's `files`**, not to
`transcribe_*` events: URL imports and "start all" change the queue without any
transcription event, and the tray's question is "what is the app doing". Pushes are
de-duplicated and coalesced to one `invoke` per 250 ms, because re-rendering the menu runs
on the main thread and events arrive much faster than that.

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
| `MONOSUB_YTDLP_PATH` | Override the bundled `yt-dlp` path (debugging / hot-fix only) |
| `MONOSUB_MODELS_DIR` | Override the models directory (wins over the in-app setting) |
| `MONOSUB_CONFIG_DIR` | Override the config directory holding `settings.json` (the CLI's only way to a non-default config path) |
| `MONOSUB_RESOURCES_DIR` | Override the `resources` root — `models.json`, `bin/`, `models/silero_vad` (the CLI would otherwise derive it from the executable's location) |
| `APPLE_SIGNING_IDENTITY` | Code signing identity (optional) |
| `APPLE_ID` / `APPLE_PASSWORD` / `APPLE_TEAM_ID` | Notarization credentials (optional) |

---

## Security Considerations

1. **Local-first**: No cloud dependencies; all media processing is local
2. **Downloaded models are verified**: byte size and, when pinned, SHA256
3. **Path safety**: model file paths are validated against directory traversal before
   any download destination or deletion is honoured
4. **File system access**: limited to user-selected directories and app data
5. **Subprocess isolation**: every bundled binary (`audiocpp_cli`, `llama-server`, `yt-dlp`)
   runs as a separate child process with `stdin` closed and no inherited environment
   secrets; `llama-server` listens only on `127.0.0.1`, and every child is `kill_on_drop` so
   it dies with its task
6. **Cookies are opt-in**: the URL import reads Bilibili cookies from a browser only after the
   user picks one in Settings or in the 412 prompt (`--cookies-from-browser`); the default is
   "not configured", so no browser data is touched unasked

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
- `yt-dlp` is **Unlicense**, but statically bundles third-party code, so **two** files
  ship: `resources/licenses/yt-dlp-LICENSE` and
  `resources/licenses/yt-dlp-THIRD_PARTY_LICENSES.txt` (both fetched by
  `fetch-ytdlp.sh`). The binary is bundled as `bin/yt-dlp-<target-triple>` and is used
  only against Bilibili; YouTube support is deferred, not shipped
  (`docs/tech-spec/url-import-design.md` §10).
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

**yt-dlp missing / URL import reports `urlImport.engineMissing`:**
```bash
./scripts/build/fetch-ytdlp.sh
```

**Something failed and you need the log:** Settings ▸ 系统 ▸ 日志目录 shows the path and
opens it (`log_directory` / `open_logs_dir`). There is exactly **one** rolling file,
`monosub.<YYYY-MM-DD>.log` — macOS `~/Library/Logs/MonoSub`, Windows
`%APPDATA%/MonoSub/logs`, Linux `~/.local/share/MonoSub/logs` — and the newest 7 are kept.
Both engines and the frontend write into it, so an `error` line sits with the `info` context
that explains it. Frontend failures from the core flows (transcription, URL import, model
download, uncaught errors) cross `log_frontend_error` and appear with `source=frontend`.

**URL import fails with `URL_IMPORT_NEEDS_COOKIES`:** pick a browser in the row's own picker
(that writes `settings.bilibiliCookiesBrowser` and retries). `HTTP Error 412` is Bilibili's
response to an anonymous request. A "needs login" *warning* is different: the import still
succeeds by falling back to transcription and only flags `login_hint`.

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
- [yt-dlp](https://github.com/yt-dlp/yt-dlp) — the URL-import downloader (bundled, pinned)
- [FunASR / SenseVoice](https://github.com/modelscope/FunASR) — model family
- [Hunyuan MT](https://huggingface.co/tencent/HY-MT1.5-1.8B-GGUF) — the translation model
- [Tailwind CSS](https://tailwindcss.com/)
- [Zustand Documentation](https://github.com/pmndrs/zustand)
- `docs/tech-spec/refactor-audiocpp.md` — the refactor spec
- `docs/tech-spec/audiocpp-poc.md` — measured CLI behaviour
- `docs/tech-spec/audiocpp-accuracy-p1-23.md` — accuracy + VAD tuning evidence
- `docs/eval/diarization-quality-eval.md` — cue-level attribution measured on a
  4-hour interview (93.90% → 99.85%); the method, the four independent judges, and
  why the residual errors are not attribution bugs
- `docs/tech-spec/translation-design.md` — translation design
- `docs/tech-spec/translation-poc.md` — measured llama.cpp / HY-MT behaviour
- `docs/tech-spec/url-import-design.md` — URL import design (flow, language rules, IPC)
- `docs/tech-spec/url-import-tasks.md` — URL import task list
