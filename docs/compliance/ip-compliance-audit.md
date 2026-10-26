# MonoSub 侵权与合规审查

审查日期：2026-10-26
审查范围：仓库文案 / UI 文案 / `models.json` 模型许可 / 捆绑二进制许可 / 分发方式
结论：**发现 3 项 P0（分发即违规）、5 项 P1、4 项 P2**。其中最严重的是捆绑 GPLv3 FFmpeg。

> 本文为工程梳理，不构成法律意见。涉及 GPL 义务、商标使用、平台 ToS 的处置，请以当地律师意见为准。

---

## 0. 结论速览

| 级别 | 问题 | 位置 | 处置 |
|---|---|---|---|
| 🔴 P0 | 捆绑 **GPLv3** FFmpeg，而本产品闭源 + 商店付费售卖 | `src-tauri/resources/bin/ffmpeg-*` | 换 LGPL 构建或改为用户自备 |
| 🔴 P0 | **8 个模型上游未发布任何许可证**仍列在下载列表 | `models.json` | 移除 |
| ✅ 已修 | ~~README 宣称 **YouTube**，但产品根本不支持~~ | `README.md:38`、`README_CN.md:38` | 已改 "online videos" / "网络视频" |
| 🟠 P1 | **recommended** 模型 `crisperwhisper_2_large_*` 是**仅非商用**许可 | `models.json` | 降级 + 打标 |
| 🟠 P1 | 翻译**默认**模型 `hy_mt_1_8b_q4_k_m` 许可**排除欧盟/英国/韩国** | `models.json` | 地域路由或换默认 |
| 🟠 P1 | 16 个不可商用模型**无任何 UI 标记** | `SettingsModelPlan.tsx` | 加标记 |
| 🟠 P1 | 许可证**只以极小截断文本展示**，且无「商用/非商用」风险分级 | `Settings.tsx:130,175` | 可点击许可证链接 + 醒目标记 |
| 🟠 P1 | 7 个 LFM 模型有 **$10M 年营收**商用阈值 | `models.json` | 提示 |
| ✅ 已修 | ~~使用 B 站商标~~ —— 界面文案已全部去除品牌名 | `en.json`、`zh-CN.json`、`landing_page` 两语种 | 已改为「网络视频 / 受支持的站点」 |
| ✅ 已修 | ~~用第三方商标替平台下断言~~（"Bilibili requires a login"） | `en.json:335,349,352` | 已改为中性表述 |
| 🟡 P2 | 读取浏览器 **bilibili.com cookie**，大概率违反平台 ToS | `SettingsUrlImport.tsx:108` | 加风险提示 |
| 🟡 P2 | 仓库**根目录无 LICENSE** 文件 | repo root | 补（注意与 GPL 冲突） |

---

## 1. 🔴 P0：捆绑 GPLv3 FFmpeg

**证据**（直接运行捆绑二进制）：

```
ffmpeg version 8.1.2-essentials_build-www.gyan.dev
configuration: --enable-gpl --enable-version3 --enable-static
               --enable-libx264 --enable-libx265 --enable-libxvid
               --enable-libass --enable-libvidstab ...
```

文件：`src-tauri/resources/bin/ffmpeg-x86_64-pc-windows-msvc.exe`（101 MB）、`ffprobe-*`（101 MB）

**为什么是 P0：**

`--enable-gpl --enable-version3` 意味着这是 **GPLv3**（不是 LGPL）构建。`libx264`/`libx265`/`libxvid`/`libvidstab` 均为 GPL 组件。将 GPL 二进制**捆绑进一个闭源、且以付费应用形式在商店售卖**的应用并对外分发，构成对 GPL 的直接违反：

- GPLv3 §5(c)：整个作品必须以 GPLv3 授权
- GPLv3 §6：必须提供 Corresponding Source 与安装信息
- 最坏情况：任一版权人可要求停止分发 + 主张侵权

这是整个项目**最严重的单项风险**，且是唯一一个已经"既成事实"（二进制已在仓库与 `dist/*.msix` 中）的问题。

**现有文档的问题：**
`src-tauri/resources/licenses/THIRD-PARTY-NOTICES.md:94-100` 只写了：

> - License: see the FFmpeg project; build-specific (LGPL/GPL) terms apply to the distributed binaries.

这**不构成合规**——必须明确写出具体许可证、构建配置、并提供源码的书面要约（written offer）。

**处置方案（按推荐度）：**

1. **首选**：改用 **LGPL-only** 构建 —— 去掉 `--enable-gpl --enable-version3`，剔除 x264/x265/xvid/vidstab，仅保留 LGPL 编解码器（如自带 VP8/VP9/AV1 解码、AAC 等）。即便如此仍需履行 LGPL 义务：附许可证全文、提供 FFmpeg 源码的书面要约、保留用户重新链接的可能性。
2. **次选**：不捆绑。首次运行时提示用户自备 FFmpeg，或由用户手动触发下载。
3. **若坚持捆绑 GPL 构建**：整个 MonoSub 必须以 GPLv3 开源 —— 与 Pro 商业授权模式冲突。

**额外注意**：`dist/` 下已有 `.msix`。GPLv3 §6 含反 DRM / 反 tivoization 条款，与 Microsoft Store 分发存在潜在冲突，走商店前需单独评估。

---

## 2. 🔴 P0：8 个模型上游未发布任何许可证

无许可证 = 默认"保留所有权利"，列出并引导下载存在直接侵权风险。

| model id | tier | license 字段 |
|---|---|---|
| `samsone_99m_bf16` | experimental | No upstream license published (SamsungLabs/samsone) |
| `samsone_99m_q8_0` | experimental | 同上 |
| `samsone_134m_bf16` | experimental | 同上 |
| `samsone_134m_q8_0` | experimental | 同上 |
| `samsone_356m_bf16` | experimental | 同上 |
| `samsone_356m_q8_0` | experimental | 同上 |
| `niagara_19m_batch_en_f32` | optional | Upstream license unpublished (待核对) |
| `niagara_38m_batch_en_f32` | optional | 同上 |

**处置**：从 `models.json` 移除。若确需保留，必须隐藏在"实验性/未验证"开关后，并在 UI 显著标注"上游未发布许可证，权利状态未知"。

> 注意：另有 `higgs_audio_v3_stt_*` 标注为 `Boson Higgs Audio v3 Research and Non-Commercial License (待核对)` —— 许可证存在但**未经核实**，且从名称看是非商用许可。应按"非商用"处理直至核实完成。

---

## 3. ✅ 已修：README 宣称 YouTube（且为虚假宣传）

```
README.md:38      - **Content creators** turning YouTube videos into blog posts
README_CN.md:38   - **内容创作者** — 将 YouTube 视频转为博客文章
```

**双重问题：**

1. **商标**："YouTube" 是 Google LLC 注册商标。
2. **虚假宣传（更严重）**：产品**根本不支持 YouTube**。URL 导入仅支持 B 站 —— 见 `AGENTS.md:942`、以及 `docs/tech-spec/url-import-design.md` §10「推迟：YouTube（已知约束，供后续立项）」，里面列出了需要 JS 运行时、代理、机翻轨识别等尚未实现的前提。

这直接摧毁了最有力的抗辩理由。商标法上的**指示性合理使用（nominative fair use）**核心是「为了说明兼容性而不得不提及」——但这里**不兼容**，所以既不构成指示性使用，又构成误导性陈述。

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

## 4. 🟠 P1：recommended / default 模型与商业变现冲突

这是本项目最尖锐的内部矛盾：**产品以付费应用形式在 App Store / Microsoft Store 售卖，但默认/推荐路径上有不可商用模型。**

（原「$20 Pro 授权」模式已于 2026-10-26 移除：Pro 功能门与 Polar 激活链路已删除，
`src/lib/license.ts` 只剩一个设置项常量，`commands/license.rs` 已并入 `commands/external.rs`。
商业模式改为「源码公开 + 商店付费」，但商业分发的性质不变，本节结论依旧成立。）

### 4.1 `crisperwhisper_2_large_*`（3 个）— 仅非商用，却标为 recommended

```
id: crisperwhisper_2_large_bf16 / _q4_k / _q8_0
tier: recommended          ← 问题在这里
license: nyra health Non-Commercial Research License
attribution: "...model weights and outputs under the nyra health
             Non-Commercial Research License (non-commercial use only;
             commercial use requires a separate license)."
```

`recommended` 是产品主动向用户推荐。推荐一个明确禁止商用的模型，在商业产品中不可接受。

**处置**：`tier` 降为 `optional` 或 `experimental`，并在 UI 打「Non-commercial only」标记。

### 4.2 `hy_mt_1_8b_q4_k_m` — 翻译默认模型，许可排除三个法域

```
id: hy_mt_1_8b_q4_k_m
tier: default               ← 翻译路径的默认模型
license: Tencent HY Community License Agreement
```

该协议（发布日期 2025-12-30）原文明确：

> THIS LICENSE AGREEMENT DOES NOT APPLY IN THE EUROPEAN UNION, UNITED KINGDOM
> AND SOUTH KOREA AND IS EXPRESSLY LIMITED TO THE TERRITORY, AS DEFINED BELOW.

即：**欧盟、英国、韩国境内的使用不在授权范围内，需另行取得腾讯商业授权。** 另外还叠加：

- **MAU ≥ 1 亿**需单独商业授权
- **禁止**用输出训练竞品/其他 AI 模型
- **禁止**以「提供 HY-MT API 服务」为主要商业模式（转售模型能力）
- 要求在**产品显著位置**（关于页 / 设置页 / 文档首页）署名「本产品使用腾讯混元 HY-MT1.5-1.8B 模型，遵循 Hunyuan Community License」并附许可证链接

`models.json` 里已有 attribution 字段，但 **UI 从不展示**（见第 6 节），署名义务实际未履行。

**处置**：
- 不能做「全球默认」。要么按地域路由（EU/UK/KR 不自选该模型），要么换一个无地域限制的默认模型（如 `qwen2_5_7b_instruct_q4_k_m`，Apache-2.0）。
- 保留则必须在 UI 展示署名 + 许可证链接 + 地域限制提示。

### 4.3 `nemotron_3_diar_q8_0` — default，商用 ✅ 但需保留声明

OpenMDW-1.1（Linux Foundation  permissive）：**商用允许**，对输出无限制；但分发时必须保留协议副本与来源/版权声明。当前无 UI 展示 → 义务未履行。

### 4.4 当前默认/推荐模型总表

| model id | tier | status | role | license | 商用 |
|---|---|---|---|---|---|
| `sensevoice_small_q8` | **default** | supported | asr | FunASR v1.1 | ✅ 需署名 §2.2 |
| `hy_mt_1_8b_q4_k_m` | **default** | unverified | translate | Tencent HY | ⚠️ **排除 EU/UK/KR** |
| `nemotron_3_diar_q8_0` | **default** | unverified | diar | OpenMDW-1.1 | ✅ 需保留声明 |
| `crisperwhisper_2_large_*` | **recommended** | unverified | asr | nyra 非商用 | ❌ **不可商用** |
| `fun_asr_nano_2512_q8_0` | recommended | supported | asr | FunASR v1.1 | ✅ 需署名 |
| `gemma_3_4b_it_q4_k_m` | recommended | unverified | translate | Gemma ToU | ⚠️ AUP |
| `moss_transcribe_diarize_q8_0` | recommended | supported | asr | Apache-2.0 | ✅ |
| `parakeet_tdt_0_6b_v3_*` | recommended | unverified | asr | CC-BY-4.0 | ✅ 需署名 |
| `pulsevad_81k_f32` | recommended | unverified | vad | MIT | ✅ |
| `qwen2_5_7b_instruct_q4_k_m` | recommended | unverified | translate | Apache-2.0 | ✅ |
| `qwen3_8b_q4_k_m` | recommended | unverified | translate | Apache-2.0 | ✅ |
| `vibevoice_asr_streaming_*` | recommended | unverified | asr | MIT | ✅ |

---

## 5. 🟠 P1：16 个不可商用模型未做任何标记

| 许可证 | 数量 | model id |
|---|---|---|
| CC-BY-NC-4.0 | 7 | `aya_23_8b_q4_k_m`, `aya_23_8b_q8_0`, `aya_expanse_8b_q4_k_m`, `aya_expanse_8b_q8_0`, `hviske_v5_3_q8_0`, `sortformer_diar_4spk_q8_0`, `sortformer_diar_4spk_v1_f16` |
| nyra health Non-Commercial Research License | 3 | `crisperwhisper_2_large_bf16`, `crisperwhisper_2_large_q4_k`, `crisperwhisper_2_large_q8_0` |
| Qwen Research License Agreement | 2 | `qwen2_5_3b_instruct_q4_k_m`, `qwen2_5_3b_instruct_q8_0` |
| Boson Higgs Audio v3 Research and Non-Commercial（待核对） | 2 | `higgs_audio_v3_stt_f16`, `higgs_audio_v3_stt_q8_0` |
| CC-BY-NC-4.0 (source) / Llama 2 Community (base) | 2 | `tower_instruct_7b_v0_1_q4_k_m`, `tower_instruct_7b_v0_1_q8_0` |

用户在 Settings → Models 里看到的是一串模型名，**没有任何信息告知他哪些能商用**。一个用小工作室/企业用途的用户可能随手下载 `sortformer_diar_4spk_v1_f16`（CC-BY-NC-4.0）并用于商业项目。

> `THIRD-PARTY-NOTICES.md:65` 其实已经写明 Sortformer 是 CC-BY-NC-4.0 且 "must be removed from any commercial build"，但这只在**源码文件**里，**用户看不到**。

---

## 6. 🟠 P1：许可证只以极小截断文本展示，无风险分级

> **更正（2026-10-26）**：初版审查误判为「UI 完全不显示许可证」。实际情况是
> `Settings.tsx:130-132` 已经渲染了 `${model.attribution} — ${model.license}`，
> 署名义务**是履行了的**。真正的问题在下面三点呈现缺陷。

`models.json` 里每个模型都有完整的 `license` / `licenses[]`（含 URL）/ `attribution` / `status`
字段——**数据齐全**。`ModelRow`（`Settings.tsx:124`）也确实把 `attribution — license`
渲染了出来（`Settings.tsx:130-132, 175-182`），CC-BY-4.0 §3(a)(i)、FunASR §2.2、
OpenMDW-1.1 的署名义务**均已履行**。

真正的问题在**呈现方式**：

1. **字号极小且会截断** —— `text-xxs` + `truncate`，许可证名会被省略号截掉，
   完整内容只在 `title` tooltip 里，用户几乎看不到。
2. **许可证 URL 不可点击** —— 只显示 `license` 名称，`licenses[].url` 从未被使用。
   OpenMDW-1.1 等要求"分发时保留协议副本"，纯文本很难满足。
3. **没有风险分级（最要紧的一点）** —— `status` 徽标表达的是"验证状态"
   （supported / unverified / unsupported），**不是**"能不能商用"。普通用户看到
   `CC-BY-NC-4.0` 和 `Apache-2.0` 两个字符串，无从分辨前者禁止商用。
   一个企业用户完全可能在不知情的情况下把 NC 模型用于商业项目。

**处置**：
- 把许可证名做成可点击链接，跳 `licenses[0].url`
- 增大字号或去掉 `truncate`，让署名完整可读
- 新增一个 `commercial` 语义字段（或由 `license` 推导），在行上打醒目徽标：
  「Non-commercial」「Commercial < $10M revenue」「Not available in your region」

---

## 7. 🟠 P1：LFM 模型的营收阈值

7 个 `lfm2_5_audio_1_5b_*` 模型使用 **LFM Open License v1.0**（基于 Apache 2.0 改）：

- 年营收 **< $10,000,000**：免费商用
- 年营收 **≥ $10,000,000**：**不再授权**，需向 `sales@liquid.ai` 取得商业许可
- 无 copyleft；但**分发时必须**包含许可证副本、保留所有版权与署名声明、含 NOTICE 文件、标记修改过的文件

**处置**：UI 提示营收阈值 + 保留署名。

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

搜索 `affiliat|trademark|not endorsed|all rights reserved` 在整个前端与文档中只有 3 处命中，且全部是 `© 2025 MonoSub. Built by Matrix Agent.`（landing page footer / spec.md）。**没有任何一处声明第三方平台商标归属或"无关联"关系。**

**处置**：在 README、landing page footer、应用「关于」页各加一段，例如：

> Bilibili（哔哩哔哩）是上海幻电信息科技有限公司的商标。MonoSub 与该平台无任何关联，亦未获得其授权、赞助或背书。URL 导入功能仅使用用户本机已有的登录凭据，仅供个人学习研究使用。

---

## 9. ✅ 已修：替第三方平台下断言的文案

| 键 | 现有文案 | 建议 |
|---|---|---|
| `urlImport.errors.needsCookies` | "Bilibili requires a login for this video" | "This video's subtitle requires a signed-in session" |
| `urlImport.errors.rateLimited` | "Bilibili is rate-limiting requests, please try again later" | "Too many requests, please try again later" |
| `urlImport.login.explain` | "Bilibili needs a login to hand out this subtitle." | "This subtitle requires a signed-in session." |
| `urlImport.login.queuedHint` | "Bilibili only offers this video's subtitle to signed-in users." | "This video's subtitle is only available to signed-in users." |

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

## 11. 🟡 P2：仓库根目录无 LICENSE

公开仓库若无 LICENSE 文件，法律上默认"保留所有权利"，他人无任何使用权限。

**注意冲突**：补 LICENSE 前必须先解决第 1 节的 GPL 问题。如果继续捆绑 GPLv3 FFmpeg，则整个分发物必须以 GPLv3 授权 —— 此时根目录 LICENSE 写 MIT 或专有条款会自相矛盾。

---

## 12. 关于「软件本身能否直接下载模型」——分三档

你问的核心问题。当前实现**姿态是对的**（`THIRD-PARTY-NOTICES.md` 已写明 "Not bundled — downloaded by the user at runtime, at the user's request"），但需要按风险分档，而不是一刀切。

**关键法律区别**：「应用自动下载」在法律上接近**应用方分发（distribution）**，可能构成直接侵权或违反许可条款；「用户主动点击下载」则是**用户自行获取**，应用只是提供索引与工具，责任显著更轻（类比浏览器下载、yt-dlp、包管理器）。

### A 档：可列出 + 可代下载（宽松许可）

| 许可证 | 数量 | 义务 |
|---|---|---|
| Apache-2.0 | 34 | 保留声明与 NOTICE |
| MIT | 23 | 保留版权声明 |
| CC-BY-4.0 | 14 | **必须署名** |
| OpenMDW-1.1 | 4 | 分发时保留协议与来源声明；对输出无限制 |
| FunASR Model License v1.1 | 3 | **§2.2 必须署名并保留模型名** |

> 注意：这一档里 `sensevoice_small_q8`（ASR 默认模型）是 FunASR v1.1。上游维护方已澄清：**官方 SenseVoiceSmall 权重在遵守许可前提下允许商用**，第 3 条是责任与风险免责、而非额外的非商用限制；但 **§2.2 的署名与保留模型名义务仍然适用**。当前 UI 未展示该署名 → 义务未履行。

### B 档：可列出，但**必须**打标 + **不得**作为默认/推荐 + **建议**改为用户自行下载

| 许可证 | 数量 | 限制 |
|---|---|---|
| CC-BY-NC-4.0 | 7 | **禁止商用** |
| nyra health Non-Commercial Research License | 3 | **禁止商用**，商用需单独授权 |
| Qwen Research License Agreement | 2 | **仅限研究用途** |
| Boson Higgs Audio v3 Research and Non-Commercial（待核对） | 2 | **非商用**；许可证待核实 |
| CC-BY-NC-4.0 (source) / Llama 2 Community (base) | 2 | **禁止商用** |
| LFM Open License v1.0 | 7 | 年营收 ≥ $10M 需商业授权 |
| Tencent HY Community License Agreement | 1 | **排除 EU/UK/韩国**；MAU ≥1 亿需授权；禁转售 |
| Llama 3 Community License | 6 | AUP；MAU ≥7 亿需授权 |
| Llama 2 Community License | 3 | AUP；MAU ≥7 亿需授权 |
| Gemma Terms of Use | 5 | AUP 限制 |
| NetEase Youdao Model Use License | 3 | MAU >1 亿 或 营收 >RMB10 亿需单独授权 |
| NGC Terms of Use | 1 | 需核验上游卡，`citrinet_asr_q8_0` 标注「待核对」 |
| CC-BY-SA-4.0 | 1 | `kroko_en_community_64_l_q8_0`；分发衍生品触发 copyleft |

**B 档处置**：
1. UI 显式标记（「Non-commercial」「Commercial threshold $10M revenue」「Not available in EU/UK/KR」等）
2. **不得**进入 `default` / `recommended` tier
3. 下载前增加一次确认弹窗，展示许可证名 + URL + 限制摘要，由用户显式确认「我已了解并自行承担合规责任」
4. 建议干脆不提供一键下载，改为展示许可证信息 + 上游页面链接，由用户自行前往获取

### C 档：**不应列出**（无上游许可证）

即第 2 节的 8 个模型。没有许可证就没有授权，列出来引导下载就是侵权风险。建议直接移除。

---

## 13. 建议的修复顺序

1. **换 FFmpeg 构建**（P0，影响最大且已既成事实）
2. **移除 8 个无许可证模型**（P0，改 `models.json` 即可）
3. **改 README 的 YouTube 表述**（P0，一行字）
4. **`crisperwhisper` 降级 + `hy_mt` 换默认/地域路由**（P1，涉及产品默认行为）
5. **UI 展示 license + attribution + 非商用徽标**（P1，工作量最大，但也是署名义务的唯一解法）
6. **补 trademark legend / cookie 风险提示 / 根目录 LICENSE**（P2）

---

## 附：许可证分布全表（`models.json`，共 129 个模型）

| 许可证 | 数量 | 商用 |
|---|---|---|
| Apache-2.0 | 34 | ✅ |
| MIT | 23 | ✅ |
| CC-BY-4.0 | 14 | ✅ 需署名 |
| CC-BY-NC-4.0 | 7 | ❌ |
| LFM Open License v1.0 | 7 | ⚠️ <$10M |
| Llama 3 Community License | 6 | ⚠️ AUP/MAU |
| **No upstream license published (SamsungLabs/samsone)** | 6 | ❌ 未授权 |
| Gemma Terms of Use | 5 | ⚠️ AUP |
| OpenMDW-1.1 | 4 | ✅ 需保留声明 |
| nyra health Non-Commercial Research License | 3 | ❌ |
| Llama 2 Community License | 3 | ⚠️ AUP/MAU |
| NetEase Youdao Model Use License Agreement | 3 | ⚠️ 阈值 |
| FunASR Model License v1.1 | 3 | ✅ 需署名 |
| Boson Higgs Audio v3 Research and Non-Commercial（待核对） | 2 | ❌ |
| **Upstream license unpublished（待核对）** | 2 | ❌ 未授权 |
| Qwen Research License Agreement | 2 | ❌ |
| CC-BY-NC-4.0 (source) / Llama 2 Community (base) | 2 | ❌ |
| NGC Terms of Use | 1 | ⚠️ 待核验 |
| Tencent HY Community License Agreement | 1 | ⚠️ 地域排除 |
| CC-BY-SA-4.0 | 1 | ⚠️ copyleft |
