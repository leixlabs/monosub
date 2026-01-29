"""
音频提取器
从视频文件中提取音频，支持音频预处理优化
"""
import os
import subprocess
from pathlib import Path
from typing import Optional


def get_ffmpeg_command() -> str:
    """
    获取 ffmpeg 命令路径
    优先使用环境变量 MONOSUB_FFMPEG_PATH，否则使用系统 ffmpeg
    """
    return os.environ.get('MONOSUB_FFMPEG_PATH', 'ffmpeg')


def get_ffprobe_command() -> str:
    """
    获取 ffprobe 命令路径
    优先使用环境变量 MONOSUB_FFPROBE_PATH，否则使用系统 ffprobe
    """
    return os.environ.get('MONOSUB_FFPROBE_PATH', 'ffprobe')


class AudioExtractor:
    """从视频提取音频，包含预处理优化"""

    @staticmethod
    def extract_audio(
        video_path: str,
        output_audio_path: Optional[str] = None,
        enable_preprocessing: bool = True,
        normalize_volume: bool = True,
        noise_reduction: bool = True
    ) -> str:
        """
        使用 ffmpeg 从视频中提取音频，支持预处理优化

        Args:
            video_path: 视频文件路径
            output_audio_path: 输出音频路径，如果为 None 则使用临时文件
            enable_preprocessing: 是否启用音频预处理
            normalize_volume: 是否进行音量标准化
            noise_reduction: 是否进行降噪处理

        Returns:
            音频文件路径
        """
        if output_audio_path is None:
            # 创建临时音频文件路径
            video_name = Path(video_path).stem
            output_audio_path = f"/tmp/{video_name}_audio.wav"

        # 构建 ffmpeg 滤镜链
        audio_filters = []

        if enable_preprocessing:
            # 1. 高通滤波器 - 去除低频噪音（如空调声、交通声）
            audio_filters.append("highpass=f=80")

            # 2. 低通滤波器 - 去除高频噪音
            audio_filters.append("lowpass=f=8000")

            if noise_reduction:
                # 3. 降噪处理 - 使用 afftdn (FFT 降噪)
                # nf: 噪声基底 (-25dB), nr: 降噪量 (12dB), nt: 噪声类型 (white)
                audio_filters.append("afftdn=nf=-25:nr=12:nt=w")

            if normalize_volume:
                # 4. 音量标准化 - loudnorm 符合 EBU R128 标准
                # I: 目标响度 (-16 LUFS), TP: 真峰值限制 (-1.5 dBTP), LRA: 响度范围 (11 LU)
                audio_filters.append("loudnorm=I=-16:TP=-1.5:LRA=11")

        # 使用 ffmpeg 提取并处理音频
        # -vn: 禁用视频
        # -acodec pcm_s16le: 使用 PCM 16-bit 编码
        # -ar 16000: 采样率 16kHz (FunASR 推荐)
        # -ac 1: 单声道
        cmd = [
            get_ffmpeg_command(),
            '-i', video_path,
            '-vn',  # 禁用视频
        ]

        # 添加音频滤镜
        if audio_filters:
            cmd.extend(['-af', ','.join(audio_filters)])

        cmd.extend([
            '-acodec', 'pcm_s16le',  # PCM 编码
            '-ar', '16000',  # 16kHz 采样率
            '-ac', '1',  # 单声道
            '-y',  # 覆盖已存在的文件
            output_audio_path
        ])

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
            # 如果高级滤镜失败，回退到基本提取
            print(f"DEBUG: Advanced audio processing failed, falling back to basic: {e.stderr.decode()[:200]}", flush=True)
            return AudioExtractor._extract_basic(video_path, output_audio_path)
        except Exception as e:
            raise RuntimeError(f"Audio extraction failed: {str(e)}")

    @staticmethod
    def _extract_basic(video_path: str, output_audio_path: str) -> str:
        """基本音频提取（无预处理）"""
        cmd = [
            get_ffmpeg_command(),
            '-i', video_path,
            '-vn',
            '-acodec', 'pcm_s16le',
            '-ar', '16000',
            '-ac', '1',
            '-y',
            output_audio_path
        ]

        try:
            subprocess.run(
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
            get_ffprobe_command(),
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

    @staticmethod
    def get_audio_info(audio_path: str) -> dict:
        """
        获取音频详细信息

        Args:
            audio_path: 音频文件路径

        Returns:
            包含 duration, sample_rate, channels, bitrate 的字典
        """
        cmd = [
            get_ffprobe_command(),
            '-v', 'error',
            '-select_streams', 'a:0',
            '-show_entries', 'stream=sample_rate,channels,bit_rate:format=duration',
            '-of', 'json',
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

            import json
            data = json.loads(result.stdout)

            info = {
                'duration': float(data.get('format', {}).get('duration', 0)),
                'sample_rate': int(data.get('streams', [{}])[0].get('sample_rate', 16000)),
                'channels': int(data.get('streams', [{}])[0].get('channels', 1)),
                'bitrate': int(data.get('streams', [{}])[0].get('bit_rate', 0) or 0),
            }

            return info

        except Exception as e:
            return {'duration': 0, 'sample_rate': 16000, 'channels': 1, 'bitrate': 0}
