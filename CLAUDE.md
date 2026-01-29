# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

MonoSub is a local-first desktop application for converting videos and audio files to subtitles without cloud dependencies. Built with Tauri2.0 (Rust), React/TypeScript frontend, and Python/FunASR for speech recognition.

## Build & Development Commands

```bash
# Development
bun run dev          # Start Tauri dev server (frontend + Rust backend)
bun run dev:servers  # Vite dev server only (frontend)
bun run check        # TypeScript type checking
bun run lint         # ESLint

# Python environment (core-engine/)
./scripts/setup-python.sh       # Initial setup: creates venv, installs deps
cd core-engine && uv run python main.py --help  # Test Python engine

# Production builds
bun run build        # Full production build
bun run build:dmg    # macOS DMG packaging
```

## Architecture

### Three-Layer Stack

1. **Frontend (src/)** - React + TypeScript + TailwindCSS
   - Zustand store at `src/store/useStore.ts` manages all app state (persisted to localStorage as "monosub-storage")
   - i18n supports 24 languages via `src/i18n/`
   - Uses `@/*` alias for imports from `src/`

2. **Tauri Backend (src-tauri/)** - Rust
   - `commands/video.rs`: FFprobe metadata extraction, FFmpeg thumbnail generation
   - `commands/transcribe.rs`: Spawns Python subprocess, streams progress via events

3. **AI Engine (core-engine/)** - Python + FunASR
   - Uses SenseVoice model for multilingual speech recognition
   - Managed via `uv` package manager
   - Outputs structured messages: `MSG:`, `PROGRESS:`, `SUCCESS:`, `ERROR:`

### Data Flow

User adds files → Tauri extracts metadata → User clicks start → Rust spawns Python subprocess → Python streams progress via stdout → Tauri emits events → React updates UI

### IPC Communication Pattern

- Frontend invokes Tauri commands: `start_transcription`, `get_video_metadata`, `generate_thumbnail`
- Python communicates via stdout with prefixed lines (`PROGRESS:0.5`, `SUCCESS:/path/to/file.srt`)
- Tauri emits events (`transcribe_progress`, `transcribe_complete`) to frontend

## Key Files

| Path | Purpose |
|------|---------|
| `src/App.tsx` | Main component, event listeners for progress/completion |
| `src/store/useStore.ts` | Zustand state: files queue, settings, storage paths |
| `src-tauri/src/commands/transcribe.rs` | Python subprocess management |
| `core-engine/main.py` | Python CLI entry point |
| `core-engine/monosub_core/transcriber.py` | FunASR model loading & inference |

## Important Notes

- **FFmpeg/FFprobe required**: App depends on these for media processing
- **Model caching**: FunASR downloads models to `~/.cache/modelscope/` on first run
- **Production Python path**: `transcribe.rs` has TODOs for bundled Python (currently uses venv path)
- **Stale closure prevention**: `App.tsx` uses refs pattern for event listener callbacks
