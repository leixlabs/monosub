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

# CRITICAL FIX #1: Monkey patch subprocess.run to replace ffmpeg/ffprobe commands
# This MUST happen BEFORE any imports that might use subprocess (especially FunASR)
# Reason: FunASR internally calls bare 'ffmpeg' command without checking env vars
import subprocess
_original_run = subprocess.run

def _patched_run(args, *pargs, **kwargs):
    """Intercept subprocess.run and replace bare ffmpeg/ffprobe with env var paths"""
    if isinstance(args, (list, tuple)) and len(args) > 0:
        if args[0] == 'ffmpeg':
            ffmpeg_path = os.environ.get('MONOSUB_FFMPEG_PATH', 'ffmpeg')
            args = list(args)
            args[0] = ffmpeg_path
        elif args[0] == 'ffprobe':
            ffprobe_path = os.environ.get('MONOSUB_FFPROBE_PATH', 'ffprobe')
            args = list(args)
            args[0] = ffprobe_path
    return _original_run(args, *pargs, **kwargs)

subprocess.run = _patched_run

# CRITICAL FIX #2: Pre-load torch to avoid circular import issues in PyInstaller
# This must happen BEFORE importing FunASR or any module that uses torch
# Resolves: "partially initialized module 'torch' has no attribute 'autograd'"
if getattr(sys, 'frozen', False):
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


def main():
    """主函数"""
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
        "--device",
        type=str,
        default="cpu",
        choices=["cpu", "cuda"],
        help="Device to use (default: cpu)",
    )
    parser.add_argument(
        "--diarization",
        action="store_true",
        help="Enable speaker diarization (multi-speaker recognition)",
    )

    args = parser.parse_args()

    try:
        # 验证输入文件
        if not os.path.exists(args.video_path):
            print(f"ERROR:Video file not found: {args.video_path}", flush=True)
            sys.exit(1)

        print(f"MSG:Starting processing: {Path(args.video_path).name}", flush=True)
        print("PROGRESS:0", flush=True)

        # 1. 提取音频 (0-10%)
        print("MSG:Extracting audio...", flush=True)
        print("PROGRESS:2", flush=True)

        extractor = AudioExtractor()
        audio_path = extractor.extract_audio(args.video_path)

        print(f"MSG:Audio extracted: {audio_path}", flush=True)
        print("PROGRESS:10", flush=True)

        # 2. 转录音频 (10-95%)
        print("MSG:Loading model...", flush=True)

        transcriber = FunASRTranscriber(
            model_name="sensevoice",  # 使用 SenseVoice 模型
            device="auto",  # 自动检测最优设备 (MPS/CUDA/CPU)
        )

        progress_callback = ProgressCallback()

        print("MSG:Transcribing audio...", flush=True)
        
        # 根据是否启用说话人分离选择不同的转录方式
        if args.diarization:
            print("MSG:Multi-speaker mode enabled", flush=True)
            segments = transcriber.transcribe_with_diarization(audio_path, progress_callback, args.language)
        else:
            segments = transcriber.transcribe(audio_path, progress_callback, args.language)

        print(f"MSG:Transcription complete, {len(segments)} segments", flush=True)
        print("PROGRESS:95", flush=True)

        # 3. 写入字幕文件 (95-100%)
        print("MSG:Writing subtitle file...", flush=True)
        print("PROGRESS:97", flush=True)

        # 确保输出目录存在
        output_dir = os.path.dirname(args.output_path)
        if output_dir and not os.path.exists(output_dir):
            os.makedirs(output_dir, exist_ok=True)

        SubtitleWriter.write(segments, args.output_path, args.format)

        print("PROGRESS:100", flush=True)
        print(f"SUCCESS:{args.output_path}", flush=True)

        # 清理临时音频文件
        if os.path.exists(audio_path) and audio_path.startswith("/tmp/"):
            try:
                os.remove(audio_path)
            except:
                pass

    except Exception as e:
        error_msg = str(e)
        print(f"ERROR:{error_msg}", flush=True)
        sys.exit(1)


if __name__ == "__main__":
    main()
