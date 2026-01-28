"""
字幕文件生成器
支持 SRT, VTT, TXT 格式
"""
from typing import List, Dict
from datetime import timedelta


class SubtitleWriter:
    """字幕写入器"""

    @staticmethod
    def format_timestamp_srt(seconds: float) -> str:
        """
        格式化时间戳为 SRT 格式
        例如: 00:00:01,500
        """
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
        td = timedelta(seconds=seconds)
        hours = int(td.total_seconds() // 3600)
        minutes = int((td.total_seconds() % 3600) // 60)
        secs = int(td.total_seconds() % 60)
        millis = int((td.total_seconds() % 1) * 1000)
        return f"{hours:02d}:{minutes:02d}:{secs:02d}.{millis:03d}"

    @staticmethod
    def write_srt(segments: List[Dict], output_path: str) -> None:
        """
        写入 SRT 格式字幕

        Args:
            segments: 字幕片段列表，每个片段包含 start, end, text
            output_path: 输出文件路径
        """
        with open(output_path, 'w', encoding='utf-8') as f:
            for idx, segment in enumerate(segments, 1):
                start = SubtitleWriter.format_timestamp_srt(segment['start'])
                end = SubtitleWriter.format_timestamp_srt(segment['end'])
                text = segment['text'].strip()

                f.write(f"{idx}\n")
                f.write(f"{start} --> {end}\n")
                f.write(f"{text}\n\n")

    @staticmethod
    def write_vtt(segments: List[Dict], output_path: str) -> None:
        """
        写入 VTT 格式字幕

        Args:
            segments: 字幕片段列表
            output_path: 输出文件路径
        """
        with open(output_path, 'w', encoding='utf-8') as f:
            f.write("WEBVTT\n\n")

            for idx, segment in enumerate(segments, 1):
                start = SubtitleWriter.format_timestamp_vtt(segment['start'])
                end = SubtitleWriter.format_timestamp_vtt(segment['end'])
                text = segment['text'].strip()

                f.write(f"{idx}\n")
                f.write(f"{start} --> {end}\n")
                f.write(f"{text}\n\n")

    @staticmethod
    def write_txt(segments: List[Dict], output_path: str) -> None:
        """
        写入纯文本格式

        Args:
            segments: 字幕片段列表
            output_path: 输出文件路径
        """
        with open(output_path, 'w', encoding='utf-8') as f:
            for segment in segments:
                text = segment['text'].strip()
                f.write(f"{text}\n")

    @classmethod
    def write(cls, segments: List[Dict], output_path: str, format: str = 'srt') -> None:
        """
        根据格式写入字幕文件

        Args:
            segments: 字幕片段列表
            output_path: 输出文件路径
            format: 格式 (srt, vtt, txt)
        """
        format = format.lower()

        if format == 'srt':
            cls.write_srt(segments, output_path)
        elif format == 'vtt':
            cls.write_vtt(segments, output_path)
        elif format == 'txt':
            cls.write_txt(segments, output_path)
        else:
            raise ValueError(f"Unsupported format: {format}")
