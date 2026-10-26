<div align="center">

# MonoSub

**将视频和音频转换为文本。本地运行。不依赖云端。**

[English](README.md) · [下载](#-下载) · [常见问题](#-常见问题)

</div>

---

## 解决的问题

你有大量的视频内容。播客、访谈、讲座、会议。但 LLM 无法观看视频或收听音频——它们只理解文本。

所以你需要转录稿。但现有的解决方案要么：
- 强迫你将私人内容上传到云端
- 对本该简单的功能收取月费
- 因为要先上传，速度慢得要命

**MonoSub 解决了这些问题。** 在你的电脑上运行，本地处理文件，快速获得文本。

---

## MonoSub 能做什么

拖入视频或音频文件。获得转录稿。就这么简单。

- **100% 本地** — 你的文件永远不会离开电脑
- **完全离线** — 安装后无需网络
- **多种格式** — MP4、MOV、MP3、WAV 等
- **导出 SRT、VTT、TXT** — 可与任何 LLM 或视频编辑器配合使用
- **速度快** — Apple Silicon 和 NVIDIA GPU 硬件加速

### 适合谁用

- **内容创作者** — 将网络视频转为博客文章
- **研究人员** — 转录访谈用于分析
- **学生** — 将课堂录音转为可搜索的笔记
- **开发者** — 将视频内容喂给 LLM 处理

---

## 📥 下载

**最新版本：** [GitHub Releases](https://github.com/leixlabs/monosub/releases/latest)

### 系统要求

- **macOS** 11.0+ (Apple Silicon M1/M2/M3 优化)
- **Windows** (即将推出)
- **Linux** (即将推出)

---

## 🚀 快速开始

1. 从 [Releases](https://github.com/leixlabs/monosub/releases/latest) **下载** MonoSub
2. 将视频/音频文件**拖入**应用
3. **点击** "开始"
4. **获得**你的转录稿

就这么简单。无需账号。无需订阅。无需网络。

---

## 💻 命令行

MonoSub 提供命令行入口 `monosub-cli`，用在窗口帮不上忙的场合：批量脚本、CI、Agent——从终端跑一整个文件夹才是它存在的理由。它跑的是与 App 相同的管线，写出的也是相同的格式。

命令行不单独配置模型——它读取 App 里已经选好的方案（存在配置文件里），所以两者用的是同一个模型。它只接受每次运行的选择：输出目录、格式、语言，以及是否翻译（不指定 `--translate --target <语言>` 就**不**翻译）。

```bash
monosub-cli video.mp4
monosub-cli video.mp4 --output-dir ~/subs --format srt,vtt
monosub-cli video.mp4 --translate --target en
monosub-cli *.mp4 --json
```

`--json` 把 JSON Lines 打到 stdout 供脚本使用，进度打到 stderr。文件顺序处理，单个失败不中断后续。加 `--translate --target en` 时，译文会另存为 `video.en.srt`，原 `video.srt` 不动。

---

## 💎 定价

**源码公开。商店版收费。**

MonoSub 是一个公开仓库。上面提到的所有功能都能从这个仓库的 checkout 直接跑起来——
没有授权码、没有账号、没有功能阉割。自己编译，不花一分钱。

**Mac App Store** 与 **Microsoft Store** 的付费上架正在准备中。付费买的是打包与分发，
不是功能：

| | GitHub Releases / 自行编译 | Mac App Store · Microsoft Store |
|---|---|---|
| 语音转录 | ✅ 无限制 | ✅ 无限制 |
| 导出 SRT、TXT、VTT、MD | ✅ | ✅ |
| 批量处理 | ✅ | ✅ |
| 说话人分离（识别谁说了什么） | ✅ | ✅ |
| 已签名公证 — 无 Gatekeeper 警告 | ❌ | ✅ |
| 自动更新 | ❌ | ✅ |

**一次买断。无订阅。**

目前提交正在审核中，商店页面尚未上线。在那之前，请使用
[GitHub Releases](https://github.com/leixlabs/monosub/releases/latest)。

---

## 📄 许可协议

源码采用 **[PolyForm Small Business License 1.0.0](https://polyformproject.org/licenses/small-business/1.0.0)**（源码公开，非开源）。

| 使用方 | 商用 |
|---|---|
| 个人、业余项目、任何非商业用途 | ✅ 免费 |
| **少于 100 人**且**年营收低于 100 万美元**的公司 | ✅ 免费 |
| 超过上述门槛的公司 | 💬 需单独购买商业授权 |

**模型权重不在本许可范围内，也不随 MonoSub 分发。** 每个模型都从发布方自己的服务器获取，各自适用发布方选择的许可 —— 目录 129 个条目中有 16 个禁止商用。下载开始前，应用会展示该模型的许可证名称并给出发布方条款链接。

三方组件（FFmpeg、audio.cpp、llama.cpp、yt-dlp）的许可披露见
[`src-tauri/resources/licenses/THIRD-PARTY-NOTICES.md`](src-tauri/resources/licenses/THIRD-PARTY-NOTICES.md)。

---

## 常见问题

### macOS 提示 "MonoSub 已损坏，无法打开"

这是 Apple 的 Gatekeeper 过度保护。在终端运行：

```bash
xattr -cr /Applications/MonoSub.app
```

然后重新打开应用。

### 需要联网才能使用吗？

**不需要。** 初次下载和设置后，MonoSub 完全离线工作。

### 支持哪些文件格式？

视频：MP4、MOV、AVI、MKV、FLV、WMV  
音频：MP3、WAV、AAC、FLAC、M4A  
只要有音频，MonoSub 就能转录。

### 支持哪些语言？

目前支持中文（普通话）、英文，更多语言正在添加中。AI 会自动检测语言。

### 有退款政策吗？

商店购买的退款由 Apple 或 Microsoft 按各自的退款政策处理——MonoSub 不经手付款。
GitHub Releases 版本免费且不含任何内购，不存在退款问题。

---

<div align="center">

为那些只想把视频转成文本的人而做。

[立即下载](https://github.com/leixlabs/monosub/releases/latest)

</div>
