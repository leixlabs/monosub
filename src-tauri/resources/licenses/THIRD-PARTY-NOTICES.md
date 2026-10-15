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

## FFmpeg

- Bundled as: `bin/ffmpeg-*`, `bin/ffprobe-*`, `bin/lib/*`
- Source: https://ffmpeg.org/
- License: see the FFmpeg project; build-specific (LGPL/GPL) terms apply to the
  distributed binaries.
