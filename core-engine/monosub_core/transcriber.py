"""
FunASR 转录器
使用 FunASR 进行语音识别
"""
import sys
from typing import List, Dict, Callable, Optional
from funasr import AutoModel


class ProgressCallback:
    """进度回调"""

    def __init__(self, callback: Optional[Callable[[float, str], None]] = None):
        self.callback = callback

    def update(self, progress: float, message: str = ""):
        """
        更新进度

        Args:
            progress: 进度百分比 (0-100)
            message: 进度消息
        """
        if self.callback:
            self.callback(progress, message)
        else:
            # 默认输出到 stdout，供 Rust 读取
            print(f"PROGRESS:{progress:.1f}", flush=True)
            if message:
                print(f"MSG:{message}", flush=True)


class FunASRTranscriber:
    """FunASR 转录器"""

    def __init__(self, model_name: str = "paraformer-zh", device: str = "cpu"):
        """
        初始化转录器

        Args:
            model_name: 模型名称
            device: 设备 (cpu 或 cuda)
        """
        self.model_name = model_name
        self.device = device
        self.model = None

    def load_model(self, progress_callback: ProgressCallback):
        """加载模型"""
        progress_callback.update(5, "Loading model...")

        try:
            # 加载 FunASR 模型
            self.model = AutoModel(
                model=self.model_name,
                device=self.device,
                # 可以指定本地模型路径
                # model_dir="./models/paraformer-zh"
            )

            progress_callback.update(15, "Model loaded")

        except Exception as e:
            raise RuntimeError(f"Failed to load model: {str(e)}")

    def transcribe(
        self,
        audio_path: str,
        progress_callback: ProgressCallback,
        language: str = "zh"
    ) -> List[Dict]:
        """
        转录音频

        Args:
            audio_path: 音频文件路径
            progress_callback: 进度回调
            language: 语言 (zh, en)

        Returns:
            转录结果列表，每个元素包含 start, end, text
        """
        if not self.model:
            self.load_model(progress_callback)

        progress_callback.update(20, "Starting transcription...")

        try:
            # 使用 FunASR 进行转录
            result = self.model.generate(
                input=audio_path,
                batch_size_s=300,  # 批处理大小（秒）
                hotword='',  # 热词
            )

            progress_callback.update(80, "Processing results...")

            # 解析 FunASR 结果
            segments = self._parse_result(result)

            progress_callback.update(95, "Transcription complete")

            return segments

        except Exception as e:
            raise RuntimeError(f"Transcription failed: {str(e)}")

    def _parse_result(self, result) -> List[Dict]:
        """
        解析 FunASR 结果为字幕片段

        Args:
            result: FunASR 返回结果

        Returns:
            字幕片段列表
        """
        segments = []

        # FunASR 返回格式可能因版本而异，这里是一个示例
        if isinstance(result, list) and len(result) > 0:
            for item in result:
                # 检查是否有时间戳信息
                if 'timestamp' in item:
                    # 有时间戳的情况
                    timestamps = item.get('timestamp', [])
                    text = item.get('text', '')

                    if timestamps:
                        for ts in timestamps:
                            segments.append({
                                'start': ts[0] / 1000.0,  # 转换为秒
                                'end': ts[1] / 1000.0,
                                'text': ts[2] if len(ts) > 2 else text
                            })
                else:
                    # 没有时间戳的情况，使用整段文本
                    text = item.get('text', '')
                    if text:
                        segments.append({
                            'start': 0.0,
                            'end': 0.0,  # 需要根据音频长度计算
                            'text': text
                        })

        # 如果没有时间戳，需要估算时间
        if segments and segments[0]['end'] == 0.0:
            segments = self._estimate_timestamps(segments)

        return segments

    def _estimate_timestamps(self, segments: List[Dict]) -> List[Dict]:
        """
        估算时间戳（当 FunASR 不返回时间戳时）

        Args:
            segments: 字幕片段

        Returns:
            添加了时间戳的片段
        """
        # 简单实现：假设每个字符 0.15 秒
        # 实际应该根据音频长度和文本长度计算
        current_time = 0.0

        for segment in segments:
            text_length = len(segment['text'])
            duration = text_length * 0.15  # 每个字符约 0.15 秒

            segment['start'] = current_time
            segment['end'] = current_time + duration
            current_time = segment['end'] + 0.5  # 片段间隔 0.5 秒

        return segments
