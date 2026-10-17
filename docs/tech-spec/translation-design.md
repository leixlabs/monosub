# 本地字幕翻译（llama.cpp）设计

> 状态：**已实现**（Phase T0–T5 已落地；PoC 结论见
> [translation-poc.md](./translation-poc.md)。本文中的“待 PoC”项已按实测回填。）
> 相关：[translation-poc.md](./translation-poc.md)、[translation-tasks.md](./translation-tasks.md)、
> [refactor-audiocpp.md](./refactor-audiocpp.md)、[model-selection-design.md](./model-selection-design.md)

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

- 新增 `scripts/build/fetch-llamacpp.sh`：钉死 llama.cpp **b11696**（rolling build；
  v0.7.0 是 pointer-only release，无二进制产物），下载 `macos-arm64` 包、校验 SHA256，
  解出 `llama-server` 到 `src-tauri/resources/bin/llama-server-<target-triple>`
  （命名沿用 `<name>-<target-triple>`）。
- **dylib 必须平铺在 `resources/bin/`**，不能放 `bin/lib/`：`llama-server` 的
  `LC_RPATH` 只有 `@loader_path`，库必须与可执行文件同目录（PoC §2.2）。
- 由 `prepare-resources.sh` 统一调度；`sign-app.sh` 原有的 `bin/*` 循环已覆盖这些
  文件，**不要**再加第二个签名循环。
- 路径解析复用 `engine/cli.rs` 的规则，新增覆盖变量 `MONOSUB_LLAMACPP_PATH`。
- llama.cpp 是 MIT，LICENSE 放进 `resources/licenses/` 并更新 THIRD-PARTY-NOTICES。
- 打包增量实测：**约 68 MB**（`llama-server` + 全部 `lib*.dylib`；tarball 12,222,784 B）。

### 3.2 调用形态：`llama-server`（PoC 已定稿）

| 方案 | 优点 | 缺点 |
|------|------|------|
| `llama-cli` 一次一调用 | 简单、无端口 | 每次都要加载模型；批量控制难 |
| **`llama-server` 子进程** | 模型只加载一次；HTTP 接口稳定；`/health` 就绪探测简单 | 要管端口和进程生命周期 |

**决定 `llama-server`**（T0-3，见 [translation-poc.md](./translation-poc.md) §4）：每个
翻译阶段启动一次（监听 `127.0.0.1` 随机端口），翻完即停；`kill_on_drop` 保证取消时
进程被回收，和 audio.cpp 的取消语义一致。实测冷启动 **0.5–5 s**、RSS **~1.4 GB**、
Metal 生效、41 条字幕整份 **11–14 s**。

> ⚠️ 原稿把「可用 JSON schema / grammar 约束输出」列为 server 的优点——**实测不成立**，
> 见 §4。`llama-cli` 未做同口径基准，落选依据是结构性的（每批都要重付模型加载）。
> 多文件队列里能否复用同一个 server 仍留作后续优化。

---

## 4. 翻译策略

- **批量 + 编号**：把若干条相邻字幕编号后一次发给模型，要求按编号原样返回。
  实测批次 **10 / 20 / 40 全部条数精确**，取 **20** 为默认（`BATCH_SIZE`）：
  上下文够用，单批失败的爆炸半径减半。
- **结果校验**：条数/编号对不上 → 缩小批次重试 → 最终退化为逐条翻译。
  某条仍失败则保留原文，不让整份字幕作废。
- **输出约束：编号行，不用结构化输出。** 原稿写「优先用 server 的 JSON schema /
  grammar」，**实测被推翻**（PoC §5.1）：本模型上 `response_format: json_schema`
  会回显原文（`minItems`/`maxItems` 变体）或返回空数组（array-of-objects 变体），
  原生 GBNF grammar 直接返回 `[]`。因此改用「要求按编号逐行返回」，解析规则为
  `^\s*(\d+)\s*[.)、:：]\s*(.*)$`，无编号的续行归属上一条编号，空行结束当前条。
- **提示词按模型家族区分**：模板配置跟着模型走（`prompt_template`），不写死在 Rust 里。
  但**模板不是照抄模型卡**：实测把卡片的**单条**模板叠在批量编号契约前面，会让模型
  **放弃编号**，20 条批次甚至直接**回显原文未翻译**（PoC §5.4）。批量路径只发编号契约，
  且契约必须自带「显式条数 + 条数句自成一段 + 两行示例」三件要素：缺示例时 2–4 条批次
  **每次都失败**，加上后 2–20 条 27/27 全对。
- **单条（重试最后一级）另走一条路**：单条用编号形式不可靠（返回无编号行或回显指令），
  故改用卡片式单段模板且**答案整体采纳**；中文源的单条指令里目标语言**必须写中文名**，
  否则 `翻译为English` 会让模型输出中文（PoC §5.4）。
- **不发送跨批上文**：实测的措辞只有「指令 + 本批编号行」这一种；加「上一批结尾是…」
  会改变已测行为。短句缺上下文是已知短板（§11），留给「按句合并后再翻」处理。

---

## 5. 源语言判断（何时跳过）

- 任务语言明确（`zh` / `en` / `ja` …）→ 直接与目标语言比较。
- 任务语言为 `auto` → 需要「检测到的语言」，实现为 `commands/transcribe.rs::detected_language`
  的优先级链：
  1. **ASR 自带的语言标签**（SenseVoice 输出里有 `<|zh|>` 等），在 `clean_text` 丢弃前
     提取（`subtitle::postprocess::extract_language_tag`）并按块汇总（`dominant_language`，
     多数票，平票取先见者）；
  2. 拿不到标签时退到请求里显式指定的语种（直出/moss 路径不吐标签）；
  3. 仍无答案时用简单的文字脚本统计兜底（`language::detect_by_script`，只回答
     无歧义脚本）；
  4. 仍不确定就**照常翻译**（`None`）。

  ⚠️ 第 1 步的「标签是否稳定存在」**本机未做 live SenseVoice 观察**（PoC §6）；
  但降级路径保证不会因此静默丢失翻译。
- 归一化：`zh` 与 `zh-tw` 视为不同目标（翻译模型可做简繁转换），比较前统一语言码
  （`language::canonical_tag`：`zh-tw`/`zh-hk`/`zh-Hant` → `zh-Hant`）。
- 目标缺失（`None` / `auto`）视为「没有可译目标」→ 跳过，不报错。

---

## 6. 模型管理

- 翻译模型进 `models.json`，复用现有的下载 / 校验 / 删除 / 安装检查流程。
- 新增 role（如 `translate`）和引擎标识（区分 `audiocpp` / `llamacpp`），
  外加支持的语言列表、提示词模板类型等。
- **选定模型**（T0-2，PoC §3.1）：**HY-MT1.5-1.8B-Q4_K_M**（`hy_mt_1_8b_q4_k_m`）。
  专用翻译模型，GGUF 官方提供，主源 ModelScope + HuggingFace / hf-mirror 两个镜像；
  size 1,133,080,512，SHA256 `4383ac0c…77c7`（HF LFS pointer 与 ModelScope 列表一致）。
  许可为 Tencent HY Community License Agreement（**只作署名记录**，见 AGENTS.md）。
  它不属于任何已有 `source` 家族，而 `Source` 是封闭枚举，因此填 `source: "other"`，
  **不新增变体**。
- **未实测的备选**：HY-MT1.5-7B（更高质量方向）、AngelSlim 的 HY-MT 1.25-bit /
  2-bit GGUF（极限量化方向）。两者都**没有**取回文件，因此不写 URL/size/SHA256；
  1.25-bit 档位必须先实测输出质量再谈收录。
- 用户选择的模型未安装 → 复用 `MODEL_NOT_INSTALLED:<id>`，引导到下载入口。
  开始任务前就检查，避免识别跑完了才发现缺翻译模型。

---

## 7. 设置与 UI

- Settings 新增「翻译」分组（`SettingsTranslation.tsx`；`Settings.tsx` 侧栏从 3 组变 4 组）：
  - 开启翻译（默认关）
  - 目标语言（`TRANSLATE_TARGET_LANGUAGES`，不含 `auto`）
  - 翻译模型下拉（展示安装状态，可直接下载/删除）
  - （可选）输出方式：仅译文 / 原文 + 译文 / 双语字幕 —— **未做**（T6-3）
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

## 9. 与现有代码的对接点（已按实现核对）

| 位置 | 现状 | 影响 / 决定 |
|------|------|-------------|
| `engine/catalog.rs` `Role` | 封闭枚举 `asr/vad/align/diar/passthrough`；`ModelSpec.family` 被当作 audio.cpp family | ✅ 已按计划：新增 `Role::Translate` 与 `engine` 字段（缺省 `audiocpp`）。翻译模型的 `family` 只作提示词模板标识，**永远不进 audio.cpp** |
| `engine/plan.rs` 求解器 | 按 role 生成候选 | ✅ 已按计划：`solve` 先滤出 `engine == Audiocpp && !has_role(Translate)` 的切片，翻译模型选择独立于 `ModelPlan`，未扩展 `StepKind` |
| `commands/models.rs` | 下载/校验/删除通用 | ✅ 已按计划：`list_models` 带出 `engine` 与 `prompt_template`（保持 snake_case） |
| `commands/transcribe.rs` `STEP_NAMES` / `TOTAL_STEPS=4` | 步骤固定 4 步 | `STEP_NAMES` 变 `[&str; 5]`，`"Translating"` **追加在末尾**；已有 4 个名称逐字不变。原稿写「`total_steps` 按请求计算」——实现为 `Steps::new(translating)` 携带 **per-run `total` 字段**（4 或 5），意同而形式不同（前端渲染 `current/total`，固定 4 会显示「5/4」） |
| `derive_output_paths` / 取消清理 | 只覆盖原文各格式 | ✅ 已按计划：译文路径纳入 `derive_output_paths`，mtime 规则不变 |
| `output_path_for` | 用 `set_extension` 推导 | ✅ 已按计划：新增独立函数 `translation_output_path` 生成 `<stem>.<lang>.<ext>`，`output_path_for` 行为一字未改 |
| `subtitle/postprocess.rs::clean_text` | 丢弃 `<\|zh\|>` 等标签 | 新增 `extract_language_tag` / `dominant_language`，`RawChunk` 加 `language` 字段；标签白名单限 `{zh,en,yue,ja,ko}`。⚠️ T0-5 **未做 live SenseVoice 观察**（PoC §6），降级为 `None` ⇒ 照常翻译 |
| `src/lib/transcription.ts` `skipExistingSubtitle` | 只看原文是否存在 | **未改**（保持原样，原文在即跳过）；「原文在、译文缺→只补翻译」仍为 T6-4（可选） |
| `useStore.ts` `PERSIST_VERSION = 3` | — | ✅ 已升到 4，迁移只新增字段（`translateEnabled` / `translateTargetLanguage` / `translationModelId`） |
| `engine/cli.rs` `resolve_bundled_binary` | 私有函数 | ✅ 改 `pub(crate)` 供 `translate/` 复用；新增 `resolve_llamacpp_path` + `MONOSUB_LLAMACPP_PATH` |

---

## 10. 分阶段

| 阶段 | 内容 |
|------|------|
| P0 PoC | ✅ 已完成（[translation-poc.md](./translation-poc.md)）：打包 llama.cpp（约 68 MB）、跑通 HY-MT1.8B 41 条字幕（11–14 s）；体积/速度/格式稳定性已记录；定 `llama-server`。**缺口**：质量回归（T6-2）与 live SenseVoice 标签观察（T0-5） |
| P1 后端 | fetch 脚本 + 路径解析；`translate/` 模块（启动、批量翻译、校验、降级）；接入写字幕之后；catalog 扩展 |
| P2 前端 | 设置分组、模型下载、步骤显示、i18n、persist 迁移 |
| P3 打磨 | `auto` 语言检测、双语输出、队列复用 server、更多模型 |

---

## 11. 风险 / 开放问题

- **包体积**：✅ **已实测**：`llama-server` + `lib*.dylib` 约 **68 MB**；只带 `llama-server` 一个可执行文件（`llama-cli` 等同 tarball 内其他可执行文件不打包）。
- **速度**：✅ **已实测**：冷启动 0.5–5 s、decode ~42–53 tok/s、约 1 分钟 / 200 条；比识别慢但有界、可预测，进度按批反馈。
- **输出格式漂移**：✅ 风险收敛：编号行下 10/20/40 全部条数精确；漂移只出现在被弃用的结构化输出上。第 4 节的校验 + 降级仍保留作为兜底。
- **短句缺上下文**：切得很碎的字幕翻译质量差，可能要按句合并后再翻（T6-6，未做）。
- **模型许可**：只作署名记录（见 AGENTS.md）。HY-MT 为 Tencent HY Community License Agreement，**按需下载、永不打包**。
- **ggml 冲突**：audio.cpp 与 llama.cpp 各自带 ggml；两者均为**独立子进程**（`llama-server` 只通过 HTTP 交互），不链接进同一进程。
- **翻译质量**：**无结论**。PoC 只验证了条数与结构，未做人工抽读判分；需在真实 ≥30 min 素材上补做（T6-2）。
