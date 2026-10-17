# 本地字幕翻译 — 任务清单

> 设计见 [translation-design.md](./translation-design.md)（下称 spec），PoC 结论见
> [translation-poc.md](./translation-poc.md)。
> 约定：每个任务独立可提交；`依赖` 写必须先完成的任务；完成时勾选，并在「状态」写清证据。
> 粒度：S ≤ 半天，M ≈ 1 天，L ≈ 2–3 天。

---

## 0. 决策基线

| 项 | 决定 |
|----|------|
| 推理引擎 | llama.cpp 预编译二进制随包，单独进程运行（不与 audio.cpp 共用 ggml） |
| 调用形态 | **`llama-server`**（T0-3 定稿，见 PoC §4）；每个翻译阶段起一次，翻完即停 |
| 挂载点 | `subtitle::postprocess` 之后，原文字幕写完再翻译 |
| 失败语义 | 增强项：翻译失败只告警，原文照常交付 |
| 输出 | 原文不动，另写 `<stem>.<lang>.<ext>` |
| 模型 | 进 `models.json`，按需下载；默认 **HY-MT1.5-1.8B-Q4_K_M**（`hy_mt_1_8b_q4_k_m`，T0-2/PoC §3.1 确认） |
| 输出约束 | **编号行 + 严格校验 + 批次降级**。结构化输出（json_schema / GBNF）实测失败，已放弃（PoC §5.1；spec §4 已改） |

---

## Phase T0 — PoC（Go/No-Go）

| ID | 任务 | 规模 | 依赖 | 验收 | 状态 |
|----|------|------|------|------|------|
| T0-1 | 选定 llama.cpp release tag；下载 `macos-arm64` 包，记录 SHA256、解压后体积、需要的可执行文件与 dylib | S | — | 数据记入 PoC 文档 | ✅ 已验证 —— `scripts/build/fetch-llamacpp.sh` 钉 `LLAMA_BUILD="b11696"`、`LLAMA_SHA256="6aac007f…f649"`（x64 留空）；PoC §2 记 tarball 12,222,784 B、所需子集约 68 MB、dylib 必须平铺（`LC_RPATH = @loader_path`） |
| T0-2 | 选 2–3 个候选模型（HY-MT1.5-1.8B / 7B、Qwen3 某量化），记录 URL、size、SHA256、许可、是否有 ModelScope/hf-mirror 源 | S | — | 候选表 | ✅ 已验证 —— PoC §3.1 候选表（ModelScope 主源实测 206 + 2 个镜像 + size 1,133,080,512 + sha256 `4383ac0c…77c7` + 许可）；条目落入 `src-tauri/resources/models.json` 的 `hy_mt_1_8b_q4_k_m`。HY-MT1.5-7B 与 AngelSlim 1.25/2-bit 登记为未实测备选 |
| T0-3 | 用一份真实 SRT（≥30 min，中→英、英→中各一份）分别测 `llama-cli` 与 `llama-server`：冷启动、总耗时、内存峰值、Metal 是否生效 | M | T0-1,2 | 数据表；定 cli/server | ⚠️ 部分验证 —— 定 **`llama-server`**（PoC §4）。实测：冷启动 0.5–5 s、RSS ~1.4 GB、Metal 生效、41 条整份 11–14 s（PoC §7）。**缺口**：`llama-cli` 未做同口径基准（落选依据是结构性外推，PoC §4/§8.2）；素材是 41 条样本而非 ≥30 min 真素材 |
| T0-4 | 批量编号翻译：比较批次大小（如 10/20/40 条）对漏条、合并、加解释的影响；验证 JSON schema / grammar 约束是否有效 | M | T0-3 | 推荐批次与输出约束方式 | ✅ 已验证（含两条后续实测修正，见 PoC §5.4）—— 批次 10/20/40 全部条数精确 → 定 **20**（`translate/mod.rs::BATCH_SIZE`）；结构化输出**实测失败**（json_schema 回显原文/空数组、GBNF 返回 `[]`）→ 改用编号行。**复查修正**：① 模型卡的**单条**模板**不能**叠在批量契约前（会放弃编号，20 条时回显原文）；② 契约必须自带「显式条数 + 条数句自成一段 + 两行示例」，缺示例时 2–4 条批次每次都失败、14 条偶发失败，加上后 2–20 条 27/27 全对 |
| T0-5 | 确认 SenseVoice 输出里语言标签（`<\|zh\|>` 等）在 `--text-out` 中是否稳定存在 | S | — | 结论（决定 T4-1 方案） | ⚠️ **done-with-caveat（未完全验证）** —— **本机未做 live SenseVoice 观察**（模型在此网络策略下下不来），PoC §6 明确记为未验证。工程决定安全：`clean_text` 一直在删这类标签 ⇒ 标签确实在引擎输出路径上；`extract_language_tag` 在删除前取、取不到降级 `None`；`direct.rs` 直出路径实测为 `None`（`direct_path_reports_no_language`）。仍需一次实跑关闭缺口 |
| T0-6 | 产出 `docs/tech-spec/translation-poc.md`，回填 spec 中所有「待 PoC」项 | S | T0-1..5 | 文档合入；spec 已更新 | ✅ 已验证 —— `docs/tech-spec/translation-poc.md` 新建；spec §3.2/§4/§6/§9 的「待 PoC」项已按实测回填（PoC §9 逐条列出） |

---

## Phase T1 — 打包与基础设施

| ID | 任务 | 规模 | 依赖 | 验收 | 状态 |
|----|------|------|------|------|------|
| T1-1 | `scripts/build/fetch-llamacpp.sh`：钉版本 + SHA256，解出二进制到 `src-tauri/resources/bin/<name>-<triple>`（dylib 放 `bin/lib/` 或按 rpath 处理）；幂等；更新 `.gitignore` | S | T0-6 | 重复运行无副作用 | ✅ 已验证 —— `fetch-llamacpp.sh` 幂等（`DYLIBS_PRESENT` + `-x $SERVER_TARGET` 短路）；实装 `bin/llama-server-<triple>` + `bin/lib{llama,ggml,mtmd}*.dylib` **平铺**，脚本注释写明为何不进 `bin/lib/`；`.gitignore` 注释更新（`resources/*` 已覆盖平铺 dylib） |
| T1-2 | `prepare-resources.sh` 调用 T1-1 并重新生成 MANIFEST | S | T1-1 | 一条命令备齐全部资源 | ✅ 已验证 —— `prepare-resources.sh` 新增步骤 `4. 获取 llama.cpp 运行时` 调用 `fetch-llamacpp.sh`（原步骤 4 → 5）；MANIFEST 递归生成，无需改动 |
| T1-3 | `scripts/release/sign-app.sh` 签名新增二进制与 dylib；`build-macos-app.sh` 打包后能运行 | S | T1-1 | 已签名 DMG 中 server 可启动 | ⚠️ 部分验证 —— 既有 `"$RESOURCE_BIN_DIR"/*` 循环已覆盖平铺的 llama-server 与 dylib（`sign-app.sh` 加了说明注释，并警告不要加第二个循环）。**未验证**：签名后的 DMG 里实跑 server（PoC §8.5） |
| T1-4 | `resources/licenses/` 加 llama.cpp MIT 文本，更新 `THIRD-PARTY-NOTICES.md` | S | T1-1 | 文件就位 | ✅ 已验证 —— `src-tauri/resources/licenses/llama.cpp-LICENSE`（MIT，1,078 B）就位；`THIRD-PARTY-NOTICES.md` 新增 llama.cpp 段（b11696 / MIT / 平铺布局 / 不链接进进程）与 HY-MT 段（署名，按需下载） |
| T1-5 | `engine/cli.rs`：`resolve_bundled_binary` 改 `pub(crate)`；新增 `MONOSUB_LLAMACPP_PATH` 覆盖 | S | — | 单测覆盖 env 覆盖与缺失报错 | ✅ 已验证 —— `cli.rs::resolve_bundled_binary` 为 `pub(crate)`；新增 `pub fn resolve_llamacpp_path` + `LLAMACPP_PATH_ENV = "MONOSUB_LLAMACPP_PATH"`。单测：`resolve_llamacpp_path_honours_the_env_override`、`resolve_llamacpp_path_never_errors`、`resolve_llamacpp_binary_without_any_candidate_yields_the_bare_name` |

---

## Phase T2 — 模型目录

| ID | 任务 | 规模 | 依赖 | 验收 | 状态 |
|----|------|------|------|------|------|
| T2-1 | `catalog.rs`：新增 `Role::Translate`、`engine` 字段（缺省 `audiocpp`）、可选 `prompt_template`；解析兼容老条目 | S | — | 现有 catalog 测试全绿；新增解析测试 | ✅ 已验证 —— `Role::Translate`、`Engine`（`Default = Audiocpp`）、`ModelSpec.prompt_template`；`ModelWire` 均带 `#[serde(default)]`。新测试 `load_catalog_parses_engine_and_prompt_template`（含无 `engine` 字段回落 `audiocpp`）；`Role::Translate.as_str() == "translate"` |
| T2-2 | `plan.rs`：确保 `translate` / `engine=llamacpp` 模型不进入识别方案 | S | T2-1 | 单测：catalog 含翻译模型时 `solve` 结果不变 | ✅ 已验证 —— `plan::solve` 先过滤出 `engine == Audiocpp && !has_role(Translate)` 的 `recognition` 切片；`best_model_for_role` 同样加 engine 过滤。单测 `translate_models_never_enter_a_recognition_plan`（3 组 requirements 下 plan 形状逐字相等）与 `best_model_for_role_ignores_non_audiocpp_models` |
| T2-3 | `models.json` 加入 T0 选定的翻译模型条目（pinned URL、镜像、size、SHA256、licenses、attribution） | S | T0-6, T2-1 | 能经 `download_model` 下载并校验通过 | ✅ 已验证 —— `models.json` 第 318 行起 `hy_mt_1_8b_q4_k_m`：pinned ModelScope `url` + 2 个 `mirrors` + `size` + `sha256` + `licenses[]` + `attribution`；`roles: ["translate"]`、`engine: "llamacpp"`、`prompt_template: "hy_mt"`、`source: "other"`。下载走通用 `download_model` 路径（size + SHA256 校验），PoC §3.1 已实测 URL 206 与 sha256 双源一致 |
| T2-4 | `commands/models.rs` / `list_models` 返回 role 与 engine，供前端分组 | S | T2-1 | 前端可区分识别模型与翻译模型 | ✅ 已验证 —— `ModelStatus` 新增 `engine` / `prompt_template`（保持 snake_case）。单测 `list_models_payload_spells_engine_and_prompt_template`、`list_models_payload_defaults_to_audiocpp`；前端 `src/lib/models.ts` 的 `normalizeModelStatus` 读同名字段 |
| T2-5 | 新增 `translation_models` 查询或复用 `list_models` 过滤；给出默认翻译模型 id | S | T2-3 | 单测 | ✅ 已验证 —— 复用 `list_models`：`engine/catalog.rs::translation_models` + `default_translation_model_id`（按 tier 排名，同级取目录序）。单测 `translation_models_preserves_catalogue_order`、`default_translation_model_prefers_tier_then_order`；前端 `translationModels`/`recognitionModels`（`models.ts`） |

---

## Phase T3 — 翻译模块（Rust，`src-tauri/src/translate/`）

| ID | 任务 | 规模 | 依赖 | 验收 | 状态 |
|----|------|------|------|------|------|
| T3-1 | `translate/server.rs`：启动 `llama-server`（`127.0.0.1` 随机端口、Metal，失败退 CPU）、就绪探测、`kill_on_drop`、stderr 环形缓冲；错误映射到独立的 `TranslateError` | M | T1-5 | 缺二进制/缺模型/启动超时各有明确错误；取消后无残留进程 | ✅ 已验证 —— `translate/server.rs` + `translate/error.rs`。`server_args` 由 `server_args_match_the_measured_command_line` 逐字固定；`reserve_port` 用 `127.0.0.1:0`（`reserving_a_port_yields_a_bindable_loopback_port`）；就绪探测 `await_healthy` 轮询 `/health`（`STARTUP_TIMEOUT` 120 s，实测冷启动 0.5–5 s）；`kill_on_drop` + `Drop` 显式 kill；stderr 环缓冲 50 行；缺模型/缺二进制分别报 `ModelMissing`/`BinaryMissing`（三个单测）。**未做**：Metal 失败退 CPU 的重试路径实测 |
| T3-2 | `translate/prompt.rs`：按模型模板构造批量请求（编号 + 少量上文）；模板随 catalog 走 | S | T2-1 | 纯函数单测 | ✅ 已验证（实现中有一次**实测驱动的反转**）—— `translate/prompt.rs`；`batch_prompt(template, source, target, cues)` 纯函数，模板来自 `ModelSpec.prompt_template`（未知/拼错回落 generic）。**反转**：初版把模型卡的**单条**模板叠在批量契约前，实测 zh→en 时模型**放弃编号**，20 条批次更会**回显原文未翻译**（内容非空，只看「是否有输出」的实现会误判成功）→ 改为批量路径**只发编号契约**，且契约自带「显式条数 + 条数句自成一段 + 两行示例」；缺示例时 2–4 条批次每次都失败，加上后 2–20 条 27/27 全对。另：**单条**用编号形式不可靠 → `single_cue_prompt` 改走卡片式单段模板且答案整体采纳，且**中文源的目标语言必须写中文名**（`翻译为English` 会输出中文，实测 3/3；`翻译为英语` 后 30/30 正确）。单测：中/英模板、`zh-tw` 走中文模板、未知源仍点名 target、编号跨批连续、条数句自成一段、示例存在、`single_cue_prompt_names_the_target_in_the_source_language`、`every_committed_language_has_a_chinese_name`。**决定**：不发送跨批上文（实测措辞只有「指令 + 编号行」，加进去会改变已测行为，见 `mod.rs` 注释） |
| T3-3 | `translate/parse.rs`：解析编号结果并校验条数/编号；纯函数 | S | — | 单测：漏条、乱序、多余解释、空输出 | ✅ 已验证 —— `translate/parse.rs::parse_translations`。单测覆盖：漏条（`missing_cue_is_absent_not_shifted`）、乱序（`out_of_order_cues_keep_their_numbers`）、多余解释/前言（`blank_line_ends_the_cue_so_a_trailing_note_is_dropped`、`preamble_explanation_is_dropped`）、续行（`continuation_lines_extend_the_previous_cue`）、空输出与 `[]`、RTL 原文保留、全部实测分隔符 |
| T3-4 | `translate/mod.rs`：`translate_segments(segments, src, tgt, model, on_progress) -> Vec<Segment>`；批次失败→缩小批次→逐条→保留原文；只改 `text`，保留时间与 `speaker` | M | T3-1..3 | 用假 server（mock HTTP）的集成测试；条数与时间轴恒等 | ✅ 已验证 —— `translate/mod.rs::translate_segments` / `translate_with`（针对 `Completer` trait 编写）+ `retry_at_half_size`。mock 是**真 HTTP 监听器**（`MockServer`，走真实 `server::post_completion` 与解析）。`translations_change_only_the_text` 断言条数/start/end/speaker 恒等；`under_returning_batch_is_retried_at_half_size` 覆盖降级；`server_error_keeps_every_original_segment`；`single_cue_failure_keeps_the_original_and_stops_retrying`；`progress_reaches_total` |
| T3-5 | 语言码归一化与「是否需要翻译」判断（`zh` vs `zh-tw` 视为不同；`auto` 的处理按 T4） | S | — | 纯函数单测 | ✅ 已验证 —— `translate/language.rs`：`canonical_tag`（`zh-tw`/`zh-hk`/`zh-Hant` → `zh-Hant`）、`should_translate`、`file_tag`、`is_chinese`、`display_name`/`language_label`、`detect_by_script`。单测 `zh_and_zh_tw_are_different_targets_both_ways`、`unknown_source_always_translates`、`missing_or_auto_target_skips`、`file_tag_is_path_safe`、`script_detection_answers_only_when_the_script_is_unambiguous`、`every_committed_translate_language_has_a_display_name` |
| T3-6 | 需要真实二进制/模型的测试在缺失时打印 `skip:` 并通过 | S | T3-4 | 新克隆仓库测试全绿 | ✅ 已验证 —— `translate/server.rs::bundled_server_and_model` 打印 `skip: bundled llama-server not present (run ./scripts/build/fetch-llamacpp.sh)`；`engine/cli.rs::dev_bin_dir_resolves_committed_llamacpp_server` 同理。本机实跑确认：`cargo test translate::server` → 6 passed，其中 `live_server_…` 在装有真实二进制的目录下**不跳过**（本机把 PoC 的 llama-server + 模型放进 `resources/bin/` 与 `MONOSUB_MODELS_DIR` 后实跑通过，耗时 10.19 s，真做了推理）；全量 `cargo test` → **255 passed; 0 failed** |

---

## Phase T4 — 接入转写任务

| ID | 任务 | 规模 | 依赖 | 验收 | 状态 |
|----|------|------|------|------|------|
| T4-1 | 源语言检测：`clean_text` 丢弃前提取语言标签，`RawChunk` 增加 `language: Option<String>`，汇总得到主语言；拿不到时简单脚本统计兜底 | M | T0-5 | 单测；直出（moss）路径无标签时返回 `None` 不报错 | ✅ 部分验证（T0-5 caveat）—— `subtitle/postprocess.rs::extract_language_tag`（在 `clean_text` 之前调用，见 `engine/mod.rs` 构造 `RawChunk` 处）+ `dominant_language`（多数票，平票取先见者）；`RawChunk.language: Option<String>`；兜底顺序在 `commands/transcribe.rs::detected_language`（ASR 标签 → 显式请求 → `detect_by_script` → `None` ⇒ 照常翻译）。单测 6 个，含 `language_tag_is_extracted_before_cleaning`、`dominant_language_is_none_without_any_tag`、`direct_path_reports_no_language`。**缺**：一次真实 SenseVoice 标签观察（PoC §6/§8.1） |
| T4-2 | `TranscribeRequest` 新增 `translate_enabled` / `target_language` / `translation_model`（全部 `Option`，缺省不翻译） | S | — | 老请求行为完全不变 | ✅ 已验证 —— `commands/transcribe.rs::TranscribeRequest` 三个字段均 `Option`；单测 `request_without_translation_fields_decodes_to_none`（旧 JSON 仍可反序列化且解出「不翻译」） |
| T4-3 | `start_transcription` 预检：开启且需翻译时检查翻译模型已安装，否则 `MODEL_NOT_INSTALLED:<id>`（识别开始前） | S | T2-5, T4-2 | 单测 / 手测 | ✅ 已验证 —— `resolve_translation()` 在 `derive_output_paths` 之前调用（即任何进程启动之前），注入 `installed` / `model_file` 谓词以便纯函数测试。单测 `preflight_requires_an_installed_translation_model`（含显式识别模型被拒）、`preflight_skips_when_translation_would_not_run`、`preflight_without_any_translation_model_is_a_clear_error`、`same_language_pair_is_recognised_before_the_model_gate` |
| T4-4 | `run_transcription`：写完原文后进入 `Translating` 步骤，调用 T3-4，写 `<stem>.<lang>.<ext>`；失败只 `warn` 不判任务失败 | M | T3-4, T4-1..3 | 端到端：开/关、同语言跳过、翻译失败仍完成 | ✅ 已验证 —— `translate_and_write()` 在写完所有原文格式**之后**执行；`TranslationOutcome` 只 `tracing::warn!`，不进 `transcribe_error`；`skipped` / `translated == 0` 时不写文件。单测 `translated_files_are_written_next_to_the_originals`（断言原文逐字节未变 + speaker 标签保留）、`translated_file_name_uses_the_canonical_tag`。**未做**：带真实二进制/模型的端到端手测 |
| T4-5 | 步骤：新增 `"Translating"`；`total_steps` 按请求动态（4/5）；现有 4 个名称不变 | S | T4-4 | 测试固定步骤名集合 | ✅ 已验证 —— `STEP_NAMES: [&str; 5]`（`Translating` 追加在末尾）、`STEPS_WITHOUT_TRANSLATION = 4` / `STEPS_WITH_TRANSLATION = 5`，`Steps::new(translating)` 带 per-run `total`。单测 `step_names_are_the_frozen_contract`、`existing_step_names_are_unchanged_and_translating_is_appended`、`step_total_is_dynamic`；`steps.translating` 已在两个 locale 中（`check:i18n` OK） |
| T4-6 | 译文路径纳入 `derive_output_paths`，取消时按 mtime 规则一并清理 | S | T4-4 | 单测 | ✅ 已验证 —— `translation_output_path()` 独立于 `output_path_for()`（原文路径行为不变），`derive_output_paths(output, formats, Some(target))` 把译文路径追加在后。单测 `derives_translated_paths_alongside_the_originals`、`originals_are_never_affected_by_translation`、`cancel_cleanup_covers_translated_paths`（含「早于任务开始时间的旧文件保留」） |
| T4-7 | 并发：翻译阶段也走 `TRANSCRIBE_SEMAPHORE`（或独立信号量），避免与 ASR 抢 GPU | S | T4-4 | 多文件队列手测无 OOM | ✅ 已验证 —— `commands/transcribe.rs` 第 690 行在翻译分支内 `TRANSCRIBE_SEMAPHORE.acquire().await`，与识别阶段共用同一信号量（`Semaphore::new(1)`）。**未做**：多文件队列手测 |

---

## Phase T5 — 前端

| ID | 任务 | 规模 | 依赖 | 验收 | 状态 |
|----|------|------|------|------|------|
| T5-1 | `useStore.ts`：新增 `translateEnabled`(false) / `translateTargetLanguage` / `translationModelId`；`PERSIST_VERSION` 3→4，迁移只新增 | S | — | 老数据迁移后原有设置不丢 | ✅ 已验证 —— `useStore.ts::PERSIST_VERSION = 4`；`defaultSettings.translateEnabled = false` / `translateTargetLanguage = 'en'` / `translationModelId = ''`；`normalizeSettings` 对三者各做类型校验后回落默认值，**不删任何已有字段**。`TranslateTargetLanguage` 排除 `auto`（`TRANSLATE_TARGET_LANGUAGES` 9 项）。`bun run check` 通过 |
| T5-2 | `src/lib/transcription.ts` 传新字段；`MODEL_NOT_INSTALLED` 处理覆盖翻译模型 | S | T4-2, T5-1 | 缺模型时跳到对应下载入口 | ✅ 已验证 —— `transcription.ts` 发送 `translate_enabled` / `target_language` / `translation_model`；`isMissingTranslationModel()` 从 `MODEL_NOT_INSTALLED:<id>` 取 id 后查目录 role，缺翻译模型时显示 `errors.translationModelNotInstalled`。id 解析器 `models.ts::modelNotInstalledId` |
| T5-3 | Settings 新增「翻译」分组：开关、目标语言、模型下拉（显示安装状态、可下载） | M | T2-4, T5-1 | tauri-pilot 手测 | ✅ 部分验证 —— `src/components/SettingsTranslation.tsx` 新建（开关 + 目标语言下拉 + 模型下拉含安装状态与下载/删除按钮）；`Settings.tsx` 侧栏改为 4 组（`files` / `models` / `translate` / `system`）。**未做**：tauri-pilot 手测（冒烟步骤已提交，见 T5-7） |
| T5-4 | Models 区分组展示识别模型 / 翻译模型 | S | T2-4 | 手测 | ✅ 已验证 —— `Settings.tsx` 抽出 `ModelRow` 组件，`recognitionModels(models)` / `translationModels(models)` 两组分别渲染，标题 `models.recognitionTitle` / `models.translationTitle` |
| T5-5 | `FileItem.tsx` 显示 `Translating` 步骤；`BottomBar` 显示目标语言 | S | T4-5 | 手测 | ✅ 已验证 —— `FileItem.tsx` **无需改动**：它把 `step_name` 小写并把空格换成 `_` 后查 `steps.<key>`，因此新增的 `Translating` → `steps.translating` 自动生效（key 已在两 locale）。`BottomBar.tsx` 新增 `Languages` 图标 chip，仅 `translateEnabled` 时显示 `bottomBar.translationTarget` + 目标语言标签 |
| T5-6 | i18n：`en.json` / `zh-CN.json` 同步新增 key（含 `steps.translating`） | S | T5-3..5 | `bun run check:i18n` 通过 | ✅ 已验证 —— 实跑 `bun run check:i18n` → `OK — 2 locales match en.json (153 keys each)`。新增 key：`steps.translating`、`bottomBar.translationTarget`、`settings.translate.*`(7)、`errors.translationModelNotInstalled`、`models.recognitionTitle`/`translationTitle` |
| T5-7 | `src-tauri/tests/pilot/smoke.toml` 增加翻译设置开关的冒烟步骤 | S | T5-3 | `tauri-pilot run` 通过 | ⚠️ 部分验证 —— `smoke.toml` 新增 7 步（`git diff` 计数）：2 步前置（清空 `monosub-storage`、重载以重新 hydrate）+ 5 步翻译（打开 `settings-category-translate`、断言出现「目标语言」、断言 `translation-enable[data-state='unchecked']`、点击开启、断言 `checked`）。**未执行** `tauri-pilot run`（需 `bun run tauri dev` 在跑） |

---

## Phase T6 — 收尾与打磨

| ID | 任务 | 规模 | 依赖 | 验收 | 状态 |
|----|------|------|------|------|------|
| T6-1 | 更新 `AGENTS.md`：架构图、目录、IPC 字段、步骤名、环境变量、资源脚本 | S | T4,T5 | 文档与代码一致 | ✅ 已验证 —— `AGENTS.md` 增补：架构图加 llama.cpp 子进程、目录树加 `src-tauri/src/translate/` 与 `fetch-llamacpp.sh`、模型方案节说明翻译模型不进 `ModelPlan`、IPC 表加 3 个请求字段与 `Translating`（`total_steps` 4/5）、`models.json` schema（`engine`/`translate` role/`prompt_template`）、`MONOSUB_LLAMACPP_PATH`、输出命名 `<stem>.<lang>.<ext>`、失败语义、许可节（llama.cpp MIT 随包 / HY-MT 仅署名） |
| T6-2 | 质量回归：T0 的样例字幕跑一遍，记录耗时与人工抽查结果 | S | T4-4 | 记录入 PoC 文档 | ⬜ **未开始** —— 需要真实 ≥30 min 素材；PoC §8.3 已把「无质量结论」写成明确缺口 |
| T6-3 | （可选）双语字幕输出（原文 + 译文同 cue） | M | T4-4 | — | ⬜ |
| T6-4 | （可选）`skipExistingSubtitle` 在原文存在、译文缺失时只补翻译 | M | T4-4 | — | ⬜ |
| T6-5 | （可选）队列内复用同一个 `llama-server` 实例 | M | T4-4 | 多文件总耗时下降 | ⬜ |
| T6-6 | （可选）短句合并后再翻译以改善上下文 | M | T3-4 | — | ⬜ |

---

## 每阶段完成前

`bun run test:rust`、`bun run lint:rust`、`bun run check`、`bun run lint`、`bun run check:i18n` 全部通过。
