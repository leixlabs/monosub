# 本地字幕翻译（llama.cpp）设计

> 状态：**方向稿**。只定整体方向、边界和关键取舍；具体字段、参数、提示词等实现时再定。
>
> 相关：[refactor-audiocpp.md](./refactor-audiocpp.md)、[model-selection-design.md](./model-selection-design.md)

---

## 1. 目标

- 识别完成后，用**本地翻译模型**把字幕翻译成用户设定的**目标语言**。
- 推理引擎是 **llama.cpp**，做法和 audio.cpp 一样：**预编译二进制随应用打包**，模型**按需下载**。
- 用户在设置里：**开/关翻译**、选**目标语言**、选**翻译模型**。
- 只有当**任务语言 ≠ 目标语言**时才翻译，否则跳过。

### 非目标

- 不接入云端翻译 API；不引入 Python。
- 不做实时 / 流式翻译，不做术语表、人工校对 UI（以后再说）。
- 翻译不改时间轴：只换文本，时间沿用识别结果。

---

## 2. 整体流程

```
现有流程：提取音频 → 识别 → 后处理 → 写字幕（原文）
                                          │
                       翻译开启 且 源语言 ≠ 目标语言？
                                          │ 是
                                          ▼
                    llama.cpp 翻译 Segment 文本 → 写字幕（译文）
```

要点：

1. **翻译挂在 `subtitle::postprocess` 之后**，输入是已切好、带时间的 `Vec<Segment>`。
   好处：断句和时间轴已经确定，翻译只是逐条替换 `text`，`speaker` 原样保留。
2. **原文字幕先落盘**。翻译是增强项（和 diarization 一样）：翻译失败只告警，
   原文字幕照常交付，任务不判失败。
3. 翻译放在 `commands/transcribe.rs` 的写字幕阶段之后，逻辑本身放到新模块
   `src-tauri/src/translate/`，与 `engine/`（audio.cpp）平级、互不依赖。

---

## 3. 运行方式：llama.cpp 二进制

### 3.1 打包

- 新增 `scripts/build/fetch-llamacpp.sh`：钉死某个 llama.cpp release tag，下载
  `macos-arm64` 包、校验 SHA256，解出所需二进制和 dylib 到
  `src-tauri/resources/bin/`（命名沿用 `<name>-<target-triple>`）。
- 由 `prepare-resources.sh` 统一调度；`sign-app.sh` 要签新加的二进制/dylib。
- 路径解析复用 `engine/cli.rs` 的规则，新增覆盖变量 `MONOSUB_LLAMACPP_PATH`。
- llama.cpp 是 MIT，LICENSE 放进 `resources/licenses/` 并更新 THIRD-PARTY-NOTICES。

### 3.2 调用形态（倾向 `llama-server`）

| 方案 | 优点 | 缺点 |
|------|------|------|
| `llama-cli` 一次一调用 | 简单、无端口 | 每次都要加载模型；批量控制难 |
| **`llama-server` 子进程** | 模型只加载一次；HTTP 接口稳定；可用 JSON schema / grammar 约束输出 | 要管端口和进程生命周期 |

倾向 **`llama-server`**：每个翻译阶段启动一次（监听 `127.0.0.1` 随机端口），
翻完即停；`kill_on_drop` 保证取消时进程被回收，和 audio.cpp 的取消语义一致。
多文件队列里能否复用同一个 server 留作后续优化。

> PoC 先验证：打包体积、Metal 是否正常、冷启动耗时、1.8B 模型翻 1 小时字幕的耗时。

---

## 4. 翻译策略

- **批量 + 编号**：把若干条相邻字幕编号后一次发给模型，要求按编号原样返回；
  带少量上文作为上下文，提升连贯性。
- **结果校验**：条数/编号对不上 → 缩小批次重试 → 最终退化为逐条翻译。
  某条仍失败则保留原文，不让整份字幕作废。
- **输出约束**：优先用 server 的 JSON schema / grammar 约束格式，减少解析失败。
- **提示词按模型家族区分**：不同模型（如 HY-MT 有官方翻译模板）用各自模板，
  模板配置跟着模型走，不写死在 Rust 里。

---

## 5. 源语言判断（何时跳过）

- 任务语言明确（`zh` / `en` / `ja` …）→ 直接与目标语言比较。
- 任务语言为 `auto` → 需要「检测到的语言」：
  - 优先用 ASR 自带的语言标签（SenseVoice 输出里有 `<|zh|>` 等，目前在
    `clean_text` 被丢掉，需要在丢弃前提取并汇总）；
  - 拿不到时，用简单的文字脚本统计兜底；仍不确定就**照常翻译**（模型对同语言输入
    基本会原样返回，代价只是耗时）。
- 归一化：`zh` 与 `zh-tw` 视为不同目标（翻译模型可做简繁转换），比较前统一语言码。

---

## 6. 模型管理

- 翻译模型进 `models.json`，复用现有的下载 / 校验 / 删除 / 安装检查流程。
- 新增 role（如 `translate`）和引擎标识（区分 `audiocpp` / `llamacpp`），
  外加支持的语言列表、提示词模板类型等。
- 候选模型方向（实现时再实测定夺）：
  - **默认**：腾讯 HY-MT1.5-1.8B（专用翻译模型，体积小，GGUF 官方提供）；
  - **高质量**：HY-MT1.5-7B 或通用指令模型（如 Qwen3 系列）的量化版。
- 用户选择的模型未安装 → 复用 `MODEL_NOT_INSTALLED:<id>`，引导到下载入口。
  开始任务前就检查，避免识别跑完了才发现缺翻译模型。

---

## 7. 设置与 UI

- Settings 新增「翻译」分组：
  - 开启翻译（默认关）
  - 目标语言
  - 翻译模型（下拉，展示安装状态，可跳转下载）
  - （可选）输出方式：仅译文 / 原文 + 译文 / 双语字幕
- 默认输出：原文字幕不变，**另写一份译文**，文件名加语言后缀，例如
  `video.srt` + `video.en.srt`，不会覆盖原文。
- 进度：新增 `Translating` 步骤（开启翻译时总步数 4 → 5）。步骤名是前端 i18n
  的冻结契约，**只新增不改名**，两个 locale 同步补 key。
- Store：persist 版本 +1，迁移只**新增**翻译相关默认值，不丢已有设置。
- `TranscribeRequest` 新增翻译相关字段，全部 `Option`，老前端不传即不翻译。

---

## 8. 取消与清理

- 翻译阶段的 server 子进程、临时文件跟随任务生命周期，取消即回收。
- 取消时删除本次任务新写的译文文件，规则同现有逻辑（只删 mtime ≥ 任务开始时间的文件）。

---

## 9. 与现有代码的对接点（核对代码后补充）

| 位置 | 现状 | 影响 / 决定 |
|------|------|-------------|
| `engine/catalog.rs` `Role` | 封闭枚举 `asr/vad/align/diar/passthrough`；`ModelSpec.family` 被当作 audio.cpp family | 新增 `Role::Translate` 与 `engine` 字段（缺省 `audiocpp`）。翻译模型的 `family` 只作提示词模板标识，**永远不进 audio.cpp** |
| `engine/plan.rs` 求解器 | 按 role 生成候选 | 必须保证 `translate` 模型**不进入**识别方案；翻译模型选择独立于 `ModelPlan`，不扩展 `StepKind` |
| `commands/models.rs` | 下载/校验/删除通用 | 基本复用；`list_models` 需带出 role/engine 供前端分组 |
| `commands/transcribe.rs` `STEP_NAMES` / `TOTAL_STEPS=4` | 步骤固定 4 步 | 新增 `"Translating"`（i18n key `steps.translating`）；`total_steps` 改为按请求计算（4 或 5），已有 4 个名称不变 |
| `derive_output_paths` / 取消清理 | 只覆盖原文各格式 | 译文路径也要纳入，取消时才会被清理 |
| `output_path_for` | 用 `set_extension` 推导 | 译文命名 `<stem>.<lang>.<ext>`，提供独立的推导函数，不改原文路径行为 |
| `subtitle/postprocess.rs::clean_text` | 丢弃 `<\|zh\|>` 等标签 | 自动语言检测需要在丢弃前提取；`RawChunk` 增加可选 `language` 字段 |
| `src/lib/transcription.ts` `skipExistingSubtitle` | 只看原文是否存在 | 开启翻译时：原文在、译文缺 → 只补翻译还是整体跳过？P1 先按「原文在即跳过」，补翻译放 P3 |
| `useStore.ts` `PERSIST_VERSION = 3` | — | 升到 4，迁移只新增字段 |
| `engine/cli.rs` `resolve_bundled_binary` | 私有函数 | 改为 `pub(crate)` 供 `translate/` 复用，不复制路径规则 |

---

## 10. 分阶段

| 阶段 | 内容 |
|------|------|
| P0 PoC | 打包 llama.cpp、跑通 HY-MT 1.8B 翻译一份真实字幕；记录体积、速度、质量、格式稳定性；定 cli/server |
| P1 后端 | fetch 脚本 + 路径解析；`translate/` 模块（启动、批量翻译、校验、降级）；接入写字幕之后；catalog 扩展 |
| P2 前端 | 设置分组、模型下载、步骤显示、i18n、persist 迁移 |
| P3 打磨 | `auto` 语言检测、双语输出、队列复用 server、更多模型 |

---

## 11. 风险 / 开放问题

- **包体积**：llama.cpp 二进制 + dylib 会增大 DMG，需实测；只带必要的那一个可执行文件。
- **速度**：长视频字幕条数多，翻译可能比识别还慢，需要进度反馈和合理批次。
- **输出格式漂移**：小模型可能漏条、合并、加解释 → 依赖第 4 节的校验 + 降级。
- **短句缺上下文**：切得很碎的字幕翻译质量差，可能要按句合并后再翻。
- **模型许可**：只作署名记录（见 AGENTS.md），但仍需确认可下载分发条款。
- **ggml 冲突**：audio.cpp 与 llama.cpp 各自带 ggml，分进程运行即可规避，不要链接进同一进程。
