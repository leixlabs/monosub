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
        "--language", type=str, default="zh", choices=["zh", "en"], help="Language (default: zh)"
    )
    parser.add_argument(
        "--device",
        type=str,
        default="cpu",
        choices=["cpu", "cuda"],
        help="Device to use (default: cpu)",
    )

    args = parser.parse_args()

    try:
        # 验证输入文件
        if not os.path.exists(args.video_path):
            print(f"ERROR:Video file not found: {args.video_path}", flush=True)
            sys.exit(1)

        print(f"MSG:Starting processing: {Path(args.video_path).name}", flush=True)
        print("PROGRESS:0", flush=True)

        # 1. 提取音频
        print("MSG:Extracting audio...", flush=True)
        print("PROGRESS:5", flush=True)

        extractor = AudioExtractor()
        audio_path = extractor.extract_audio(args.video_path)

        print(f"MSG:Audio extracted: {audio_path}", flush=True)
        print("PROGRESS:15", flush=True)

        # 2. 转录音频
        print("MSG:Loading model...", flush=True)

        transcriber = FunASRTranscriber(
            model_name="paraformer-zh" if args.language == "zh" else "paraformer-en",
            device=args.device,
        )

        progress_callback = ProgressCallback()

        print("MSG:Transcribing audio...", flush=True)
        segments = transcriber.transcribe(audio_path, progress_callback, args.language)

        print(f"MSG:Transcription complete, {len(segments)} segments", flush=True)
        print("PROGRESS:90", flush=True)

        # 3. 写入字幕文件
        print("MSG:Writing subtitle file...", flush=True)

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
