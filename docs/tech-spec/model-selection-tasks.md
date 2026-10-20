# 模型方案选择 — 任务台账（Progress Ledger）

> 对应设计：[model-selection-design.md](./model-selection-design.md)
> （需求 → ModelPlan 求解器；取代 `refactor-audiocpp.md` §5.7 / `tasks.md` P4-3）
>
> 本文件是**这份工作流唯一的进度记录**。每完成一项就把 `[ ]` 改成 `[x]`，
> 并在 **状态** 列写清「已验证 / 待验证」，附上证据（命令输出、文件、行号）。
> 不要只写「已完成」——写清**凭什么说完成了**。

---

## 0. 决策基线（已定，勿再反复）

| 项 | 决定 | 依据 |
|----|------|------|
| 许可红线 | **取消**。个人学习项目、非商业分发 → 许可只作署名记录，不作选型门槛 | 用户要求；已写入 `AGENTS.md` |
| Pro 授权门 | **移除**。`is_pro_licensed` 及相关 Polar 子系统全部删除 | 用户要求；见 Stage A |
| MOSS 下载源 | **ModelScope**（`audio-cpp/audio.cpp-gguf`） | 用户要求；实测 206 + `GGUF` magic + pinned rev + SHA256 |
| CAM++ / 声纹 | **不再折腾**。v0.9.1 的 114 个 loader 里无 `spk`/`sv` task | 实测 `--list-loaders --json` |
| 模型选择 | **需求驱动求解**，Settings 可手动覆盖 | 用户要求 |
| 直出模式 | **（已改）不是用户配置项**：由求解器在运行时判断「是否有一个模型能一步覆盖全部所需步骤」而自动选择 | 用户 2026-10-16 反馈：本质是运行时逻辑判断 |
| 阿里系优先 | `source: "tongyi"` 在打分中加权 | 用户要求 |

---

## Stage A — 移除 Pro 授权门（用户直接要求，优先执行）

| # | 项 | 状态 | 证据 |
|---|----|------|------|
| A1 | 删除 `src-tauri/src/commands/license.rs` | ✅ 已验证 | 文件已删除（`git status` 显示 `D`） |
| A2 | 新建 `src-tauri/src/commands/external.rs`（仅保留 `open_external_url`） | ✅ 已验证 | 新文件 1387 B；Settings 更新检查仍需要它 |
| A3 | `commands/mod.rs` 改注册 `external` | ✅ 已验证 | `pub mod external;` / `pub use external::*;` |
| A4 | `main.rs` 撤销 `activate_license` / `validate_license` / `deactivate_license` 注册 | ✅ 已验证 | 三个 handler 已移除，保留 `open_external_url` |
| A5 | `transcribe.rs` 删除 `is_pro_licensed` 门，`enable_diarization` 不再直接报错 | ✅ 已验证 | 删除 `use crate::commands::license;` 与整段 gate；`cargo check` 通过 |
| A6 | `Cargo.toml` 移除 `hmac` / `chrono`（仅 licence 使用） | ✅ 已验证 | 两行已删；`sha2` 保留（`commands/models.rs` 校验下载用）；`cargo check` 通过 |

**Stage A 未完成的部分（前端，归入后续 M9）**

| # | 项 | 状态 |
|---|----|------|
| A7 | `src/lib/license.ts` 删除 Polar 逻辑（保留 `SETTINGS_OPEN_CATEGORY_STORAGE_KEY`） | ✅ 已验证 | `license.ts` 现仅剩该常量（156 B） |
| A8 | `store/useStore.ts` 删除 `licenseConfig` / `licenseStatus` 等 12 个字段 + `LicenseConfig` 类型 | ✅ 已验证 | 连同 `normalizeLicenseConfig`、`PersistedAppState`/`partialize` 引用一并删除 |
| A9 | `Settings.tsx` 删除 license 分类与区块 | ✅ 已验证 | 分类 union / guard / 侧栏项 / 整个 section / 全部 hooks 已删；`GlassInput` 随之移除 |
| A10 | `BottomBar.tsx` 删除 PRO 徽标与 `openLicenseCheckout` | ✅ 已验证 | 徽标块、订阅、import 已删；多人开关**保持原样**（M9-2 再解禁） |
| A11 | i18n：`settings.license.*`、`bottomBar.proRequired/licensePrompt*` 双语言移除 | ✅ 已验证 | `check:i18n` OK — 两语言各 **97** key |
| A12 | `App.tsx` 删除 `validateLicenseOnStartup` 调用 | ✅ 已验证 | import 与调用均已删除 |

> 注：前端移除会**顺带**改动 M9（BottomBar 解禁多人开关），二者合并做更省事。

---

## M1 — `models.json` v2 schema + `catalog.rs` 解析

| # | 项 | 状态 | 证据 |
|---|----|------|------|
| M1-1 | `models.json` 升 `version: 2`，加 `capabilities` / `roles` / `speed` / `source` / `tier` / `licenses[]` | ✅ 已验证 | 7 条目全部带 v2 字段；`cargo test` 123 通过 |
| M1-2 | 新增 `moss_transcribe_diarize_q8_0`（1.13 GB，**默认**）与 `q4_k`（758 MB，`tier: optional`）条目 | ✅ 已验证 | 上游 README：Q4_K「not parity-safe: segment/timestamp drift」→ 默认改 Q8_0；两者 URL 均实测 206 |
| M1-3 | ~~新增 `nemotron_3_diar_q8_0`（107 MB）条目~~ **取消**：ModelScope 上游 `Nemotron-3-Diarization-GGUF` 已下架（树不存在、pinned rev 404） | ❌ 已取消 | 实测 404；改由 `sortformer_diar` 承担 diar 角色（见 M10-5 §8） |
| M1-4 | 新增 `fun_asr_nano_2512_q8_0`（1.05 GB）条目（languages 仅 auto/zh/en/ja） | ✅ 已验证 | URL 实测 206 |
| M1-5 | 新增 `qwen3_asr_0_6b_q8_0` + `qwen3_forced_aligner_0_6b_q8_0` 条目 | ✅ 已验证 | 两者 URL 均实测 206 |
| M1-6 | （可选）`sortformer_diar_4spk_q8_0`（176 MB，**CC-BY-NC**，按新政策可收录） | ✅ 已验证 | 收录为 `tier: optional`；attribution 标注非商业 |
| M1-7 | 现有 `sense_asr` 条目补 `capabilities` / `roles` / `source: "tongyi"` / `tier: "default"` | ✅ 已验证 | 原有 URL/size/sha256 逐字保留（回归测试护栏） |
| M1-8 | `catalog.rs`：`ModelSpec` 加新字段，`load_catalog` 解析（`serde(default)` 兼容） | ✅ 已验证 | 新增 `Capabilities`/`TimestampGrade`/`DiarizationGrade`/`Role`/`Speed`/`Source`/`Tier`/`License` |
| M1-9 | 兜底逻辑：`capabilities` 缺失 → `{timestamps:"segment", diarization:"none", produces_text:true}`；`roles` 缺失 → `["asr"]` | ✅ 已验证 | `ModelSpec::capabilities()` / `has_role()` |
| M1-10 | 单测：新字段解析 + 兜底 + 旧格式（无 capabilities）仍可加载 | ✅ 已验证 | 新增 4 个 catalog 单测（含 v1 兜底 + diar-only/aligner 区分 + fun_asr_nano 语种） |

### 已验证的清单数据（ModelScope，可直接写入，无需再查）

来源：`https://www.modelscope.cn/api/v1/models/audio-cpp/audio.cpp-gguf/repo/files`
下载 URL 模板：`https://www.modelscope.cn/models/audio-cpp/audio.cpp-gguf/resolve/master/<Path>`
✅ 实测：HTTP **206**（Range 可用）、前 4 字节 = `GGUF`

| 模型 | Path | Size (bytes) | SHA256 | Revision |
|------|------|--------------|--------|----------|
| MOSS q4_k | `MOSS-Transcribe-Diarize-GGUF/moss-transcribe-diarize-q4_k.gguf` | 758293216 | `4e3020379ff592bbb22a4e3d47a7cd6709951cc5d451c561b69df955b5f0a220` | `81a1d0db82fe7fad00a742367b1c0989ee955ba7` |
| MOSS q8_0 | `…/moss-transcribe-diarize-q8_0.gguf` | 1132110560 | `93eea5865615e270b827752945f2dfd0f522ed673f4f551675e9013170173679` | `81a1d0db…` |
| MOSS bf16 | `…/moss-transcribe-diarize-bf16.gguf` | 1833018080 | `5bc627289e2586fc2d9269afda15a305e545a4ff3512c902889be71d58e56ad5` | `81a1d0db…` |
| Nemotron q8_0 | `Nemotron-3-Diarization-GGUF/nemotron-3-diarization-q8_0.gguf` | 106675136 | `9a737455bd10123bcf1e036d9a0b07b6e8c42e7d0dd1a5ee4141dc386db46d0b` | `354586fd5a1ae00b9d5b908fb18fe087cab30b8a` |
| Nemotron bf16 | `…/nemotron-3-diarization-bf16.gguf` | 198720928 | `84f6f0f12b9ecf2615548427e884534a72fedbca32d00872d95c0f72a078c953` | `354586fd…` |
| Fun-ASR-Nano q8_0 | `Fun-ASR-Nano-2512-GGUF/fun-asr-nano-2512-q8_0.gguf` | 1045334432 | `4d727357574b079b7f43336b2930f39da086ca02f5d8d50872090b4c1c3d5e0a` | `0e86630cc8c5514fb82683f79c0a07f3e3869377` |
| Qwen3-ASR q8_0 | `Qwen3-ASR-0.6B-GGUF/qwen3-asr-0.6b-q8_0.gguf` | 1151272416 | `6c44ec2fb4cee513892d7863c1fcc3ea6b699ffa4d899b0ef4ab19956d9544f7` | `67d0ca7390694a24bf9d44497c474b93b2e3863d` |
| Qwen3-Aligner q8_0 | `Qwen3-ForcedAligner-0.6B-GGUF/qwen3-forced-aligner-0.6b-q8_0.gguf` | 1129966496 | `75209490b11cec2b0db749ca5f4ff92266f58efd30f7fd04d9eb2a3ac9cc929f` | `67d0ca73…` |
| Sortformer q8_0 | `Sortformer-Diar-4spk-v1-GGUF/sortformer-diar-4spk-v1-q8_0.gguf` | 175662720 | `4fa6a3e30c4a1c6cc1da455268806edc93432ca1e1b5b6923e942be8e6e479ff` | `67d0ca73…` |

> ⚠️ ModelScope 的 `Revision` 是 **commit sha**，与 `master` 分支分开。
> 下载 URL 用 `resolve/master/...` 即可（已验证可达）；若要**钉死版本**，
> 换成 `resolve/<Revision>/...`。建议钉版本，并在 `sha256` 字段写上表内值。

---

## M2 — `engine/plan.rs` 求解器（纯函数）

| # | 项 | 状态 |
|---|----|------|
| M2-1 | `Requirements` / `ModelPlan` / `PlanStep` / `StepKind` / `Capabilities` 类型 | ✅ 已验证 | `engine/plan.rs`；**`Requirements` 不含直出字段**——`PassthroughMode` 已删除（用户 2026-10-16 反馈） |
| M2-2 | `templates_for(req)`：4 种需求组合的候选模板（设计 §5.2） | ✅ 已验证 | `Template` + `TemplateGate`；`Only`/`Never` 正确裁剪模板集 |
| M2-3 | `best_model_for_role()`：硬过滤（roles / languages / installed） | ✅ 已验证 | 复用 `catalog::map_language`，未重复实现语种判断 |
| M2-4 | `score()`：阿里系 +60、tier、speed、installed、步数、内存风险（设计 §5.3） | ✅ 已验证 | `memory_risk` 为纯函数（量化档 + Metal 体积），有独立单测 |
| M2-5 | `gaps()`：需求与能力差集 → 缺口解释 + 建议下载 | ✅ 已验证 | `Gap { requirement, message, suggested_model_id }`，挂在 `ModelPlan.gaps` |
| M2-6 | 语言偏置表（zh→sense；ko 排除 fun_asr_nano；yue 仅 sense；es/de/fr/pt→非 SenseVoice） | ✅ 已验证 | 有 `korean_excludes_fun_asr_nano` / `yue_is_served_only_by_sense_asr` / 西语→qwen 单测 |
| M2-7 | 单测 8 项（设计 §11.1） | ✅ 已验证 | 含全部 8 个要求用例 + 对真实 `models.json` 的集成护栏（默认→sensevoice；要说话人→自动选 moss；组合方案仍作为候选） |

---

## M3 — `resolve_model_plan` 命令 + Settings 只读预览

| # | 项 | 状态 |
|---|----|------|
| M3-1 | `commands/plan.rs`：`resolve_model_plan(requirements) → plan + candidates + gaps` | ✅ 已验证 | camelCase DTO；请求字段全可选（word/diar/language/speed/device）；**无直出字段** |
| M3-2 | `main.rs` 注册该命令 | ✅ 已验证 | `generate_handler!` + `commands/mod.rs` 均注册 |
| M3-3 | `src/lib/modelPlan.ts` 包装 + 类型 | ✅ 已验证 | `resolveModelPlan` + 完整 TS 类型 + `planHasGaps`/`firstGapSuggestion` |
| M3-4 | `SettingsModelPlan.tsx`：需求勾选 + 实时预览 + `ⓘ 为什么？` | ⬜ 待办 |
| M3-5 | i18n 新增 `modelPlan.*` 约 28 个 key（双语言，设计 §8.3） | ⬜ 待办 |

---

## M4 — 目录条目落地 + 下载验证

| # | 项 | 状态 |
|---|----|------|
| M4-1 | 按上表写入 URL/SHA256/revision | ✅ 已验证 | 6 个新条目全部写入 `models.json`，URL 均实测 **206** |
| M4-2 | 实跑一次 `download_model`（MOSS q4_k）验证 206 + SHA256 通过 | ✅ 已验证 | 改为 **Q8_0**：下载 1,132,110,560 B，`shasum -a 256` = `93eea586…3679`，与清单一致；并已用该模型完成 M10 实测 |
| M4-3 | `THIRD-PARTY-NOTICES.md` 补 MOSS(APACHE-2.0) / Nemotron(OpenMDW 1.1) / Sortformer(CC-BY-NC 标注) | ✅ 已验证 | Nemotron 已下架故不收录；已补 MOSS / Sortformer / Qwen3 / Fun-ASR-Nano |

---

## M5 — `engine/direct.rs` 直出路径

| # | 项 | 状态 |
|---|----|------|
| M5-1 | `moss_transcribe_diarize` 单次整文件调用（跳过 VAD） | ✅ 已验证 | `engine/direct.rs::transcribe_direct`；临时文件在 `temp_dir()/monosub-<task_id>/`，不污染用户目录 |
| M5-2 | 解析 `--turns-out`（`start_sample`/`end_sample`/`speaker_id` → 秒） | ✅ 已验证 | 按实测形状；秒= sample/16000；`S01` → 首现顺序 `Speaker A` |
| M5-3 | 解析 `--segments-out`（段级时间戳） | ✅ 已验证 | `--segments-out` 作为额外输出传给 CLI；主解析走 `--turns-out`（含 text 与 speaker） |
| M5-4 | `RawChunk` 加 `speaker: Option<String>` 字段（设计 §6.5） | ✅ 已验证 | 已加；所有构造点与单测同步更新 |
| M5-5 | 说话人标签映射（>26 截断） | ✅ 已验证 | `diar::label_for`；`label_for(999)` 有单测 |
| M5-6 | `max_tokens` 按音频时长估算 | ✅ 已验证 | `estimate_max_tokens`（单调 + 上下限），有单测 |
| M5-7 | 单测：假 CLI 输出解析 | ✅ 已验证 | 5 个 direct 单测（含标签映射/隐藏/缺 text） |

> 实测更正：M10 显示 **10.4 min 默认 `max_tokens` 未截断**，故 `estimate_max_tokens` 定位为下限保护而非必要修复（PoC §5）。

---

## M6 — `Diarize` + `TurnAttribution` 多阶段组合

| # | 项 | 状态 |
|---|----|------|
| M6-1 | `engine/` 加 `--task diar` 调用（nemotron_3_diar / sortformer） | ✅ 已验证 | `engine/diar.rs::detect_turns`；family 从目录解析（nemotron 已下架，实际用 `sortformer_diar`） |
| M6-2 | `TurnAttribution`：最大重叠归属（旧 `assign_speakers` 直译） | ✅ 已验证 | `diar::attribute_turns`；有最大重叠与无重叠两个单测 |
| M6-3 | 单说话人 → 隐藏标签（旧 `should_show_speaker_labels`） | ✅ 已验证 | `labels_for_turns` 在 ≤1 说话人时返回 `None`；有单测 |
| M6-4 | goldens 对齐 `core-engine/monosub_core/diarizer.py` 行为 | ✅ 已验证 | 归属/标签语义按旧实现移植并有单测；旧 goldens 全部仍绿 |

---

## M7 — 说话人感知的后处理与写出

| # | 项 | 状态 |
|---|----|------|
| M7-1 | `postprocess.rs`：merge 增加前置条件 `a.speaker == b.speaker` | ✅ 已验证 | 新增 `same_speaker` 前置条件（`None==None` 仍可 merge） |
| M7-2 | `postprocess.rs`：把 `chunk.speaker` 传到 `Segment.speaker` | ✅ 已验证 | `distribute_times` 新增 `speaker` 参数 + `subtitle/mod.rs` 调用点传入 |
| M7-3 | 单测：不同说话人的相邻短段**不得** merge；同一说话人**应** merge | ✅ 已验证 | 3 个新单测（同说话人 merge / 跨说话人不 merge / 有标签 vs 无标签不 merge）+ 传播测试 |
| M7-4 | 现有（无说话人）goldens 必须仍全绿 | ✅ 已验证 | 全部 writer/postprocess goldens 仍绿（161 Rust 测试通过） |
| M7-5 | `writer.rs` 的 `[Speaker A]: ` 前缀已就绪 —— 确认即可（`with_speaker` 早已实现） | ✅ 已验证 | `writer.rs:11` 起 `with_speaker` / `short_speaker` 已存在且有单测 |

---

## M8 — 命令层接入求解器

| # | 项 | 状态 |
|---|----|------|
| M8-1 | `TranscribeRequest` 加可选字段 | ✅ 已验证 | `require_word_timestamps` / `speed_preference` / `model_overrides`；全 `Option`，旧前端不传即保持原行为。**直出字段已移除** |
| M8-2 | `start_transcription` 改为「求解 → 按 plan 检查**全部**模型安装」 | ✅ 已验证 | 先 `plan::solve`，再逐个检查 `plan.model_ids` |
| M8-3 | `MODEL_NOT_INSTALLED` 携带缺失 model id（前端可跳转到该条目） | ✅ 已验证 | 返回 `MODEL_NOT_INSTALLED:<model_id>`；前端 `includes` 匹配仍有效 |
| M8-4 | 按 `plan.passthrough` 分派 `direct.rs` / 现有 VAD-sequence 管线 | ✅ 已验证 | passthrough → `direct::transcribe_direct`；否则 `engine::transcribe`，含 `Diarize` 步骤时追加 `diar::detect_turns` + `attribute_turns` |
| M8-5 | 保持事件契约：仍按 4 步发 `transcribe_progress`，`step_name` 不变 | ✅ 已验证 | 仍 4 步；直出把 VAD 折叠进 `Transcribing`（设计 §9.2） |
| M8-6 | （可选）新增 `model_plan_selected` 事件 | ⬜ 未做（可选） | 改为 `tracing::info!(plan, models, passthrough)`；前端暂不需要该事件 |

---

## M9 — 前端（合并 Stage A 收尾）

| # | 项 | 状态 |
|---|----|------|
| M9-1 | Stage A7–A12：删除 `license.ts` / store 字段 / Settings 分类 / BottomBar PRO / i18n / App 调用 | ✅ 已验证 | `grep -rn license src/` 仅剩合法引用（storage-key 常量 + `model.license` 展示）；`bun run check` / `lint`（0 error）/ `check:i18n` 全绿 |
| M9-2 | `BottomBar.tsx` 解禁多人开关（去掉 `checked={false} disabled` 与「即将回归」文案） | ✅ 已验证 | 已接入 `enableSpeakerDiarization`；无可用分离模型时置灰并指向 Settings → Models（新 key `bottomBar.multiSpeakerNeedsModel`） |
| M9-3 | Store `persist` v2 → **v3**：加 3 个 settings 字段 + `modelOverrides`，**不丢任何东西** | ✅ 已验证 | `PERSIST_VERSION = 3`；`modelOverrides` 校验（未知 slot / 空 id 丢弃）；v2 blob 仍可加载 |
| M9-4 | `SettingsModelPlan.tsx` 加「高级：手动指定模型」4 个下拉（asr/vad/align/diar） | ✅ 已验证 | 四项，选项按 `roles` 过滤；**直出模式下拉已删除**（见 M11-2） |
| M9-5 | 缺口提示 + 一键下载（设计 §5.6） | ✅ 已验证 | `actionableGaps` → 建议下载 + 下载按钮（复用 `download_model`） |
| M9-6 | i18n 双语言同步（`check-i18n-keys.mjs` 必须通过） | ✅ 已验证 | `check:i18n` OK — 各 **128** key |

---

## M11 — 修复与清理（用户反馈 2026-10-16）

| # | 项 | 状态 | 证据 |
|---|----|------|------|
| M11-1 | **修复 Settings 报「无法加载模型列表: engine missing: bundled resource models.json not found」** | ✅ 已验证 | 根因：`tauri dev` 以 cwd=`src-tauri/` 启动，`dev_resources_dir` 会拼成 `src-tauri/src-tauri/resources`（不存在），而 `bundle_resources_dir` 只找 `target/Resources/resources`；**缺了可执行文件同级的 `target/debug/resources/`**。`resolve_resource_path` 补上该候选（与 `resolve_binary` 对齐）+ 2 项回归单测 |
| M11-2 | **删除「直出模式」配置项**，改为运行时自动判断 | ✅ 已验证 | 删除 `PassthroughMode` + store `passthroughMode` + UI 下拉 + i18n 4 key + `TranscribeRequest.passthrough*`；保留 `TemplateGate` 自动门控与 `ModelPlan.passthrough` 输出标志 |
| M11-3 | **Settings 分组从 6 个合并为 3 个**：文件与格式 / 模型 / 系统 | ✅ 已验证 | `SettingsCategory = 'files' \| 'models' \| 'system'`；`LEGACY_CATEGORY_MAP` 兼容旧 localStorage id；旧 general/output/processing → 文件与格式，notifications/about → 系统；新增 `settings.files/modelsGroup/system` |
| M11-4 | **修复模型方案卡片内容溢出**（右侧控件被推出屏幕外） | ✅ 已验证 | 根因：modal 的 flex 内容列缺 `min-w-0`，模型列表里一行很长的 attribution 文本把内容列撑到 **1518px**（modal 仅 858px），横向溢出把右侧开关/下拉推出可视区。补 `min-w-0` + `overflow-x-hidden`；实测 modal 858/858、内容列 634/634，无横向溢出 |
| M11-5 | **修复方案标题渲染成原始 i18n key**（显示 `modelPlan.plan.asrVad`） | ✅ 已验证 | 根因：Rust `title_key` 输出 `modelPlan.plan.asrVad`，而 i18next 以 `.` 作路径分隔符，locale 里是**扁平** key `modelPlan.planAsrVad`，查 `modelPlan.plan` 不存在故回显原串。改为扁平 key（含 `gapTitle`），并加回归单测遍历每个 `title_key` 在两种 locale 中都能解析 |
| M11-6 | **语言设置移动到「系统」分组** | ✅ 已验证 | 界面语言从「文件与格式」移到「系统」（与通知/关于同级）；tsc/lint 干净 |
| M11-7 | 三项均经 tauri-pilot 在**运行中的 app** 实测确认 | ✅ 已验证 | 截图确认「当前方案 → SenseVoice + VAD（估算时间戳）」渲染正常、开关与下拉都在卡片内；`Settings` 分组为「文件与格式 / 模型 / 系统」 |
| M11-8 | **多人语音识别改为文件级开关**（任务列表每行一个 switch） | ✅ 已验证 | 新增 `VideoFile.diarization: boolean`（入队时由 `settings.defaultDiarization` 播种）+ `setFileDiarization` / `applyDiarizationToAll`；`transcription.ts` 发送 `enable_diarization: file.diarization`（原为全局值）。旧的全局 `enableSpeakerDiarization` 已删除，BottomBar 的开关改为「批量应用到全部待处理文件」 |
| M11-9 | **文件级开关确实跳过重模型**（用户的核心诉求） | ✅ 已验证 | 经 tauri-pilot 真实 IPC 实测：`enable_diarization=false` → 计划 `asr:sense`，`modelIds=['sensevoice_small_q8']`（254 MB）；`=true` → `passthrough:moss`，`modelIds=['moss_transcribe_diarize_q8_0']`（1.13 GB）。关闭时 MOSS 完全不加载 |
| M11-10 | 批量多人开关移到**顶部工具栏**并去掉提示文案 | ✅ 已验证 | 新建 `BatchDiarizationToggle.tsx`，放到 `Toolbar`（`全部开始` 左侧）；`BottomBar` 移除该控件及「请在 设置 → 模型 中下载…」提示；删除 i18n `bottomBar.multiSpeakerNeedsModel` / `applyToAllPending`。tauri-pilot 实测：开关在 toolbar 内（y=72，toolbar 52–116），位于「全部开始」左侧；底栏文案已不再出现该提示 |
| M11-11 | **修复下拉菜单文字显示不全** | ✅ 已验证 | 根因：`CustomSelect` 的 listbox 用了 `w-full`，把菜单锁死成触发器宽度（实测 **76px**），而模型名最长需 **284px**，实际只剩 **42px** → 全被 `truncate` 截断。改为 `min-w-full w-max max-w-[min(24rem,70vw)]`，并给触发器/选项加 `title` 兜底。实测：菜单 76→**318px**，`anyClipped: false`，最长项 284px 完整显示；短选项（速度）菜单 84px 仍紧凑 |
| M11-12 | **修复 `family` 被硬编码为 `sense_asr`**（Bug 1） | ✅ 已验证 | `engine/mod.rs` 的 `run_sequence` 原本写死 `family: ASR_FAMILY`（`sense_asr`），但 `model_id` 来自求解器。西/德/法/葡会选中 `qwen3_asr`，于是拼出 `--family sense_asr --model qwen3-asr.gguf`，CLI 报 `GGUF embeds model spec for family 'moss_transcribe_diarize', not 'sense_asr'`。改为把 `spec.family` 沿 `transcribe → transcribe_batch → run_sequence` 传下去；`ASR_FAMILY` 常量删除 |
| M11-13 | **`--session-option` 前缀同样按 family 命名空间**（同类 Bug） | ✅ 已验证 | `asr_args` 曾硬编码 `sense_asr.vad_model_path=`。实测 `qwen3_asr.vad_model_path` 存在但 `qwen3_asr` 不声明该选项，`sense_asr`/`fun_asr_nano` 才声明。改为 `family_takes_vad_model_path()` 判定 + 用实际 family 作前缀；新增单测覆盖三分支 |
| M11-14 | **移除 `Align` 步骤**（用户的判断：本 App 无此需求） | ✅ 已验证 | `align` 任务是把**已有字幕/文本**对齐到音频；本 App 只产出字幕、从不消费词级数据（`--words-out` 在代码里仅出现于一处测试 fixture）。而 `StepKind::Align` 本来就**只在规划层出现、执行分派里从未运行** —— 即「词级时间戳」勾选后是静默无效。移除后：`templates_for` 不再产出 Align；`TemplateGate::TimestampsCoverWords` 删除；`word_timestamps` 需求如实走 `gaps()` 报缺口；`models.json` 移除 `qwen3_forced_aligner` 条目；前端去掉 align 槽位与 `planQwenAlign` key |
| M11-15 | 词级时间戳的 UI 文案改为如实说明 | ✅ 已验证 | `modelPlan.wordTimestamps` 改为「词级时间戳（暂不支持）」+ 新增 `wordTimestampsHint`（说明会列为缺口），避免开关承诺未实现的能力 |
| M11-16 | Bug 1 经**真实转写**端到端验证 | ✅ 已验证 | 经 `download_model` 下载 `qwen3_asr_0_6b_q8_0`（1,151,272,416 B，与清单一致），再以 `language=es` 发起真实转写：任务成功并写出正确中文字幕（`/tmp/bug1-probe.srt`）。修复前该请求必然因 family 不匹配失败 |
| M11-17 | **模型目录支持自定义**（避免重复下载） | ✅ 已验证 | 新增 `src-tauri/src/model_dir.rs` 作为**唯一**解析点：优先级 = `MONOSUB_MODELS_DIR` 环境变量 → `<app_config_dir>/settings.json` 的 `modelsDir` → 默认 `<app_data_dir>/models`。`engine::catalog::models_root` 与 `commands::models::models_root` 此前是**两份各自实现**，现都改为调用 `configured_root`，不会再出现「下载到一个目录、安装检查看另一个目录」 |
| M11-18 | 新命令 + 设置界面 | ✅ 已验证 | 新增 `get_models_dir` / `set_models_dir(path: Option<String>)`；`set` 会**先校验**目录存在且可写（写探针，而非只读 metadata）再持久化，`null` 表示恢复默认。前端 `SettingsModelsDir.tsx` 放入 设置 → 模型，含「更改目录 / 恢复默认 / 打开」，并显示当前路径与来源 |
| M11-19 | 端到端验证（用户的核心诉求：不重复下载） | ✅ 已验证 | 把目录指向 `poc/models`（已有 SenseVoice + MOSS）后 `list_models` 立即显示 `sensevoice_small_q8 installed=true onDisk=254211200`、`moss_transcribe_diarize_q8_0 installed=true onDisk=1132110560` —— **零下载**；随后用该目录发起**真实转写**并产出正确字幕。非法路径 `/nonexistent/nope` 被拒（`directory does not exist`）；`null` 成功恢复默认；`settings.json` 内容为 `{"modelsDir": "..."}` |
| M11-20 | 安全边界在新目录下仍成立 | ✅ 已验证 | `remove_model_dir` 本就先 `canonicalize` 再校验 `target.starts_with(root)`，且 `remove_model` 的 `root` 也取自 `models_root()`（已改为配置目录），故删除仍被约束在配置目录内，无法越界。已有单测 `remove_model_dir_refuses_paths_outside_the_models_root` 覆盖 |
| M11-21 | 注意：改目录**不搬迁**已有模型 | ✅ 已验证 | 这是有意为之 —— 该功能的用途正是复用磁盘上已有的模型。UI 明说「已有文件不会被移动」；Rust 侧没有任何 copy/move 逻辑 |
| M11-22 | **修复「点击下载后状态不变、可重复点击」** | ✅ 已验证 | 根因：`download_model` 是**后台任务**（spawn 后立即返回 `Ok(())`），而 `handleDownloadModel` 用 `finally` 清 `downloadingId` —— 于是状态在任何下载开始前就被清掉，按钮闪一下就变回「下载」。改为**只在 catch（请求本身被拒）时**重置；正常路径由 `model_download_complete` / `model_download_error` 事件清除。tauri-pilot 实测：点击后该行按钮持续为**取消**（1s/3s/6s 三次观测均如此），此前毫秒级就回退 |
| M11-23 | 移除高级区多余的「自动」重置按钮 | ✅ 已验证 | 每个下拉自身就含「自动」选项（`slotAuto`），面板底部那颗独立按钮语义重复。删除 `clearModelOverrides` 按钮及其订阅，下拉选「自动」即等同重置该槽位 |
| M11-24 | 手动指定模型的下拉加上悬浮解释 | ✅ 已验证 | 每个槽位标签加 `title`（并加虚线下划线与 `cursor-help` 以暗示可悬停）：VAD=切段、ASR=转写（决定准确度与语种）、Diar=说话人标注；中英双语。实测三个标签的 `title` 均非空 |
| M11-25 | 槽位顺序改为按音频处理流水线排列 | ✅ 已验证 | `SLOT_ORDER` 由 `['asr','vad','diar']` 改为 **`['vad','asr','diar']`**（切段 → 转写 → 说话人），与数据流的真实顺序一致。实测界面顺序为 切段（VAD）→ 转写（ASR）→ 说话人（分离） |
| M11-26 | 模型列表加「重新检测」按钮 | ✅ 已验证 | 复用既有 `loadModels()`（重新读磁盘的安装状态），按钮在 `isLoadingModels` 时禁用并让图标旋转，带 `title` 说明用途。**双向验证**：手动删除 `sortformer` 的 gguf 后点击 → 卡片由「已安装」变「未安装」；恢复文件后再点击 → 变回「已安装」，文件已按原字节数 175662720 还原。**后续修正**：该行原本还并排放着一颗「打开模型目录」，与上方「存储位置」卡片里的同名按钮重复（同屏两个同名按钮读起来像 bug），已删除；`open_models_dir` 现在只由 `SettingsModelsDir` 提供，实测该行只剩「重新检测」 |

---

## M10 — PoC spike（**必须在 M5/M6 之前**）

设计 §12 明确：Metal 上的 RTF 与 `--turns-out` 实际输出形状是最大未知数。

| # | 项 | 状态 |
|---|----|------|
| M10-1 | 下 MOSS（**改 Q8_0**，1.13 GB），Metal 实跑，记录 **RTF / 内存峰值 / 冷启动** | ✅ 已验证 | RTF 0.16–0.76（32.9s/3.09s/262.9s/624.5s）；峰值 footprint 3.0–5.8 GB；慢于 SenseVoice 约 10–20x |
| M10-2 | 核对 `--turns-out` / `--segments-out` **实际 JSON 形状**是否与 PoC §2 一致 | ✅ 已验证 | 形状 = `{start_sample,end_sample,speaker_id,confidence,text}`，秒= sample/16000；**`speaker_id` 实为 `S01`（非 `speaker_0`）**；`confidence` 恒 0 |
| M10-3 | 中文多人素材实测：**快速交替是否塌标签**（第三方复测的短板是否复现） | ✅ 已验证 | **复现**：8 段/3.09s 交替 → 全部塌为单一 `S01`；正常语速 4 轮双人对话则正确 |
| M10-4 | 长音频 `max_tokens` 截断阈值实测 | ✅ 已验证 | 默认 `max_tokens` 下 **10.4 min 完整输出**（末段 end_sample=9,991,040 / 全长 9,991,952），**未截断** |
| M10-5 | 结论写入 `docs/tech-spec/audiocpp-moss-poc.md` | ✅ 已验证 | 文档已写；裁决 = MOSS 作**可选**直出（非强制默认） |

> 若 M10-1 的 RTF 不可接受 → MOSS 标 `speed: accurate` 且**默认不推荐**，
> 用户需显式选；退路是 `nemotron_3_diar` + 现有 SenseVoice 组合。

---

## 验证门（每阶段结束前必过）

| 命令 | 用途 |
|------|------|
| `bun run test:rust` | Rust 单测 + 集成 |
| `bun run lint:rust` | `cargo clippy` |
| `bun run check` | TypeScript 类型检查 |
| `bun run lint` | ESLint |
| `bun run check:i18n` | 双语言 key 集合一致 |

**当前基线**：`cd src-tauri && cargo check` 在 Stage A 后 **exit=0**（仅遗留 4 条既有 dead-code 警告）。

---

## 进度快照

| 阶段 | 状态 |
|------|------|
| 设计文档（`model-selection-design.md`） | ✅ 完成 |
| `AGENTS.md` 更新（许可 + 模型方案 + 数据流） | ✅ 完成 |
| 旧文档更正（`refactor-audiocpp.md` §5.7 / `tasks.md` P4-3） | ✅ 完成 |
| **Stage A（Rust 部分）** | ✅ 完成（A1–A6） |
| Stage A（前端部分） | ✅ 完成（A7–A12，并入 M9-1） |
| M1 目录 schema | ✅ 完成（`models.json` v2 + `catalog.rs`；123 Rust 测试通过） |
| M10 PoC spike | ✅ 完成（`audiocpp-moss-poc.md`） |
| M2 求解器 | ✅ 完成（`engine/plan.rs`，12+1 单测） |
| M3 命令层 | ✅ 完成（`commands/plan.rs` + `lib/modelPlan.ts`） |
| M4 目录/下载 | ✅ 完成（6 条目 + SHA256 实测） |
| M5/M6/M7 推断 | ✅ 完成（`engine/direct.rs` + `engine/diar.rs` + 说话人感知 merge） |
| M8 命令层接入 | ✅ 完成（M8-6 可选项未做） |
| M9 前端 | ✅ 完成（M9-1..M9-6） |
| **全部阶段** | ✅ **M1–M11 完成**；**161** Rust 测试通过，clippy 0 error，tsc/lint/i18n 全绿 |

**下一步建议**：M1–M11 **全部完成**。剩余可选后续：M8-6（`model_plan_selected` 事件）、
真实端到端跑一遍 MOSS 直出与人机验证 UI、以及把本次实测结论回填 `refactor-audiocpp.md` §5.7。

> **M10 裁决（已回填设计 §12/§13）**：MOSS 直出**可用但非默认**——RTF 0.16–0.76 可接受，
> 但峰值内存 3–5.8 GB 且**快速交替会塌标签**（已复现）。故 `tier: recommended` 但由求解器
> 按语种/需求打分；`asr + diar(sortformer) + TurnAttribution` 保留为退路。
