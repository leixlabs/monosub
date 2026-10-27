<div align="center">

# MonoSub

**将视频和音频转换为文本。本地运行。不依赖云端。**

[English](README.md) · [微软商店](https://apps.microsoft.com/detail/9P6WTCCRQTVW) · [下载](#-下载) · [常见问题](#-常见问题)

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

| 平台 | 获取方式 |
|---|---|
| **Windows** 10 / 11 (x64) | [**微软商店**](https://apps.microsoft.com/detail/9P6WTCCRQTVW) —— **¥48 / $9.99，一次买断** |
| **macOS** 11.0+ (Apple Silicon) | [GitHub Releases](https://github.com/leixlabs/monosub/releases/latest) |
| **Linux** | 即将推出 |

<a href="https://apps.microsoft.com/detail/9P6WTCCRQTVW">
  <img src="https://get.microsoft.com/images/zh-cn%20dark.svg" alt="从微软商店获取" height="52" />
</a>

**Windows 版是付费应用：¥48 / $9.99 一次买断**，没有订阅，也没有内购。商店版由微软签名
与校验，一键安装、自动更新。你付的钱买的是打包与维护，不是功能——同一个程序也可以从本
仓库源码自行编译（见[定价](#-定价)）。

**macOS 版**通过 [GitHub Releases](https://github.com/leixlabs/monosub/releases/latest) 分发。

---

## 🚀 快速开始

1. **获取** MonoSub —— Windows 从[微软商店](https://apps.microsoft.com/detail/9P6WTCCRQTVW)
   安装（¥48 / $9.99），macOS 从 [Releases](https://github.com/leixlabs/monosub/releases/latest) 下载
2. 打开**设置 → 模型**，下载语音模型（约 250 MB，只需一次）
3. 将视频/音频文件**拖入**应用
4. **点击** "开始"
5. **获得**你的转录稿

就这么简单。无需账号。无需订阅。模型下载完成后无需网络。

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

**Windows 版：¥48 / $9.99，微软商店一次买断。** 没有订阅、没有内购、没有二次收费，
付完就结束了。

| | 微软商店（Windows） | macOS · GitHub Releases | 自行编译 |
| --- | --- | --- | --- |
| 价格 | **¥48 / $9.99，买断** | 见 Releases | 免费 |
| 语音转录 | ✅ 无限制 | ✅ 无限制 | ✅ 无限制 |
| 导出 SRT、TXT、VTT、MD | ✅ | ✅ | ✅ |
| 批量处理 | ✅ | ✅ | ✅ |
| 字幕翻译 | ✅ | ✅ | ✅ |
| 说话人分离（识别谁说了什么） | ✅ | ✅ | ✅ |
| 词级时间戳 | ✅ | ✅ | ✅ |
| 平台签名验证 | ✅ 微软 | ❌（需 xattr 处理，见常见问题） | ❌ |
| 自动更新 | **✅** | **❌** | **❌** |
| 安装成本 | 一键 | 拖一下 | 装工具链 + 一次编译 |

**¥48 买到什么。** 微软的签名、一键安装，以及不用你操心的自动更新。买的不是功能——
上面列的每一项功能，免费的源码版里都有。

**不付钱也完全可行。** MonoSub 源码以源码可见许可公开（见下文）。克隆下来自己编译，
得到的是完全相同的程序：没有授权码、没有账号、没有功能阉割、没有埋点。商店版卖的是省事。

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

### 多少钱？

Windows 版通过微软商店 **¥48 / $9.99 一次买断**——没有订阅、没有内购。macOS 版见
[Releases](https://github.com/leixlabs/monosub/releases/latest)。两个平台都可以自行编译，不花钱。

### 有退款政策吗？

微软商店的购买由微软按其[退款政策](https://support.microsoft.com/help/2884681)处理——
MonoSub 不经手付款，退款请向商店申请。

### 付费版功能更多吗？

没有。同一个引擎、同一批模型、同样的功能、同样的输出格式。商店版多出来的是微软的签名、
一键安装与自动更新。

---

<div align="center">

为那些只想把视频转成文本的人而做。

[**Windows 版 · 微软商店**](https://apps.microsoft.com/detail/9P6WTCCRQTVW) · [macOS 版 · 下载](https://github.com/leixlabs/monosub/releases/latest)

</div>
