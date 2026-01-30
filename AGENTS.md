# MonoSub - Agent's Guide

> This document is intended for AI coding agents working on the MonoSub project.
> Read this file to understand the project architecture, conventions, and development workflow.

## Project Overview

**MonoSub** is a local-first desktop application for converting videos and audio files to subtitles without cloud dependencies. It provides a clean, modern UI for batch processing media files using local AI speech recognition.

### Key Features

- 🎬 **Multi-file queue processing** - Manage multiple video/audio files simultaneously
- 🖼️ **Video thumbnails** - Automatic preview generation
- ⚡ **Local AI processing** - Uses FunASR for high-quality speech recognition
- 📝 **Multiple output formats** - SRT, VTT, TXT subtitle formats
- 🌐 **Multi-language UI** - Supports 24 languages
- 💾 **Flexible storage** - Save to same directory or custom location
- 🎨 **Modern UI** - Apple-inspired design system with dark/light mode

---

## Technology Stack

### Three-Layer Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│  Frontend (src/)                                                │
│  ├── React 18 + TypeScript                                      │
│  ├── TailwindCSS (Apple design system)                          │
│  ├── Zustand (state management)                                 │
│  ├── i18next (24 languages)                                     │
│  └── Framer Motion (animations)                                 │
├─────────────────────────────────────────────────────────────────┤
│  Backend (src-tauri/)                                           │
│  ├── Tauri 2.0 (Rust)                                           │
│  ├── FFmpeg/FFprobe integration                                 │
│  ├── Python subprocess management                               │
│  └── System tray support                                        │
├─────────────────────────────────────────────────────────────────┤
│  AI Engine (core-engine/)                                       │
│  ├── Python 3.9+                                                │
│  ├── FunASR (SenseVoice model)                                  │
│  ├── PyTorch (MPS/CUDA/CPU)                                     │
│  └── FFmpeg Python bindings                                     │
└─────────────────────────────────────────────────────────────────┘
```

### Package Managers

| Component | Package Manager | File |
|-----------|----------------|------|
| Frontend | Bun | `package.json` |
| Rust Backend | Cargo | `src-tauri/Cargo.toml` |
| Python Engine | uv | `core-engine/pyproject.toml` |

---

## Project Structure

```
monosub/
├── src/                          # Frontend (React/TypeScript)
│   ├── components/              # React components
│   │   ├── Header.tsx
│   │   ├── Toolbar.tsx
│   │   ├── FileList.tsx
│   │   ├── FileItem.tsx
│   │   ├── BottomBar.tsx
│   │   ├── Settings.tsx
│   │   └── ThemeToggle.tsx
│   ├── store/
│   │   └── useStore.ts          # Zustand state management
│   ├── i18n/
│   │   ├── index.ts             # i18n configuration
│   │   └── locales/             # 24 translation files
│   ├── contexts/
│   │   └── ThemeContext.tsx     # Dark/light mode
│   ├── hooks/
│   │   └── useTheme.ts
│   ├── lib/
│   │   └── utils.ts             # Utility functions
│   ├── App.tsx                  # Main application component
│   └── main.tsx                 # Entry point
│
├── src-tauri/                   # Rust backend
│   ├── src/
│   │   ├── main.rs              # Application entry
│   │   ├── commands/
│   │   │   ├── video.rs         # FFmpeg metadata & thumbnails
│   │   │   └── transcribe.rs    # Python subprocess management
│   │   ├── logging.rs           # Structured logging
│   │   └── tray.rs              # System tray
│   ├── Cargo.toml
│   └── tauri.conf.json          # Tauri configuration
│
├── core-engine/                 # Python AI engine
│   ├── main.py                  # CLI entry point
│   ├── monosub_core/
│   │   ├── __init__.py
│   │   ├── audio_extractor.py   # Audio extraction from video
│   │   ├── transcriber.py       # FunASR model & inference
│   │   └── subtitle_writer.py   # SRT/VTT/TXT generation
│   ├── pyproject.toml           # uv project config
│   └── .venv/                   # Python virtual environment
│
├── scripts/                     # Build & deployment scripts
│   ├── setup-python.sh          # Initial Python setup
│   ├── build-dmg.sh             # macOS DMG packaging
│   ├── build-windows.sh         # Windows packaging
│   ├── prepare-lite.sh          # Lite build preparation
│   └── prepare-venv.sh          # Full venv preparation
│
├── design/                      # UI design files
├── _docs/                       # Product documentation
├── landing_page/                # Marketing website
├── package.json                 # Frontend dependencies
├── tailwind.config.js           # Tailwind with custom theme
├── tsconfig.json                # TypeScript config
└── vite.config.ts               # Vite build config
```

---

## Development Commands

### Prerequisites

- **Bun** - Frontend package manager ([Install](https://bun.sh/))
- **uv** - Python package manager ([Install](https://docs.astral.sh/uv/))
- **Rust** 1.70+ - For Tauri backend
- **FFmpeg** - System dependency for media processing

### Initial Setup

```bash
# 1. Install frontend dependencies
bun install

# 2. Setup Python environment
./scripts/setup-python.sh

# 3. Verify Python engine
cd core-engine && uv run python main.py --help
```

### Development Workflow

```bash
# Start Tauri dev server (frontend + Rust backend)
bun run dev

# Vite dev server only (frontend only)
bun run dev:servers

# TypeScript type checking
bun run check

# ESLint
bun run lint
```

### Production Builds

```bash
# Build frontend for production
bun run build

# macOS DMG packaging (Lite version - recommended)
bun run build:dmg
# Or: BUILD_TYPE=lite ./scripts/build-dmg.sh

# macOS DMG packaging (Full version - larger, more stable)
BUILD_TYPE=full ./scripts/build-dmg.sh

# Windows packaging
bun run build:windows
# Or: BUILD_TYPE=lite ./scripts/build-windows.sh
```

**Build Types:**
- **Lite** (~80MB) - Python interpreter + uv + source, installs deps on first run
- **Full** (~800MB) - Complete Python venv bundled, works offline immediately

---

## Data Flow

```
User adds files
    ↓
Tauri extracts metadata (FFprobe) + generates thumbnail (FFmpeg)
    ↓
User clicks "Start"
    ↓
Rust spawns Python subprocess (main.py)
    ↓
Python: Extract audio → Load FunASR model → Transcribe → Write subtitles
    ↓
Python streams progress via stdout (PROGRESS:, MSG:, SUCCESS:, ERROR:)
    ↓
Tauri parses output → Emits events to frontend
    ↓
React updates UI (progress bars, status, notifications)
```

---

## IPC Communication Pattern

### Frontend → Backend (Tauri Commands)

| Command | Purpose |
|---------|---------|
| `start_transcription` | Begin transcription task |
| `pause_transcription` | Pause current task |
| `cancel_transcription` | Cancel running task |
| `get_video_metadata` | Extract duration, resolution, size |
| `generate_thumbnail` | Create video preview |
| `open_in_finder` | Reveal file in system file manager |

### Backend → Frontend (Tauri Events)

| Event | Payload | Purpose |
|-------|---------|---------|
| `transcribe_progress` | `{task_id, progress, status}` | Progress updates |
| `transcribe_complete` | `{task_id}` | Task finished successfully |
| `transcribe_error` | `{task_id, status}` | Task failed |

### Python → Rust (stdout protocol)

```
PROGRESS:0.5       → Progress percentage (0-100)
MSG:Loading...     → Status message
SUCCESS:/path/file → Completion with output path
ERROR:message      → Error message
```

---

## Key Files Reference

| Path | Purpose |
|------|---------|
| `src/App.tsx` | Main component, event listeners for progress/completion |
| `src/store/useStore.ts` | Zustand state: files queue, settings, storage paths |
| `src-tauri/src/commands/transcribe.rs` | Python subprocess management |
| `src-tauri/src/commands/video.rs` | FFmpeg/FFprobe integration |
| `core-engine/main.py` | Python CLI entry point |
| `core-engine/monosub_core/transcriber.py` | FunASR model loading & inference |
| `core-engine/monosub_core/audio_extractor.py` | Audio extraction from video |
| `core-engine/monosub_core/subtitle_writer.py` | Subtitle format generation |

---

## Code Style Guidelines

### TypeScript/React

- Use functional components with hooks
- Import order: React → Third-party → Local → Types
- Use `@/` alias for imports from `src/`
- Tailwind classes: group related styles, use custom theme colors
- State management: Zustand with persistence for settings

### Rust

- Use `tracing` for structured logging
- Error handling: `Result<T, String>` for Tauri commands
- Async/await with `tokio`
- Follow existing module structure

### Python

- Formatting: Black (line length 100)
- Linting: Ruff
- Type hints where helpful
- `print(flush=True)` for progress communication
- Debug logs prefixed with `DEBUG:`

---

## State Management (Zustand)

```typescript
// Key state slices:
files: VideoFile[]              // File queue
storageMode: 'same' | 'custom'  // Output location
customStoragePath: string       // Custom output directory
settings: AppSettings           // User preferences
isProcessing: boolean           // Global processing state
currentTaskId: string | null    // Active task
```

State is persisted to `localStorage` as `monosub-storage` (settings only, not files).

---

## Testing Strategy

Currently, the project relies on:

1. **TypeScript type checking**: `bun run check`
2. **ESLint**: `bun run lint`
3. **Manual testing** of the Python engine: `uv run python main.py <video> <output>`

No automated unit tests are currently implemented. When adding tests:

- Python: Use `pytest` (already in dev dependencies)
- Rust: Use built-in `cargo test`
- Frontend: Consider Vitest for component testing

---

## Important Implementation Details

### FFmpeg Path Resolution

The app supports multiple FFmpeg sources (bundled, system, dev paths). See `video.rs` for the resolution order:

1. Dev environment: `src-tauri/resources/bin/ffmpeg-{arch}`
2. Bundled binary: `ffmpeg-{arch}` in executable directory
3. Common system paths (Homebrew, etc.)
4. Fallback to PATH

### Development vs Production Mode

Detected by checking for `core-engine/.venv` existence:
- **Dev mode**: Uses Python venv directly
- **Production**: Uses sidecar binary (`monosub-engine`)

### Stale Closure Prevention

`App.tsx` uses refs pattern for event listeners to access latest state:

```typescript
const filesRef = useRef(useStore.getState().files);
useEffect(() => {
  const unsubscribe = useStore.subscribe((state) => {
    filesRef.current = state.files;
  });
  return unsubscribe;
}, []);
```

### Model Caching

FunASR downloads models to `~/.cache/modelscope/` on first run. This is shared across app restarts.

---

## Environment Variables

| Variable | Purpose |
|----------|---------|
| `MONOSUB_FFMPEG_PATH` | Override FFmpeg binary path (Python subprocess) |
| `MONOSUB_FFPROBE_PATH` | Override FFprobe binary path (Python subprocess) |
| `BUILD_TYPE` | `lite` or `full` for packaging scripts |
| `APPLE_SIGNING_IDENTITY` | Code signing identity (optional) |
| `APPLE_ID` / `APPLE_PASSWORD` / `APPLE_TEAM_ID` | Notarization credentials (optional) |

---

## Security Considerations

1. **Local-first**: No cloud dependencies, all processing happens locally
2. **File system access**: Limited to user-selected directories via Tauri dialog API
3. **Subprocess isolation**: Python runs in separate process with limited environment
4. **FFmpeg path validation**: Paths are resolved and validated before execution

---

## Troubleshooting

### Common Issues

**uv not installed:**
```bash
curl -LsSf https://astral.sh/uv/install.sh | sh
# Or: brew install uv
```

**Python dependencies issues:**
```bash
cd core-engine
uv sync --reinstall
```

**FFmpeg not found:**
```bash
# macOS
brew install ffmpeg

# Ubuntu/Debian
sudo apt-get install ffmpeg
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
3. Test both dev mode and production builds for cross-platform features
4. Ensure i18n strings are added to all locale files
5. Log important events using `tracing` (Rust) or `console.log` (Frontend)

---

## Resources

- [Tauri Documentation](https://tauri.app/)
- [FunASR Documentation](https://github.com/alibaba-damo-academy/FunASR)
- [Tailwind CSS](https://tailwindcss.com/)
- [Zustand Documentation](https://github.com/pmndrs/zustand)
- [uv Documentation](https://docs.astral.sh/uv/)
