"""
字幕文件生成器
支持 SRT, VTT, TXT 格式
支持双语字幕
"""
from typing import List, Dict, Optional
from datetime import timedelta


class SubtitleWriter:
    """字幕写入器 - 支持多格式和双语字幕"""

    @staticmethod
    def format_timestamp_srt(seconds: float) -> str:
        """
        格式化时间戳为 SRT 格式
        例如: 00:00:01,500
        """
        if seconds < 0:
            seconds = 0
        td = timedelta(seconds=seconds)
        hours = int(td.total_seconds() // 3600)
        minutes = int((td.total_seconds() % 3600) // 60)
        secs = int(td.total_seconds() % 60)
        millis = int((td.total_seconds() % 1) * 1000)
        return f"{hours:02d}:{minutes:02d}:{secs:02d},{millis:03d}"

    @staticmethod
    def format_timestamp_vtt(seconds: float) -> str:
        """
        格式化时间戳为 VTT 格式
        例如: 00:00:01.500
        """
        if seconds < 0:
            seconds = 0
        td = timedelta(seconds=seconds)
        hours = int(td.total_seconds() // 3600)
        minutes = int((td.total_seconds() % 3600) // 60)
        secs = int(td.total_seconds() % 60)
        millis = int((td.total_seconds() % 1) * 1000)
        return f"{hours:02d}:{minutes:02d}:{secs:02d}.{millis:03d}"

    @staticmethod
    def write_srt(
        segments: List[Dict],
        output_path: str,
        translation_segments: Optional[List[Dict]] = None
    ) -> None:
        """
        写入 SRT 格式字幕

        Args:
            segments: 字幕片段列表，每个片段包含 start, end, text
            output_path: 输出文件路径
            translation_segments: 翻译字幕片段（可选，用于双语字幕）
        """
        with open(output_path, 'w', encoding='utf-8') as f:
            for idx, segment in enumerate(segments, 1):
                start = SubtitleWriter.format_timestamp_srt(segment['start'])
                end = SubtitleWriter.format_timestamp_srt(segment['end'])
                text = segment['text'].strip()

                f.write(f"{idx}\n")
                f.write(f"{start} --> {end}\n")

                # 双语字幕：原文在上，翻译在下
                if translation_segments and idx <= len(translation_segments):
                    trans_text = translation_segments[idx - 1].get('text', '').strip()
                    if trans_text:
                        f.write(f"{text}\n{trans_text}\n\n")
                    else:
                        f.write(f"{text}\n\n")
                else:
                    f.write(f"{text}\n\n")

    @staticmethod
    def write_vtt(
        segments: List[Dict],
        output_path: str,
        translation_segments: Optional[List[Dict]] = None
    ) -> None:
        """
        写入 VTT 格式字幕

        Args:
            segments: 字幕片段列表
            output_path: 输出文件路径
            translation_segments: 翻译字幕片段（可选）
        """
        with open(output_path, 'w', encoding='utf-8') as f:
            f.write("WEBVTT\n\n")

            for idx, segment in enumerate(segments, 1):
                start = SubtitleWriter.format_timestamp_vtt(segment['start'])
                end = SubtitleWriter.format_timestamp_vtt(segment['end'])
                text = segment['text'].strip()

                f.write(f"{idx}\n")
                f.write(f"{start} --> {end}\n")

                if translation_segments and idx <= len(translation_segments):
                    trans_text = translation_segments[idx - 1].get('text', '').strip()
                    if trans_text:
                        f.write(f"{text}\n{trans_text}\n\n")
                    else:
                        f.write(f"{text}\n\n")
                else:
                    f.write(f"{text}\n\n")

    @staticmethod
    def write_txt(
        segments: List[Dict],
        output_path: str,
        translation_segments: Optional[List[Dict]] = None
    ) -> None:
        """
        写入纯文本格式

        Args:
            segments: 字幕片段列表
            output_path: 输出文件路径
            translation_segments: 翻译字幕片段（可选）
        """
        with open(output_path, 'w', encoding='utf-8') as f:
            for idx, segment in enumerate(segments):
                text = segment['text'].strip()
                f.write(f"{text}\n")

                if translation_segments and idx < len(translation_segments):
                    trans_text = translation_segments[idx].get('text', '').strip()
                    if trans_text:
                        f.write(f"{trans_text}\n")

    @classmethod
    def write(
        cls,
        segments: List[Dict],
        output_path: str,
        format: str = 'srt',
        translation_segments: Optional[List[Dict]] = None
    ) -> None:
        """
        根据格式写入字幕文件

        Args:
            segments: 字幕片段列表
            output_path: 输出文件路径
            format: 格式 (srt, vtt, txt)
            translation_segments: 翻译字幕片段（可选，用于双语字幕）
        """
        format = format.lower()

        if format == 'srt':
            cls.write_srt(segments, output_path, translation_segments)
        elif format == 'vtt':
            cls.write_vtt(segments, output_path, translation_segments)
        elif format == 'txt':
            cls.write_txt(segments, output_path, translation_segments)
        else:
            raise ValueError(f"Unsupported format: {format}")

    @staticmethod
    def write_bilingual_separate(
        segments: List[Dict],
        output_path: str,
        translation_segments: List[Dict],
        format: str = 'srt'
    ) -> tuple:
        """
        写入双语字幕到单独的文件

        Args:
            segments: 原文字幕片段
            output_path: 原文输出路径
            translation_segments: 翻译字幕片段
            format: 格式

        Returns:
            (原文路径, 翻译路径)
        """
        import os

        # 原文文件
        SubtitleWriter.write(segments, output_path, format)

        # 翻译文件 - 在文件名后添加语言标识
        base, ext = os.path.splitext(output_path)
        trans_path = f"{base}.translated{ext}"

        # 为翻译片段复制时间戳
        trans_with_time = []
        for i, seg in enumerate(segments):
            if i < len(translation_segments):
                trans_with_time.append({
                    'start': seg['start'],
                    'end': seg['end'],
                    'text': translation_segments[i].get('text', '')
                })

        SubtitleWriter.write(trans_with_time, trans_path, format)

        return output_path, trans_path
