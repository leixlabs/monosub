"""
FunASR 转录器
使用 FunASR 进行语音识别
支持 Paraformer 和 SenseVoice 模型
针对 Mac 优化 (MPS 加速)
"""
import re
import platform
import os
from typing import List, Dict, Callable, Optional
from funasr import AutoModel


def get_optimal_device() -> str:
    """
    自动检测最优设备

    Returns:
        'mps' - Mac Apple Silicon (M1/M2/M3)
        'cuda' - NVIDIA GPU
        'cpu' - 默认 CPU
    """
    try:
        import torch

        # 优先检测 Mac MPS (Metal Performance Shaders)
        if platform.system() == 'Darwin':
            if hasattr(torch.backends, 'mps') and torch.backends.mps.is_available():
                print("DEBUG: MPS (Metal) acceleration available", flush=True)
                return 'mps'

        # 检测 CUDA
        if torch.cuda.is_available():
            print(f"DEBUG: CUDA acceleration available: {torch.cuda.get_device_name(0)}", flush=True)
            return 'cuda'

    except ImportError:
        pass

    print("DEBUG: Using CPU for inference", flush=True)
    return 'cpu'


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
    """FunASR 转录器 - 支持多种模型配置和 Mac MPS 加速"""

    DEFAULT_INFERENCE_PROFILES = {
        # MPS 对超大矩阵更敏感，保守配置避免 Invalid buffer size
        "mps": {"batch_size_s": 90, "max_single_segment_time_ms": 12000, "merge_length_s": 8},
        # CUDA 通常可承受更高吞吐
        "cuda": {"batch_size_s": 240, "max_single_segment_time_ms": 30000, "merge_length_s": 15},
        # CPU 走稳态配置
        "cpu": {"batch_size_s": 120, "max_single_segment_time_ms": 15000, "merge_length_s": 10},
    }

    # 预定义模型配置
    MODEL_CONFIGS = {
        # Paraformer 系列 - 传统ASR模型
        "paraformer-zh": {
            "model": "paraformer-zh",
            "vad_model": "fsmn-vad",
            "punc_model": "ct-punc",
            "use_itn": False,
            "type": "paraformer"
        },
        # SenseVoice 系列 - 2024年新模型，多语言+情感+事件检测
        "sensevoice": {
            "model": "iic/SenseVoiceSmall",
            "vad_model": "fsmn-vad",
            "punc_model": None,  # SenseVoice 内置标点
            "use_itn": True,
            "type": "sensevoice"
        },
        # Paraformer Large - 高精度模型
        "paraformer-large": {
            "model": "iic/speech_paraformer-large-vad-punc_asr_nat-zh-cn-16k-common-vocab8404-pytorch",
            "vad_model": None,  # 内置VAD
            "punc_model": None,  # 内置Punc
            "use_itn": False,
            "type": "paraformer"
        }
    }

    # 字幕合并参数
    MERGE_CONFIG = {
        'min_duration': 0.8,  # 最小字幕时长（秒）
        'max_duration': 10.0,  # 最大字幕时长（秒）
        'max_chars': 50,  # 单条字幕最大字符数
        'gap_threshold': 0.5,  # 合并间隔阈值（秒）
    }

    def __init__(
        self,
        model_name: str = "paraformer-zh",
        device: str = "auto",
        use_vad: bool = True,
        use_punc: bool = True,
        vad_model: Optional[str] = "fsmn-vad",
        punc_model: Optional[str] = "ct-punc"
    ):
        """
        初始化转录器

        Args:
            model_name: 模型名称 (paraformer-zh, sensevoice, paraformer-large) 或自定义模型路径
            device: 设备 (auto, cpu, cuda, mps)
            use_vad: 是否使用 VAD 模型进行语音活动检测
            use_punc: 是否使用标点预测模型
            vad_model: VAD 模型名称 (仅在使用自定义模型时生效)
            punc_model: 标点预测模型名称 (仅在使用自定义模型时生效)
        """
        # 自动检测最优设备
        if device == "auto":
            self.device = get_optimal_device()
        else:
            normalized_device = (device or "").strip().lower()
            if normalized_device in {"cpu", "cuda", "mps"}:
                self.device = normalized_device
            else:
                print(f"DEBUG: Unsupported device '{device}', fallback to cpu", flush=True)
                self.device = "cpu"

        self.use_vad = use_vad
        self.use_punc = use_punc
        self.model = None
        self._resolve_inference_profile()

        # 解析模型配置
        if model_name in self.MODEL_CONFIGS:
            config = self.MODEL_CONFIGS[model_name]
            self.model_name = config["model"]
            self.vad_model_name = config["vad_model"] if use_vad else None
            self.punc_model_name = config["punc_model"] if use_punc else None
            self.use_itn = config["use_itn"]
            self.model_type = config["type"]
        else:
            # 自定义模型配置
            self.model_name = model_name
            self.vad_model_name = vad_model if use_vad else None
            self.punc_model_name = punc_model if use_punc else None
            self.use_itn = False
            self.model_type = "paraformer"

        print(f"DEBUG: Model config - model={self.model_name}, vad={self.vad_model_name}, punc={self.punc_model_name}, type={self.model_type}, device={self.device}", flush=True)
        print(
            "DEBUG: Inference profile - "
            f"batch_size_s={self.batch_size_s}, "
            f"max_single_segment_time_ms={self.max_single_segment_time_ms}, "
            f"merge_length_s={self.merge_length_s}",
            flush=True,
        )

    @staticmethod
    def _read_positive_int_env(name: str, default: int) -> int:
        raw_value = os.environ.get(name)
        if not raw_value:
            return default

        try:
            parsed = int(raw_value)
            if parsed > 0:
                return parsed
        except ValueError:
            pass

        print(f"DEBUG: Invalid env {name}={raw_value}, using default={default}", flush=True)
        return default

    def _resolve_inference_profile(self):
        base_profile = self.DEFAULT_INFERENCE_PROFILES.get(
            self.device,
            self.DEFAULT_INFERENCE_PROFILES["cpu"],
        ).copy()

        self.batch_size_s = self._read_positive_int_env(
            "MONOSUB_BATCH_SIZE_S",
            base_profile["batch_size_s"],
        )
        self.max_single_segment_time_ms = self._read_positive_int_env(
            "MONOSUB_MAX_SINGLE_SEGMENT_TIME_MS",
            base_profile["max_single_segment_time_ms"],
        )
        self.merge_length_s = self._read_positive_int_env(
            "MONOSUB_MERGE_LENGTH_S",
            base_profile["merge_length_s"],
        )

    def load_model(self, progress_callback: ProgressCallback):
        """加载模型 - VAD和Punc在初始化时一起加载"""
        progress_callback.update(12, "Loading models...")

        try:
            # 构建模型参数 - 关键修复：VAD和Punc在AutoModel初始化时传入模型名称
            model_kwargs = {
                "model": self.model_name,
                "device": self.device,
            }

            # 添加VAD模型 (传模型名称字符串，不是模型对象！)
            if self.vad_model_name:
                model_kwargs["vad_model"] = self.vad_model_name
                model_kwargs["vad_kwargs"] = {
                    "max_single_segment_time": self.max_single_segment_time_ms
                }

            # 添加标点模型 (传模型名称字符串，不是模型对象！)
            if self.punc_model_name:
                model_kwargs["punc_model"] = self.punc_model_name

            print(f"DEBUG: Loading model with kwargs: {model_kwargs}", flush=True)

            # 一次性加载所有模型
            self.model = AutoModel(**model_kwargs)

            progress_callback.update(25, "All models loaded")

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
            language: 语言 (zh, en, auto)

        Returns:
            转录结果列表，每个元素包含 start, end, text
        """
        if not self.model:
            self.load_model(progress_callback)

        progress_callback.update(30, "Starting transcription...")

        try:
            # 构建generate参数
            generate_kwargs = {
                "input": audio_path,
                "batch_size_s": self.batch_size_s,
            }

            # SenseVoice 特有参数
            if self.model_type == "sensevoice":
                generate_kwargs["language"] = "auto" if language == "auto" else language
                generate_kwargs["use_itn"] = self.use_itn
                generate_kwargs["merge_vad"] = True
                generate_kwargs["merge_length_s"] = self.merge_length_s

            print(f"DEBUG: Calling generate with kwargs: {generate_kwargs}", flush=True)

            # 执行转录
            result = self.model.generate(**generate_kwargs)

            print(f"DEBUG: Raw result type: {type(result)}", flush=True)
            if isinstance(result, list) and len(result) > 0:
                print(f"DEBUG: First result keys: {result[0].keys() if isinstance(result[0], dict) else 'not dict'}", flush=True)
                if isinstance(result[0], dict):
                    print(f"DEBUG: First result text preview: {result[0].get('text', '')[:200]}", flush=True)
                    if 'timestamp' in result[0]:
                        ts = result[0]['timestamp']
                        print(f"DEBUG: Timestamp count: {len(ts) if ts else 0}", flush=True)
                        if ts and len(ts) > 0:
                            print(f"DEBUG: First few timestamps: {ts[:3]}", flush=True)

            progress_callback.update(85, "Processing results...")

            # 解析结果
            segments = self._parse_result(result)

            # 合并过短字幕
            segments = self._merge_short_segments(segments)

            progress_callback.update(92, "Transcription complete")

            return segments

        except Exception as e:
            import traceback
            traceback.print_exc()
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

        if not isinstance(result, list) or len(result) == 0:
            return segments

        for item in result:
            if not isinstance(item, dict):
                continue

            text = item.get('text', '')

            # SenseVoice 特殊处理 - 解析 VAD 分段标记
            if self.model_type == "sensevoice":
                segments.extend(self._parse_sensevoice_result(text))
                continue

            # 检查是否有句子级别的时间戳 (sentence_info)
            if 'sentence_info' in item and item['sentence_info']:
                segments.extend(self._parse_sentence_info(item['sentence_info']))
            # 检查是否有字级别时间戳
            elif 'timestamp' in item and item['timestamp']:
                timestamps = item['timestamp']
                if timestamps and text:
                    segments.extend(self._split_by_punctuation(timestamps, text))
            # 没有时间戳的情况
            elif text:
                segments.append({
                    'start': 0.0,
                    'end': 0.0,
                    'text': text.strip()
                })

        # 如果没有时间戳，需要估算
        if segments and segments[0]['end'] == 0.0:
            segments = self._estimate_timestamps(segments)

        return segments

    def _parse_sensevoice_result(self, raw_text: str) -> List[Dict]:
        """
        解析 SenseVoice 的输出结果

        SenseVoice 返回格式: <|zh|><|NEUTRAL|><|Speech|><|withitn|>文本内容 <|zh|>...
        每个 <|...|> 标记块表示一个 VAD 分段

        Args:
            raw_text: SenseVoice 原始输出

        Returns:
            字幕片段列表
        """
        segments = []

        # 使用正则分割 VAD 分段
        # 匹配模式: <|语言|><|情感|><|事件|><|itn|>
        pattern = r'<\|[^|]+\|><\|[^|]+\|><\|[^|]+\|><\|[^|]+\|>'

        # 分割文本
        parts = re.split(pattern, raw_text)

        # 过滤空白片段
        vad_segments = [p.strip() for p in parts if p.strip()]

        print(f"DEBUG: SenseVoice VAD segments count: {len(vad_segments)}", flush=True)
        if vad_segments:
            print(f"DEBUG: First VAD segment: {vad_segments[0][:50]}...", flush=True)

        # 对每个 VAD 分段按标点进一步分句
        for vad_text in vad_segments:
            # 去除可能的特殊标记残留
            clean_text = re.sub(r'<\|[^|]+\|>', '', vad_text).strip()
            if not clean_text:
                continue

            # 按句子结束标点分句
            sentences = self._split_text_by_punctuation(clean_text)
            for sent in sentences:
                if sent.strip():
                    segments.append({
                        'start': 0.0,
                        'end': 0.0,
                        'text': sent.strip()
                    })

        print(f"DEBUG: SenseVoice total segments after splitting: {len(segments)}", flush=True)
        return segments

    def _split_text_by_punctuation(self, text: str) -> List[str]:
        """
        按标点符号分割文本为句子

        Args:
            text: 输入文本

        Returns:
            句子列表
        """
        # 句子结束标点
        sentence_ends = r'([。！？.!?；;])'

        # 分割
        parts = re.split(sentence_ends, text)

        # 合并标点和句子
        sentences = []
        for i in range(0, len(parts) - 1, 2):
            sent = parts[i]
            punct = parts[i + 1] if i + 1 < len(parts) else ''
            combined = (sent + punct).strip()
            if combined:
                sentences.append(combined)

        # 处理最后一部分（可能没有标点）
        if len(parts) % 2 == 1 and parts[-1].strip():
            sentences.append(parts[-1].strip())

        return sentences

    def _parse_sentence_info(self, sentence_info: List[Dict]) -> List[Dict]:
        """
        解析 FunASR 的 sentence_info (句子级别的时间戳)

        Args:
            sentence_info: FunASR返回的句子信息列表

        Returns:
            字幕片段列表
        """
        segments = []

        for sent in sentence_info:
            text = sent.get('text', '').strip()
            start = sent.get('start', 0)
            end = sent.get('end', 0)

            if text:
                # 转换为秒
                start_sec = start / 1000.0 if start > 100 else start
                end_sec = end / 1000.0 if end > 100 else end

                segments.append({
                    'start': start_sec,
                    'end': end_sec,
                    'text': text
                })

        print(f"DEBUG: Parsed {len(segments)} segments from sentence_info", flush=True)
        return segments

    def _split_by_punctuation(
        self,
        timestamps: List,
        full_text: str,
        max_duration: float = 8000,
        min_duration: float = 800,
        pause_threshold: float = 400
    ) -> List[Dict]:
        """
        根据标点符号和停顿智能分句

        Args:
            timestamps: FunASR返回的timestamp数组 [[start_ms, end_ms], ...]
            full_text: 完整的识别文本（带标点）
            max_duration: 单个segment最大时长(毫秒)
            min_duration: 单个segment最小时长(毫秒)
            pause_threshold: 停顿阈值(毫秒)

        Returns:
            拆分后的segment列表
        """
        if not timestamps or not full_text:
            return []

        # 清理文本
        clean_text = full_text.replace(' ', '')

        # 如果时间戳数量和文本长度相差太大，说明格式可能不对
        # 某些模型返回 [[start, end]] 而不是逐字时间戳
        if len(timestamps) < len(clean_text) * 0.5:
            print(f"DEBUG: Timestamp count ({len(timestamps)}) much less than text length ({len(clean_text)}), using segment-based parsing", flush=True)
            return self._parse_segment_timestamps(timestamps, clean_text)

        # 标点符号集合
        sentence_end = {'。', '！', '？', '.', '!', '?', '；', ';'}
        minor_pause = {'，', ',', '、', '：', ':'}

        print(f"DEBUG: Smart splitting - text length={len(clean_text)}, timestamps={len(timestamps)}", flush=True)
        print(f"DEBUG: Text preview: {clean_text[:100]}...", flush=True)

        segments = []
        seg_start_idx = 0
        seg_start_time = timestamps[0][0] if timestamps else 0

        for i, ts in enumerate(timestamps):
            if i >= len(clean_text):
                break

            char = clean_text[i]
            start_ms, end_ms = ts[0], ts[1]
            duration = end_ms - seg_start_time

            # 计算停顿
            pause = start_ms - timestamps[i-1][1] if i > 0 else 0

            should_split = False
            reason = ""

            # 规则1: 句子结束标点 + 最小时长
            if char in sentence_end and duration >= min_duration:
                should_split = True
                reason = f"end_punct({char})"

            # 规则2: 超过最大时长时，在标点或停顿处切分
            elif duration > max_duration:
                if char in (sentence_end | minor_pause):
                    should_split = True
                    reason = f"max_dur+punct({char})"
                elif pause > pause_threshold:
                    should_split = True
                    reason = f"max_dur+pause({pause}ms)"

            # 规则3: 较长停顿 + 次要标点
            elif char in minor_pause and pause > pause_threshold * 1.2 and duration >= min_duration:
                should_split = True
                reason = f"pause+minor({char},{pause}ms)"

            if should_split and i > seg_start_idx:
                seg_text = clean_text[seg_start_idx:i+1]
                if seg_text.strip():
                    segments.append({
                        'start': seg_start_time / 1000.0,
                        'end': end_ms / 1000.0,
                        'text': seg_text
                    })
                    print(f"DEBUG: Split [{reason}]: {seg_text[:30]}...", flush=True)

                seg_start_idx = i + 1
                seg_start_time = timestamps[i+1][0] if i+1 < len(timestamps) else end_ms

        # 最后一段
        if seg_start_idx < len(clean_text):
            seg_text = clean_text[seg_start_idx:]
            last_ts = timestamps[min(len(timestamps)-1, len(clean_text)-1)]
            if seg_text.strip():
                segments.append({
                    'start': seg_start_time / 1000.0,
                    'end': last_ts[1] / 1000.0,
                    'text': seg_text
                })

        print(f"DEBUG: Generated {len(segments)} segments", flush=True)
        return segments

    def _parse_segment_timestamps(self, timestamps: List, text: str) -> List[Dict]:
        """
        解析分段时间戳 (当时间戳是分段而非逐字时)

        Args:
            timestamps: 分段时间戳列表
            text: 完整文本

        Returns:
            字幕片段列表
        """
        segments = []

        # 按标点分句
        sentences = re.split(r'([。！？.!?；;])', text)

        # 合并标点和句子
        merged = []
        for i in range(0, len(sentences)-1, 2):
            sent = sentences[i]
            punct = sentences[i+1] if i+1 < len(sentences) else ''
            merged.append(sent + punct)
        if len(sentences) % 2 == 1 and sentences[-1]:
            merged.append(sentences[-1])

        # 分配时间戳
        if len(timestamps) >= len(merged):
            # 每个句子对应一个时间戳
            for i, sent in enumerate(merged):
                if sent.strip() and i < len(timestamps):
                    ts = timestamps[i]
                    segments.append({
                        'start': ts[0] / 1000.0,
                        'end': ts[1] / 1000.0,
                        'text': sent.strip()
                    })
        else:
            # 时间戳少于句子，均匀分配
            total_chars = sum(len(s) for s in merged)
            if total_chars > 0 and timestamps:
                total_duration = timestamps[-1][1] - timestamps[0][0]
                current_time = timestamps[0][0]

                for sent in merged:
                    if sent.strip():
                        duration = (len(sent) / total_chars) * total_duration
                        segments.append({
                            'start': current_time / 1000.0,
                            'end': (current_time + duration) / 1000.0,
                            'text': sent.strip()
                        })
                        current_time += duration

        print(f"DEBUG: Parsed {len(segments)} segments from segment timestamps", flush=True)
        return segments

    def _estimate_timestamps(self, segments: List[Dict]) -> List[Dict]:
        """
        估算时间戳（当 FunASR 不返回时间戳时）
        改进版：基于字符类型和标点进行更精确的估算

        Args:
            segments: 字幕片段

        Returns:
            添加了时间戳的片段
        """
        current_time = 0.0

        for segment in segments:
            text = segment['text']
            duration = self._estimate_duration(text)

            segment['start'] = current_time
            segment['end'] = current_time + duration
            current_time = segment['end'] + 0.3  # 片段间隔

        return segments

    def _estimate_duration(self, text: str) -> float:
        """
        估算文本朗读时长（更精确的算法）

        Args:
            text: 文本内容

        Returns:
            估算时长（秒）
        """
        duration = 0.0

        for char in text:
            if '\u4e00' <= char <= '\u9fff':
                # 中文字符：约 0.22 秒/字
                duration += 0.22
            elif char.isalpha():
                # 英文字母：约 0.08 秒/字母
                duration += 0.08
            elif char.isdigit():
                # 数字：约 0.15 秒/数字
                duration += 0.15
            elif char in '。！？.!?':
                # 句末标点：增加停顿
                duration += 0.3
            elif char in '，、；：,;:':
                # 句中标点：增加小停顿
                duration += 0.15

        # 最小时长 0.5 秒
        return max(duration, 0.5)

    def _merge_short_segments(self, segments: List[Dict]) -> List[Dict]:
        """
        合并过短的字幕片段

        规则：
        1. 时长小于 min_duration 的片段与前后合并
        2. 合并后不超过 max_duration 和 max_chars
        3. 间隔小于 gap_threshold 的相邻片段优先合并

        Args:
            segments: 原始字幕片段

        Returns:
            合并后的字幕片段
        """
        if not segments or len(segments) <= 1:
            return segments

        config = self.MERGE_CONFIG
        merged = []

        i = 0
        while i < len(segments):
            current = segments[i].copy()
            current_duration = current['end'] - current['start']
            current_chars = len(current['text'])

            # 如果当前片段太短，尝试与下一个合并
            while (current_duration < config['min_duration'] or current_chars < 5) and i + 1 < len(segments):
                next_seg = segments[i + 1]
                gap = next_seg['start'] - current['end']
                combined_duration = next_seg['end'] - current['start']
                combined_chars = len(current['text']) + len(next_seg['text'])

                # 检查是否可以合并
                if (gap < config['gap_threshold'] and
                    combined_duration <= config['max_duration'] and
                    combined_chars <= config['max_chars']):
                    # 合并
                    current['end'] = next_seg['end']
                    current['text'] = current['text'] + next_seg['text']
                    current_duration = current['end'] - current['start']
                    current_chars = len(current['text'])
                    i += 1
                else:
                    break

            merged.append(current)
            i += 1

        print(f"DEBUG: Merged segments: {len(segments)} -> {len(merged)}", flush=True)
        return merged

    def transcribe_with_diarization(
        self,
        audio_path: str,
        progress_callback: ProgressCallback,
        language: str = "zh"
    ) -> List[Dict]:
        """
        带说话人分离的转录（使用 FunASR 内置 spk_model）

        使用 FunASR 的 spk_model="cam++" 一次性完成 ASR + 说话人分离

        Args:
            audio_path: 音频文件路径
            progress_callback: 进度回调
            language: 语言

        Returns:
            带 speaker 字段的转录结果列表
        """
        progress_callback.update(5, "Loading models with speaker diarization...")

        try:
            # 使用 paraformer-zh + spk_model 组合
            model_kwargs = {
                "model": "paraformer-zh",
                "device": self.device,
                "vad_model": "fsmn-vad",
                "vad_kwargs": {"max_single_segment_time": self.max_single_segment_time_ms},
                "punc_model": "ct-punc",
                "spk_model": "cam++",  # 启用说话人分离
            }

            print(f"DEBUG: Loading model with spk_model: {model_kwargs}", flush=True)

            # 加载模型（如果还没加载，或者当前模型没有 spk_model）
            spk_model = AutoModel(**model_kwargs)

            progress_callback.update(25, "Models loaded")
            progress_callback.update(30, "Starting transcription with speaker diarization...")

            # 执行转录
            result = spk_model.generate(
                input=audio_path,
                batch_size_s=self.batch_size_s,
            )

            print(f"DEBUG: Raw result with spk: {type(result)}", flush=True)
            if isinstance(result, list) and len(result) > 0:
                print(f"DEBUG: First result keys: {result[0].keys() if isinstance(result[0], dict) else 'not dict'}", flush=True)
                if isinstance(result[0], dict):
                    # 检查是否有 sentence_info（包含说话人信息）
                    if 'sentence_info' in result[0]:
                        print(f"DEBUG: sentence_info count: {len(result[0]['sentence_info'])}", flush=True)
                        if result[0]['sentence_info']:
                            print(f"DEBUG: First sentence_info: {result[0]['sentence_info'][0]}", flush=True)

            progress_callback.update(85, "Processing results...")

            # 解析带说话人信息的结果
            segments = self._parse_result_with_speaker(result)

            # 合并过短字幕
            segments = self._merge_short_segments(segments)

            # 检查是否只有一个说话人
            unique_speakers = set(s.get('speaker') for s in segments if s.get('speaker'))
            if len(unique_speakers) <= 1:
                print("DEBUG: Only one speaker detected, removing speaker labels", flush=True)
                for seg in segments:
                    seg['speaker'] = None

            progress_callback.update(92, "Transcription with diarization complete")

            return segments

        except Exception as e:
            import traceback
            traceback.print_exc()
            raise RuntimeError(f"Transcription with diarization failed: {str(e)}")

    def _parse_result_with_speaker(self, result) -> List[Dict]:
        """
        解析带说话人信息的 FunASR 结果

        Args:
            result: FunASR 返回结果（带 spk_model）

        Returns:
            字幕片段列表，每个元素包含 start, end, text, speaker
        """
        segments = []

        if not isinstance(result, list) or len(result) == 0:
            return segments

        for item in result:
            if not isinstance(item, dict):
                continue

            # FunASR with spk_model 返回 sentence_info 包含每句的说话人信息
            if 'sentence_info' in item and item['sentence_info']:
                for sent in item['sentence_info']:
                    text = sent.get('text', '').strip()
                    start = sent.get('start', 0)
                    end = sent.get('end', 0)
                    spk = sent.get('spk', None)

                    if text:
                        # 转换时间戳（毫秒 -> 秒）
                        start_sec = start / 1000.0 if start > 100 else start
                        end_sec = end / 1000.0 if end > 100 else end

                        # 映射说话人 ID 为可读标签
                        speaker_label = None
                        if spk is not None:
                            speaker_label = f"Speaker {chr(ord('A') + int(spk) % 26)}"

                        segments.append({
                            'start': start_sec,
                            'end': end_sec,
                            'text': text,
                            'speaker': speaker_label
                        })
            else:
                # 回退到普通解析（无说话人信息）
                parsed = self._parse_result([item])
                for seg in parsed:
                    seg['speaker'] = None
                segments.extend(parsed)

        print(f"DEBUG: Parsed {len(segments)} segments with speaker info", flush=True)
        return segments
