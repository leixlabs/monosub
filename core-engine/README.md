# FunASR Core Engine

Python 引擎，使用 FunASR 进行语音识别并生成字幕。

## 目录结构

```
core-engine/
├── monosub_core/                      # 核心模块
│   ├── audio_extractor.py   # 音频提取
│   ├── transcriber.py       # FunASR 转录
│   └── subtitle_writer.py   # 字幕写入
├── main.py                   # 主程序入口
├── pyproject.toml            # Python 项目配置 (uv)
└── README.md                 # 本文件
```

## 安装依赖

### 使用 uv (推荐)

**uv** 是一个快速的 Python 包管理器，比 pip 快 10-100 倍。

```bash
# 安装 uv
curl -LsSf https://astral.sh/uv/install.sh | sh
# 或使用 Homebrew
brew install uv

# 同步依赖（自动创建虚拟环境 .venv）
cd core-engine
uv sync
```

 

### 安装 FFmpeg

macOS:
```bash
brew install ffmpeg
```

Ubuntu/Debian:
```bash
sudo apt-get install ffmpeg
```

## 使用方法

### 命令行使用

#### 使用 uv (推荐)

```bash
cd core-engine
uv run python main.py <video_path> <output_path> [options]
```

 

**参数:**
- `video_path`: 输入视频文件路径
- `output_path`: 输出字幕文件路径
- `--format`: 字幕格式 (srt, vtt, txt)，默认 srt
- `--language`: 语言 (zh, en)，默认 zh
- `--device`: 设备 (cpu, cuda)，默认 auto

**示例:**

```bash
# 基本使用 (uv)
uv run python main.py video.mp4 output.srt

# 指定格式和语言
uv run python main.py video.mp4 output.vtt --format vtt --language zh

# 使用 GPU
uv run python main.py video.mp4 output.srt --device cuda
```

### 输出格式

程序通过 stdout 输出进度信息，格式如下：

```
MSG:Starting processing: video.mp4
PROGRESS:0
MSG:Extracting audio...
PROGRESS:15
MSG:Loading model...
PROGRESS:20
MSG:Transcribing audio...
PROGRESS:80
MSG:Writing subtitle file...
PROGRESS:100
SUCCESS:/path/to/output.srt
```

**输出格式说明:**
- `MSG:<message>` - 状态消息
- `PROGRESS:<percent>` - 进度百分比 (0-100)
- `SUCCESS:<path>` - 成功，输出文件路径
- `ERROR:<message>` - 错误消息

## 模块说明

### audio_extractor.py
从视频文件中提取音频，使用 FFmpeg。

**功能:**
- 提取音频为 WAV 格式（16kHz, 单声道）
- 获取音频时长

### transcriber.py
使用 FunASR 进行语音识别。

**功能:**
- 加载 FunASR 模型
- 转录音频文件
- 解析结果为字幕片段

**支持的模型:**
- paraformer-zh (中文)
- paraformer-en (英文)

### subtitle_writer.py
将转录结果写入字幕文件。

**支持的格式:**
- SRT (SubRip)
- VTT (WebVTT)
- TXT (纯文本)

## 与 Tauri 集成

在 Tauri Rust 代码中调用：

```rust
use std::process::Command;

let output = Command::new("python")
    .arg("core-engine/main.py")
    .arg(video_path)
    .arg(output_path)
    .arg("--format")
    .arg("srt")
    .output()?;
```

监听 stdout 获取进度：

```rust
use std::process::{Command, Stdio};
use std::io::{BufRead, BufReader};

let mut child = Command::new("python")
    .arg("core-engine/main.py")
    .arg(video_path)
    .arg(output_path)
    .stdout(Stdio::piped())
    .spawn()?;

let stdout = child.stdout.take().unwrap();
let reader = BufReader::new(stdout);

for line in reader.lines() {
    let line = line?;
    if line.starts_with("PROGRESS:") {
        let progress: f64 = line[9..].parse()?;
        // 更新 UI 进度
    }
}
```

## 开发和调试

### 测试单个模块

```bash
# 测试音频提取
python -c "from asr.audio_extractor import AudioExtractor; print(AudioExtractor.extract_audio('test.mp4'))"

# 测试字幕写入
python -c "from asr.subtitle_writer import SubtitleWriter; SubtitleWriter.write([{'start': 0, 'end': 2, 'text': 'Hello'}], 'test.srt')"
```

### 打包为可执行文件（未来）

使用 PyInstaller:

```bash
pip install pyinstaller
pyinstaller --onefile --name funasr-worker main.py
```

## 注意事项

1. **模型下载**: 首次运行时，FunASR 会自动下载模型到 `~/.cache/modelscope/`
2. **内存使用**: 模型加载需要约 1-2GB 内存
3. **性能**: CPU 模式下，1 分钟音频约需要 10-30 秒处理时间
4. **GPU 加速**: 如需 GPU 加速，需要安装 CUDA 和对应的 PyTorch 版本

## 常见问题

### Q: ImportError: No module named 'funasr'
A: 运行 `uv sync` 或 `pip install funasr modelscope torch torchaudio`

### Q: FFmpeg 未找到
A: 安装 FFmpeg: `brew install ffmpeg` (macOS) 或 `sudo apt-get install ffmpeg` (Ubuntu)

### Q: 模型下载太慢
A: 可以手动下载模型到 `models/` 目录，然后修改 `transcriber.py` 中的模型路径

### Q: uv sync 失败
A: 确保 Python 3.9+ 已安装。运行 `uv sync --reinstall` 重新安装

## 后续优化

- [ ] 支持更多 FunASR 模型
- [ ] 支持断点续传
- [ ] 支持批量处理
- [ ] 优化内存使用
- [ ] 添加单元测试
