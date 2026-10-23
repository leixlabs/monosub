# 模型方案选择（Model Plan）设计

> 状态：**设计稿（待实现）**。取代 `refactor-audiocpp.md` §5.7 的「多说话人 Phase 4 单一选型」思路。
>
> 相关文档：[refactor-audiocpp.md](./refactor-audiocpp.md)（总体重构）、
> [audiocpp-poc.md](./audiocpp-poc.md)（CLI 实测行为）、[tasks.md](./tasks.md)（任务清单）。
>
> 全文的 family / task 名称均以**钉住的 audio.cpp v0.9.1** 二进制
> `audiocpp_cli --list-loaders --json`（114 个 family）为准。

---

## 0. 修订：四个问题（现行实现）

§3.1 最初把 `prefer: SpeedPreference` 当作一个独立需求。实现后改为**四个问题**，用户
回答完这四个，方案就完全确定：

| # | 设置 | 取值 | 决定什么 |
|---|---|---|---|
| 1 | `language_profile` 语种场景 | `mandarin` / `english` / `mixed` / `multilingual` | §5.4 的 family 偏置表。`english` 偏向英文专精家族（`parakeet_tdt` 首位），补上 `mandarin` + `auto` 到不了的位置。`mixed`/`multilingual` 把表重排向能跟随句中语言切换的模型，于是「一次会话一个语言 token」的 `sense_asr` 不再是中文首选。 |
| 2 | `diarization` 说话人分离 | bool | 是否枚举 `Passthrough` 或 `Asr+Diarize+TurnAttribution` 模板 |
| 3 | `word_timestamps` 词级时间戳 | bool | `word_shortfall_rank`：**排序项，不是过滤器**，所以目录里没有词级模型时仍然能选出一个并报缺口 |
| 4 | `memory` 显存占用 | `small` / `medium` / `large` | `footprint(spec)`（权重文件字节数）→ `over_budget_rank` + 评分项 `memory_match` |

**速度偏好被删除了。** 在这个目录里，权重体积同时决定显存和吞吐（SenseVoice Q8 254 MB
且最快；1.0–1.2 GB 的模型才是慢的），所以「快」和「小」是同一个问题，分开问只会多问一次。
前端 store 的 v6 迁移把 v5 的 `fast`/`balanced`/`accurate` 映射成 `small`/`medium`/
`large`，而不是把它丢掉。

两条实现约束：

- **显存预算是排序项，永远不是硬过滤。** `footprint` 按下载字节分类（< 512 MB 小、
  < 1.5 GB 中、否则大），**不看量化文件名**——一个仍然 1.5 GB 的 q4 文件照样要占
  1.5 GB。没有任何候选符合预算时仍然选最接近的那个，且**不报缺口**：体积超标是成本，
  不是缺失的能力。`MEMORY_OVERSHOOT_PENALTY`（150）大于
  `RECOMMENDED_TIER_BONUS + TONGYI_SOURCE_BONUS`（共 160），这就是让「小」真正能压过
  一个 `tier: recommended` 的 1 GB 通义模型的原因。
- **语种场景也是排序项。** 目录里没有声明「哪些模型真的能句中切换语言」，硬造这个字段
  等于让 App 声称一个它无法验证的事实；因此 `language_profile` 只重排
  `bias_families`，不参与硬过滤。
- **能力优先于成本。** `best_model_for_role` 的排序键顺序是：词级时间戳可达性 →
  语种场景偏置 → 显存拟合 → 总分。**能力永远压过预算**：`中英混合` + 显存小 仍然会选
  1 GB 的多语种模型，因为便宜的那个会漏掉半句话。把预算排在前面是实测出来的 bug
  （中英混合音频被派了 254 MB 的 `sense_asr`）；app 会在方案预览里报告实际占用的显存
  档位，超标是可见的，不是静默的。

---

## 1. 目标与非目标

### 1.1 目标

把「用哪个模型、怎么跑」从**硬编码**变成**按需求求解**：

- 用户在 Settings 里按**能力**提需求，而不是按模型名选模型。
- 需求 → 自动选出一个**模型方案（Plan）**：一个或多个 audiocpp task 的有序组合。
- 用户始终可以**手动接管**，指定具体模型（覆盖自动求解）。
- 不做 CAM++ / 声纹嵌入的重写——只组合 audiocpp 已有的 `task`。

### 1.2 非目标

- 不引入 Python / torch / ModelScope pipeline（`core-engine/` 已删）。
- 不为 diarization 自己实现说话人归属算法（除非某个组合真的需要，见 §6.4）。
- 不改动现有的事件契约字段（只**新增**可选字段，见 §9）。

### 1.3 设计原则

1. **阿里系（通义/FunASR）优先**：`sense_asr`、`fun_asr_nano` 是默认候选。
2. **能力驱动**：先算「需要什么」，再找「谁能提供」。
3. **可解释**：UI 必须能说明「为什么选了它」，以及「为什么某个组合不可用」。
4. **可降级**：任一需求找不到满足者时，明确告知缺哪个模型，并给出下载入口。
5. **可覆盖**：自动求解是默认值，不是强制。

---

## 2. 事实基线（已实测/已核对）

### 2.1 v0.9.1 里与"人"相关的 task

跑 `audiocpp_cli --list-loaders --json` 得到的**全部**相关 family：

| task | family | modes | 说明 |
|------|--------|-------|------|
| `vad` | `silero_vad` | offline, streaming | **已随包内置**（1.2 MB） |
| `vad` | `marblenet_vad` | offline | 备选 VAD |
| `vad` | `pulsevad` | offline | 备选 VAD |
| `diar` | `nemotron_3_diar` | offline, streaming | turns only，≤8 人 |
| `diar` | `sortformer_diar` | offline | turns only，4 人 |
| `diar` | `sortformer_diar_v2` | offline, streaming | turns only，4 人，流式 |
| `align` | `qwen3_forced_aligner` | offline | 词级时间戳 |
| `align` | `mms_forced_aligner` | offline | 词级时间戳（英文系） |
| `asr` | `sense_asr` | offline, streaming | **默认**，无时间戳 |
| `asr` | `fun_asr_nano` | offline | 无时间戳、无 diar |
| `asr` | `qwen3_asr` | offline, streaming | 需配 aligner 出词级 |
| `asr` | `moss_transcribe_diarize` | offline, streaming | **转写 + 时间戳 + 说话人** |

**关键否定事实**：114 个 family 的任务集合里**没有 `spk` / `sv` 任务**，也不存在
CAM++ / speaker-embedding loader。所以旧的 `diarizer.py`（ModelScope CAM++ pipeline）
**无法**在 audiocpp 侧复现。说话人能力只能来自 `diar` task，或 `asr` 里自带说话人的
`moss_transcribe_diarize`。

### 2.2 各模型的能力矩阵

| 模型 | family | 阿里系 | 时间戳 | 说话人 | 语言 | 体积 | 许可 |
|------|--------|--------|--------|--------|------|------|------|
| SenseVoice Small Q8 | `sense_asr` | ✅ | ❌ 无 | ❌ | zh/en/yue/ja/ko + 23 tag | 254 MB | FunASR Model License v1.1 |
| Fun-ASR-Nano-2512 | `fun_asr_nano` | ✅ | ❌ 未暴露 | ❌ 需组合 | auto/zh/en/ja | ~1.2 GB | FunASR Model License v1.1 |
| Qwen3-ASR-0.6B | `qwen3_asr` | ❌ | **词级**（+aligner） | ❌ | 52 种 | 中 | Apache-2.0 |
| MOSS-Transcribe-Diarize 0.9B | `moss_transcribe_diarize` | ❌ | **段级** | ✅ **自带 `[Sxx]`** | 50+ | Q4_K 758 MB / Q8_0 987 MB / BF16 1.83 GB | **Apache-2.0** |
| Nemotron 3 Diarization | `nemotron_3_diar` | ❌ | turns 级 | ✅ turns only | — | **107 MB** (q8_0) | OpenMDW 1.1 |
| Sortformer Diar 4spk v1 | `sortformer_diar` | ❌ | turns 级 | ✅ turns only，≤4 人 | — | 235 MB (f16) | **CC-BY-NC-4.0** |
| Sortformer Diar v2.1 | `sortformer_diar_v2` | ❌ | turns 级 | ✅ turns only | — | — | NVIDIA，**仅本地使用，待批准** |

**许可证说明**：本项目为个人学习用途，非商用；`sortformer_diar` 的 CC-BY-NC-4.0
因此**可以使用**（商用项目则必须排除它）。`sortformer_diar_v2` 仍需谨慎：上游标注
"local-use only pending NVIDIA Open Model License redistribution approval"，**不适合
写进可下载清单**，除非确认已批准。

> ⚠️ **重要结论**：**阿里系（`sense_asr` / `fun_asr_nano`）在 audiocpp 里没有任何一个
> 模型支持说话人分离**。FunASR 生态里那个「Fun-ASR-Nano + VAD + CAM++」的组合能力，
> 是 Python `AutoModel(... spk_model="cam++")` 的**编排能力**，不是 checkpoint 输出；
> audio.cpp 对 `fun_asr_nano` 的移植明确写着 `Timestamps: Not exposed`，更没有 diar。
> 所以「说话人分离」这一项必须落到非阿里系模型上（`moss` 或 `diar`+`asr` 组合）。

### 2.3 MOSS 质量参考（第三方复测，RTX 4090 / vLLM，`collar=0.25s`）

| 数据集 | 语言 | 时长 | DER ↓ | WER ↓ |
|--------|------|------|-------|-------|
| OpenAI 发布会 | 英 | 26:13 | 14.88 % | 4.02 % |
| TheValley101 | 中英混合 | 27:08 | **4.98 %** | 4.32 % |

- ✅ 转录质量好；中文场景 DER 低。
- ⚠️ **快速交替说话人会塌成一个标签**（发布会里 Mark Chen 与 Barrett Zoph 被合并）。
- ⚠️ **欠分割**（只出 5 个说话人 vs 参考约 10 个）。
- ⚠️ 自回归 LLM 解码，有 `max_tokens` 截断风险（默认约 5120 token 就必须显式调大）。

> 这些短板正是「**必须能在 Settings 里换方案**」的理由：MOSS 在中文单人/主导说话人
> 场景很好，在快速交替多人场景会退化。用户的设备资源、语种、素材类型都不同，
> 不能替用户拍板。

---

## 3. 核心概念：需求 → 方案（Plan）

### 3.1 能力需求（Capability Requirements）

用户不提"模型"，只提**需求**：

```rust
/// 一次转写对模型能力的硬性需求。
pub struct Requirements {
    /// 是否需要词级时间戳（精确对齐 / 剪裁）。
    /// false = 段级或估算时间戳即可。
    pub word_timestamps: bool,
    /// 是否需要说话人分离。
    pub diarization: bool,
    /// 是否允许「一个模型一次出全部结果」（直出）。
    /// false = 必须走「ASR + 后处理/turns 归属」的多阶段组合。
    pub allow_passthrough: bool,
    /// 应用语言（`zh` / `zh-tw` / `en` / …）。
    pub language: String,
    /// 语种场景（§0）：决定 §5.4 偏置表，并决定 `passthrough_beats_asr` 的第 3 个条件。
    pub language_profile: LanguageProfile,   // Mandarin | Mixed | Multilingual
    /// 显存占用（§0）：取代原来的 `prefer: SpeedPreference`。
    pub memory: MemoryBudget,                // Small | Medium | Large
    /// 设备后端（metal / cpu）。今日的 `footprint` 分类与后端无关，保留字段是为了
    /// 让请求里的 device 选择有唯一读取点。
    pub backend: Backend,
}
```

### 3.2 模型方案（Plan）

```rust
/// 一个可执行的模型方案：有序的 task 步骤。
pub struct ModelPlan {
    /// 稳定 id，用于 UI 展示与日志（如 `passthrough:moss`、`asr+vad+turns:nemotron`）。
    pub id: String,
    /// 人类可读标题的 i18n key（见 §8.3）。
    pub title_key: String,
    /// 需下载的模型 id 列表（可能多个）。
    pub model_ids: Vec<String>,
    /// 有序步骤。
    pub steps: Vec<PlanStep>,
    /// 直出模式：单个 step 同时产出文本 + 时间戳 + 说话人。
    pub passthrough: bool,
    /// 相对成本估计（用于 UI 标注与排序，不是精确预测）。
    pub cost: PlanCost,
    /// 该方案的能力覆盖情况；缺口也显式列出（用于解释与降级提示）。
    pub capabilities: Capabilities,
}

pub struct PlanStep {
    pub kind: StepKind,
    /// 选中的 model id。
    pub model_id: String,
    /// audiocpp `--task`。
    pub task: String,
    /// audiocpp `--family`。
    pub family: String,
}

pub enum StepKind {
    Vad,            // 切段（silero_vad）
    Asr,            // 整段或分段转写
    Align,          // 词级对齐（qwen3_forced_aligner）
    Diarize,        // 只出 speaker turns（nemotron_3_diar / sortformer_diar）
    /// 直出：单步给出文本 + 时间戳 + 说话人（moss_transcribe_diarize）
    Passthrough,
    /// 纯 Rust：把 turns 按时间重叠归属到 ASR 段（旧 assign_speakers 的移植）。
    TurnAttribution,
}
```

`Capabilities` 描述一个模型/方案**实际能提供**什么，是求解与 UI 解释的唯一依据：

```rust
pub struct Capabilities {
    /// `None`=无，`Segment`=段级，`Word`=词级。
    pub timestamps: Option<TimestampGrade>,
    pub diarization: DiarizationGrade,   // None | Turns | TurnsWithText
    /// 该模型自带文本输出（diar-only 模型为 false）。
    pub produces_text: bool,
}
```

### 3.3 能力从哪来

**不再硬编码在 Rust 里**，而是写进 `models.json`（§4）。求解器只读目录，因此
新增模型 = 改 JSON，不改代码。

---

## 4. `models.json` 扩展

### 4.1 新增字段

```jsonc
{
  "version": 2,
  "default": "sensevoice_small_q8",
  "models": [
    {
      "id": "sensevoice_small_q8",
      "family": "sense_asr",
      "display_name": "SenseVoice Small (Q8)",
      // ...
      "capabilities": {
        "timestamps": "segment",       // none | turns | segment | word
        "diarization": "none",         // none | turns | turns_with_text
        "produces_text": true
      },
      "roles": ["asr"],                // asr | vad | align | diar | passthrough
      "speed": "fast",                 // fast | balanced | accurate
      "source": "tongyi",              // tongyi | nvidia | openmoss | other（用于"阿里系优先"排序）
      "tier": "default"                // default | recommended | optional | experimental
    },
    {
      "id": "moss_transcribe_diarize_q4_k",
      "family": "moss_transcribe_diarize",
      "display_name": "MOSS-Transcribe-Diarize 0.9B (Q4_K)",
      "target_directory": "MOSS-Transcribe-Diarize-GGUF",
      "model_file": "moss-transcribe-diarize-q4_k.gguf",
      "capabilities": {
        "timestamps": "segment",
        "diarization": "turns_with_text",
        "produces_text": true
      },
      "roles": ["passthrough", "asr"],
      "speed": "balanced",
      "source": "openmoss",
      "tier": "optional",
      "licenses": [{ "name": "Apache-2.0", "url": "..." }]
    },
    {
      "id": "nemotron_3_diar_q8_0",
      "family": "nemotron_3_diar",
      "capabilities": {
        "timestamps": "turns",
        "diarization": "turns",
        "produces_text": false
      },
      "roles": ["diar"],
      "source": "nvidia",
      "tier": "optional"
    }
    // ...
  ]
}
```

### 4.2 schema 变更要点

| 字段 | 必要性 | 用途 |
|------|--------|------|
| `capabilities.timestamps` | **必需** | 求解「是否需要词级」 |
| `capabilities.diarization` | **必需** | 求解「是否需要说话人」 |
| `capabilities.produces_text` | **必需** | 区分 diar-only 模型（turns 无文本） |
| `roles` | **必需** | 约束每个 step 能从哪些模型里选 |
| `speed` | 可选 | 同能力排序 |
| `source` | 可选 | **阿里系优先**排序（`tongyi` 排在前面） |
| `tier` | 可选 | UI 分组与默认可见性 |
| `licenses[]` | 可选 | 替代单一 `license: string`，支持多许可（如模型 + 转换产物） |
| `languages` | 已有 | 语种筛选（§5.3） |

**向后兼容**：`capabilities` 缺失时按 `{ timestamps:"segment", diarization:"none",
produces_text:true }` 兜底，即「普通 ASR」；`licenses[]` 缺失时回落到 `license`。
`version` 从 1 升到 2，读入时按 version 分支（或直接用 `Option` + `serde(default)`）。

---

## 5. 求解器（Solver）

### 5.1 输入 / 输出

```
Requirements  ──►  Solver  ──►  Vec<ModelPlan>（按成本排序）
                    │
                    ├── installed 状态（只推荐已装模型，或给出下载入口）
                    └── catalogue（models.json）
```

### 5.2 候选生成规则

按需求的组合，枚举**候选方案模板**，再为每个模板挑选具体模型：

| 需求 | 候选模板（优先级从上到下） |
|------|---------------------------|
| 词级时间戳 **否**，说话人 **否** | ① `Asr`（直出文本，走 VAD+sequence 现有管线）<br>② `Passthrough`（若模型更快/更合语种） |
| 词级时间戳 **否**，说话人 **是** | ① `Passthrough`（MOSS，一步出全部）<br>② `Asr` + `Diarize` + `TurnAttribution`（复用 SenseVoice） |
| 词级时间戳 **是**，说话人 **否** | ① `Asr` + `Align`（qwen3_asr + qwen3_forced_aligner）<br>② `Passthrough`（若其时间戳粒度够用，则需 `allow_passthrough`） |
| 词级时间戳 **是**，说话人 **是** | ① `Asr(+Align)` + `Diarize` + `TurnAttribution`<br>② `Passthrough` + `Align`（MOSS 出段级 → aligner 细化）<br>（两者都无完美解，需明确提示精度折衷） |

### 5.3 过滤与排序

对每个候选模板，选出满足约束的模型集合，再打分排序：

**硬过滤（不满足则模板不可用）**

1. `capabilities` 覆盖该 step 角色（`roles` 含该角色）。
2. `languages` 包含归一化后的应用语言（复用 `catalog::map_language` 的
   `normalize_language_tag`）；`auto` 永远通过。
3. 模型已安装 —— **未安装不致命**，但要标记（§7.2）。

**软排序（打分）**

```
score =  100 * (tier == "recommended" ? 1 : 0)
       +  60 * (source == "tongyi" ? 1 : 0)        // 阿里系优先
       +  memory_match(spec, memory)                // 见下，取代 speed_match
       +  30 * (installed ? 1 : 0)                 // 已装优先
       +  20 * passthrough                         // 直出加分
       -  50 * (step_count - 1)                    // 步骤越少越好
```

`memory_match` 只在候选**没有超出**显存预算时给 +40（类一致），比预算小一档时给 +20；
超出时按 `over_budget_rank` 每档扣 `MEMORY_OVERSHOOT_PENALTY = 150`。150 是刻意大于
`100 + 60` 的：否则 `tier: recommended` 的 1 GB 通义模型会压过 254 MB 的 SenseVoice，
「显存小」就成了摆设。

### 5.4 语言驱动的模型选择（用户明确要求）

`bias_families(language, language_profile)`。`Mandarin` 走下面这张按语言的表；
`English`（英语为主）把表换成 `parakeet_tdt → sense_asr → qwen3_asr`——`Mandarin` +
`auto` 的首选是 `sense_asr`，所以若不单列，英语场景永远到不了英文专精家族；
`Mixed`（中英混合）与 `Multilingual`（多语种）把表换成
`qwen3_asr → moss_transcribe_diarize`——只有这两家能跟随句中语言切换，`sense_asr` /
`fun_asr_nano` 是「一次会话一个语言 token」的中文家族。

**偏置，不覆盖 `capabilities` 硬过滤**：

| 语言 | 首选 ASR | 备注 |
|------|----------|------|
| `zh` / `zh-tw` | `sense_asr` → `fun_asr_nano` → `qwen3_asr` | 阿里系中文强 |
| `en` | `sense_asr` → `parakeet_tdt` → `qwen3_asr` | Parakeet 英文极快 |
| `ja` | `sense_asr` → `fun_asr_nano` | 两者均支持 |
| `ko` | `sense_asr` | `fun_asr_nano` 不支持 ko |
| `yue` | `sense_asr` | 粤语，SenseVoice 独有 |
| `auto` | `sense_asr`（先跑语言识别） | — |
| `es/de/fr/pt` | `sense_asr` 会回落 `auto`；→ `qwen3_asr` / `cohere_asr` / `canary_asr` | 见 §2.2 的语言列 |

> 注：`sense_asr` 只把 `{auto, zh, en, yue, ja, ko, nospeech}` 映射为真实 token，
> 其余 23 个 tag 一律内部回落 `auto`（PoC §4.2）。这条规则已在
> `catalog::map_language` 实现，求解器复用同一函数，不重复判断。

### 5.5 求解器伪码

```rust
pub fn solve(
    catalogue: &[ModelSpec],
    installed: &dyn Fn(&str) -> bool,
    req: &Requirements,
) -> Vec<ModelPlan> {
    let mut plans = Vec::new();

    for template in templates_for(req) {          // §5.2
        let mut steps = Vec::new();
        let mut ok = true;
        for role in template.roles {
            let Some(best) = best_model_for_role(catalogue, installed, role, req) else {
                ok = false;                       // 该模板缺模型
                break;
            };
            steps.push(PlanStep { kind: role.into(), ..best });
        }
        if !ok { continue; }                      // 记录缺口，供 UI 解释（§8.4）

        let caps = union_capabilities(&steps);    // 组合后的实际能力
        let missing = gaps(req, &caps);           // 需求与能力的差集
        plans.push(ModelPlan { /* … */, capabilities: caps, /* … */ });
        let _ = missing;
    }

    plans.sort_by(|a, b| score(b, req).cmp(&score(a, req)));
    plans
}
```

### 5.6 缺口处理（降级）

若**所有**模板都缺模型，返回带缺口说明的空方案，UI 显示：

> 「需要说话人分离，但当前没有可用的分离模型。建议下载 MOSS-Transcribe-Diarize (Q4_K, 758 MB)。」
> 　　［去下载］

这条路径必须走通，否则用户会看到"功能坏了"。

---

## 6. 直出（Passthrough）与多阶段

### 6.1 为什么要区分

`moss_transcribe_diarize` 一次调用能同时给**文本 + 时间戳 + 说话人**。如果用户勾了
「直出」，我们应该**跳过整个 VAD → 切片 → requests.json → sequence** 管线，
直接一次 CLI 调用。这带来三个好处：

- 少一次模型加载、少一堆临时切片文件（长音频尤其明显）。
- 时间戳来自模型本身，不是按字数估算的，**更准**。
- 说话人标签与文本同源，不存在 turns↔文本归属误差。

代价：

- 失去 VAD 分段带来的"稳"，且 MOSS 的快速交替会塌标签（§2.3）。
- 长音频输出 token 上限（需调 `max_tokens`）。

### 6.2 开关语义（用户明确要求的"多一个配置项"）

`Requirements.allow_passthrough` 是一个**独立配置项**：

- **ON**：允许求解器选直出方案（当它得分更高时）。
- **OFF**：强制多阶段组合，**不考虑** `passthrough` 角色模型。

UI 上是一个三态而非二态更合适（见 §8.2）：

| UI 选项 | `allow_passthrough` | 行为 |
|---------|---------------------|------|
| 自动 | `true` | 求解器自由选 |
| 只要直出 | `true` + 只保留 `Passthrough` 模板 | 强制单模型 |
| 不要直出 | `false` | 强制多阶段组合 |

### 6.3 直出调用形状

```bash
audiocpp_cli --task asr --family moss_transcribe_diarize \
  --model <model.gguf> --backend metal --audio audio.wav \
  --text-out transcript.txt \
  --segments-out segments.json \
  --turns-out turns.json \
  --metrics
```

- **整段音频**，不切分。
- `--text-out` 是**带时间戳和说话人的** transcript；`--segments-out` 给段级时间戳。
- 长音频需要 `--request-option max_tokens=<n>`（经 `requests.json` 或单请求路径传递；
  注意 PoC §4.1：**`--request-option` 与 `--request-sequence` 不兼容**，直出走单请求
  路径所以 `--request-option` 可用，但为统一起见建议仍走 `--instruct`/CLI 直参）。

### 6.4 多阶段组合：turns 归属

`Asr + Diarize + TurnAttribution` 中的 `TurnAttribution` 是**纯 Rust**步骤，
移植旧 `diarizer.py::assign_speakers` 的语义：

```
输入：asr_segments[{start,end,text}]、diar_turns[{start,end,speaker_id}]
对每个 asr 段：取与之重叠时间最长的 turn 的 speaker_id
speaker_id → 标签：按首次出现顺序映射为 "Speaker A"、"Speaker B"、…（超出 Z 截断）
若最终只有 1 个说话人 → 隐藏标签（对齐旧 should_show_speaker_labels）
```

这与 `subtitle/writer.rs` 里已预留的 `[Speaker A]: ` 前缀完全对接。
**这是唯一需要 Rust 自己写算法的地方**，且它是旧代码的直译，不是新研究。

### 6.5 直出与多阶段的输出统一

`engine::transcribe` 的返回类型需要扩展，因为现在要携带 speaker：

```rust
// engine/mod.rs（签名扩展，保持向后兼容）
pub struct RawChunk {
    pub start_s: f64,
    pub end_s: f64,
    pub text: String,
    /// 新增：已格式化的说话人标签（`Speaker A` / `None`）。
    pub speaker: Option<String>,      // ← 新增字段
}
```

`subtitle::postprocess` 需要把 `chunk.speaker` 传到 `Segment.speaker`：

- **同说话人连续段**在 merge 阶段不应被合并（否则标签会错）；`gap_threshold`
  合并规则增加一条前置条件：`a.speaker == b.speaker`。
- 这在 `postprocess.rs` 里是一个小改动，但要**单独测试**（§11）。

---

## 7. Rust 侧改动清单

### 7.1 新模块

| 文件 | 职责 |
|------|------|
| `engine/plan.rs` | `Requirements` / `ModelPlan` / 求解器 / 打分。**纯函数，可完整单测**（不依赖 `AppHandle`）。 |
| `engine/direct.rs` | 直出路径：单次 CLI 调用 + 解析 `--turns-out` / `--segments-out`。 |

### 7.2 `engine/mod.rs`

- `TranscribeOpts` 增加 `plan: ModelPlan`（或 `requirements: Requirements` 由内部求解）。
- `transcribe()` 按 `plan.passthrough` 分派：直出走 `direct.rs`，否则走现有
  VAD+sequence 管线。
- 多阶段组合在现有管线后追加 `Diarize` + `TurnAttribution`。
- `RawChunk` 加 `speaker` 字段（§6.5）。

### 7.3 `engine/catalog.rs`

- `ModelSpec` 增加 `capabilities` / `roles` / `speed` / `source` / `tier` / `licenses`。
- `load_catalog` 解析新字段（带 `serde(default)` 兼容）。
- 新增 `ModelSpec::capabilities()` 兜底逻辑（§4.2）。
- 现有测试全部保留；新增新字段的解析测试。

### 7.4 `commands/transcribe.rs`

- `TranscribeRequest` 增加**可选**字段（见 §9.1），旧前端不发也能跑。
- 删除 §5.7 的「diarization 直接报错」分支，改为**求解 + 执行**。
- 「未装模型」判定改为**按求解出的 plan 里所有模型**检查，返回 `MODEL_NOT_INSTALLED`
  （携带缺失的 model id，供前端跳转）。

### 7.5 命令层新增

| 命令 | 用途 |
|------|------|
| `resolve_model_plan` | 前端调用：传 `Requirements`，返回求解结果（含候选与缺口解释），用于 UI 实时预览「当前设置会用什么方案」。 |
| `get_effective_plan` | 返回当前任务实际使用的 plan（用于日志/调试面板）。 |

### 7.6 `commands/models.rs`

- 下载/校验/删除逻辑**不变**（已支持任意 model id）。
- `list_models` 返回值增加 `capabilities` / `roles` / `source` / `tier`，
  供前端分组与解释。

---

## 8. 前端 UI 设计

### 8.1 Settings 新增「识别方案 / Model Plan」分类

在现有 `models` 分类**之前或合并**新增一个分类（建议合并进 `models`，减少分类数）：

```
Settings ▸ Models
├── 方案选择（Plan）
│   ├── [ ] 词级时间戳（精确对齐，模型更大更慢）
│   ├── [x] 说话人分离（多人对话）
│   ├── 直出模式： ( ) 自动  ( ) 只要直出  ( ) 不要直出
│   ├── 速度偏好： ( ) 快  ( ) 均衡  ( ) 准
│   └── ▸ 当前方案：MOSS-Transcribe-Diarize (Q4_K)  ⓘ 为什么？
│        └── 提示：直出不支持词级时间戳，如需词级请关闭"直出"
├── 高级：手动指定模型
│   ├── ASR：[SenseVoice Small (Q8) ▾]        （自动 / 具体模型）
│   ├── VAD：[Silero VAD（内置）▾]
│   ├── 对齐：[（不使用）▾]                    （qwen3_forced_aligner / mms）
│   └── 分离：[（不使用）▾]                    （nemotron_3_diar / sortformer_diar）
└── 模型列表（现有）
    ├── SenseVoice Small (Q8)     254 MB   已安装    [删除]
    ├── MOSS-Transcribe-Diarize (Q4_K)  758 MB   未安装 [下载]
    └── ...
```

### 8.2 关键交互

1. **方案预览必须是实时的**：每改一个选项就调 `resolve_model_plan`，把结果写在
   「当前方案」一行，并标注原因（`ⓘ 为什么？` 展开打分/过滤说明）。
2. **未安装的模型**：方案行显示 `未安装` + `[下载]`，点击即走现有下载流。
3. **手动指定覆盖**：任一手动下拉设成非「自动」即**锁定**该 step，求解器只填其余 step。
4. **缺口提示**：求解不出时，明确说缺哪个能力、推荐哪个模型、一键下载（§5.6）。
5. **BottomBar 的多人开关**：解禁（去掉 `multiSpeakerComingBack`），改为直接驱动
   `enableSpeakerDiarization`；若当前未装分离模型，置灰 + 提示去 Settings。

### 8.3 i18n（两个 locale 必须同步）

新增 key（`en.json` / `zh-CN.json` 都加）：

```
modelPlan.title                 识别方案
modelPlan.wordTimestamps        词级时间戳（精确对齐，模型更大更慢）
modelPlan.diarization           说话人分离（多人对话）
modelPlan.passthrough           直出模式
modelPlan.passthrough.auto      自动
modelPlan.passthrough.only      只要直出
modelPlan.passthrough.never     不要直出
modelPlan.speed                 速度偏好
modelPlan.speed.fast            快
modelPlan.speed.balanced        均衡
modelPlan.speed.accurate        准
modelPlan.effective             当前方案
modelPlan.why                   为什么？
modelPlan.notInstalled          未安装
modelPlan.download              下载
modelPlan.advanced              高级：手动指定模型
modelPlan.slot.asr              ASR
modelPlan.slot.vad              VAD
modelPlan.slot.align            对齐（词级时间戳）
modelPlan.slot.diar             分离（说话人）
modelPlan.slot.auto             自动
modelPlan.slot.none             （不使用）
modelPlan.gap.title             当前需求没有可用的模型方案
modelPlan.gap.suggest           建议下载 {{model}}（{{size}}）
modelPlan.plan.passthroughMoss   MOSS 直出（转写+时间戳+说话人）
modelPlan.plan.asrVad           SenseVoice + VAD（估算时间戳）
modelPlan.plan.asrDiar          SenseVoice + VAD + 说话人分离
modelPlan.plan.qwenAlign        Qwen3-ASR + 词级对齐
modelPlan.reason.tongyi         阿里系优先
modelPlan.reason.installed      已安装
modelPlan.reason.passthrough    直出模式更少步骤
modelPlan.reason.words          唯一支持词级时间戳的方案
```

移除：`bottomBar.multiSpeakerComingBack`（改为正常可用状态）。

### 8.4 `BottomBar.tsx`

```tsx
// 之前：checked={false} disabled={true}  + "即将回归" tooltip
// 之后：
<GlassSwitch
  checked={enableSpeakerDiarization}
  onChange={setEnableSpeakerDiarization}
  disabled={!hasUsableDiarizationPlan}   // 未装分离模型时置灰
  title={hasUsableDiarizationPlan ? t('bottomBar.multiSpeaker')
                                  : t('modelPlan.gap.diarization')}
/>
```

---

## 9. 事件契约扩展（保持向后兼容）

### 9.1 `TranscribeRequest` 新增字段（全部 `Option`）

```rust
pub struct TranscribeRequest {
    // ... 现有字段不变 ...
    /// 词级时间戳需求。
    pub require_word_timestamps: Option<bool>,
    /// 直出模式：None=自动，Some(true)=只要直出，Some(false)=不要直出。
    pub passthrough: Option<bool>,
    /// 速度偏好："fast" | "balanced" | "accurate"。
    pub speed_preference: Option<String>,
    /// 手动覆盖的 step → model id。
    pub model_overrides: Option<HashMap<String, String>>,   // role -> model_id
}
```

旧前端不传 → 全部走默认（§5 求解器默认 `sense_asr`），**行为与今天一致**。

### 9.2 `transcribe_progress.step`

- `step_name` **保持冻结的 4 个**不变（`FileItem.tsx` 依赖它派生 i18n key）。
- 直出模式步骤数更少？**不要改 `total_steps`**。做法：直出时把 `VAD` 与 `切片`
  折叠进 `Transcribing` 的 elapsed 里，仍按 4 步发事件。这样前端零改动。

### 9.3 新增事件（可选）

| 事件 | payload | 用途 |
|------|---------|------|
| `model_plan_selected` | `{task_id, plan_id, model_ids, passthrough}` | 前端显示「本次用了什么方案」 |

**只新增，不改旧事件**。

---

## 10. Store / 持久化

### 10.1 `AppSettings` 新增

```ts
export interface AppSettings {
  outputFormat: SubtitleFormat
  outputLanguage: OutputLanguage
  skipExistingSubtitle: boolean
  showCompletedNotification: boolean

  // 新增：识别方案需求（现行是 §0 的四个问题）
  requireWordTimestamps: boolean      // false
  languageProfile: 'mandarin' | 'english' | 'mixed' | 'multilingual'  // 'mandarin'
  memoryBudget: 'small' | 'medium' | 'large'              // 'medium'
}
// `passthroughMode` 与 `speedPreference` 都没有实现：前者由求解器按能力自动决定（§6.2
// 的三态没有进 UI），后者在 v6 被 `memoryBudget` 取代并做了值迁移。
```

### 10.2 手动覆盖

现有 `enableSpeakerDiarization: boolean`（顶层）保留，语义不变，映射到
`Requirements.diarization`。新增：

```ts
modelOverrides: Partial<Record<'asr'|'vad'|'align'|'diar', string>>
```

### 10.3 `persist` 迁移

`version: 2` → `version: 3`：

```ts
migrate: (persisted, version) => {
  if (version < 3) {
    return {
      ...persisted,
      settings: {
        ...persisted.settings,
        requireWordTimestamps: false,
        passthroughMode: 'auto',
        speedPreference: 'balanced',
      },
      modelOverrides: {},
    }
  }
  return persisted
}
```

实际落地是 v3 加需求字段、v4 加翻译、v5 加 URL 导入，这三步**只加字段**。v6 是唯一
一次替换字段：`speedPreference` 被 `languageProfile` + `memoryBudget` 取代，迁移把
`fast → small`、`accurate → large`、其余 `→ medium` 映射过去，而不是直接丢掉（否则
用户表达过的偏好会被静默重置）。

**注意**：v1→v2 的迁移里**丢弃**了 `enableNoiseReduction` 等 4 个字段（它们从没到过
引擎）。v3 以后**不要再丢任何东西**，只加默认值（v6 的替换也保留了语义）。

---

## 11. 测试策略

### 11.1 纯函数单测（`engine/plan.rs`）

求解器不依赖 `AppHandle`，可用**假目录 + 假 installed 闭包**完整覆盖：

| 用例 | 断言 |
|------|------|
| 无词级 / 无说话人（默认） | 首选 `sense_asr`（阿里系 + 已装） |
| 只要直出 + 说话人 | 必选 `moss_transcribe_diarize` |
| 要说话人 + 不要直出 | 选 `asr + diar(turns) + attribution` |
| 词级 + 说话人 | 明确返回两份方案并标注各自的精度折衷 |
| `language: ko` | 排除 `fun_asr_nano`（不支持 ko），保留 `sense_asr` |
| `language: yue` | 只有 `sense_asr` 通过 |
| 无任何模型已安装 | 返回带缺口的方案，不 panic |
| 目录里没有 diar 模型 | 「要说话人」时返回缺口 + 建议下载 |

### 11.2 `engine/direct.rs`

- 用**假 CLI 输出**（不用真模型，缺失时 `skip:`）解析 `--turns-out` / `--segments-out`，
  断言 `RawChunk.speaker` 正确。
- 采样点 → 秒（`sample / 16000`）。

### 11.3 `TurnAttribution`

移植旧 `assign_speakers`，用**goldens** 对齐 Python 行为（`tests/fixtures/`）：

- 最大重叠归属。
- 单说话人 → 隐藏标签。
- 超出 26 个说话人 → 截断到 Z。

### 11.4 `postprocess.rs` 说话人边界

新增用例：**不同说话人的相邻短段不得被 merge**（否则标签串位），
**同一说话人的相邻短段仍应 merge**。

### 11.5 前端

- `resolve_model_plan` 的 mock 返回驱动 Settings 单测（Vitest + Testing Library）。
- i18n 校验脚本（`check-i18n-keys.mjs`）自动覆盖新 key。

---

## 12. 分阶段实施

| 阶段 | 内容 | 规模 | 依赖 |
|------|------|------|------|
| **M1** | `models.json` v2 schema + `catalog.rs` 解析新字段（`capabilities`/`roles`/`source`/`tier`）+ 兜底逻辑 + 单测 | M | — |
| **M2** | `engine/plan.rs`：`Requirements`/`ModelPlan`/求解器/打分/缺口，纯函数 + 单测 | L | M1 |
| **M3** | `resolve_model_plan` 命令 + 前端 Settings「方案选择」区块（只读预览，先不改转写行为） | M | M2 |
| **M4** | `models.json` 补 `moss_transcribe_diarize` / `nemotron_3_diar` 条目（含 URL/SHA256，需实测 pinned revision）+ 下载验证 | M | M1 |
| **M5** | `engine/direct.rs` 直出路径 + `RawChunk.speaker` + 假输出单测 | L | M2, M4 |
| **M6** | `Diarize` + `TurnAttribution` 多阶段组合 + goldens 对齐 | L | M2, M4 |
| **M7** | `postprocess.rs` 说话人感知的 merge + `writer.rs` 恢复 `[Speaker A]: ` 前缀 | M | M5/M6 |
| **M8** | `commands/transcribe.rs` 接求解器、删「直接报错」分支、按 plan 检查安装；`TranscribeRequest` 新字段 | M | M5, M6 |
| **M9** | 前端：BottomBar 解禁、Store v3 迁移、i18n、手动覆盖下拉 | M | M3, M8 |
| **M10** | `tools/` 加 PoC spike 脚本（Metal 实测 Moss、turns 输出核对、下载可达性） | S | M4 |

**M10 必须在 M5/M6 之前跑完**，因为 Metal 上的 RTF 与 `--turns-out` 的实际输出形状
是最大的两个未知数（§13）。

---

## 13. 风险与开放问题

| # | 风险 | 处置 |
|---|------|------|
| 1 | **MOSS 在 Metal 上的 RTF 未知**（自回归 LLM 解码，可能远慢于 SenseVoice 单次前向） | M10 实测；若不可接受，`speed: accurate` 标注 + 默认不推荐，让用户显式开 |
| 2 | **`--turns-out` 在 v0.9.1 的实际形状/成熟度未知** | M10 用真模型核对；若形状不符，改解析逻辑或评估升 audio.cpp 版本 |
| 3 | **MOSS 下载可达性**：`hf-mirror.com` 对 `audio-cpp/audio.cpp-gguf` 只 308 跳到 `huggingface.co`（不像 SenseVoice 那样镜像得了） | ModelScope 上 `OpenMOSS/MOSS-Transcribe-Diarize` 的 safetensors 可达（1.82 GB）；但 GGUF 需要另寻镜像或自转（需 `audiocpp_gguf` 工具，**未随 CLI 包分发**） |
| 4 | **长音频 token 截断**：MOSS 默认约 5120 token 就截断 | 显式传 `max_tokens`；按音频时长估算下限 |
| 5 | **快速交替说话人塌标签**（§2.3 实测） | 这就是"可选方案"存在的理由；UI 标注 MOSS 的局限，并把 `asr+diar` 组合作为备选 |
| 6 | **`sortformer_diar` 是 CC-BY-NC** | 个人学习可用；若日后商用必须移出清单。`sortformer_diar_v2` 是 NVIDIA local-use-only，**默认不进清单** |
| 7 | **求解器复杂度蔓延**：需求维度一多，模板组合爆炸 | 模板**硬编码在 `templates_for()`**（4 种组合，§5.2），不做通用约束求解 |
| 8 | **`postprocess` 的 merge 改动可能影响现有（无说话人）输出** | 说话人感知的 merge 只在 `speaker.is_some()` 时改变行为；现有 goldens 必须仍然全绿 |
| 9 | 前端 Settings 已 991 行，再加区块会过大 | 拆出 `SettingsModelPlan.tsx` 子组件 |
| 10 | `enableSpeakerDiarization` 是 Pro 授权功能，现在解禁是否影响收费逻辑 | 需求确认：个人学习项目，授权 gate 可保留为**软提示**，不再是硬阻断 |

---

## 14. 与现有文档的差异（需要更正的旧结论）

`refactor-audiocpp.md` §5.7 原文建议「Phase 4 评估两条路径：① `moss_transcribe_diarize`；
② `sortformer_diar` + 重叠归属」。本文档的更正：

1. **不存在第三条路**（CAM++ / `spk` task 在 audio.cpp 里根本没有），原文没写错但
   容易让人以为可以复现 Python 方案。
2. `sortformer_diar` (v1) 是 **CC-BY-NC-4.0**，商用项目不可用；原文未标注许可差异。
3. `fun_asr_nano` **不能**提供说话人分离（原文表格里 `--turns-out` 一节容易误导）。
4. 新增 `nemotron_3_diar`（107 MB，OpenMDW 1.1，≤8 人）作为 **diar-only 首选**——
   比 sortformer 体积小 2 倍且许可宽松。
5. **不应固定单一模型**，而应做成本文档的**需求驱动求解器**。

`tasks.md` 的 `P4-3` 建议重写为 M1–M10 的任务分解（见 §12）。
