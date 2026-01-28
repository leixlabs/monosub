# Voice to Subtitle

基于 Tauri + React + FunASR 的本地语音转字幕桌面应用。

![UI Design](design/UI.pen)

## 特性

- 🎬 **多文件队列处理** - 同时管理多个视频文件
- 🖼️ **视频缩略图** - 自动生成视频预览
- ⚡ **本地 AI 处理** - 使用 FunASR 进行高质量语音识别
- 📝 **多格式支持** - 输出 SRT、VTT、TXT 格式字幕
- 🌐 **多语言支持** - 支持中文、英文
- 💾 **灵活存储** - 同目录或自定义目录保存
- 🎨 **现代化 UI** - 基于设计系统的精美界面

## 技术栈

### 前端
- React 18 + TypeScript
- TailwindCSS
- Zustand (状态管理)
- Lucide React (图标)
- **包管理**: Bun

### 后端
- Tauri 2.0 (Rust)
- FunASR (Python)
- FFmpeg (视频处理)
- **包管理**: uv

## 快速开始

### 环境要求

- **Bun** (前端包管理器) - [安装](https://bun.sh/)
- **uv** (Python 包管理器) - [安装](https://docs.astral.sh/uv/)
- **Rust** 1.70+
- **FFmpeg** (视频处理)

### 安装依赖

1. **安装前端依赖 (使用 Bun)**
```bash
bun install
```

2. **设置 Python 环境 (使用 uv)**
```bash
./scripts/setup-python.sh
```

这个脚本会：
- 检查 uv 和 FFmpeg 是否安装
- 使用 uv 创建虚拟环境
- 安装 FunASR 和所有依赖

3. **验证安装**
```bash
# 使用 uv 运行 Python 引擎
cd core-engine
uv run python main.py --help
```

### 开发

```bash
# 启动开发服务器
bun run dev

# 类型检查
bun run check

# 代码检查
bun run lint
```

### 构建

```bash
# 构建应用
bun run build
```

构建产物：
- macOS: `src-tauri/target/release/bundle/macos/`
- Windows: `src-tauri/target/release/bundle/msi/`

## 项目结构

```
voice-to-subtitle/
├── src/                      # 前端代码
│   ├── components/          # React 组件
│   ├── store/              # Zustand 状态管理
│   └── App.tsx             # 主应用
│
├── src-tauri/               # Rust 后端
│   ├── src/
│   │   ├── commands/       # Tauri 命令
│   │   │   ├── video.rs   # 视频处理
│   │   │   └── transcribe.rs  # 转录管理
│   │   └── main.rs        # 入口
│   └── Cargo.toml
│
├── core-engine/             # Python AI 引擎
│   ├── asr/                # 核心模块
│   │   ├── audio_extractor.py
│   │   ├── transcriber.py
│   │   └── subtitle_writer.py
│   ├── main.py             # 入口
│   └── requirements.txt
│
├── design/                  # UI 设计文件
├── _docs/                   # 产品文档
└── scripts/                 # 辅助脚本
```

## 使用说明

### 1. 添加视频文件
- 点击"添加文件"按钮
- 或拖拽视频文件到窗口

### 2. 选择存储位置
- **同目录**: 字幕保存在视频文件同目录
- **指定目录**: 选择自定义目录保存所有字幕

### 3. 开始处理
- 点击文件项的"开始"按钮
- 查看实时进度

### 4. 管理任务
- **暂停**: 暂停当前任务（开发中）
- **删除**: 移除文件项
- **清空**: 清空所有文件

## 配置

### 修改模型
编辑 `core-engine/asr/transcriber.py`:
```python
model_name = "paraformer-zh"  # 中文
# model_name = "paraformer-en"  # 英文
```

### 修改输出格式
在前端或通过命令行参数:
```bash
cd core-engine
uv run python main.py video.mp4 output.srt --format vtt
```

支持的格式：`srt`, `vtt`, `txt`

## 开发指南

详见以下文档：
- [PROJECT.md](PROJECT.md) - 项目详细说明
- [core-engine/README.md](core-engine/README.md) - Python 引擎文档
- [CLEANUP_SUMMARY.md](CLEANUP_SUMMARY.md) - 项目重构记录

## 故障排除

### uv 未安装
```bash
# macOS/Linux
curl -LsSf https://astral.sh/uv/install.sh | sh

# 或使用 Homebrew (macOS)
brew install uv
```

### Python 依赖问题
```bash
cd core-engine
# 重新同步依赖
uv sync --reinstall
```

### FFmpeg 未安装
```bash
# macOS
brew install ffmpeg

# Ubuntu/Debian
sudo apt-get install ffmpeg
```

### Rust 编译错误
```bash
# 更新 Rust
rustup update

# 清理并重新构建
cd src-tauri
cargo clean
cargo build
```

## 路线图

- [ ] 完成 FunASR 集成测试
- [ ] 支持 GPU 加速
- [ ] 批量处理队列
- [ ] 字幕编辑功能
- [ ] 导出设置保存
- [ ] 多语言界面
- [ ] Windows 和 Linux 支持

## 贡献

欢迎提交 Issue 和 Pull Request！

## 许可证

MIT License

## 致谢

- [Tauri](https://tauri.app/) - 桌面应用框架
- [FunASR](https://github.com/alibaba-damo-academy/FunASR) - 语音识别模型
- [FFmpeg](https://ffmpeg.org/) - 视频处理工具
