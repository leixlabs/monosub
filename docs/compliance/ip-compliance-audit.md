# MonoSub 侵权与合规审查

审查日期：2026-10-26  
审查范围：仓库文案 / UI 文案 / `models.json` 模型许可 / 捆绑二进制许可 / 分发方式  
结论：**发现 3 项 P0（分发即违规）、5 项 P1、4 项 P2**。其中最严重的是捆绑 GPLv3 FFmpeg。

**进度**：P0-FFmpeg、P0-无许可证模型、P1-许可分级与展示、P2-LICENSE 均已修复（详见 §1、§2、§6、§13）。
剩余待处理：翻译默认模型的地域排除（§4.2）、cookie 读取提示（§10）。

> 本文为工程梳理，不构成法律意见。涉及 GPL 义务、商标使用、平台 ToS 的处置，请以当地律师意见为准。

---

## 0. 结论速览

| 级别    | 问题                                                        | 位置                                                           | 处置                          |
| ----- | --------------------------------------------------------- | ------------------------------------------------------------ | --------------------------- |
| ✅ 已修  | ~~捆绑 **GPLv3** FFmpeg（Win + macOS 两个平台）~~                 | `prepare-resources.mjs`、`bundle-ffmpeg.mjs`、`lib/ffmpeg.mjs` | 已换 LGPL 构建 + 发布期许可护栏        |
| ✅ 已修  | ~~**8 个模型上游未发布任何许可证**仍列在下载列表~~ —— 改为 `unknown` 分级 + 下载前告知       | `models.json`、`ModelLicensePrompt.tsx`                       | 保留列出，但不给任何「可商用」暗示           |
| ✅ 已修  | ~~README 宣称 **YouTube**，但产品根本不支持~~                        | `README.md:38`、`README_CN.md:38`                             | 已改 "online videos" / "网络视频" |
| ✅ 已修  | ~~**recommended** 模型 `crisperwhisper_2_large_*` 是**仅非商用**许可~~ —— `tier` 已整体移除，该模型不再被推荐 | `models.json`                                                | 现为 `noncommercial` 徽标         |
| 🟠 P1 | 翻译**默认**模型 `hy_mt_1_8b_q4_k_m` 许可**排除欧盟/英国/韩国**           | `models.json`                                                | 地域路由或换默认（**仍未处理**）          |
| ✅ 已修  | ~~16 个不可商用模型**无任何 UI 标记**~~                                | `Settings.tsx` `ModelRow`                                     | 红色「仅限非商用」徽标                 |
| ✅ 已修  | ~~许可证只以极小截断文本展示，且无商用风险分级~~                              | `Settings.tsx`、`ModelLicensePrompt.tsx`                       | 四档徽标 + 下载前确认弹窗 + 许可证外链      |
| ✅ 已修  | ~~7 个 LFM 模型有 **$10M 年营收**商用阈值~~ —— 统一归入 `restricted` 并在弹窗说明条件 | `models.json`                                                | 弹窗列出「用户量阈值」等条件类型            |
| ✅ 已修  | ~~使用 B 站商标~~ —— 界面文案已全部去除品牌名                              | `en.json`、`zh-CN.json`、`landing_page` 两语种                    | 已改为「网络视频 / 受支持的站点」          |
| ✅ 已修  | ~~用第三方商标替平台下断言~~（"Bilibili requires a login"）             | `en.json:335,349,352`                                        | 已改为中性表述                     |
| 🟡 P2 | 读取浏览器 **bilibili.com cookie**，大概率违反平台 ToS                 | `SettingsUrlImport.tsx:108`                                  | 加风险提示                       |
| ✅ 已修  | ~~仓库**根目录无 LICENSE** 文件~~                                 | `LICENSE`、`package.json`、`Cargo.toml`                        | 已补 **PolyForm Small Business 1.0.0**，三方组件指向 notices |

---

## 1. ✅ 已修：捆绑 GPLv3 FFmpeg

### 修复内容（2026-10-26）

1. **Windows**：`fetchFfmpegWindows()` 从 gyan.dev `essentials_build`（GPLv3）改为  
   BtbN `ffmpeg-n8.1.3-24-g92611900bb-win64-lgpl-8.1.zip`，tag 与文件名、sha256  
   一起固定，保证可复现。已实测：配置行无 `--enable-gpl`／`--enable-nonfree`，  
   x264/x265/xvid/vidstab 均为 `--disable-*`；PCM 抽取、缩略图、ffprobe 元数据  
   全部通过，功能零损失。
2. **macOS**：`brew install ffmpeg` 是 GPL-3.0-or-later，且没有主流预编译 LGPL  
   构建，因此新增 `scripts/release/build-ffmpeg-lgpl-macos.mjs` 从固定源码  
   （`ffmpeg-8.1.3.tar.xz`）构建 GPL-free 版本。
3. **护栏**：新增 `scripts/lib/ffmpeg.mjs`，解析 `ffmpeg -version` 的  
   configuration 行，命中禁用标记即 `die()`。`prepare-resources.mjs`（Win +  
   macOS 两处）与 `bundle-ffmpeg.mjs` 均接入。护栏放在「已存在则跳过」判断  
   **之前**，否则历史遗留的 GPL 二进制会被永久重复打包。
4. **披露**：`THIRD-PARTY-NOTICES.md` 的 FFmpeg 条目补全为完整 LGPL 披露  
   （许可版本、构建来源、configure 标记、源码获取方式、FFmpeg 要求的署名文案）。

---

**原始问题描述**（保留以备审计追溯）

**证据**（直接运行捆绑二进制）：

```
ffmpeg version 8.1.2-essentials_build-www.gyan.dev
configuration: --enable-gpl --enable-version3 --enable-static
               --enable-libx264 --enable-libx265 --enable-libxvid
               --enable-libass --enable-libvidstab ...
```

文件：`src-tauri/resources/bin/ffmpeg-x86_64-pc-windows-msvc.exe`（101 MB）、`ffprobe-*`（101 MB）

**先划清一个常见误解（重要）：**

> 「GPL 不禁止商业售卖，GPLv3 §4 明确允许对每个副本收取任意价格」——这句话**完全正确**，  
> 但它对应的是「**我自己的软件整体以 GPLv3 开源，同时放到商店收费卖**」这一情形，  
> 此时确实 100% 合规。
>
> MonoSub 的现状是**相反的情形**：MonoSub 自身闭源（且无 LICENSE），**捆绑**了 GPLv3 的  
> FFmpeg 二进制。问的不是「能不能收费」，而是「**闭源应用能不能分发 GPLv3 二进制**」。  
> 这是 §5 / §6 / §10 的管辖范围，§4 的"可以收费"在这里救不了。

**再更正一个本审查的过度断言：**

FFmpeg 在本项目中是作为**独立可执行子进程**调用的，不是链接进应用的库：

```
src-tauri/src/commands/transcribe.rs:1713   tokio::process::Command::new(ffmpeg)
src-tauri/src/commands/video.rs:383         Command::new(&ffmpeg_cmd)
```

按 GPLv3 §5 末段的 **aggregate（聚合）** 条款，以及 FSF 一贯的 "arm's length communication"  
立场（两个程序通过 exec / 命令行 / 管道通信，互不依赖对方内部结构），MonoSub 与 FFmpeg  
**大概率构成"聚合"而非"衍生作品"**，因此 §5(c) 的传染条款（整个作品必须以 GPLv3 授权）  
**并不必然触发**。本审查先前写的「直接违反 §5(c)」属过度断言，特此更正。

**那它为什么还是 P0？**

因为聚合豁免是**有条件的**，而本项目两个条件都不满足：

1. **§4 / §6 义务不因聚合而免除。** 你在 `.msix`／`.app` 里分发了 GPLv3 二进制，就是在  
   convey 该二进制，就必须：保留版权声明、附 GPLv3 全文、提供 Corresponding Source  
   （含构建脚本）或一份 ≥3 年有效的书面要约。现状（见下）一条都没做到。
2. **§5 的聚合豁免本身带条件：**
   > "...if the compilation and its resulting copyright are **not used to limit the access or  
   > legal rights of the compilation's users beyond what the individual works permit**."
   >
   > 而 GPLv3 §10 更直接：**"You may not impose any further restrictions on the exercise of  
   > the rights granted or affirmed under this License."**
   Mac App Store 的 LA EULA 恰恰施加了 further restrictions：仅限 Apple 品牌设备、  
   仅限个人非商业用途、家庭共享数量上限等。这与 §10 直接冲突。**FSF 的公开立场是无法同时  
   满足两者**——2011 年 VLC 从 App Store 下架即缘于此（VLC 为 GPLv2，作者主动撤下）。

   **所以这里有个反直觉但关键的结论：恰恰因为你要上 Mac App Store，GPLv3 是绝对不能留的。**  
   把它留下，等于"MAS 条款违反 §10（无法履行）"与"不带 GPLv3 源码要约分发（违反 §6）"二选一。

**另一个必须澄清的点：把 MonoSub 整体 GPLv3 化，解决不了模型许可问题。**

GPLv3 只约束 MonoSub 自身代码及其捆绑的 FFmpeg。模型权重是**各自独立的作品**，  
CC-BY-NC-4.0 / Tencent HY / Llama 2·3 Community / LFM Open 等许可不受 GPLv3 影响，  
该非商用的依然非商用。这是两个互不相干的问题，不要指望用开源 App 一次性化解。

**并且开源会反噬商业模式：** 一旦整体 GPLv3，任何人都能合法地免费再分发你的应用  
（包括未签名版本），而商店版卖的正是"已签名公证 / 免 Gatekeeper / 自动更新"。  
叠加 MAS 条款冲突，这条路两头不讨好。

**范围重新确认（新发现）：macOS 侧同样是 GPLv3，不只是 Windows。**

`scripts/release/bundle-ffmpeg.mjs:33` 用的是 `brew install ffmpeg`，而 Homebrew 官方  
formula 的 `license` 字段就是 **`GPL-3.0-or-later`**。  
（homebrew-core 有切到 LGPL-only 的 PR #276228，但 CI 未通过、尚未合入。）

因此**两个平台的 FFmpeg 都需要替换**，不能只修 Windows。

**现有文档的问题：**  
`src-tauri/resources/licenses/THIRD-PARTY-NOTICES.md:94-100` 只写了：

> - License: see the FFmpeg project; build-specific (LGPL/GPL) terms apply to the distributed binaries.

这**不构成合规**——必须明确写出具体许可证、构建配置、并提供源码的书面要约（written offer）。

**处置方案（按推荐度）：**

**结论：只有一条路 —— 换 LGPL 构建。**

FFmpeg 本体是 **LGPL-2.1-or-later**，只有 `--enable-gpl` 才会让**整个二进制**变成 GPL。  
而本项目实际根本没用到任何 GPL 组件：

```
transcribe.rs:1700   ["-acodec","pcm_s16le","-ar","16000","-ac","1","-y"]
aligned.rs:526       "-c:a" ... "pcm_s16le"
```

音频抽取全部走**原生 PCM**，缩略图与 ffprobe 元数据也是原生能力。  
`--enable-libx264/--enable-libx265/--enable-libxvid/--enable-libvidstab` 编进去但从未调用。

→ **换 LGPL 构建 = 零功能损失。**

| 平台      | 现状                                                  | 替换为                                                                             |
| ------- | --------------------------------------------------- | ------------------------------------------------------------------------------- |
| Windows | gyan.dev `ffmpeg-8.1.2-essentials_build.zip`（GPL）   | BtbN `ffmpeg-master-latest-win64-lgpl.zip`（明确 lgpl），或 gyan.dev 的 shared/LGPL 构建 |
| macOS   | `brew install ffmpeg`（formula 标 `GPL-3.0-or-later`） | 需 `--disable-gpl` 自编译，或换用 LGPL 发行源；`bundle-ffmpeg.mjs` 随之调整                     |

**换完之后仍需履行的 LGPLv2.1 义务（比 GPL 轻得多，但不能省）：**

- 附 LGPL-2.1 许可证全文 + FFmpeg 版权声明
- 提供 FFmpeg 对应源码（或 ≥3 年有效的书面要约）
- 保留用户替换 FFmpeg 库的可能性（当前"独立可执行文件 + 可 `MONOSUB_FFMPEG_PATH` 覆盖"  
  的架构**天然满足**这一点，见 `video.rs:120` 的 `resolve_binary_path`）

---

## 2. ✅ 已修：8 个模型上游未发布任何许可证

无许可证 = 默认"保留所有权利"。原建议是直接从 `models.json` 移除；实际采用**保留列出 + 标为 `unknown` + 下载前告知**的路线，理由见 §12。

| model id | `commercial_use` | license 字段 |
| --- | --- | --- |
| `samsone_{99m,134m,356m}_{bf16,q8_0}`（6 个） | `unknown` | No upstream license published (SamsungLabs/samsone) |
| `niagara_{19m,38m}_batch_en_f32`（2 个） | `unknown` | Upstream license unpublished (待核对) |

**为什么这样就够了**：

- `unknown` 在 UI 上只以「许可未知」徽标呈现，**从不**降级成 `allowed`
- 三处解析都以 `unknown` 兜底：`models.json` 缺字段 → Rust `default_commercial_use()`；前端 `asCommercialUse()`；DTO 原样透传
- 下载前弹窗写明「发布方未提供任何许可证，没有授权可依据，除个人试验外的用途应先取得发布方许可」
- 权重不经 MonoSub 分发，下载直连上游

> `higgs_audio_v3_stt_*` 标注为 `Boson Higgs Audio v3 Research and Non-Commercial License (待核对)`：许可证存在但未经核实，名称本身含 Research and Non-Commercial。已按 `noncommercial` 处理；核实后若要改判，只动 `models.json` 一个字段。



---

## 3. ✅ 已修：README 宣称 YouTube（且为虚假宣传）

```
README.md:38      - **Content creators** turning YouTube videos into blog posts
README_CN.md:38   - **内容创作者** — 将 YouTube 视频转为博客文章
```

**双重问题：**

1. **商标**："YouTube" 是 Google LLC 注册商标。
2. **虚假宣传（更严重）**：产品**根本不支持 YouTube**。URL 导入仅支持 B 站 —— 见 `AGENTS.md:942`、以及 `docs/tech-spec/url-import-design.md` §10「推迟：YouTube（已知约束，供后续立项）」，里面列出了需要 JS 运行时、代理、机翻轨识别等尚未实现的前提。

这直接摧毁了最有力的抗辩理由。商标法上的**指示性合理使用（nominative fair use）**&#x6838;心是「为了说明兼容性而不得不提及」——但这里**不兼容**，所以既不构成指示性使用，又构成误导性陈述。

**处置**：

```diff
- **Content creators** turning YouTube videos into blog posts
+ **Content creators** turning online videos into blog posts

- **内容创作者** — 将 YouTube 视频转为博客文章
+ **内容创作者** — 将网络视频转为博客文章
```

**修复状态（2026-10-26）**：已改完。同一轮还把所有**面向用户的界面文案**里的  
B 站字样一并去掉（见 §8、§9），包括应用 i18n 与落地页两个语种。

---

## 4. ✅ 已修（除 4.2）：默认/推荐模型与商业变现冲突

这是本项目最尖锐的内部矛盾：**产品以付费应用形式在 App Store / Microsoft Store 售卖，但默认/推荐路径上有不可商用模型。**

（原「$20 Pro 授权」模式已于 2026-10-26 移除：Pro 功能门与 Polar 激活链路已删除，  
`src/lib/license.ts` 只剩一个设置项常量，`commands/license.rs` 已并入 `commands/external.rs`。  
商业模式改为「源码公开 + 商店付费」，商业分发的性质不变。）

**这条矛盾的载体已经移除。** 问题出在 `tier`：一个与 `status` 正交的「编辑推荐」枚举，14 个 `recommended` 里有 12 个是 `unverified`，且其中一个明确禁止商用。移除 `tier` 后：

- 默认模型改由 `status: supported` 决定（`default_translation_model_id` 按「已验证优先 + 目录序」选取）
- 求解器评分里的 `RECOMMENDED_TIER_BONUS`（+100）删除，`VERIFIED_BONUS`（+45）成为唯一编辑权重
- 不可商用模型再没有任何路径进入「默认/推荐」—— 它只存在，并被标出来

### 4.1 `crisperwhisper_2_large_*`（3 个）— 仅非商用

```
id: crisperwhisper_2_large_bf16 / _q4_k / _q8_0
commercial_use: noncommercial
license: nyra health Non-Commercial Research License
attribution: "...model weights and outputs under the nyra health
             Non-Commercial Research License (non-commercial use only;
             commercial use requires a separate license)."
```

原状态是 `tier: recommended` —— 产品主动推荐一个明确禁止商用的模型。`tier` 移除后它不再被推荐，剩下的信号是行内红色「仅限非商用」徽标加下载前确认弹窗。

### 4.2 `hy_mt_1_8b_q4_k_m` — 翻译默认模型，许可排除三个法域 🟠 **仍未处理**

```
id: hy_mt_1_8b_q4_k_m
status: supported           ← 唯一的已验证翻译模型，因此也是默认（2026-10-26 由 tier 迁移而来）
commercial_use: restricted
license: Tencent HY Community License Agreement
```

> 迁移说明：这个模型原本靠 `tier: "default"` 占住默认位。移除 `tier` 后，`status` 接替了这个职责，而它此前是 `unverified` —— 若不改为 `supported`，`default_translation_model_id` 会退化成按目录序取第一个（`alma_13b_q4_k_m`，7.8 GB，Llama 2 协议）。已标记为 `supported`，并由 `committed_translation_default_is_the_verified_model` 测试守住。

该协议（发布日期 2025-12-30）原文明确：

> THIS LICENSE AGREEMENT DOES NOT APPLY IN THE EUROPEAN UNION, UNITED KINGDOM  
> AND SOUTH KOREA AND IS EXPRESSLY LIMITED TO THE TERRITORY, AS DEFINED BELOW.

即：**欧盟、英国、韩国境内的使用不在授权范围内，需另行取得腾讯商业授权。** 另外还叠加：

- **MAU ≥ 1 亿**需单独商业授权
- **禁止**用输出训练竞品/其他 AI 模型
- **禁止**以「提供 HY-MT API 服务」为主要商业模式（转售模型能力）
- 要求在**产品显著位置**（关于页 / 设置页 / 文档首页）署名「本产品使用腾讯混元 HY-MT1.5-1.8B 模型，遵循 Hunyuan Community License」并附许可证链接

署名义务**已履行**：`attribution` 在模型行内展示，下载前弹窗还会给出许可证名称与上游链接（见 §6）。

**但地域限制仍未解决，这是全文剩余风险最高的一项。** 弹窗说的是「许可证可能带条件」，它没有、也无法判断用户是否在欧盟/英国/韩国。相当于把这条限制完全交给用户自查。

**处置（待定）**：

- 首选：换一个无地域限制的默认翻译模型（如 `qwen2_5_7b_instruct_q4_k_m`，Apache-2.0），把 HY-MT 降为可选。
- 或按地域路由：EU/UK/KR 的界面语言不自动选中该模型。
- 在此之前，至少要在弹窗的 `restricted` 文案里点名「地域限制」这一条件类型 —— 目前只说「用户量阈值、地域排除、相同方式共享或注册」，属于泛指。

### 4.3 `nemotron_3_diar_q8_0` — 商用 ✅，但需保留声明

OpenMDW-1.1（Linux Foundation permissive）：**商用允许**，对输出无限制；分发时须保留协议副本与来源/版权声明。`commercial_use: allowed`，行内不显示徽标，许可证名称照常展示。

原先的 `tier: default` 只是文档性标记 —— 代码里从来不存在 diar 的默认选择函数，所以移除它没有行为变化（对应的测试断言一并删除）。

### 4.4 现在真正会被选中的模型

`status` 成为唯一编辑轴后，值得盯的只有两类：默认模型，以及全部 `supported` 条目（它们拿到 `VERIFIED_BONUS`）。

| model id | 为什么重要 | status | `commercial_use` | license |
| --- | --- | --- | --- | --- |
| `sensevoice_small_q8` | ASR 默认（`default_model_id()` 硬编码） | supported | `allowed` | FunASR v1.1 — 需署名 §2.2 |
| `hy_mt_1_8b_q4_k_m` | 翻译默认（唯一 `supported` 翻译模型） | supported | `restricted` | Tencent HY — **排除 EU/UK/KR** |
| `fun_asr_nano_2512_q8_0` | 求解器高分候选 | supported | `allowed` | FunASR v1.1 — 需署名 |
| `moss_transcribe_diarize_q8_0` | 求解器高分候选 | supported | `allowed` | Apache-2.0 |
| `qwen3_asr_0_6b_q8_0` | 求解器高分候选 | supported | `allowed` | Apache-2.0 |
| `qwen3_forced_aligner_0_6b_q8_0` | 对齐依赖 | supported | `allowed` | Apache-2.0 |
| `nemotron_3_diar_q8_0` | diar 候选 | unverified | `allowed` | OpenMDW-1.1 — 需保留声明 |
| `crisperwhisper_2_large_*` | 已不再推荐 | unverified | `noncommercial` | nyra 非商用 |

结论：**默认路径上已无不商用模型**。唯一的 `restricted` 默认是翻译模型，其风险是地域排除而非商用禁止。


## 6. ✅ 已修：许可证展示无风险分级

> **更正（2026-10-26）**：初版审查误判为「UI 完全不显示许可证」。实际情况是  
> `Settings.tsx:130-132` 已经渲染了 `${model.attribution} — ${model.license}`，  
> 署名义务**是履行了的**。真正的问题在下面三点呈现缺陷。

`models.json` 里每个模型都有完整的 `license` / `licenses[]`（含 URL）/ `attribution` / `status`  
字段——**数据齐全**。`ModelRow`（`Settings.tsx:124`）也确实把 `attribution — license`  
渲染了出来（`Settings.tsx:130-132, 175-182`），CC-BY-4.0 §3(a)(i)、FunASR §2.2、  
OpenMDW-1.1 的署名义务**均已履行**。

真正的问题在**呈现方式**。三条逐一对应现状：

| 原问题 | 现状 |
| --- | --- |
| 字号极小 + `truncate`，许可证名被省略号截掉 | 行内保持原样（列表密度需要），但下载前弹窗完整显示许可证名，`title` tooltip 仍保留全文 |
| `licenses[].url` 从未被使用，OpenMDW-1.1 等「保留协议副本」义务难满足 | 弹窗提供「查看许可证原文」，走 `invoke('open_external_url')`，失败回落 `window.open` |
| 没有商用风险分级 | `models.json` 新增 `commercial_use` 四档；行内按非 `allowed` 打徽标，下载前弹窗逐档说明条件类型 |

### 实现要点

- **数据**：`commercial_use` 由 `license` 字符串派生，映射表见附录；派生脚本遇到未收录的许可证会直接报错退出，避免新模型静默落错档
- **Rust**：`CommercialUse` 枚举 + `ModelSpec.commercial_use` + DTO 透传。字段缺失时 `default_commercial_use()` 返回 `Unknown`
- **TypeScript**：`ModelCommercialUse` + `asCommercialUse()`，无法识别时同样回落 `unknown`，**绝不**回落 `allowed`
- **UI**：`ModelRow` 徽标（`noncommercial` 红、`restricted`/`unknown` 黄）+ `ModelLicensePrompt` 弹窗
- **两个下载入口都过闸**：`Settings.handleDownloadModel` 与 `MissingModelPrompt.handleDownload`。只堵 Settings 会漏掉更短的那条路径（报错 → 修 → 点下载）

### 为什么是「告知」而不是「禁止」

见 §12。核心：MonoSub 不分发权重，下载直连上游，许可证约束的是**用户的用途**。应用能做的是把事实摆到用户面前，而不是替用户判断用途是否合规。

---

## 7. ✅ 已修：LFM 模型的营收阈值

7 个 `lfm2_5_audio_1_5b_*` 模型使用 **LFM Open License v1.0**（基于 Apache 2.0 改）：

- 年营收 **< $10,000,000**：免费商用
- 年营收 **≥ $10,000,000**：**不再授权**，需向 `sales@liquid.ai` 取得商业许可
- 无 copyleft；但**分发时必须**包含许可证副本、保留所有版权与署名声明、含 NOTICE 文件、标记修改过的文件

**处置**：已归入 `commercial_use: restricted` —— 行内黄色「有条件商用」徽标，下载前弹窗说明「商用可能被允许，但附带用户量阈值、地域排除、相同方式共享或注册等条件」。

**残留**：具体阈值（$10M 年营收）没有写进 UI。弹窗刻意不给数字，因为改述上游条款容易失真，只给原文链接。若要让用户无需跳转就知道，需要给每个模型加一个 `commercial_use_note` 字段承载定制说明 —— 目前未做。

---

## 8. ✅ 已修：B 站商标使用

> **修复状态（2026-10-26）**：按用户决定「完全避免风险」，界面文案里的 B 站字样  
> **全部去除**，改为「网络视频 / 受支持的视频链接 / 该站点」。覆盖范围：  
> `src/i18n/locales/{en,zh-CN}.json` 的 `urlImport.*` 与 `clipboardPrompt.title`、  
> `landing_page/src/i18n/locales/{en,zh-CN}.ts` 的 hero/showcase/feature 三条、  
> 以及 `landing_page/src/i18n/privacy.ts` 的链接导入条款。
>
> **保留不动的**：Rust 侧 `site/*.rs` 里的 `bilibili.com` / `b23.tv` 域名字符串  
> （URL 白名单解析所必需，属功能性代码）与内部标识符  
> `settings.bilibiliCookiesBrowser`（非用户可见）。这些是内部技术引用，不对外构成  
> 商标性使用。
>
> **可用性代价（需知悉）**：不再告知用户具体支持哪些站点，用户只能靠粘贴后  
> 收到的 `unsupported_site` 报错来判断。若日后需要兼顾，可考虑「仅文档/帮助  
> 区列出支持的域名」而不在营销文案中出现品牌名。

以下为原始分析，存档备查：

先说结论：**UI 里出现"Bilibili"是必要且基本可抗辩的，不用删。**

原因是这与 YouTube 的情形有本质区别：产品**确实只支持 B 站**，要告诉用户能用哪些链接就**必须点名**，这正是指示性合理使用（nominative fair use）的典型场景。而且现有文案是纯事实描述（"Paste a Bilibili link and MonoSub fetches the platform subtitle"），**没有暗示授权、背书或合作关系**——这一条没有问题，不要改坏。

**真正的缺口是：全项目没有任何 trademark legend。**

|               |           |              |                                                                                                                                                   |
| ------------- | --------- | ------------ | ------------------------------------------------------------------------------------------------------------------------------------------------- |
| 搜索 \`affiliat | trademark | not endorsed | all rights reserved`在整个前端与文档中只有 3 处命中，且全部是`© 2025 MonoSub. Built by Matrix Agent.\`（landing page footer / spec.md）。**没有任何一处声明第三方平台商标归属或"无关联"关系。** |

**处置**：在 README、landing page footer、应用「关于」页各加一段，例如：

> Bilibili（哔哩哔哩）是上海幻电信息科技有限公司的商标。MonoSub 与该平台无任何关联，亦未获得其授权、赞助或背书。URL 导入功能仅使用用户本机已有的登录凭据，仅供个人学习研究使用。

---

## 9. ✅ 已修：替第三方平台下断言的文案

| 键                               | 现有文案                                                             | 建议                                                            |
| ------------------------------- | ---------------------------------------------------------------- | ------------------------------------------------------------- |
| `urlImport.errors.needsCookies` | "Bilibili requires a login for this video"                       | "This video's subtitle requires a signed-in session"          |
| `urlImport.errors.rateLimited`  | "Bilibili is rate-limiting requests, please try again later"     | "Too many requests, please try again later"                   |
| `urlImport.login.explain`       | "Bilibili needs a login to hand out this subtitle."              | "This subtitle requires a signed-in session."                 |
| `urlImport.login.queuedHint`    | "Bilibili only offers this video's subtitle to signed-in users." | "This video's subtitle is only available to signed-in users." |

用第三方商标去陈述其内部系统行为（"requires a login for this video"、"is rate-limiting"），既不准确也无法核实。改成中性表述，既保留可用性又降低风险。

保留不动的：`urlImport.placeholder`（`https://www.bilibili.com/video/BV…`）、`urlImport.settings.cookiesBrowser`（"Bilibili cookies"）—— 这些是功能性指代，必要且无风险。

---

## 10. 🟡 P2：读取浏览器 cookie 的平台 ToS 风险

`src/components/SettingsUrlImport.tsx:108-109` → `settings.bilibiliCookiesBrowser` → `src/lib/urlImport.ts:322` → yt-dlp `--cookies-from-browser`

**定性**：技术上只在本机使用用户自己的凭据，**不构成**著作权或商标侵权。但它很可能**违反 B 站用户协议**中关于自动化访问、登录态内容获取的条款 —— 这是**合同风险**，且在部分法域会叠加「未经授权访问」的刑事风险（类似 CFAA 的认定逻辑）。

现行实现已经做了正确的两点，要保住：

- 默认值为 `''`（未配置），不主动读取 —— `useStore.ts:386`
- UI 有隐私说明 —— `en.json:336,372`

**补充建议**：在 cookie 设置项旁加一句风险提示，明确「使用本功能前请自行确认符合平台用户协议；MonoSub 不承担由此产生的责任」。

---

## 11. ✅ 已修：仓库根目录无 LICENSE

原状态：公开仓库无 LICENSE，法律上默认「保留所有权利」，他人无任何使用权限。

**最终采用 PolyForm Small Business 1.0.0**（2026-10-26），不是 MIT，也不是 Noncommercial：

- SPDX 标识符 `PolyForm-Small-Business-1.0.0`，同步写入 `package.json` 与 `src-tauri/Cargo.toml`
- 文件内含 `Required Notice:` 行 —— PolyForm 的 Notices 条款要求接收方同时拿到该行
- 末尾声明：该许可证只覆盖 MonoSub 自身代码；模型权重不在其中，也不随应用分发

**为什么不用 MIT**：MIT 允许任何人合法地重新编译并免费再分发，与付费版正面竞争。PolyForm 让源码保持公开（source-available）而不放弃收入。

**为什么从 Noncommercial 改 Small Business**：Noncommercial 下**任何**商业使用都不被授权，包括愿意付费的自由职业者和小工作室。而产品本身就是卖给这类人的 —— 结果是「持续销售、却禁止买家按规定使用」，最坏时可能被主张构成默示许可（implied license）。Small Business 用一条可查的规模门槛替代：**100 人以下且年营收 100 万美元以下**可免费商用，超过才需要单独商业授权。既接住了绝大多数真实付费用户，又留下企业收费的闸门。

**为什么现在才补**：原稿标注的冲突前提已经消失 —— GPL 问题在 §1 解决（FFmpeg 换 LGPL 构建，且以独立子进程调用，构成聚合）。

> 对外文案不要自称「开源 / open source」—— 准确说法是「源码公开 / source-available」。超过门槛的公司需另签商业授权，那份合同**无权覆盖**模型权重（见 `LICENSE` 末尾的 Scope 段）。

---

## 12. 关于「软件本身能否直接下载模型」——分三档

你问的核心问题。当前实现**姿态是对的**（`THIRD-PARTY-NOTICES.md` 已写明 "Not bundled — downloaded by the user at runtime, at the user's request"），但需要按风险分档，而不是一刀切。

**关键法律区别**：「应用自动下载」在法律上接近**应用方分发（distribution）**，可能构成直接侵权或违反许可条款；「用户主动点击下载」则是**用户自行获取**，应用只是提供索引与工具，责任显著更轻（类比浏览器下载、yt-dlp、包管理器）。

下文 A/B/C 是**风险视角**的三档；落到数据里是 `commercial_use` 四值，对应关系：

| 本文章节 | `commercial_use` | 数量 |
| --- | --- | --- |
| A 档 | `allowed` | 78 |
| B 档 — 禁止商用 | `noncommercial` | 16 |
| B 档 — 有条件商用 | `restricted` | 27 |
| C 档 | `unknown` | 8 |

### A 档：可列出 + 可代下载（宽松许可）

| 许可证                       | 数量 | 义务                  |
| ------------------------- | -- | ------------------- |
| Apache-2.0                | 34 | 保留声明与 NOTICE        |
| MIT                       | 23 | 保留版权声明              |
| CC-BY-4.0                 | 14 | **必须署名**            |
| OpenMDW-1.1               | 4  | 分发时保留协议与来源声明；对输出无限制 |
| FunASR Model License v1.1 | 3  | **§2.2 必须署名并保留模型名** |

> 注意：这一档里 `sensevoice_small_q8`（ASR 默认模型）是 FunASR v1.1。上游维护方已澄清：**官方 SenseVoiceSmall 权重在遵守许可前提下允许商用**，第 3 条是责任与风险免责、而非额外的非商用限制；但 **§2.2 的署名与保留模型名义务仍然适用**。该署名已在 `ModelRow` 行内展示，义务已履行。
>
> OpenMDW-1.1 同样如此 —— 本次查证确认其原文为 "permission is hereby granted, free of charge, to deal in the Model Materials without restriction"，商用明确允许，仅要求分发时保留协议与声明。

### B 档：可列出，但**必须**打标 + **不得**作为默认/推荐 + **建议**改为用户自行下载

| 许可证                                                   | 数量 | 限制                                              |
| ----------------------------------------------------- | -- | ----------------------------------------------- |
| CC-BY-NC-4.0                                          | 7  | **禁止商用**                                        |
| nyra health Non-Commercial Research License           | 3  | **禁止商用**，商用需单独授权                                |
| Qwen Research License Agreement                       | 2  | **仅限研究用途**                                      |
| Boson Higgs Audio v3 Research and Non-Commercial（待核对） | 2  | **非商用**；许可证待核实                                  |
| CC-BY-NC-4.0 (source) / Llama 2 Community (base)      | 2  | **禁止商用**                                        |
| LFM Open License v1.0                                 | 7  | 年营收 ≥ $10M 需商业授权                                |
| Tencent HY Community License Agreement                | 1  | **排除 EU/UK/韩国**；MAU ≥1 亿需授权；禁转售                 |
| Llama 3 Community License                             | 6  | AUP；MAU ≥7 亿需授权                                 |
| Llama 2 Community License                             | 3  | AUP；MAU ≥7 亿需授权                                 |
| Gemma Terms of Use                                    | 5  | AUP 限制                                          |
| NetEase Youdao Model Use License                      | 3  | MAU >1 亿 或 营收 >RMB10 亿需单独授权                     |
| NGC Terms of Use                                      | 1  | 需核验上游卡，`citrinet_asr_q8_0` 标注「待核对」              |
| CC-BY-SA-4.0                                          | 1  | `kroko_en_community_64_l_q8_0`；分发衍生品触发 copyleft |

**B 档最终处置**：

1. ✅ UI 显式标记 —— `Commercial use conditional` / `Non-commercial only` 徽标
2. ✅ **不得**进入默认/推荐路径 —— `tier` 字段整体移除，默认改由 `status` 决定，这些模型全是 `unverified`
3. ✅ 下载前确认弹窗 —— 许可证名 + 上游原文链接 + 该档位的限制**类型** + 「MonoSub 不托管、不分发权重，合规责任在你」
4. ❌ 未改为「只给链接」—— 仍保留一键下载

第 4 条没做是有意取舍：产品核心价值就是「点一下能用」，改成外链会让 129 个模型里的 51 个变成手工操作。风险由第 1–3 条承接 —— 徽标让用户在选择时就知道，弹窗让用户在下载时确认。

> 若日后要更保守，最省事的改法是：对 `noncommercial` 的 16 个模型禁用应用内下载，主按钮换成「前往上游页面」。`commercial_use` 字段已经在，改动只是 `ModelRow` 里一个条件分支。

### C 档：无上游许可证 —— 标为 `unknown` 列出，不给任何「可用」暗示

即第 2 节的 8 个模型。没有许可证就没有授权。

原建议是直接移除；最终保留，但：

- `commercial_use: unknown`，行内黄色「许可未知」徽标
- 弹窗直言「发布方未提供任何许可证，没有授权可依据，除个人试验外的用途应先取得发布方许可」
- 解析层三处兜底都是 `unknown`，绝不会因字段缺失而降级成 `allowed`

保留的理由：这 8 个模型在模型调查里有实测数据，删掉等于丢弃测量结果；而「列出 + 明确标注无授权」并不比「静默删除」更像背书。

---

## 13. 修复顺序与实际完成情况

| # | 项目 | 状态 |
| --- | --- | --- |
| 1 | 换 FFmpeg 为 LGPL 构建（P0） | ✅ §1 |
| 2 | 8 个无许可证模型标为 `unknown` + 下载前告知（P0） | ✅ §2 |
| 3 | 改 README 的 YouTube 表述（P0） | ✅ §3 |
| 4a | 移除 `tier`，消除「推荐一个禁止商用的模型」（P1） | ✅ §4 |
| 4b | `hy_mt` 的地域排除（P1） | 🟠 **未处理**，见 §4.2 |
| 5 | UI 许可徽标 + 下载前确认弹窗（P1） | ✅ §6、§7 |
| 6 | 根目录 LICENSE（P2） | ✅ PolyForm Small Business 1.0.0，见 §11 |
| 7 | cookie 读取风险提示（P2） | 🟠 **未处理**，见 §10 |

**剩余两项**都不大，但都需要产品决策而不是纯代码改动：`hy_mt` 要定「换默认模型还是做地域路由」；cookie 提示要定「是否保留读取浏览器 cookie 这个功能」。

---

## 附：许可证分布全表（`models.json`，共 129 个模型）

这一列就是 `models.json` 里 `commercial_use` 字段的派生依据 —— 新增模型时按此表归档，遇到表里没有的许可证要单独判断，不能猜。

| 许可证 | 数量 | `commercial_use` | 备注 |
| --- | --: | --- | --- |
| Apache-2.0 | 34 | `allowed` | 保留版权与 NOTICE |
| MIT | 23 | `allowed` | 保留版权声明 |
| CC-BY-4.0 | 14 | `allowed` | **必须署名** |
| CC-BY-NC-4.0 | 7 | `noncommercial` | 禁止商用 |
| LFM Open License v1.0 | 7 | `restricted` | 年营收 ≥ $10M 需商业授权 |
| Llama 3 Community License | 6 | `restricted` | AUP；MAU ≥ 7 亿需授权 |
| No upstream license published (SamsungLabs/samsone) | 6 | `unknown` | 无授权可依据 |
| Gemma Terms of Use | 5 | `restricted` | AUP 限制 |
| OpenMDW-1.1 | 4 | `allowed` | 保留协议与来源声明；对输出无限制 |
| nyra health Non-Commercial Research License | 3 | `noncommercial` | 商用需单独授权 |
| Llama 2 Community License | 3 | `restricted` | AUP；MAU ≥ 7 亿需授权 |
| NetEase Youdao Model Use License Agreement | 3 | `restricted` | MAU / 营收阈值 |
| FunASR Model License v1.1 | 3 | `allowed` | **§2.2 必须署名并保留模型名** |
| Boson Higgs Audio v3 Research and Non-Commercial（待核对） | 2 | `noncommercial` | 名称含 Research and Non-Commercial，待核实 |
| Upstream license unpublished（待核对） | 2 | `unknown` | 无授权可依据 |
| Qwen Research License Agreement | 2 | `noncommercial` | 仅限研究用途 |
| CC-BY-NC-4.0 (source) / Llama 2 Community (base) | 2 | `noncommercial` | 按更严的 NC 处理 |
| NGC Terms of Use | 1 | `restricted` | 需核验上游卡 |
| Tencent HY Community License Agreement | 1 | `restricted` | **排除 EU / UK / KR**；MAU ≥ 1 亿需授权 |
| CC-BY-SA-4.0 | 1 | `restricted` | 分发衍生品触发 copyleft |

合计：`allowed` 78 / `restricted` 27 / `noncommercial` 16 / `unknown` 8 = **129**。
