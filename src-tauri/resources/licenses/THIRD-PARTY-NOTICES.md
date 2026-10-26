# Third-Party Notices

MonoSub bundles or depends on the following third-party components.

## Model weights: not distributed by MonoSub

MonoSub ships **no model files**. Everything in
`src-tauri/resources/models.json` is metadata: each entry's weights are fetched at
run time from the publisher's own host (Hugging Face, ModelScope, NGC, …), and
only after the user asks for that specific model.

That split is what keeps the model licences a question about the *user's* use
rather than a redistribution question for this project — no weight passes through
MonoSub's servers, and none is included in an installer or a store package.

Every entry carries a `commercial_use` verdict derived from its upstream licence,
so the app can say where the user stands before a transfer starts:

| `commercial_use` | Meaning | Entries |
|---|---|---|
| `allowed` | Permissive, or explicitly permits commercial use | 78 |
| `restricted` | Commercial use possible only under conditions: user-count thresholds, territory exclusions, share-alike, registration or acceptable-use terms | 27 |
| `noncommercial` | Upstream forbids commercial use | 16 |
| `unknown` | No licence published upstream — no grant to rely on | 8 |

`allowed` entries carry no badge: their licence name is already printed on the
row. The other 51 show one, and their download opens a confirmation that names
the licence, links to the publisher's own text, and states plainly that MonoSub
neither hosts nor redistributes the weights and cannot verify that the licence
covers the user's intended use.

The verdict summarises the upstream terms; it is not legal advice. It is authored
in `models.json` rather than inferred at run time, and the per-licence mapping
behind it is recorded in `docs/compliance/ip-compliance-audit.md`.

## audio.cpp

- Version: v0.9.1 (git `cf124a67`)
- Source: https://github.com/0xShug0/audio.cpp
- License: Apache License 2.0 — full text in `audio.cpp-LICENSE`
- Bundled as: `bin/audiocpp_cli-*`
- Modifications: none (unmodified upstream binary)

The Apache License 2.0 permits redistribution provided the license text and
copyright notice are included; `audio.cpp-LICENSE` contains both.

## llama.cpp

- Version: b11696 (rolling build)
- Source: https://github.com/ggml-org/llama.cpp
- License: MIT — full text in `llama.cpp-LICENSE`
- Bundled as: `bin/llama-server-*` plus its `libllama*` / `libggml*` / `libmtmd*`
  dylibs (flat in `bin/`, because the executable's only `LC_RPATH` is
  `@loader_path`). The tarball's other executables (`llama-cli`,
  `llama-tokenize`, …) are deliberately not bundled.
- Modifications: none (unmodified upstream binary)
- Runs as a separate child process, never linked into the app: audio.cpp and
  llama.cpp each carry their own ggml, and linking both into one process would
  conflict (translation-design §11).

## Silence detection model (Silero VAD)

- File: `models/silero_vad/silero_vad_16k.safetensors`
- Source: https://github.com/0xShug0/audio.cpp (v0.9.1 `assets/framework/models/silero_vad/`)
- Upstream project: https://github.com/snakers4/silero-vad
- License: MIT

## SenseVoice-Small (speech recognition model)

- Not bundled — downloaded by the user at runtime, at the user's request.
- Source: https://huggingface.co/FunAudioLLM/SenseVoiceSmall-GGUF-audiocpp
- Upstream: FunAudioLLM / FunASR — https://github.com/modelscope/FunASR
- License: FunASR Model License v1.1
- Required attribution: model name "SenseVoice-Small", produced by FunAudioLLM.

## MOSS-Transcribe-Diarize (speech recognition + speaker diarization model)

- Not bundled — downloaded by the user at runtime, at the user's request.
- Source (GGUF): https://www.modelscope.cn/models/audio-cpp/audio.cpp-gguf (MOSS-Transcribe-Diarize-GGUF)
- Upstream: OpenMOSS-Team / MOSS-Transcribe-Diarize — https://huggingface.co/OpenMOSS-Team/MOSS-Transcribe-Diarize
- License: Apache License 2.0
- A single call emits text + segment timestamps + speaker labels. Q8_0 is the
  default; Q4_K is offered as an optional download but upstream documents
  segment/timestamp drift for it.

## Sortformer Diar 4spk v1 (speaker diarization model)

- Not bundled — downloaded by the user at runtime, at the user's request.
- Source (GGUF): https://www.modelscope.cn/models/audio-cpp/audio.cpp-gguf (Sortformer-Diar-4spk-v1-GGUF)
- Upstream: nvidia/diar_sortformer_4spk-v1
- License: **CC-BY-NC-4.0** — commercial use is not permitted.
- Tagged `commercial_use: noncommercial` in `models.json`. The app shows that
  verdict as a badge on the row and requires a confirmation naming the licence
  before the download starts. The weights themselves are fetched from the URL
  above, on the user's own request: MonoSub never hosts or redistributes them, so
  this licence governs what the *user* may do with the model, not what MonoSub is
  licensed to distribute.
- Emits speaker turns only (no transcript), up to 4 speakers.

## Qwen3-ASR / Qwen3-ForcedAligner (speech recognition + alignment models)

- Not bundled — downloaded by the user at runtime, at the user's request.
- Source (GGUF): https://www.modelscope.cn/models/audio-cpp/audio.cpp-gguf
- Upstream: Qwen/Qwen3-ASR-0.6B, Qwen/Qwen3-ForcedAligner-0.6B
- License: Apache License 2.0
- Word-level timestamps require running the forced aligner after ASR.

## Fun-ASR-Nano-2512 (speech recognition model)

- Not bundled — downloaded by the user at runtime, at the user's request.
- Source (GGUF): https://www.modelscope.cn/models/audio-cpp/audio.cpp-gguf (Fun-ASR-Nano-2512-GGUF)
- Upstream: FunAudioLLM / Fun-ASR-Nano-2512 — https://github.com/modelscope/FunASR
- License: FunASR Model License v1.1
- Required attribution: model name "Fun-ASR-Nano", produced by FunAudioLLM.
- No timestamps and no speaker output are exposed by the audio.cpp port.

## HY-MT1.5-1.8B (translation model)

- Not bundled — downloaded by the user at runtime, at the user's request.
- Source (GGUF): https://www.modelscope.cn/models/Tencent-Hunyuan/HY-MT1.5-1.8B-GGUF
  (mirrors: https://huggingface.co/tencent/HY-MT1.5-1.8B-GGUF,
  https://hf-mirror.com/tencent/HY-MT1.5-1.8B-GGUF)
- Upstream: Tencent Hunyuan — HY-MT1.5-1.8B, licence release date 2025-12-30
- License: Tencent HY Community License Agreement
  (https://huggingface.co/tencent/HY-MT1.5-1.8B-GGUF/blob/main/License.txt)
- Required attribution: model name "HY-MT1.5-1.8B", produced by Tencent Hunyuan.
- Run by the bundled `llama-server` (llama.cpp), not by audio.cpp.

## FFmpeg

- Bundled as: `bin/ffmpeg-*`, `bin/ffprobe-*` (plus `bin/lib/*` on macOS)
- Source: https://ffmpeg.org/
- License: **GNU Lesser General Public License, version 2.1 or later (LGPL-2.1-or-later)**.
  Full text in `ffmpeg-LICENSE`.
- Trademark: FFmpeg is a trademark of Fabrice Bellard, originator of the FFmpeg
  project.

### Why LGPL and not GPL

FFmpeg's own licence is LGPL-2.1-or-later, but configuring it with `--enable-gpl`
puts the **entire** binary under the GPL. That distinction decides whether we can ship
it at all:

- Earlier builds bundled gyan.dev's `essentials_build`, which is
  `--enable-gpl --enable-version3` — i.e. **GPLv3**.
- MonoSub's own source is published under the PolyForm Small Business License
  1.0.0 and the app is sold through the Mac App Store and Microsoft Store.
  Conveying a GPL binary there fails on two counts: §6 requires Corresponding Source
  (including build scripts) that we never shipped, and §10 forbids the further
  restrictions Apple's EULA imposes (Apple-branded hardware only, personal
  non-commercial use, device-count limits).

Building GPL-free keeps FFmpeg under the LGPL. Its copyleft reaches only FFmpeg itself,
not a program that merely runs it — and MonoSub spawns `ffmpeg`/`ffprobe` as **separate
child processes** (`commands/transcribe.rs`, `commands/video.rs`), never linking them.

### Verified build provenance

| Platform | Package | Configure marker |
|---|---|---|
| Windows | BtbN `ffmpeg-n8.1.3-24-g92611900bb-win64-lgpl-8.1.zip` (pinned autobuild tag) | `--disable-gpl`, `--disable-nonfree`, `--enable-version3` → LGPLv3 |
| macOS | built from `ffmpeg-8.1.3.tar.xz` by `scripts/release/build-ffmpeg-lgpl-macos.mjs` | `--disable-gpl`, `--disable-nonfree`, `--disable-version3` → LGPLv2.1 |

Both are checked at release time: `scripts/lib/ffmpeg.mjs` parses the configure line
out of `ffmpeg -version` and fails the build if `--enable-gpl` or `--enable-nonfree`
appears. The check inspects the artifact rather than trusting the download URL, so it
also catches a GPL binary copied off a developer's Homebrew — which is exactly how the
old gyan.dev build got in.

### LGPL obligations we must keep meeting

Per the FFmpeg project's published compliance checklist:

1. Build FFmpeg without `--enable-gpl` and without `--enable-nonfree`. ✅ enforced by
   `verifyFfmpegLicense()`.
2. Distribute the FFmpeg source for the exact binaries shipped, or provide a written
   offer valid for at least three years. Source for the pinned build:
   - Windows: `https://github.com/FFmpeg/FFmpeg/archive/92611900bb.zip`
     (commit `92611900bb`, matching tag `n8.1.3-24-g92611900bb`)
   - macOS: `https://ffmpeg.org/releases/ffmpeg-8.1.3.tar.xz`
   - `ffmpeg-LICENSE` ships with the app so the licence text is always present.
3. Include the configure line used. Recorded above, and reproduced by
   `scripts/release/build-ffmpeg-lgpl-macos.mjs` on macOS.
4. Preserve the user's ability to relink against a modified FFmpeg. Satisfied by the
   architecture: the tools are standalone executables and their paths can be
   overridden with `MONOSUB_FFMPEG_PATH` / `MONOSUB_FFPROBE_PATH`.
5. Do not rename the FFmpeg binaries/dylibs to something obfuscated. We keep
   upstream names, suffixed with the target triple only.
6. Repeat 1-5 for every external library compiled in. The Windows package links
   LGPL/BSD/MIT libraries (libdav1d, libopus, libmp3lame, libvorbis, SDL2, …);
   nothing GPL-only is enabled.

### Required attribution text

FFmpeg's checklist asks for these notices. Use them verbatim, with the link
pointing at wherever the corresponding source is hosted:

> This software uses code of <a href="http://ffmpeg.org">FFmpeg</a> licensed under
> the <a href="http://www.gnu.org/licenses/old-licenses/lgpl-2.1.html">LGPLv2.1</a>
> and its source can be downloaded <a href="link_to_your_sources">here</a>

> This software uses libraries from the FFmpeg project under the LGPLv2.1

The second belongs in the app's About box; the first belongs on any page that
offers an app download. If the EULA claims ownership over the code, it must also
state explicitly that MonoSub does not own FFmpeg and where the FFmpeg copyright
holders can be found, and it must not prohibit reverse engineering.

