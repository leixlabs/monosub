"""
Speaker Diarization using ModelScope CAM++ model
识别音频中的多个说话人
"""

from typing import List, Tuple, Dict, Optional, Callable
import platform


def get_optimal_device() -> str:
    """自动检测最优设备"""
    try:
        import torch

        if platform.system() == 'Darwin':
            if hasattr(torch.backends, 'mps') and torch.backends.mps.is_available():
                return 'mps'

        if torch.cuda.is_available():
            return 'cuda'

    except ImportError:
        pass

    return 'cpu'


class SpeakerDiarizer:
    """
    Speaker diarization using iic/speech_campplus_speaker-diarization_common
    
    识别"谁在什么时间说话"，输出 (start, end, speaker_id) 列表
    """

    MODEL_ID = "iic/speech_campplus_speaker-diarization_common"
    MODEL_REVISION = "v1.0.0"

    def __init__(self, device: str = "auto"):
        """
        初始化说话人分离器

        Args:
            device: 设备 (auto, cpu, cuda, mps)
        """
        if device == "auto":
            self.device = get_optimal_device()
        else:
            self.device = device

        self.pipeline = None
        print(f"DEBUG: SpeakerDiarizer initialized with device={self.device}", flush=True)

    def load_model(self, progress_callback: Optional[Callable[[float, str], None]] = None):
        """加载 CAM++ 说话人分离模型 (使用 ModelScope pipeline)"""
        if progress_callback:
            progress_callback(5, "Loading speaker diarization model...")

        print(f"DEBUG: Loading diarization model via ModelScope pipeline: {self.MODEL_ID}", flush=True)

        from modelscope.pipelines import pipeline
        from modelscope.utils.constant import Tasks

        self.pipeline = pipeline(
            task=Tasks.speaker_diarization,
            model=self.MODEL_ID,
        )

        print("DEBUG: Diarization model loaded successfully", flush=True)

    def diarize(self, audio_path: str) -> List[Tuple[float, float, str]]:
        """
        对音频进行说话人分离

        Args:
            audio_path: 音频文件路径

        Returns:
            List of (start_seconds, end_seconds, speaker_id) tuples
            Example: [(0.0, 3.5, 'spk_0'), (3.8, 6.2, 'spk_1'), ...]
        """
        if not self.pipeline:
            self.load_model()

        print(f"DEBUG: Running diarization on: {audio_path}", flush=True)

        # ModelScope pipeline uses audio parameter
        result = self.pipeline(audio=audio_path)

        print(f"DEBUG: Diarization raw result type: {type(result)}", flush=True)
        if result:
            print(f"DEBUG: Diarization result preview: {str(result)[:500]}", flush=True)

        # 解析结果
        segments = self._parse_diarization_result(result)

        print(f"DEBUG: Diarization found {len(segments)} segments", flush=True)
        if segments:
            # 统计说话人数量
            speakers = set(s[2] for s in segments)
            print(f"DEBUG: Detected {len(speakers)} speakers: {speakers}", flush=True)

        return segments

    def _parse_diarization_result(self, result) -> List[Tuple[float, float, str]]:
        """
        解析说话人分离结果

        不同版本的 FunASR 返回格式可能不同，这里处理多种情况
        """
        segments = []

        if result is None:
            return segments

        # 情况1: 直接返回列表 [[start, end, speaker], ...]
        if isinstance(result, list):
            for item in result:
                if isinstance(item, (list, tuple)) and len(item) >= 3:
                    start, end, spk = item[0], item[1], item[2]
                    segments.append((float(start), float(end), str(spk)))
                elif isinstance(item, dict):
                    # 情况2: 列表中包含字典
                    start = item.get('start', item.get('begin', 0))
                    end = item.get('end', 0)
                    spk = item.get('speaker', item.get('spk', 'unknown'))
                    if start is not None and end is not None:
                        segments.append((float(start), float(end), str(spk)))

        # 情况3: 返回字典，包含 segments 或 result 键
        elif isinstance(result, dict):
            if 'segments' in result:
                return self._parse_diarization_result(result['segments'])
            elif 'result' in result:
                return self._parse_diarization_result(result['result'])

        return segments

    @staticmethod
    def assign_speakers(
        asr_segments: List[Dict],
        diarization_segments: List[Tuple[float, float, str]]
    ) -> List[Dict]:
        """
        根据时间戳重叠将说话人分配给 ASR 片段

        Args:
            asr_segments: ASR 输出的 [{start, end, text}, ...]
            diarization_segments: 说话人分离输出的 [(start, end, speaker_id), ...]

        Returns:
            添加了 speaker 字段的 ASR 片段列表
        """
        if not diarization_segments:
            print("DEBUG: No diarization segments, skipping speaker assignment", flush=True)
            return asr_segments

        # 将原始 speaker_id 映射为可读的标签 (A, B, C, ...)
        speaker_map = {}
        speaker_counter = 0

        # 先按出现顺序建立映射
        for _, _, spk_id in diarization_segments:
            if spk_id not in speaker_map:
                speaker_map[spk_id] = chr(ord('A') + speaker_counter)
                speaker_counter += 1
                if speaker_counter > 25:  # 超过 Z 就停止
                    speaker_counter = 25

        print(f"DEBUG: Speaker mapping: {speaker_map}", flush=True)

        # 为每个 ASR 片段分配说话人
        for seg in asr_segments:
            seg_start = seg.get('start', 0)
            seg_end = seg.get('end', 0)

            # 找到最大重叠的说话人
            best_speaker = None
            best_overlap = 0

            for d_start, d_end, spk_id in diarization_segments:
                # 计算时间重叠
                overlap_start = max(seg_start, d_start)
                overlap_end = min(seg_end, d_end)
                overlap = max(0, overlap_end - overlap_start)

                if overlap > best_overlap:
                    best_overlap = overlap
                    best_speaker = spk_id

            # 分配说话人标签
            if best_speaker and best_speaker in speaker_map:
                seg['speaker'] = f"Speaker {speaker_map[best_speaker]}"
            else:
                seg['speaker'] = None

        return asr_segments

    @staticmethod
    def should_show_speaker_labels(segments: List[Dict]) -> bool:
        """
        判断是否应该显示说话人标签

        如果只检测到一个说话人，则不显示标签以保持输出简洁

        Args:
            segments: 带有 speaker 字段的片段列表

        Returns:
            True 如果应该显示标签，False 如果应该隐藏
        """
        unique_speakers = set(
            s.get('speaker') for s in segments 
            if s.get('speaker') is not None
        )
        return len(unique_speakers) > 1
