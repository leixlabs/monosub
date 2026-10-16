# 本地字幕翻译 — 任务清单

> 设计见 [translation-design.md](./translation-design.md)（下称 spec）。
> 约定：每个任务独立可提交；`依赖` 写必须先完成的任务；完成时勾选，并在「状态」写清证据。
> 粒度：S ≤ 半天，M ≈ 1 天，L ≈ 2–3 天。

---

## 0. 决策基线

| 项 | 决定 |
|----|------|
| 推理引擎 | llama.cpp 预编译二进制随包，单独进程运行（不与 audio.cpp 共用 ggml） |
| 调用形态 | 倾向 `llama-server`，T0 PoC 后定稿 |
| 挂载点 | `subtitle::postprocess` 之后，原文字幕写完再翻译 |
| 失败语义 | 增强项：翻译失败只告警，原文照常交付 |
| 输出 | 原文不动，另写 `<stem>.<lang>.<ext>` |
| 模型 | 进 `models.json`，按需下载；默认候选 HY-MT1.5-1.8B（待 PoC 确认） |

---

## Phase T0 — PoC（Go/No-Go）

| ID | 任务 | 规模 | 依赖 | 验收 | 状态 |
|----|------|------|------|------|------|
| T0-1 | 选定 llama.cpp release tag；下载 `macos-arm64` 包，记录 SHA256、解压后体积、需要的可执行文件与 dylib | S | — | 数据记入 PoC 文档 | ⬜ |
| T0-2 | 选 2–3 个候选模型（HY-MT1.5-1.8B / 7B、Qwen3 某量化），记录 URL、size、SHA256、许可、是否有 ModelScope/hf-mirror 源 | S | — | 候选表 | ⬜ |
| T0-3 | 用一份真实 SRT（≥30 min，中→英、英→中各一份）分别测 `llama-cli` 与 `llama-server`：冷启动、总耗时、内存峰值、Metal 是否生效 | M | T0-1,2 | 数据表；定 cli/server | ⬜ |
| T0-4 | 批量编号翻译：比较批次大小（如 10/20/40 条）对漏条、合并、加解释的影响；验证 JSON schema / grammar 约束是否有效 | M | T0-3 | 推荐批次与输出约束方式 | ⬜ |
| T0-5 | 确认 SenseVoice 输出里语言标签（`<\|zh\|>` 等）在 `--text-out` 中是否稳定存在 | S | — | 结论（决定 T4-1 方案） | ⬜ |
| T0-6 | 产出 `docs/tech-spec/translation-poc.md`，回填 spec 中所有「待 PoC」项 | S | T0-1..5 | 文档合入；spec 已更新 | ⬜ |

---

## Phase T1 — 打包与基础设施

| ID | 任务 | 规模 | 依赖 | 验收 | 状态 |
|----|------|------|------|------|------|
| T1-1 | `scripts/build/fetch-llamacpp.sh`：钉版本 + SHA256，解出二进制到 `src-tauri/resources/bin/<name>-<triple>`（dylib 放 `bin/lib/` 或按 rpath 处理）；幂等；更新 `.gitignore` | S | T0-6 | 重复运行无副作用 | ⬜ |
| T1-2 | `prepare-resources.sh` 调用 T1-1 并重新生成 MANIFEST | S | T1-1 | 一条命令备齐全部资源 | ⬜ |
| T1-3 | `scripts/release/sign-app.sh` 签名新增二进制与 dylib；`build-macos-app.sh` 打包后能运行 | S | T1-1 | 已签名 DMG 中 server 可启动 | ⬜ |
| T1-4 | `resources/licenses/` 加 llama.cpp MIT 文本，更新 `THIRD-PARTY-NOTICES.md` | S | T1-1 | 文件就位 | ⬜ |
| T1-5 | `engine/cli.rs`：`resolve_bundled_binary` 改 `pub(crate)`；新增 `MONOSUB_LLAMACPP_PATH` 覆盖 | S | — | 单测覆盖 env 覆盖与缺失报错 | ⬜ |

---

## Phase T2 — 模型目录

| ID | 任务 | 规模 | 依赖 | 验收 | 状态 |
|----|------|------|------|------|------|
| T2-1 | `catalog.rs`：新增 `Role::Translate`、`engine` 字段（缺省 `audiocpp`）、可选 `prompt_template`；解析兼容老条目 | S | — | 现有 catalog 测试全绿；新增解析测试 | ⬜ |
| T2-2 | `plan.rs`：确保 `translate` / `engine=llamacpp` 模型不进入识别方案 | S | T2-1 | 单测：catalog 含翻译模型时 `solve` 结果不变 | ⬜ |
| T2-3 | `models.json` 加入 T0 选定的翻译模型条目（pinned URL、镜像、size、SHA256、licenses、attribution） | S | T0-6, T2-1 | 能经 `download_model` 下载并校验通过 | ⬜ |
| T2-4 | `commands/models.rs` / `list_models` 返回 role 与 engine，供前端分组 | S | T2-1 | 前端可区分识别模型与翻译模型 | ⬜ |
| T2-5 | 新增 `translation_models` 查询或复用 `list_models` 过滤；给出默认翻译模型 id | S | T2-3 | 单测 | ⬜ |

---

## Phase T3 — 翻译模块（Rust，`src-tauri/src/translate/`）

| ID | 任务 | 规模 | 依赖 | 验收 | 状态 |
|----|------|------|------|------|------|
| T3-1 | `translate/server.rs`：启动 `llama-server`（`127.0.0.1` 随机端口、Metal，失败退 CPU）、就绪探测、`kill_on_drop`、stderr 环形缓冲；错误映射到独立的 `TranslateError` | M | T1-5 | 缺二进制/缺模型/启动超时各有明确错误；取消后无残留进程 | ⬜ |
| T3-2 | `translate/prompt.rs`：按模型模板构造批量请求（编号 + 少量上文）；模板随 catalog 走 | S | T2-1 | 纯函数单测 | ⬜ |
| T3-3 | `translate/parse.rs`：解析编号结果并校验条数/编号；纯函数 | S | — | 单测：漏条、乱序、多余解释、空输出 | ⬜ |
| T3-4 | `translate/mod.rs`：`translate_segments(segments, src, tgt, model, on_progress) -> Vec<Segment>`；批次失败→缩小批次→逐条→保留原文；只改 `text`，保留时间与 `speaker` | M | T3-1..3 | 用假 server（mock HTTP）的集成测试；条数与时间轴恒等 | ⬜ |
| T3-5 | 语言码归一化与「是否需要翻译」判断（`zh` vs `zh-tw` 视为不同；`auto` 的处理按 T4） | S | — | 纯函数单测 | ⬜ |
| T3-6 | 需要真实二进制/模型的测试在缺失时打印 `skip:` 并通过 | S | T3-4 | 新克隆仓库测试全绿 | ⬜ |

---

## Phase T4 — 接入转写任务

| ID | 任务 | 规模 | 依赖 | 验收 | 状态 |
|----|------|------|------|------|------|
| T4-1 | 源语言检测：`clean_text` 丢弃前提取语言标签，`RawChunk` 增加 `language: Option<String>`，汇总得到主语言；拿不到时简单脚本统计兜底 | M | T0-5 | 单测；直出（moss）路径无标签时返回 `None` 不报错 | ⬜ |
| T4-2 | `TranscribeRequest` 新增 `translate_enabled` / `target_language` / `translation_model`（全部 `Option`，缺省不翻译） | S | — | 老请求行为完全不变 | ⬜ |
| T4-3 | `start_transcription` 预检：开启且需翻译时检查翻译模型已安装，否则 `MODEL_NOT_INSTALLED:<id>`（识别开始前） | S | T2-5, T4-2 | 单测 / 手测 | ⬜ |
| T4-4 | `run_transcription`：写完原文后进入 `Translating` 步骤，调用 T3-4，写 `<stem>.<lang>.<ext>`；失败只 `warn` 不判任务失败 | M | T3-4, T4-1..3 | 端到端：开/关、同语言跳过、翻译失败仍完成 | ⬜ |
| T4-5 | 步骤：新增 `"Translating"`；`total_steps` 按请求动态（4/5）；现有 4 个名称不变 | S | T4-4 | 测试固定步骤名集合 | ⬜ |
| T4-6 | 译文路径纳入 `derive_output_paths`，取消时按 mtime 规则一并清理 | S | T4-4 | 单测 | ⬜ |
| T4-7 | 并发：翻译阶段也走 `TRANSCRIBE_SEMAPHORE`（或独立信号量），避免与 ASR 抢 GPU | S | T4-4 | 多文件队列手测无 OOM | ⬜ |

---

## Phase T5 — 前端

| ID | 任务 | 规模 | 依赖 | 验收 | 状态 |
|----|------|------|------|------|------|
| T5-1 | `useStore.ts`：新增 `translateEnabled`(false) / `translateTargetLanguage` / `translationModelId`；`PERSIST_VERSION` 3→4，迁移只新增 | S | — | 老数据迁移后原有设置不丢 | ⬜ |
| T5-2 | `src/lib/transcription.ts` 传新字段；`MODEL_NOT_INSTALLED` 处理覆盖翻译模型 | S | T4-2, T5-1 | 缺模型时跳到对应下载入口 | ⬜ |
| T5-3 | Settings 新增「翻译」分组：开关、目标语言、模型下拉（显示安装状态、可下载） | M | T2-4, T5-1 | tauri-pilot 手测 | ⬜ |
| T5-4 | Models 区分组展示识别模型 / 翻译模型 | S | T2-4 | 手测 | ⬜ |
| T5-5 | `FileItem.tsx` 显示 `Translating` 步骤；`BottomBar` 显示目标语言 | S | T4-5 | 手测 | ⬜ |
| T5-6 | i18n：`en.json` / `zh-CN.json` 同步新增 key（含 `steps.translating`） | S | T5-3..5 | `bun run check:i18n` 通过 | ⬜ |
| T5-7 | `src-tauri/tests/pilot/smoke.toml` 增加翻译设置开关的冒烟步骤 | S | T5-3 | `tauri-pilot run` 通过 | ⬜ |

---

## Phase T6 — 收尾与打磨

| ID | 任务 | 规模 | 依赖 | 验收 | 状态 |
|----|------|------|------|------|------|
| T6-1 | 更新 `AGENTS.md`：架构图、目录、IPC 字段、步骤名、环境变量、资源脚本 | S | T4,T5 | 文档与代码一致 | ⬜ |
| T6-2 | 质量回归：T0 的样例字幕跑一遍，记录耗时与人工抽查结果 | S | T4-4 | 记录入 PoC 文档 | ⬜ |
| T6-3 | （可选）双语字幕输出（原文 + 译文同 cue） | M | T4-4 | — | ⬜ |
| T6-4 | （可选）`skipExistingSubtitle` 在原文存在、译文缺失时只补翻译 | M | T4-4 | — | ⬜ |
| T6-5 | （可选）队列内复用同一个 `llama-server` 实例 | M | T4-4 | 多文件总耗时下降 | ⬜ |
| T6-6 | （可选）短句合并后再翻译以改善上下文 | M | T3-4 | — | ⬜ |

---

## 每阶段完成前

`bun run test:rust`、`bun run lint:rust`、`bun run check`、`bun run lint`、`bun run check:i18n` 全部通过。
