#!/usr/bin/env python3
"""
FunASR 语音转字幕主程序
通过命令行调用，供 Tauri 使用

用法:
    python main.py <video_path> <output_path> [--format srt|vtt|txt] [--language zh|en]
"""

import sys
import os
import argparse
from pathlib import Path
import tempfile

# CRITICAL FIX #1: Monkey patch subprocess.run to replace ffmpeg/ffprobe commands
# This MUST happen BEFORE any imports that might use subprocess (especially FunASR)
# Reason: FunASR internally calls bare 'ffmpeg' command without checking env vars
import subprocess

_original_run = subprocess.run


def _patched_run(args, *pargs, **kwargs):
    """Intercept subprocess.run and replace bare ffmpeg/ffprobe with env var paths"""
    if isinstance(args, (list, tuple)) and len(args) > 0:
        if args[0] == "ffmpeg":
            ffmpeg_path = os.environ.get("MONOSUB_FFMPEG_PATH", "ffmpeg")
            args = list(args)
            args[0] = ffmpeg_path
        elif args[0] == "ffprobe":
            ffprobe_path = os.environ.get("MONOSUB_FFPROBE_PATH", "ffprobe")
            args = list(args)
            args[0] = ffprobe_path
    return _original_run(args, *pargs, **kwargs)


subprocess.run = _patched_run

# CRITICAL FIX #2: Pre-load torch to avoid circular import issues in PyInstaller
# This must happen BEFORE importing FunASR or any module that uses torch
# Resolves: "partially initialized module 'torch' has no attribute 'autograd'"
if getattr(sys, "frozen", False):
    print("DEBUG: Pre-loading torch to avoid circular imports...", flush=True)
    try:
        import torch

        # Trigger full initialization
        _ = torch.tensor([1.0])
        print("DEBUG: torch pre-loaded successfully", flush=True)
    except Exception as e:
        print(f"WARNING: Failed to pre-load torch: {e}", flush=True)

# 添加当前目录到 Python 路径
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from monosub_core.audio_extractor import AudioExtractor
from monosub_core.transcriber import FunASRTranscriber, ProgressCallback
from monosub_core.subtitle_writer import SubtitleWriter
import time


SUPPORTED_AUDIO_EXTENSIONS = {".mp3", ".wav", ".flac", ".aac", ".m4a", ".ogg", ".wma"}


def is_audio_input_file(path: str) -> bool:
    """Check if the input file extension is an audio format."""
    return Path(path).suffix.lower() in SUPPORTED_AUDIO_EXTENSIONS


class StepTracker:
    """Track processing steps and elapsed time"""

    STEPS = ["Extracting audio", "Loading model", "Transcribing", "Writing subtitle"]

    def __init__(self):
        self.start_time = time.time()
        self.step_start_time = self.start_time
        self.current_step = 0
        self.total_steps = len(self.STEPS)

    def emit_step(self, step_num: int, step_name: str = None):
        """Emit current step info"""
        self.current_step = step_num
        if step_name is None:
            step_name = self.STEPS[step_num - 1] if step_num <= len(self.STEPS) else "Processing"
        self.step_start_time = time.time()
        total_elapsed = time.time() - self.start_time
        print(f"STEP:{step_num}/{self.total_steps}:{step_name}", flush=True)
        print(f"ELAPSED:{total_elapsed:.1f}", flush=True)

    def emit_elapsed(self):
        """Emit current elapsed time"""
        step_elapsed = time.time() - self.step_start_time
        total_elapsed = time.time() - self.start_time
        print(f"STEP_ELAPSED:{step_elapsed:.1f}", flush=True)
        print(f"ELAPSED:{total_elapsed:.1f}", flush=True)

    def get_total_time(self) -> float:
        """Get total processing time"""
        return time.time() - self.start_time


_LICENSE_HMAC_KEY = b"ms_lic_v1_7f3a9c2e5b8d4f1a6e0c3b9d2f5a8e1c"


def _hmac_hex(payload: str) -> str:
    import hmac as _hmac
    import hashlib

    return _hmac.new(_LICENSE_HMAC_KEY, payload.encode(), hashlib.sha256).hexdigest()


def _xor_crypt(data: bytes) -> bytes:
    """HMAC-SHA256 stream cipher (CTR mode) — mirrors the Rust xor_crypt function."""
    import hmac as _hmac
    import hashlib

    result = []
    for block_idx, i in enumerate(range(0, len(data), 32)):
        chunk = data[i : i + 32]
        keystream = _hmac.new(
            _LICENSE_HMAC_KEY,
            b"enc:" + block_idx.to_bytes(4, "big"),
            hashlib.sha256,
        ).digest()
        result.append(bytes(b ^ k for b, k in zip(chunk, keystream)))
    return b"".join(result)


def _is_pro_licensed() -> bool:
    """Check if a valid Pro license has been previously validated by the app."""
    import json
    import hmac as _hmac

    state_path = Path.home() / ".monosub" / "license_state.json"
    try:
        raw = state_path.read_bytes().strip()
        # New encrypted format: hex string
        # Old plaintext format (starts with '{') is rejected — user must re-validate
        if raw.startswith(b"{"):
            return False
        decrypted = _xor_crypt(bytes.fromhex(raw.decode("ascii")))
        state = json.loads(decrypted.decode("utf-8"))
        if state.get("valid") is not True:
            return False
        validated_at = state.get("validated_at", "")
        expires_at_str = state.get("expires_at") or ""
        activation_id_str = state.get("activation_id") or ""
        stored_sig = state.get("sig", "")
        payload = f"1|{validated_at}|{expires_at_str}|{activation_id_str}"
        return _hmac.compare_digest(stored_sig, _hmac_hex(payload))
    except Exception:
        return False


def main():
    """主函数"""
    supported_devices = {"auto", "cpu", "cuda", "mps"}
    default_device = os.environ.get("MONOSUB_ASR_DEVICE", "auto").strip().lower()
    if default_device not in supported_devices:
        default_device = "auto"

    parser = argparse.ArgumentParser(description="FunASR Voice to Subtitle")
    parser.add_argument("video_path", type=str, help="Input video file path")
    parser.add_argument("output_path", type=str, help="Output subtitle file path")
    parser.add_argument(
        "--format",
        type=str,
        default="srt",
        choices=["srt", "vtt", "txt"],
        help="Subtitle format (default: srt)",
    )
    parser.add_argument(
        "--language",
        type=str,
        default="auto",
        choices=["auto", "en", "zh", "es", "ja", "de", "fr", "pt", "zh-tw", "ko"],
        help="Language (default: auto)",
    )
    parser.add_argument(
        "--diarization",
        action="store_true",
        help="Enable speaker diarization (multi-speaker recognition)",
    )
    parser.add_argument(
        "--device",
        type=str,
        default=default_device,
        choices=["auto", "cpu", "cuda", "mps"],
        help="Inference device (default: auto, can be overridden by MONOSUB_ASR_DEVICE)",
    )

    args = parser.parse_args()

    if args.diarization and not _is_pro_licensed():
        print(
            "ERROR:Multi-speaker recognition requires a valid MonoSub Pro license. "
            "Please validate your license in the app.",
            flush=True,
        )
        sys.exit(1)

    tracker = StepTracker()

    try:
        # 验证输入文件
        if not os.path.exists(args.video_path):
            print(f"ERROR:Video file not found: {args.video_path}", flush=True)
            sys.exit(1)

        print(f"MSG:Starting processing: {Path(args.video_path).name}", flush=True)

        # Step 1: 准备音频（音频文件优先直通，视频文件提取音轨）
        tracker.emit_step(1, "Extracting audio")

        extractor = AudioExtractor()
        input_is_audio = is_audio_input_file(args.video_path)
        audio_path = args.video_path
        cleanup_paths = []

        if input_is_audio:
            print(
                "MSG:Audio input detected, trying direct transcription without conversion",
                flush=True,
            )
        else:
            audio_path = extractor.extract_audio(args.video_path)
        tracker.emit_elapsed()

        # Step 2: 加载模型
        tracker.emit_step(2, "Loading model")

        transcriber = FunASRTranscriber(
            model_name="sensevoice",  # 使用 SenseVoice 模型
            device=args.device,
        )
        print(f"MSG:Using device: {transcriber.device}", flush=True)
        print(
            "MSG:Inference profile "
            f"(batch_size_s={transcriber.batch_size_s}, "
            f"max_single_segment_time_ms={transcriber.max_single_segment_time_ms}, "
            f"merge_length_s={transcriber.merge_length_s})",
            flush=True,
        )

        # 创建一个不发送 PROGRESS 的回调
        progress_callback = ProgressCallback(callback=lambda p, m: None)
        tracker.emit_elapsed()

        # Step 3: 转录音频
        tracker.emit_step(3, "Transcribing")

        def run_transcription(target_audio_path: str):
            if args.diarization:
                print("MSG:Multi-speaker mode enabled", flush=True)
                return transcriber.transcribe_with_diarization(
                    target_audio_path,
                    progress_callback,
                    args.language,
                )
            return transcriber.transcribe(target_audio_path, progress_callback, args.language)

        try:
            segments = run_transcription(audio_path)
        except Exception as direct_error:
            # 音频文件直通失败时，自动回退为标准 16k mono WAV 后重试
            if input_is_audio and audio_path == args.video_path:
                print(f"DEBUG: Direct audio transcription failed: {direct_error}", flush=True)
                input_path = Path(args.video_path)
                temp_dir = Path(tempfile.gettempdir())
                is_hidden_audio_cache_wav = (
                    input_path.suffix.lower() == ".wav" and input_path.name.endswith("_audio.wav")
                )
                is_rust_preextracted_wav = (
                    input_path.suffix.lower() == ".wav"
                    and input_path.parent == temp_dir
                    and input_path.name.startswith("extracted_")
                )
                if is_rust_preextracted_wav or is_hidden_audio_cache_wav:
                    # Rust 端已预提取为 16k mono WAV 时，避免再次昂贵转换
                    print(
                        "MSG:Pre-extracted WAV transcription failed, skipping reconversion",
                        flush=True,
                    )
                    raise

                print("MSG:Direct transcription failed, falling back to WAV conversion", flush=True)

                fallback_audio_path = os.path.join(
                    tempfile.gettempdir(),
                    f"monosub_fallback_{Path(args.video_path).stem}_{int(time.time() * 1000)}.wav",
                )
                audio_path = extractor.extract_audio(
                    args.video_path,
                    output_audio_path=fallback_audio_path,
                    enable_preprocessing=False,
                    normalize_volume=False,
                    noise_reduction=False,
                    use_cache=False,
                )
                cleanup_paths.append(audio_path)
                print("MSG:Retrying transcription with converted WAV audio", flush=True)
                segments = run_transcription(audio_path)
            else:
                raise

        print(f"MSG:{len(segments)} segments found", flush=True)
        tracker.emit_elapsed()

        # Step 4: 写入字幕文件
        tracker.emit_step(4, "Writing subtitle")

        # 确保输出目录存在
        output_dir = os.path.dirname(args.output_path)
        if output_dir and not os.path.exists(output_dir):
            os.makedirs(output_dir, exist_ok=True)

        SubtitleWriter.write(segments, args.output_path, args.format)
        tracker.emit_elapsed()

        # 完成
        total_time = tracker.get_total_time()
        print(f"TOTAL_TIME:{total_time:.1f}", flush=True)
        print(f"SUCCESS:{args.output_path}", flush=True)

        # 清理临时音频文件
        for temp_path in cleanup_paths:
            if os.path.exists(temp_path):
                try:
                    os.remove(temp_path)
                except Exception:
                    pass

        if os.path.exists(audio_path) and audio_path.startswith("/tmp/"):
            try:
                os.remove(audio_path)
            except Exception:
                pass

    except Exception as e:
        error_msg = str(e)
        print(f"ERROR:{error_msg}", flush=True)
        sys.exit(1)


if __name__ == "__main__":
    main()
