"""
音频提取器
从视频文件中提取音频
"""
import os
import subprocess
from pathlib import Path


class AudioExtractor:
    """从视频提取音频"""

    @staticmethod
    def extract_audio(video_path: str, output_audio_path: str = None) -> str:
        """
        使用 ffmpeg 从视频中提取音频

        Args:
            video_path: 视频文件路径
            output_audio_path: 输出音频路径，如果为 None 则使用临时文件

        Returns:
            音频文件路径
        """
        if output_audio_path is None:
            # 创建临时音频文件路径
            video_name = Path(video_path).stem
            output_audio_path = f"/tmp/{video_name}_audio.wav"

        # 使用 ffmpeg 提取音频
        # -vn: 禁用视频
        # -acodec pcm_s16le: 使用 PCM 16-bit 编码
        # -ar 16000: 采样率 16kHz (FunASR 常用)
        # -ac 1: 单声道
        cmd = [
            'ffmpeg',
            '-i', video_path,
            '-vn',  # 禁用视频
            '-acodec', 'pcm_s16le',  # PCM 编码
            '-ar', '16000',  # 16kHz 采样率
            '-ac', '1',  # 单声道
            '-y',  # 覆盖已存在的文件
            output_audio_path
        ]

        try:
            # 执行 ffmpeg 命令
            result = subprocess.run(
                cmd,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                check=True
            )

            if not os.path.exists(output_audio_path):
                raise RuntimeError("Audio extraction failed: output file not created")

            return output_audio_path

        except subprocess.CalledProcessError as e:
            raise RuntimeError(f"FFmpeg error: {e.stderr.decode()}")
        except Exception as e:
            raise RuntimeError(f"Audio extraction failed: {str(e)}")

    @staticmethod
    def get_audio_duration(audio_path: str) -> float:
        """
        获取音频时长（秒）

        Args:
            audio_path: 音频文件路径

        Returns:
            音频时长（秒）
        """
        cmd = [
            'ffprobe',
            '-v', 'error',
            '-show_entries', 'format=duration',
            '-of', 'default=noprint_wrappers=1:nokey=1',
            audio_path
        ]

        try:
            result = subprocess.run(
                cmd,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                check=True,
                text=True
            )

            duration = float(result.stdout.strip())
            return duration

        except Exception as e:
            raise RuntimeError(f"Failed to get audio duration: {str(e)}")
