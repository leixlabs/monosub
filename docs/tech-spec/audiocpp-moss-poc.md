# MOSS-Transcribe-Diarize PoC（M10 spike）

> 状态：**已完成（结论：可用，但有明确短板）**
> 执行日期：2026-10-16
> 测试机：Apple M4 / macOS（Darwin 25.6.0），arm64
> 上游：audio.cpp **v0.9.1**（commit `cf124a67`），`audiocpp_cli`
> 模型：`moss-transcribe-diarize-q8_0.gguf`
> （size 1,132,110,560 B，sha256 `93eea5865615e270b827752945f2dfd0f522ed673f4f551675e9013170173679`）
> 相关：[model-selection-design.md](./model-selection-design.md) §12/§13、[model-selection-tasks.md](./model-selection-tasks.md) M10

---

## 1. 结论摘要

M10 的两个最大未知数（Metal 上的 RTF、`--turns-out` 的真实输出形状）**都已实测**：

| 问题 | 结论 |
|------|------|
| `--turns-out` / `--segments-out` 形状 | ✅ 与设计预期一致（见 §3），可直接解析 |
| Metal RTF | ✅ 0.16–0.76（约 1.3–6x 实时），**可接受**；但显著慢于 SenseVoice 单次前向 |
| 长音频 `max_tokens` 截断 | ✅ 默认值下 10.4 min 仍完整输出，**未触发截断** |
| 快速交替说话人 | ⚠️ **复现短板**：8 次快速交替被塌成单一标签 |

**裁决**：MOSS 作为**可选**直出方案（`tier: recommended`，可被求解器选中），
但**不作为强制默认**，且 UI 必须提示其「快速交替会塌标签」的局限。
退路（`sortformer_diar` + SenseVoice ASR + turns 归属）保留。

---

## 2. 实测方法与素材

在本机用 `say` 合成中文双人对话（避免版权与隐私问题），
`ffmpeg` 统一转为 16 kHz mono s16le WAV（与 App 的音频提取链一致）：

| 素材 | 时长 | 构造 | 用途 |
|------|------|------|------|
| `twospeaker.wav` | 32.87 s | Tingting / Meijia 交替 4 轮 | 基本功能 + 形状核对 |
| `fastalt.wav` | 3.09 s | 8 段快速交替（每段约 0.3 s） | 复现「塌标签」短板 |
| `long.wav` | 262.9 s | `twospeaker` 循环 8 次 | 中长音频稳定性 |
| `long10.wav` | 624.5 s | `twospeaker` 循环 19 次 | 逼近 `max_tokens` 截断边界 |

调用（与 App 计划采用的直出形状一致）：

```bash
audiocpp_cli --task asr --family moss_transcribe_diarize \
  --model moss-transcribe-diarize-q8_0.gguf --backend metal --threads 4 \
  --audio <wav> --text-out <txt> --segments-out <json> --turns-out <json> --metrics
```

---

## 3. 输出形状（已核对，可直接解析）

### 3.1 `--turns-out`（含说话人）

```json
[{"start_sample":1120,"end_sample":159360,"speaker_id":"S01","confidence":0,
  "text":"大家好，我是婷婷。今天我们来讨论字幕生成的问题。首先我想问一下，你们现在用什么工具做字幕？"},
 {"start_sample":159200,"end_sample":300960,"speaker_id":"S02","confidence":0,
  "text":"我们目前用的是桌面软件，本地跑模型。速度还可以，但是多人对话的时候说话人分不清楚。"}]
```

- **时间单位是采样点**，`秒 = sample / 16000`（`start_sample=1120` → 0.07 s，与 `--text-out` 的 `[0.07]` 一致）。
- `speaker_id` 形如 `S01`/`S02`（不是 `speaker_0`；与 [refactor-audiocpp.md](./refactor-audiocpp.md) §5.7 的示意不同，**以实测为准**）。
- `confidence` 恒为 `0`（未填充），**不可用作置信度门槛**。
- 每段**自带 `text`**，所以直出路径不需要另跑 ASR。

### 3.2 `--segments-out`（段级时间戳）

与 `--turns-out` **同一数组形状，只是没有 `speaker_id`**（同样带 `text`）：

```json
[{"start_sample":1120,"end_sample":159360,"confidence":0,"text":"…"}]
```

### 3.3 `--text-out`（内联标记）

`transcript.txt` 是**内联标记流**，不是纯文本：

```
[0.07][S01]大家好，…[9.96][9.95][S02]我们目前…[18.81][18.79][S01]对，…[32.85]
```

- `[start][Sxx]text[end]`，下一个段以 `[start][Sxx]` 继续。
- 解析应以 `--turns-out`/`--segments-out` 的 JSON 为准；`--text-out` 只作为兜底/日志。

### 3.4 stdout 行协议

CLI 在 stdout 打印，可用于进度与路径发现：

```
segments_out=<path>
turns_out=<path>
text_out=<path>
metrics.wall_ms=7448.81
metrics.rtf=0.226626
metrics.x_realtime=4.41255
metrics.memory.peak_footprint_mb=3147
metrics.audio_duration_ms=32868.2
```

`--request-option` **未使用**：直出走单请求路径，`--max-tokens` 可直接作为 CLI 直参
（PoC §4.1 的「`--request-option` 与 `--request-sequence` 互斥」在此不适用）。

---

## 4. 性能（Metal，M4，`--threads 4`）

| 素材 | 音频时长 | wall | **RTF** | x_realtime | 峰值 footprint |
|------|---------:|-----:|--------:|-----------:|---------------:|
| `twospeaker` | 32.9 s | 7.45 s | **0.2266** | 4.41x | 3147 MB |
| `fastalt` | 3.09 s | 2.35 s | **0.7578** | 1.32x | 3015 MB |
| `long` | 262.9 s | 41.6 s | **0.1583** | 6.32x | 4120 MB |
| `long10` | 624.5 s | 164.1 s | **0.2628** | 3.81x | 5764 MB |

- **RTF 随时长下降**（固定加载成本被摊薄）：10 分钟级约 **0.16–0.26**，即 **4–6x 实时**。
- **短音频 RTF 高**（3 s 素材 0.76）——固定冷启动占主导，短文件不划算。
- 峰值内存 **3.0–5.8 GB**（含 Metal 显存/统一内存），远高于 SenseVoice 的 ~560 MB。
  这是求解器 `memory_risk` 项存在的理由，也是「直出可选而非默认」的理由之一。
- 与 SenseVoice 的对比：SenseVoice 9.36 min 音频 wall **6.78 s**（RTF 0.0121）。
  即 MOSS 在同等音频上**慢约 10–20x**。**质量换速度**是明确取舍。

---

## 5. 长音频 `max_tokens`（未截断）

- `long10.wav`（**10.4 min**，76 turns）在**默认 `max_tokens` 下完整输出**：
  `last end_sample=9,991,040`，音频总长 `9,991,952` 采样点 —— 覆盖到结尾，
  尾部文本完整（`…也许可以试试新的模型。`）。
- 结论：**默认上限在本机 10 分钟内不构成截断风险**。
  设计 §13-4 仍保留「按音频时长估算 `max_tokens`」的下限保护，作为长素材（>15 min）的保险，
  但**不是**本 PoC 的必要修复。

---

## 6. 质量短板（复现）

### 6.1 快速交替塌标签 ✅ 复现

`fastalt.wav`（8 段、每段 ~0.3 s、Tingting/Meijia 交替）结果：

```
[0.06][S01]好，嗯，对，不是，来，走，行吗？[2.62][2.62][S01]可以。[2.98]
turns.json: 全部 speaker_id = "S01"（8 个真实说话人交替 → 1 个标签）
```

这与设计 §2.3 引用的第三方复测（发布会里 Mark Chen 与 Barrett Zoph 被合并）**一致**。
**这是模型行为，不是解析 bug**：`--turns-out` 只吐一个 `S01`。

### 6.2 正常语速双人对话 ✅ 正确

`twospeaker.wav`（4 轮、每轮 ~9 s）正确给出 `S01/S02/S01/S02`，边界与音频一致（误差 < 0.2 s）。

### 6.3 结论

- **单人 / 主导说话人 / 正常语速对话**：MOSS 直出可用，且时间戳来自模型本身（优于 VAD 估算）。
- **快速交替多人**：会退化。这正是「方案可在 Settings 里切换」的存在理由——
  用户可改用 `asr + diar(sortformer) + TurnAttribution` 组合。

---

## 7. 对实现的影响（回填 M5/M6）

1. **解析器**（`engine/direct.rs`）按 §3 的 JSON 形状实现：
   `start_sample/16000`，`speaker_id` → 首现顺序映射 `Speaker A/B/…`（>26 截断）。
2. **单说话人隐藏标签**：与 `sortformer` 组合一致，仅 1 个说话人时不加前缀。
3. **`--max-tokens`** 作为 CLI 直参按 `ceil(duration_s * k)` 传（k 取经验值，默认上限即可）。
4. **`confidence` 恒 0** → 不要用它做过滤或 UI 展示。
5. **求解器**：MOSS 标 `speed: balanced`（Q8_0）/ `accurate`（Q4_K）；
   `memory_risk` 必须体现「>1 GB + Metal」的高内存占用。

---

## 8. 数据更正（相对 tasks.md 原稿）

| 原稿 | 实测更正 |
|------|----------|
| M1-3 收录 `nemotron_3_diar_q8_0`（107 MB） | ❌ **ModelScope 上游已下架**（`Nemotron-3-Diarization-GGUF` 树不存在，pinned rev 404）。**不收录** |
| M4-1 默认 `moss q4_k`（758 MB） | ⚠️ 上游 README 明示 Q4_K *「not parity-safe: segment/timestamp drift」*（匹配段最多 680 ms 漂移）。**默认改 Q8_0**（1.13 GB，README 称「preserves words and speakers」）；Q4_K 保留为 `tier: optional` |
| §5.7 示意 `speaker_id: "speaker_0"` | 实测为 **`S01`**（含前导 `S` + 两位数字） |

> 所有下载 URL 均已用 Range 请求实测 **HTTP 206**，size/sha256 与 ModelScope 目录树一致。
</content>
