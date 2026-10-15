# Phase 0 PoC 报告：audio.cpp 可行性验证

> 状态：**Go（附条件）**
> 执行日期：2026-10-15
> 测试机：Apple M4 / macOS (Darwin 25.6.0)，arm64
> 上游版本：audio.cpp **v0.9.1**（commit `cf124a67`，2026-10-07）
> 结论回填：[refactor-audiocpp.md](./refactor-audiocpp.md)

---

## 1. 结论摘要

**Go。** 预编译 CLI 可以在不引入 Python 的前提下跑通「VAD 切段 → 逐段 SenseVoice 转写 → 拿回分段文本」的完整链路，速度与内存均**显著优于**现状 FunASR。

同时发现 **1 个会让 P1-9 首次实现直接失败的上游行为**（见 §4.1），以及 spec 中 3 处需要修正的描述。

| 维度 | 现状 FunASR (v1.1.38) | audio.cpp v0.9.1 | 结论 |
|------|----------------------|------------------|------|
| 9.36 min 音频 wall time | 12.98 s（二次） / 16.78 s（首次） | **6.78 s** | ✅ 快约 2x |
| 9.36 min 音频 峰值 RSS | 2.86 – 3.00 GB | **560 MB** | ✅ 降约 5x |
| RTF | ~0.023 | **0.0121**（约 83x 实时） | ✅ |
| 需分发的运行时 | Python 3.10 + torch + funasr | **单文件 31.6 MB，无第三方 dylib** | ✅ |
| 分段能力 | 内部 VAD，时间戳为估算 | 外部 VAD 给采样点级切段 | ✅ 更可控 |
| 中文准确率 | — | — | ⚠️ **未充分验证**，见 §6 |

---

## 2. 产物与校验值（P0-1 / P0-2）

| 产物 | 字节数 | SHA256 |
|------|--------|--------|
| `audio-v0.9.1-bin-macos-arm64-metal.tar.gz` | 30,519,606 | `960436787b84bf137a70ea1713ac460207ef2ac7b2617380fb7a1f4650d1100d` |
| ↳ `audiocpp_cli`（解出后） | 31,605,344 | — |
| `silero_vad_16k.safetensors` | 1,239,748 | `c59271c284ae9c8335d795d60e0bfdb71aaaceec578d9bd9ffc1b8153c319ea1` |
| `sensevoice-small-q8-audiocpp-v1.gguf` | 254,211,200 (242.4 MiB) | `4dedf169f625437fb336f2959674f399819729a765e184128c0e25a6e16ff0ec` |

已填入 `src-tauri/resources/models.json` 与 `scripts/build/fetch-audiocpp.sh`（均实测，非估算）。

**Silero VAD 来源修正**：spec §5.1 称「由同一脚本从上游仓库 `assets/framework/models/silero_vad/` 下载（固定 commit）」。实测该文件**在 v0.9.1 tag 中存在**，用 `raw.githubusercontent.com/.../v0.9.1/assets/framework/models/silero_vad/silero_vad_16k.safetensors` 即可，无需回溯 commit。已按 tag 固定。

**动态依赖实测**（`otool -L audiocpp_cli`）——与 spec §2 一致：

```
Accelerate, Foundation, Metal, MetalKit, CoreFoundation, libSystem, libc++, libobjc
```

全部为系统 framework，**无第三方 dylib**，Metal shader 已内嵌。✅ 单文件分发成立。
签名状态：`adhoc, linker-signed`（非 Developer ID），P2-4 需重签。

---

## 3. `--list-loaders --json`（P0-1）

共 **114** 个 loader。所需 6 个全部存在：

| family | tasks | modes |
|--------|-------|-------|
| `sense_asr` | asr | offline, streaming |
| `silero_vad` | vad | offline, streaming |
| `qwen3_asr` | asr | offline, streaming |
| `qwen3_forced_aligner` | align | offline |
| `moss_transcribe_diarize` | asr | offline, streaming |
| `sortformer_diar` | diar | offline |

`audiocpp_cli --version` → `audio.cpp 0.9.1 / git cf124a67 / backends: cpu,metal`。

---

## 4. 关键设计验证

### 4.1 ⚠️ `--request-option` 与 `--request-sequence` 不兼容（**spec 未覆盖，会阻断 P1-9**）

**现象**：按 spec §5.3 的命令行原样执行，直接失败：

```
audiocpp_cli failed: unknown options: --request-option --request-option
```

**根因**（`app/cli/main.cpp`）：CLI 用 `require_known_args()` 做「未识别参数」校验——任何**查询代码没跑过**的选项都会被判为拼写错误。而 `--request-option` 只在 `build_request_from_cli()`（`app/cli/request.cpp:335`）里被读取，该函数**仅**在单请求路径调用；进入 batch 分支后：

```cpp
// app/cli/main.cpp:876
engine::runtime::TaskRequest base_request =
    optional_path_arg(argc, argv, "--request-sequence").has_value()
        ? engine::runtime::TaskRequest{}      // ← 空，不调用 build_request_from_cli
        : build_request_from_cli(argc, argv);
```

因此 sequence 模式下 `--request-option` 永远不被读取，随后被 `require_known_args()` 拒绝。

**正确做法**：逐请求选项写进 `requests.json` 的每个元素里。JSON 元素由 `build_request_from_json()` 解析，支持：

- 顶层 `language`（等价于 `--request-option language=...`）
- `options: { ... }` 对象（等价于任意 `--request-option`）

```jsonc
{
  "requests": [
    { "id": "c0000", "audio": "chunk_0000.wav",
      "language": "zh",
      "options": { "audio_chunk_mode": "none" } }
  ]
}
```

`--session-option` **不受影响**（`main.cpp:837` 在 batch 分支内被读取），`sense_asr.vad_model_path` 照常可用。

> **对 P1-9 的影响**：`engine/mod.rs` 生成 `requests.json` 时必须把 `language` 与 `audio_chunk_mode` 写进每个请求对象，而不是拼到命令行。请在实现时直接照此处理。

### 4.2 P0-5：sequence 三项确认 ✅

**① 音频字段名 = `audio`**（`build_request_from_json`，`app/cli/request.cpp:134`）。
相对路径以 `requests.json` 所在目录为 base（`resolve_case_path(base_dir, ...)`）。

**② `--text-out` 命名规则**：`<stem>_<safe_id><ext>`（`app/cli/main.cpp:909`）。
`safe_output_name()` 把非 `[A-Za-z0-9_-]` 的字符替换为 `_`。实测 `--text-out seg.txt` + id `c0000` → `seg_c0000.txt`。

**③ `metrics[<id>]` 逐条实时输出** ✅ —— 不需要退化为不确定进度。
实测 stdout 按请求完成顺序逐条打印，`text_out[<id>]=<path>` 也逐条打印：

```
text_output=开放时间早上9点至下午5点。
metrics[c0000].wall_ms=113.668
metrics[c0000].rtf=0.0228433
metrics[c0000].x_realtime=43.7766
metrics[c0000].audio_duration_ms=4976
text_out[c0000]=/tmp/.../seg_c0000.txt
text_output=开放时间早上9点至下午5点。
metrics[c0001].wall_ms=129.839
...
```

> 实现建议：`engine/mod.rs` 直接解析 `text_out[<id>]=<path>` 行拿输出路径，比自行拼 `<stem>_<id>.txt` 更稳（避免 safe_output_name 转义不一致）。同时用 `metrics[<id>].` 计数推进进度。

### 4.3 VAD（P0-4）✅

`--vad-chunks-out` 输出格式与 spec §2 一致，**单位是采样点**：

```json
[{"index":0,"start_sample":7296,"end_sample":86912}]
```

`--vad-chunk-padding-seconds` 语义实测为**替换而非外扩**：Silero 报的语音区间是 `11296..82912`，`padding=0.25` 后 chunk 为 `7296..86912`（两侧各 ±0.25s × 16000 = 4000 采样点）。✅

参数对比（112.32s 音频，20 段语音）：`max=15s` 下 20 段 → 20 chunks，每段约 5.0s（受 padding 影响）。切段长度由 `merge-gap` 与 `padding` 共同决定，Phase 1 可作为常量调优。

> ⚠️ **`padding` 疑似影响识别质量，不只是对齐**：同一句话（5.6s 样例）直接送入 audio.cpp 稳定识别为「开**饭**时间」（3/3），而经 VAD 加 `padding=0.25` 切段后识别为「开**放**时间」（与 FunASR 一致）。切段改变了模型的可见上下文。**P0-9 未覆盖的调参项**，建议在 P1-23 用真实素材一并验证 padding 取值。

**全静音**：10s 静音输入 → `vad_chunks_out` 输出 `[]`，**退出码 0**。✅ 与 spec「空结果直接写空字幕并完成」一致。

VAD 性能：112s 音频 **0.61–0.65s**，峰值 RSS **~50 MB**。

### 4.4 完整流水线（P0-6）✅

PoC 脚本 `poc_pipeline.py`（VAD → 按采样点切 WAV → `requests.json` → `--request-sequence` → 逐段读 `seg_<id>.txt`）对 112.32s 与 561.6s 两段音频均跑通，**100/100 段文本全部取回**。

### 4.5 后端与失败回退

`--backend metal` 正常工作（M4）。`--backend cpu` 亦可用（VAD 即以 cpu 全程跑通）。
spec §5.3 的「metal 失败自动 cpu 重试一次」策略未触发过，无法实测；保持原设计。

---

## 5. 性能数据（P0-7）

全部为 `/usr/bin/time -l` 实测，同一台 M4。

### 112.32 s 音频（20 chunks）

| 阶段 | wall time | 峰值 RSS |
|------|-----------|----------|
| VAD | 0.61 – 0.65 s | ~50 MB |
| ASR sequence（20 段，一次会话） | 1.13 – 1.67 s | 560 MB |
| **全流水线（VAD+切片+ASR）** | **1.77 – 2.32 s** | 560 MB |
| 对比：单段单独调用一次 | 0.56 s | 560 MB |

### 561.6 s (9.36 min) 音频（100 chunks）

| 方案 | wall time | 峰值 RSS |
|------|-----------|----------|
| **audio.cpp 全流水线** | **6.78 s** | **560 MB** |
| FunASR 现状（首次） | 16.78 s | 2.86 GB |
| FunASR 现状（二次） | 12.98 s | 3.00 GB |

- audio.cpp RTF ≈ 0.0121（**约 83x 实时**）；FunASR ≈ 0.023。
- **内存降低约 5.3x，速度提升约 1.9x。**

### 冷启动成本

- 首次运行 metal 需编译 shader（`ggml_metal_library_init: loaded in 7.482 sec`），后续运行命中缓存，单次调用总耗时约 0.56 s。
- 含义：模型加载 + 后端初始化约占单次调用的绝大部分；**这正是 `--request-sequence` 的价值所在**——20 段一次会话 1.13–1.67 s，若逐段各起一次进程则需约 11 s。
- 批量小文件场景仍值得按 spec §5.8 记录加载占比，以决定是否需要 P4-6（常驻 server）。**本 PoC 未覆盖该场景**。

---

## 6. ⚠️ 未验证项（本机条件不足）

以下项目**没有**完成，不应视为通过：

1. **中文准确率对照（P0-7 核心）未充分验证。** 本机只能构造合成音频（单句音频重复拼接），不构成有意义的评测集。spec 要求的「≥1h 长音频、多人对话、中英混合」3 个样例需真实素材。
   - 仅有的观察：在 **5.6s 原始样例**上，FunASR 输出「**开放**时间」，audio.cpp 稳定输出「**开饭**时间」（3/3 复现）——**FunASR 更准**。
   - 但同一句话**经 VAD 切段并加 0.25s padding 后**，audio.cpp 输出「开放时间」，与 FunASR 一致。即该错误对**切段边界敏感**。
   - 含义：不能据此判定准确率劣化（n=1 句），但也不能声称持平。padding 取值可能是质量相关的调参项，而非纯粹的对齐参数。
   - **这是 Go 结论中唯一的实质性缺口**，建议在 P1-23 用真实素材补测后再决定是否发版。
2. **时间轴观感**：PoC 未实现后处理与字幕写出（P0-6 只到「拿回分段文本」），未生成 SRT，无法主观对比轴观感。
3. **长音频（≥2h）**：未测。切片数量与临时文件体积增长见 spec §7 风险 9，仍待验证。
4. **Intel / universal 构建**：未测（本机 arm64）。x64 asset 的 SHA256 在 `fetch-audiocpp.sh` 中留空待填。
5. **代码签名 / 公证 / 干净机**：属于 P2-4 / P2-5。

---

## 7. 对 spec 的修正项

| # | spec 原文 | 实测 | 处理 |
|---|-----------|------|------|
| 1 | §5.3 用 `--request-option language=zh --request-option audio_chunk_mode=none` 配合 `--request-sequence` | **会导致 CLI 报 `unknown options`** | 改为写进 `requests.json` 每个请求对象。见 §4.1 |
| 2 | §5.6「模型仅硬编码 5 种语言…es/de/fr/pt 回落 auto」 | `is_language_tag()`（`sense_asr/session.cpp:136`）接受 **23 种**语言标签，不会报错；但 `language_query_token()`（同文件:250）只把 `{auto,zh,en,yue,ja,ko,nospeech}` 映射为真实 token，其余**返回 0（即 auto）** | **结论（回落 auto）正确，机制描述需修正**。`models.json` 的 `languages` 应填完整 23 项（决定 CLI 是否报错），而映射到 `auto` 的判定单独实现 |
| 3 | §5.1 Silero VAD 需「固定 commit」下载 | v0.9.1 tag 内直接可取 | 按 tag 固定即可 |
| 4 | §5.3 `--text-out` 由「命名规则 `<stem>_<id><ext>`」推断 | 正确，且 CLI 会打印 `text_out[<id>]=<path>` | 建议直接解析该行而非自行拼接 |

### P0-9：三个设置项确认全部失效 ✅

| 设置 | 结论 |
|------|------|
| `enableNoiseReduction` | **死代码**。除 `useStore.ts` 外全仓无引用；`audio_extractor.py` 硬编码 `noise_reduction=True` 且带 `afftdn` 滤镜，调用方从不传参 |
| `enableVolumeNormalization` | **死代码**。同上，硬编码 `normalize_volume=True` + `loudnorm` 滤镜 |
| `enableBilingual` | 已死。UI 为置灰的 "Coming Soon" toggle，引擎侧无任何实现 |

**处理**：P1-21 的 `migrate` 丢弃 `enableNoiseReduction`、`enableVolumeNormalization`、`enableBilingual`、`bilingualTargetLang` 四个字段。
⚠️ 注意：`afftdn`+`loudnorm` 目前是**始终开启**的，转到 audio.cpp 后 Rust 侧 `ffmpeg` 抽音频是否保留该滤镜链会**改变识别输入**，建议保留以维持行为一致。

### P0-8：`zh-tw` 行为确认 ✅

| 引擎 | 输入 `zh-tw` 的实测输出 |
|------|------------------------|
| FunASR（现状） | `开放时间早上9点至下午5点。`（**简体**） |
| audio.cpp | `开饭时间早上9点至下午5点。`（**简体**） |

两者都输出简体，**现状本就不做繁转**。→ **无行为回归**；P4-5（OpenCC 繁转）纯属增强，非必需。

---

## 8. 附带发现：现有代码 bug（与本重构无关）

`core-engine/main.py:390`：

```python
if os.path.exists(audio_path) and audio_path.startswith("/tmp/"):
    os.remove(audio_path)
```

对**音频输入**，`audio_path` 初始值就是 `args.video_path`（用户自己的文件，见 `main.py:253`）。因此当用户转写的音频文件位于 `/tmp/` 下时，**程序会在成功后删除用户的原始文件**。（本次 PoC 的测试 WAV 就是这样丢失的。）

与 audio.cpp 重构无关，建议独立修复——例如只删除 `cleanup_paths` 中记录的文件，不要用路径前缀判断。

---

## 9. Go / No-Go 判定

spec §Phase 0 的 Go 条件逐条对照：

| 条件 | 判定 |
|------|------|
| 中文准确率主观不劣 | ⚠️ **未充分验证**（§6.1）。单样例上 FunASR 略优，且该差异对 VAD 切段边界敏感。**唯一缺口** |
| 时间轴可用 | ✅ VAD 给出采样点级切段，精度高于现状的纯估算 |
| 速度与内存不劣于现状 | ✅ **明确更优**（1.9x 快、5.3x 省内存） |
| P0-5 有可行方案 | ✅ `--request-sequence` 完全可行（含逐条进度） |

**结论：Go**，但附带两个条件：

1. 在 P1-23 用**真实评测素材**补做中文准确率对照；若明显劣于 FunASR，需在 P1-9 前评估 `qwen3_asr`（P4-1）作为默认模型。
2. P1-9 实现必须按 §4.1 处理逐请求选项，否则首次运行即失败。

---

## 10. 复现步骤

```bash
# 1. 取二进制与 VAD 模型（已落地为正式脚本）
./scripts/build/fetch-audiocpp.sh

# 2. 取 SenseVoice GGUF（需能访问 HF；本机经 Clash 7897 代理）
HTTPS_PROXY=http://127.0.0.1:7897 \
curl -L -o sensevoice-small-q8-audiocpp-v1.gguf \
  "https://huggingface.co/FunAudioLLM/SenseVoiceSmall-GGUF-audiocpp/resolve/5c3fcfe748a8714216bc135476d5863084fddb72/sensevoice-small-q8-audiocpp-v1.gguf"
# 校验：254211200 字节 / 4dedf169…6ff0ec

# 3. VAD
audiocpp_cli --task vad --family silero_vad \
  --model src-tauri/resources/models/silero_vad --backend cpu \
  --audio input.wav --vad-chunks-out vad_chunks.json \
  --vad-chunk-max-seconds 15 --vad-chunk-merge-gap-seconds 0.5 --vad-chunk-padding-seconds 0.25

# 4. 切片后按 §4.1 生成 requests.json，再：
audiocpp_cli --task asr --family sense_asr \
  --model sensevoice-small-q8-audiocpp-v1.gguf --backend metal \
  --request-sequence requests.json --text-out out/seg.txt \
  --session-option sense_asr.vad_model_path=src-tauri/resources/models/silero_vad \
  --threads 8 --metrics
```

PoC 脚本与原始数据在 `poc/`（手工产物，未入库）：`poc_pipeline.py`、`bench.sh`、`out/`。若要留档，建议移入 `scripts/poc/`。
