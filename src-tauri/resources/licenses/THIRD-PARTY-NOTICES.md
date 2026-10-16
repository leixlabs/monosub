# Third-Party Notices

MonoSub bundles or depends on the following third-party components.

## audio.cpp

- Version: v0.9.1 (git `cf124a67`)
- Source: https://github.com/0xShug0/audio.cpp
- License: Apache License 2.0 — full text in `audio.cpp-LICENSE`
- Bundled as: `bin/audiocpp_cli-*`
- Modifications: none (unmodified upstream binary)

The Apache License 2.0 permits redistribution provided the license text and
copyright notice are included; `audio.cpp-LICENSE` contains both.

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
- License: **CC-BY-NC-4.0** (non-commercial). MonoSub is a personal learning
  project and is not distributed commercially, so this model may be listed for
  local use and attribution. It must be removed from any commercial build.
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

## FFmpeg

- Bundled as: `bin/ffmpeg-*`, `bin/ffprobe-*`, `bin/lib/*`
- Source: https://ffmpeg.org/
- License: see the FFmpeg project; build-specific (LGPL/GPL) terms apply to the
  distributed binaries.
