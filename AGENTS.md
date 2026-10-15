# MonoSub - Agent's Guide

> This document is intended for AI coding agents working on the MonoSub project.
> Read this file to understand the project architecture, conventions, and development workflow.

## Project Overview

**MonoSub** is a local-first desktop application for converting videos and audio files to subtitles without cloud dependencies. It provides a clean, modern UI for batch processing media files using local AI speech recognition.

Speech recognition runs through **audio.cpp** (`audiocpp_cli`), a C++ inference engine
distributed as a single prebuilt binary. There is **no Python** in the runtime.

### Key Features

- 🎬 **Multi-file queue processing** - Manage multiple video/audio files simultaneously
- 🖼️ **Video thumbnails** - Automatic preview generation
- ⚡ **Local AI processing** - audio.cpp + SenseVoice for speech recognition, Metal accelerated
- 📦 **In-app model management** - Download/remove models on demand; nothing large is bundled
- 📝 **Multiple output formats** - SRT, VTT, TXT, MD (and JSON)
- 🌐 **Multi-language UI** - Supports 24 languages
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
│  ├── i18next (24 languages)                                     │
│  └── Framer Motion (animations)                                 │
├─────────────────────────────────────────────────────────────────┤
│  App (src-tauri/)  — everything below happens in Rust           │
│  ├── Tauri 2.0, system tray                                     │
│  ├── FFmpeg/FFprobe: audio extraction, thumbnails, metadata     │
│  ├── Model download + integrity verification                    │
│  ├── Silero VAD segmentation (audiocpp_cli --task vad)          │
│  ├── ASR orchestration (audiocpp_cli --task asr)                │
│  └── Subtitle post-processing + writers                         │
└─────────────────────────────────────────────────────────────────┘
                    │ spawns
                    ▼
        audiocpp_cli (prebuilt, bundled; ~31 MB, no deps)
```

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
│   │   ├── Settings.tsx         # Includes the Models section
│   │   └── ThemeToggle.tsx
│   ├── store/
│   │   └── useStore.ts          # Zustand state + persist v2 migration
│   ├── i18n/
│   │   ├── index.ts
│   │   └── locales/             # 24 translation files (kept at parity)
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
│   │   │   ├── vad.rs           # silero_vad invocation + chunk parsing
│   │   │   └── wav.rs           # 16k mono s16le read + sample slicing
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
│   │   ├── models.json          # Model catalogue (source, committed)
│   │   ├── licenses/            # audio.cpp Apache-2.0 text + third-party notices
│   │   ├── bin/                 # fetched: ffmpeg, ffprobe, audiocpp_cli, lib/
│   │   └── models/silero_vad/   # fetched: silero_vad_16k.safetensors
│   ├── tests/fixtures/          # Python-generated subtitle goldens
│   ├── Cargo.toml
│   └── tauri.conf.json
│
├── scripts/
│   ├── build/
│   │   ├── fetch-audiocpp.sh    # Pinned download + SHA256 of CLI + VAD model
│   │   ├── prepare-ffmpeg.sh
│   │   └── prepare-resources.sh # Orchestrates the above; regenerates MANIFEST
│   ├── release/
│   │   ├── build-macos-app.sh
│   │   ├── sign-app.sh          # Signs bundled binaries, then the app
│   │   ├── bump-version.sh
│   │   └── release.sh
│   └── utils/
│       ├── version-utils.sh
│       ├── check-i18n-keys.mjs  # Asserts all 24 locales match en.json
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

# 2. Fetch the bundled runtime (audiocpp_cli + silero VAD) and ffmpeg
./scripts/build/prepare-resources.sh
```

### Development Workflow

```bash
bun run dev          # Tauri dev server (frontend + Rust)
bun run dev:servers  # Vite only (frontend only)
bun run check        # TypeScript type check
bun run lint         # ESLint
```

### Tests

```bash
cd src-tauri && cargo test     # Rust unit + integration tests
cargo clippy                   # Lints
node scripts/utils/check-i18n-keys.mjs   # Locale parity
```

Tests that need the bundled binary/model print a `skip:` reason and pass when those
gitignored artifacts are absent, so a fresh clone still has a green suite.

### Production Builds

```bash
bun run build         # Frontend only
bun run build:dmg     # macOS DMG (scripts/build/build-macos-app.sh)
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
Rust checks the model is installed (else MODEL_NOT_INSTALLED → download UI)
    ↓
FFmpeg extracts 16 kHz mono s16le WAV (cached next to the media as *_audio.wav)
    ↓
audiocpp_cli --task vad        → vad_chunks.json (speech segments)
    ↓
Rust slices the WAV per chunk, writes requests.json
    ↓
audiocpp_cli --task asr --request-sequence   → one text file per chunk
    ↓
Rust post-processes (split/estimate/merge) → writes every requested format
    ↓
Rust emits events → React updates the UI
```

### Why VAD runs in Rust, not in the model

`sense_asr` (SenseVoice) has **no word/segment timestamps**; it emits one flat text
blob. So segmentation is done up front by Silero VAD, and timestamps are then
*estimated* inside each VAD chunk in proportion to character count. Chunk boundaries
are therefore the only real timing anchor.

Two VAD constants are quality-sensitive and were tuned on measured evidence (see
`docs/tech-spec/audiocpp-accuracy-p1-23.md`):

```rust
// engine/vad.rs — pinned by a test
pub const VAD_MAX_SECONDS: f64 = 15.0;
pub const VAD_MERGE_GAP_SECONDS: f64 = 0.2;
pub const VAD_PADDING_SECONDS: f64 = 0.1;
```

`padding p` widens each speech span by `p` on both sides, so a raw silence gap `g`
becomes `g - 2p`, and two spans merge when `g - 2p < merge_gap`. Over-merging
irrecoverably fuses two utterances and misplaces their boundary; under-merging is
recovered downstream by `subtitle::postprocess`, which re-joins cues closer than its
own 0.5 s `gap_threshold`. Prefer under-merging when in doubt.

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
`Writing subtitle`. `FileItem.tsx` maps those to i18n keys, so **do not reword them**.

There is no stdout protocol any more — the engine is called directly, not via a
line-parsing subprocess.

---

## Key Files Reference

| Path | Purpose |
|------|---------|
| `src/App.tsx` | Event listeners for progress/completion/error |
| `src/store/useStore.ts` | Zustand state, `persist` v2 + migrate |
| `src/lib/models.ts` | Model commands/events, `MODEL_NOT_INSTALLED_ERROR` |
| `src/components/Settings.tsx` | Settings incl. the Models section |
| `src-tauri/src/engine/mod.rs` | The transcription pipeline |
| `src-tauri/src/engine/cli.rs` | Binary resolution + process management |
| `src-tauri/src/engine/catalog.rs` | `models.json`, install checks, language mapping |
| `src-tauri/src/commands/transcribe.rs` | Task registry, events, cancellation |
| `src-tauri/src/commands/models.rs` | Model download/verify/remove |
| `src-tauri/src/subtitle/writer.rs` | Format writers |
| `src-tauri/resources/models.json` | Model catalogue |
| `src-tauri/tests/fixtures/` | Subtitle goldens (+ their generator) |
| `scripts/build/fetch-audiocpp.sh` | Pinned upstream binary + VAD download |

---

## Code Style Guidelines

### TypeScript/React

- Functional components with hooks
- Import order: React → Third-party → Local → Types
- `@/` alias for imports from `src/`
- Zustand with persistence for settings
- Keep all 24 locales in sync (see `scripts/utils/check-i18n-keys.mjs`)

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

Persisted to `localStorage` as `monosub-storage` at `version: 2`. The `migrate`
function **drops** `enableNoiseReduction`, `enableVolumeNormalization`,
`enableBilingual` and `bilingualTargetLang`: those settings never reached the engine
in the old code either (the Python engine hardcoded the filters), so they were dead.
The equivalent ffmpeg filters are now always applied.

---

## Important Implementation Details

### Binary Path Resolution

Bundled binaries (`ffmpeg`, `ffprobe`, `audiocpp_cli`) are resolved in this order:

1. `MONOSUB_FFMPEG_PATH` / `MONOSUB_FFPROBE_PATH` / `MONOSUB_AUDIOCPP_PATH` override
2. Dev checkout: `src-tauri/resources/bin/`
3. Next to the executable
4. `<Contents>/Resources/resources/bin/` (app bundle)
5. System paths / `PATH` (ffmpeg only)

Names use the `<name>-<target-triple>` convention, e.g.
`audiocpp_cli-aarch64-apple-darwin`.

### Model Storage

Models live under `<app_data_dir>/models/<target_directory>/`, mirroring upstream's
`model_specs` layout so a user can drop files in by hand. `is_installed` checks file
existence plus size match and deliberately **never hashes** — hashing a 250 MB file on
every startup is not acceptable. Hashing happens once, after download.

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
`es`/`de`/`fr`/`pt` → `auto`. Neither engine has ever converted to Traditional.

### Cancellation

`cancel_transcription` aborts the task and then awaits it, so cleanup is
deterministic. The CLI child dies via `kill_on_drop`, and `engine::transcribe` owns a
`Drop` guard for its temp directory — aborting the task is sufficient for both.
Partially written subtitle files are removed **only if their mtime is at or after the
task start**, so a pre-existing subtitle the user already had is not destroyed.

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
| `APPLE_SIGNING_IDENTITY` | Code signing identity (optional) |
| `APPLE_ID` / `APPLE_PASSWORD` / `APPLE_TEAM_ID` | Notarization credentials (optional) |

---

## Security Considerations

1. **Local-first**: No cloud dependencies; all media processing is local
2. **Downloaded models are verified**: byte size and, when pinned, SHA256
3. **Path safety**: model file paths are validated against directory traversal before
   any download destination or deletion is honoured
4. **File system access**: limited to user-selected directories and app data
5. **Subprocess isolation**: the inference binary runs as a separate process with
   `stdin` closed and no inherited environment secrets

---

## Licensing / Attribution

`audiocpp_cli` is **Apache-2.0**, so its LICENSE text must ship with the app —
see `src-tauri/resources/licenses/`. SenseVoice weights carry the **FunASR Model
License v1.1**, which requires naming the model; it is *not* bundled (the user
downloads it). Adding a bundled component means updating
`src-tauri/resources/licenses/THIRD-PARTY-NOTICES.md`.

---

## Troubleshooting

**audiocpp_cli missing / not executable:**
```bash
./scripts/build/fetch-audiocpp.sh
```

**Model missing:** the app reports `MODEL_NOT_INSTALLED`; download it from
Settings → Models. Models can also be placed manually under
`<app_data_dir>/models/<target_directory>/`.

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
3. Run `cargo test`, `bun run check`, `bun run lint`, and the i18n check before committing
4. Ensure i18n strings are added to **all 24** locales
5. Log important events using `tracing` (Rust) or `console.log` (Frontend)
6. Tests that need the bundled binary or model must skip with a reason, not fail

---

## Resources

- [Tauri Documentation](https://tauri.app/)
- [audio.cpp](https://github.com/0xShug0/audio.cpp) — the inference engine
- [FunASR / SenseVoice](https://github.com/modelscope/FunASR) — model family
- [Tailwind CSS](https://tailwindcss.com/)
- [Zustand Documentation](https://github.com/pmndrs/zustand)
- `docs/tech-spec/refactor-audiocpp.md` — the refactor spec
- `docs/tech-spec/audiocpp-poc.md` — measured CLI behaviour
- `docs/tech-spec/audiocpp-accuracy-p1-23.md` — accuracy + VAD tuning evidence
