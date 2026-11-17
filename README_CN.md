<div align="center">

# MonoSub

**把视频和音频变成字幕。在你自己电脑上，不上传云端。**

[English](README.md) · [下载](#-下载) · [定价](#-定价) · [常见问题](#-常见问题)

本地转录与翻译 · macOS 与 Windows · 免费开始使用

</div>

---

## 解决的问题

你有大量的视频内容。播客、访谈、讲座、会议。但 LLM 无法观看视频或收听音频——它们只理解文本。

所以你需要转录稿。但现有的工具要么：

- 强迫你把私人内容上传到云端
- 为本该简单的功能收月费、甚至按分钟计费
- 因为要先上传，速度慢得要命

**MonoSub 解决了这些问题。** 在你的电脑上运行，本地处理文件，快速拿到文本。而且它免费就能开始：只有当你需要一次处理一整个文件夹时，才需要一次性付费。

---

## MonoSub 能做什么

拖入视频或音频文件。得到转录稿，或者直接可用的字幕文件。就这么简单。

- **100% 本地** — 文件永远不离开你的电脑，什么都不上传
- **完全离线** — 一次性下载模型后，无需网络
- **转录 + 翻译** — 本地语音识别 + 本地翻译模型，没有按分钟计费的账单
- **不只是文本，是字幕** — 说话人分离、词级时间戳，导出 SRT、VTT、TXT、MD、JSON
- **链接导入** — 粘贴一条受支持的视频链接，取用平台字幕或在本地转写音频
- **速度快** — Apple Silicon 与 NVIDIA GPU 硬件加速
- **需要时才批量** — 免费版一次处理一个文件；Pro 解除限制，让整个素材库并行跑完

### 适合谁用

- **内容创作者** — 将网络视频转为博客文章或字幕
- **研究人员** — 转录访谈用于分析
- **学生** — 将课堂录音转为可搜索的笔记
- **开发者** — 将视频内容喂给 LLM 处理
- **手里有敏感素材的人** — 客户访谈、内部会议、未公开作品——这些绝不能上传

---

## 📥 下载

| 平台 | 获取方式 |
|---|---|
| **macOS** 12+ (Apple Silicon) | [**Mac App Store**](https://apps.apple.com/us/app/monosub/id6829689694) —— **免费下载**，Pro 为一次性内购 |
| **Windows** 10 / 11 (x64) | [**微软商店**](https://apps.microsoft.com/detail/9P6WTCCRQTVW) —— **免费下载**，Pro 为一次性内购 |
| **macOS / Windows · 直营店** | [**checkout.leix.dev/p/monosub-pro**](https://checkout.leix.dev/p/monosub-pro) —— Pro 离线授权 **¥36 / $8.60**；免费构建见 [Releases](https://github.com/leixlabs/monosub/releases/latest) |
| **Linux** | 即将推出 |

<a href="https://apps.apple.com/us/app/monosub/id6829689694">
  <img src="https://tools.applemediaservices.com/api/badges/download-on-the-app-store/black/zh-cn?size=250x83" alt="从 Mac App Store 下载" height="52" />
</a>
&nbsp;
<a href="https://apps.microsoft.com/detail/9P6WTCCRQTVW">
  <img src="https://get.microsoft.com/images/zh-cn%20dark.svg" alt="从微软商店获取" height="52" />
</a>

**MonoSub 免费下载、一次一个文件免费用**——转录、翻译、说话人分离和全部导出格式都包含在内。一次性 **Pro** 授权只解除单文件限制，让你批量处理整个文件夹。没有订阅，也没有按分钟计费，永远没有。

- **Mac App Store**：免费下载 + Pro 一次性内购（$9.99 / ¥48）。已在美国及多数地区上架，**中国大陆区仍在备案中**。
- **Windows / 微软商店**：免费下载 + Pro 一次性内购（¥48 / $9.99）——微软签名、一键安装、自动更新。
- **直营店**：在 [checkout.leix.dev](https://checkout.leix.dev/p/monosub-pro) 购买 **Pro 离线授权**，¥36 / $8.60。授权不绑设备，macOS 与 Windows 通用；免费构建本身在 [Releases](https://github.com/leixlabs/monosub/releases/latest)。
- **自行编译**：源码以源码可见许可公开（见下文），自己构建永远免费。

---

## 🚀 快速开始

1. **获取** MonoSub —— [Mac App Store](https://apps.apple.com/us/app/monosub/id6829689694)（免费）、[微软商店](https://apps.microsoft.com/detail/9P6WTCCRQTVW)（Windows），或从 [Releases](https://github.com/leixlabs/monosub/releases/latest) 下载直装版
2. 打开**设置 → 模型**，下载语音模型（约 250 MB，只需一次）
3. 将视频/音频文件**拖入**应用
4. **点击** "开始"
5. **拿到**你的转录稿——导出 SRT、VTT、TXT、MD 或 JSON

就这么简单。无需账号。无需订阅。模型下载完成后无需网络。免费版一次处理一个文件；Pro 解锁队列。

> **v2.0 说明：** 语音识别已改用随包的 [audio.cpp](https://github.com/0xShug0/audio.cpp)  
> 引擎，不再依赖 Python + PyTorch。应用更小、内存占用低得多，但识别模型不再随安装包分发——  
> 在「设置 → 模型」里下载一次即可。详见发布说明。

---

## 💻 命令行

MonoSub 提供命令行入口 `monosub-cli`，用在窗口帮不上忙的场合：批量脚本、CI、Agent——从终端跑一整个文件夹才是它存在的理由。它跑的是与 App 相同的管线，写出的也是相同的格式。

命令行不单独配置模型——它读取 App 里已经选好的方案（存在配置文件里），所以两者用的是同一个模型。它只接受每次运行的选择：输出目录、格式、语言、是否翻译（不指定 `--translate --target <语言>` 就**不**翻译），以及要不要词级时间戳。`--model <id>` 可以在**这一次运行**里换掉一个模型——`--list-models` 会列出可用的 id——且永远不会写回你的设置。

```bash
monosub-cli video.mp4
monosub-cli video.mp4 --output-dir ~/subs --format srt,vtt
monosub-cli video.mp4 --translate --target en
monosub-cli video.mp4 --word-timestamps      # 另出 video.words.json
monosub-cli video.mp4 --model qwen3_asr_0_6b_q8_0 --word-timestamps
monosub-cli --list-models
monosub-cli *.mp4 --json
```

`--json` 把 JSON Lines 打到 stdout 供脚本使用，进度打到 stderr。文件顺序处理，单个失败不中断后续。加 `--translate --target en` 时，译文会另存为 `video.en.srt`，原 `video.srt` 不动。

`--word-timestamps` 会在字幕旁边多写一份 `video.words.json`：每个词的起止时间，以及词与词之间的静音——卡拉 OK 高亮、动画时间轴都是从这份数据长出来的。它要求方案本身就按词级时间戳求解（`qwen3_asr` + 强制对齐模型），所以如果保存的方案不能对齐，命令行会在开跑前就说明，并以退出码 6 收尾，而不是悄悄退回估算时间。

---

## 💎 定价

**免费下载，一次一个文件免费用。Pro 是一次性解锁批量——永远不是订阅。**

| | 免费版 | Pro |
| --- | --- | --- |
| 价格 | **¥0** | **¥48 / $9.99**（App Store 内购）—— 或直营店 **¥36 / $8.60** |
| 转录与翻译 | ✅ | ✅ |
| 导出 SRT、VTT、TXT、MD、JSON | ✅ | ✅ |
| 说话人分离与词级时间戳 | ✅ | ✅ |
| 队列中同时处理的文件数 | **一个** | **不限** |
| 整个素材库并行批量 | — | ✅ |
| 订阅 / 按分钟计费 | 永不 | 永不 |

### 在哪儿获取

| 平台 | 下载 | 价格 / Pro 怎么算 |
| --- | --- | --- |
| **macOS · App Store** | 免费 | Pro 内购 **¥48 / $9.99** · 中国区备案中 |
| **macOS / Windows · 直营店** | 免费构建 | Pro 离线授权 **¥36 / $8.60**——不绑设备，两平台通用 |
| **Windows · 微软商店** | 免费 | Pro 内购 **¥48 / $9.99** |
| **任意平台 · 源码构建** | 免费 | — |

价格以美元标示；App Store 与微软商店会按你所在地区结算。

**Pro 到底解锁了什么。** 只有一件事：单文件限制。所有能力——说话人分离、词级时间戳、翻译、全部导出格式——免费版都有。Pro 是给「处理一整个文件夹」的人，而不是「处理一个文件」的人。

**自行编译始终免费。** MonoSub 源码以源码可见许可公开（见下文）。克隆下来自己构建，得到的是完全相同的程序：没有授权码、没有账号、没有功能门禁、没有埋点。

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

这只影响 **GitHub Releases** 的直装版（ad-hoc 签名）。Mac App Store 版由 Apple 签名与公证，可正常打开。若用的是 Releases 版，在终端运行：

```bash
xattr -cr /Applications/MonoSub.app
```

然后重新打开应用。

### MonoSub 真的免费吗？

是。完整引擎免费下载、一次一个文件免费用——转录、翻译、说话人分离和全部导出格式都包含。Pro 是一次性购买，只解除单文件限制。

### Pro 多少钱，是订阅吗？

不是订阅。Pro 是一次性购买：Mac App Store 内购 **¥48 / $9.99**，或直营店 [checkout.leix.dev](https://checkout.leix.dev/p/monosub-pro) 的离线授权 **¥36 / $8.60**。Windows 微软商店版同样是免费下载，Pro 为一次性 **¥48 / $9.99** 内购。自行编译在任何平台都免费。

### Mac App Store 版在中国能下载吗？

macOS 版已在美国及多数地区上架，**中国大陆区仍在备案中**。在此之前，直营店 [checkout.leix.dev](https://checkout.leix.dev/p/monosub-pro) 的离线授权在中国可用，Windows 版在微软商店。

### 需要联网才能使用吗？

**转录不需要。** 初次使用时，在「设置 → 模型」下载语音模型（约 250 MB，只需一次）。之后 MonoSub 完全离线工作。

### 支持哪些文件格式？

视频：MP4、MOV、AVI、MKV、FLV、WMV  
音频：MP3、WAV、AAC、FLAC、M4A  
只要有音频，MonoSub 就能转录。

### 支持哪些语言？

目前支持中文（普通话）、英文，更多语言正在添加中。AI 会自动检测语言。

### 有退款政策吗？

退款按购买渠道处理：Mac App Store 由 Apple、微软商店由微软分别按其自身政策处理；直营店授权请在你的订单页联系支持：[checkout.leix.dev](https://checkout.leix.dev/p/monosub-pro)。

### Pro 功能更多吗？

没有。同一个引擎、同一批模型、同样的功能、同样的输出格式。Pro 只解除单文件队列限制，让你能批量处理。Windows 与 macOS 完全一样：免费一次一个文件，Pro 用于批量。

---

<div align="center">

为那些只想把视频转成文本、又不想上传的人而做。

[**下载 Mac 版**](https://apps.apple.com/us/app/monosub/id6829689694) · [**获取 Windows 版**](https://apps.microsoft.com/detail/9P6WTCCRQTVW) · [**直营店购买 Pro**](https://checkout.leix.dev/p/monosub-pro)

</div>
