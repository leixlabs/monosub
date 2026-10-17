# Phase T0 PoC 报告：llama.cpp 本地翻译可行性验证

> 状态：**Go（附条件）**
> 执行日期：2026-10-16
> 测试机：Apple M4 / macOS (Darwin 25.6.0)，arm64
> 上游版本：llama.cpp **b11696**（rolling build；v0.7.0 是 pointer-only release，无二进制产物）
> 模型：`HY-MT1.5-1.8B-Q4_K_M.gguf`（size 1,133,080,512 B，sha256 `4383ac0c…77c7`）
> 结论回填：[translation-design.md](./translation-design.md)

---

## 1. 结论摘要

**Go。** 预编译 `llama-server` 可以随应用打包，模型按需下载，翻译在识别完成后单独跑一个
子进程，**不需要 Python，也不与 audio.cpp 共用 ggml**。

同时推翻了 spec 的一处关键取舍：**结构化输出（JSON schema / grammar）在本模型上不可用**，
必须改用「编号行 + 严格校验」（见 §5、§9）。这正是 spec §4 原本的首选方案。

| 维度 | 实测 | 结论 |
|------|------|------|
| 打包增量 | `llama-server` + 其 `lib*.dylib` 子集 **约 68 MB**（tarball 12,222,784 B） | ✅ 可接受 |
| 冷启动 | **0.5 – 5 s**（模型只加载一次，之后每批只是 HTTP 请求） | ✅ |
| 内存 | 峰值 RSS **~1.4 GB** | ✅ |
| Metal | ✅ 生效（`libggml-metal` 载入，AGXMetal 驱动打开） | ✅ |
| 速度 | prompt ~600–800 tok/s，decode **~42–53 tok/s**；41 条字幕整份 **11–14 s** | ✅ 慢于 ASR，但有界且可预测 |
| 输出格式 | ❌ json_schema / GBNF 失败；✅ 编号行 41/41、20/20、10/10 全部条数精确 | ⚠️ 见 §5 |
| 语言标签（T0-5） | ⚠️ **本机未做 SenseVoice 实跑确认** | ⚠️ 见 §6，另有工程兜底 |

---

## 2. 产物与校验值（T0-1）

| 产物 | 字节数 | SHA256 |
|------|--------|--------|
| `llama-b11696-bin-macos-arm64.tar.gz` | 12,222,784 | `6aac007fbe153efefe06250d2cf05d0487583aa0706379082ebef2b05579f649` |
| ↳ 解出的所需子集（`llama-server` + `lib*.dylib`，落盘后） | 约 68 MB | — |
| ↳ 归档内 `llama-b11696/LICENSE`（MIT） | — | — |
| `HY-MT1.5-1.8B-Q4_K_M.gguf` | 1,133,080,512 | `4383ac0c3c8e476de98ff979c2a3f069f8c4fb385e7860cf2d28da896cc477c7` |
| `llama-b11696-bin-macos-x64.tar.gz` | 未取 | **留空**（与 `fetch-audiocpp.sh` 对 x64 asset 的处理一致） |

已填入 `scripts/build/fetch-llamacpp.sh` 与 `src-tauri/resources/models.json`（均实测，非估算）。

### 2.1 归档内容与「需要的子集」

tarball 解出一个平铺目录 `llama-b11696/`，约 **36** 个成员。需要的只有：

- `llama-server`（唯一要打包的可执行文件）
- 全部 `lib*.dylib`：`libllama*.dylib`、`libggml*.dylib`、`libmtmd*.dylib`、
  `libllama-server-impl.dylib`、`libllama-common*.dylib`

**不打包**的成员（`llama-cli`、`llama-tokenize` 等）直接留在临时目录里，进不了 DMG。

### 2.2 ⚠️ 布局是硬约束：dylib 必须与可执行文件同目录

`llama-server` 的 `LC_RPATH` **只有 `@loader_path`**。因此：

```
src-tauri/resources/bin/llama-server-<target-triple>   ← 可执行文件
src-tauri/resources/bin/libllama*.dylib                ← 平铺，NOT bin/lib/
src-tauri/resources/bin/libggml*.dylib
src-tauri/resources/bin/libmtmd*.dylib
...
```

- 平铺装好之后 **无需 `DYLD_LIBRARY_PATH`** 即可启动（本机实跑确认）。
- **不要**把 llama 的 dylib 塞进 `bin/lib/`：那里是 ffmpeg 的 dylib，靠 ffmpeg 自己的
  rpath 解析，两套布局不能合并。
- 副作用（好的）：`sign-app.sh` 原有的 `"$RESOURCE_BIN_DIR"/*` 循环已经覆盖这些文件，
  **不需要第二个签名循环**——加了反而会把同一批文件重签两次。

---

## 3. 模型候选（T0-2）

### 3.1 选定：HY-MT1.5-1.8B-Q4_K_M

| 项 | 值 |
|----|----|
| 文件 | `HY-MT1.5-1.8B-Q4_K_M.gguf` |
| 主源（实测 206 partial content） | `https://www.modelscope.cn/models/Tencent-Hunyuan/HY-MT1.5-1.8B-GGUF/resolve/master/HY-MT1.5-1.8B-Q4_K_M.gguf` |
| 镜像 1 | `https://huggingface.co/tencent/HY-MT1.5-1.8B-GGUF/resolve/265b2e615a7dc9b06c435dc878829ad99a512ba2/HY-MT1.5-1.8B-Q4_K_M.gguf` |
| 镜像 2 | `https://hf-mirror.com/tencent/HY-MT1.5-1.8B-GGUF/resolve/265b2e615a7dc9b06c435dc878829ad99a512ba2/HY-MT1.5-1.8B-Q4_K_M.gguf` |
| size | 1,133,080,512 |
| SHA256 | `4383ac0c3c8e476de98ff979c2a3f069f8c4fb385e7860cf2d28da896cc477c7` |
| 许可 | **Tencent HY Community License Agreement**（release date 2025-12-30） |
| 许可文本 | `https://huggingface.co/tencent/HY-MT1.5-1.8B-GGUF/blob/main/License.txt` |
| attribution | `HY-MT1.5-1.8B (Tencent Hunyuan)` |
| 目录 source | `other`（`engine/catalog.rs` 的 `Source` 是封闭枚举，不为它加新变体） |

SHA256 在 **HF 的 LFS pointer 与 ModelScope 仓库列表里一致**——两个来源互相印证，不是单一来源。

GGUF KV 实测：`general.architecture = hunyuan-dense`，`block_count 32`，
`embedding_length 2048`，`attention.head_count 16 / head_count_kv 4`，
`context_length 262144`，`tokenizer.ggml.model = gpt2`，chat template 存在。
KV 里的推荐采样是 `temp 0.7 / top_k 20 / top_p 0.8`，**模型卡写的是 0.6，采用 0.6**
（模型卡权威；seed 无关，因为温度已固定）。

### 3.2 未实测的备选（登记，不选）

| 候选 | 状态 | 说明 |
|------|------|------|
| **HY-MT1.5-7B** | 未下载、未实测 | 更高质量方向；体积与耗时都会显著上升，留待质量回归（T6-2）有结论后再评估 |
| **AngelSlim 的 HY-MT 1.25-bit / 2-bit GGUF** | 未下载、未实测 | 极限量化方向；1.25-bit 这类档位的输出质量**必须先实测**再谈收录 |

两者都**没有**取回文件，因此没有 URL/size/SHA256 可写；不臆造。

---

## 4. 调用形态：llama-cli vs llama-server（T0-3）

**决定：`llama-server`。**

| 方案 | 冷启动 | 整份 41 条耗时 | 内存 | 结论 |
|------|--------|----------------|------|------|
| `llama-server` 子进程（本机实测） | **0.5 – 5 s，一次** | **11 – 14 s** | RSS ~1.4 GB | ✅ 采用 |
| `llama-cli` 一次一调用 | 每次调用都要重新加载 1.06 GB 模型 | 未实测 | 未实测 | ❌ 退出 |

`llama-cli` 一侧**没有做同口径的 head-to-head 基准**（见 §8.2）——它的落选依据是结构性
的，不是测量出来的：模型加载是冷启动的绝大部分，而整份字幕至少要发 3 批（41 条 ÷ 20），
所以 cli 至少要付 3 次加载。按实测量级粗算，`3 × (0.5–5 s 加载) + 解码 ≈ 16–30 s`，
比 server 的 11–14 s 差，且随批次线性放大。这是**外推**，不是实测值。

### 实测调用形状（与 `translate/server.rs::server_args` 逐字一致）

```bash
llama-server --model HY-MT1.5-1.8B-Q4_K_M.gguf --host 127.0.0.1 --port <port> \
  -c 4096 -ngl 99 --no-webui -np 1 -lv 1
```

- 每个翻译阶段起一次，监听 `127.0.0.1` 随机端口，翻完即停。
- **就绪探测**：`GET http://127.0.0.1:<port>/health` → `{"status":"ok"}`。
- `-ngl 99` 全量 offload，Metal 确认生效。
- 生成：`POST /v1/chat/completions`，body 形如
  `{"messages":[{"role":"user","content":<prompt>}], "temperature":0.7, "top_k":20, "top_p":0.6, "repeat_penalty":1.05, "n_predict":<n>, "cache_prompt":true}`；
  取 `.choices[0].message.content`。
- **多文件队列能否复用同一个 server 仍未做**（仍作为后续优化，T6-5）。

---

## 5. 批次与输出约束（T0-4）

### 5.1 结构化输出：实测失败，已放弃

| 约束方式 | 实测结果 | 判定 |
|----------|----------|------|
| `response_format: json_schema`（带 `minItems`/`maxItems`） | **回显原文，未翻译** | ❌ |
| `response_format: json_schema`（array of objects） | **返回空数组** | ❌ |
| 原生 **GBNF grammar** | **返回 `[]`** | ❌ |
| **编号行**（numbered lines） | 41/41、20/20、10/10 条数精确，无合并、无漏条、无多余解释 | ✅ **采用** |

结论：**不要用 `response_format`，也不要用 grammar。** 改用「要求按编号逐行返回」，
再在 Rust 侧解析与校验。这是 spec §4 那次取舍的反转（见 §9）。

### 5.2 批次大小

| 批次 | 实测条数比 | 结论 |
|------|-----------|------|
| 10 | 10/10 | 可用 |
| **20** | 20/20 | ✅ **默认**（`translate/mod.rs::BATCH_SIZE = 20`） |
| 40 | 40/40 | 可用，但单批失败时的爆炸半径翻倍 |

三者都精确，所以选 20：上下文够用，同时一个坏批次最多损失 20 条 + 一次重试。

### 5.3 解析规则（实测覆盖全部观察到的响应行）

```
^\s*(\d+)\s*[.)、:：]\s*(.*)$
```

- 分隔符 `1.` `1)` `1、` `1:` `1：` 等价接受（`translate/parse.rs` 手写实现，
  仓库没有 regex 依赖）。
- 响应**有时会把译文折到下一行**：没有前导编号的续行归属上一条编号。
- 空行结束当前 cue，因此尾部的「希望有帮助」之类不会被塞进最后一条译文。
- `en → zh` 与 `zh → en` 用同一套编号契约，实测 `en → zh` 10/10。

### 5.4 提示词（实测后与模型卡不同：不能把卡片模板叠在批量契约上）

**结论先行**：模型卡给的是**单条**模板，把它放在「批量编号契约」前面，会得到两个互相竞争的指令，
1.8B 模型的选择是**放弃编号**。实测 zh→en（40 次请求）：

| 批次 | 卡片行 + 批量契约 | 只留批量契约 |
|------|------------------|-------------|
| 1 | 0 条编号 | 全部正确 |
| **20（默认）** | **0 条编号，且返回的是原文（未翻译）** | **全部正确** |

返回原文这一点最危险：内容非空，只看「有没有输出」的实现会把它当成翻译成功。

因此实现里 **不再前置卡片模板**，批量契约自身带三件必备要素（`translate/prompt.rs`）：

1. 显式条数；
2. **条数句必须自成一段**——并进指令句里会让模型停止编号（实测）；
3. **两行示例**（`1. First.` / `2. Second.`）。

第 3 条不是装饰：只给条数、不给示例时，2–4 条的批次**每次都失败**、14 条偶发失败；
加上示例后，2–20 条共 **27/27 全部精确**。示例用目标语言书写，并已验证不会污染
非英语目标（ja/fr/ko/de 均正确）。

实测措辞（zh→xx 与 xx→xx 同构，只换语言）：

```
将以下 {count} 条带编号的字幕逐条翻译为{target_language}，逐条对应，保持编号（1. 2. 3. …），每行一条，不要合并、不要漏条、不要额外解释：

输出必须严格为 {count} 条。例如 2 条的输入会得到 2 行输出：
1. First.
2. Second.

1. <text>
2. <text>
```

模板归属跟着 `models.json` 的 `prompt_template` 走（本条目为 `"hy_mt"`），
Rust 里没有任何一处写死 HY-MT。非中文源用同一契约的英文措辞。

#### 单条（重试阶梯最后一级）需要单独处理

两个额外实测结论，都影响实现（`prompt::single_cue_prompt`）：

- **单条用编号形式不可靠**：模型会返回无编号的一行，或把指令本身回显出来。
  因此单条改走卡片式的**单段**模板，答案**按原文整体采纳**（`parse::parse_single_translation`）。
- **中文源的单条指令里，目标语言必须用中文名**：`将以下文本翻译为English` 会让模型
  **输出中文**（实测 3/3 探测都是中文）——它把 English 当成正文而非目标语言；
  改成 `翻译为英语` 后 30/30 全部正确。只有单条提示词对这个敏感，批量契约不受影响。

---

## 6. 语言标签（T0-5）：⚠️ 未在本机实跑确认

**T0-5 要问的问题是**：SenseVoice 输出里的 `<|zh|>` 之类的语言标签，是否稳定出现在
`--text-out` 中？

**本机没有拿到答案。** SenseVoice 模型在这个网络策略下**下载不下来**，因此没能做一次
真实的 SenseVoice 解码来观察原始文本。这一项**不是「已验证」，是「未验证」**。

不过工程决定是安全的，理由可核对：

1. `subtitle/postprocess.rs::clean_text` 一直在删 `<|…|>` 标签——**说明这条路径上确实会
   出现这种标签**；否则那段清理逻辑没有存在理由。
2. T4-1 的实现是**在删标签之前先取**（`extract_language_tag`，在
   `engine/mod.rs` 构造 `RawChunk` 时调用），因此「标签存在则拿得到」。
3. **标签不存在时降级为 `None`**，而不是报错。`engine/direct.rs::turns_into_chunks`
   的直出（moss）路径实测确认返回 `None`——该模型本来就不吐标签。
4. `None` 在下游的含义是「不确定 → 照常翻译」（spec §5），最坏代价只是一次多余的翻译，
   **不会静默丢掉用户要的翻译**。

因此 T4-1 记为 **done-with-caveat**：代码与单测齐备（
`subtitle::postprocess::tests::language_tag_is_extracted_before_cleaning`、
`dominant_language_uses_majority_then_first_seen`、
`engine::direct::tests::direct_path_reports_no_language`），
但「真实 SenseVoice 输出里标签是否 100% 存在」**仍缺一次实跑证据**，与 §8.1 同一缺口。

标签白名单是 `{zh, en, yue, ja, ko}`：同一对定界符还承载情绪（`<|NEUTRAL|>`）、
事件（`<|Speech|>`）与 ITN 标记，那些都**不是**语言，判错会跳过一次真正需要的翻译。

---

## 7. 性能数据

| 项 | 实测 |
|----|------|
| 冷启动（server 起来到 `/health` = ok） | 0.5 – 5 s |
| 峰值 RSS | ~1.4 GB |
| prompt 吞吐 | ~600 – 800 tok/s |
| decode 吞吐 | ~42 – 53 tok/s |
| 整份 41 条字幕（zh → en） | **11 – 14 s** |

外推：**约 1 分钟 / 200 条**。比 ASR 明显慢，但有界、可预测，且进度按批推进
（`translate/mod.rs` 每批回调 `on_progress(done, total)`）。

### 模型载入占比

模型只加载一次，所以**server 形态下加载成本被摊薄**；这正是 §4 选 server 的核心。
单批的净耗时主要是 decode，与批大小成正比。

---

## 8. ⚠️ 未验证项

以下项目**没有**完成，不应视为通过：

1. **T0-5 的 live SenseVoice 标签观察**（§6）——模型下载不可用。这是本 PoC 最主要的缺口。
2. **`llama-cli` 同口径基准**——只在结构上论证其劣势，没有实测数据。
3. **≥30 min 真实素材的整份质量回归（T6-2）**。本次用的是 41 条字幕的样本，
   **不是** spec 设想的 ≥30 min 长素材；条数与结构经过核对，**但没有做人工抽读判分**。
   「翻译得好不好」这件事本 PoC **没有结论**。
4. **Intel / x64 asset**——本机 arm64；x64 的 SHA256 在 `fetch-llamacpp.sh` 中留空待填。
5. **代码签名 / 公证 / 干净机启动**——`sign-app.sh` 的既有循环理论上覆盖平铺 dylib，
   但没有在签名后的 DMG 里实跑过 server。

---

## 9. 对 spec 的修正项

| # | spec 原文 | 实测 | 处理 |
|---|-----------|------|------|
| 1 | §3.2 表：`llama-server` 的优点含「**可用 JSON schema / grammar 约束输出**」 | 本模型上两种约束都失败（§5.1） | 该优点**删除**，替换为「HTTP 接口稳定、模型只加载一次」 |
| 2 | §4「**输出约束：优先用 server 的 JSON schema / grammar 约束格式**，减少解析失败」 | 实测：json_schema 回显原文或空数组，GBNF 返回 `[]` | **反转**为「编号行契约 + 解析校验 + 批次降级」；记录 parse 规则 |
| 3 | §4「批量 + 编号」未定批次大小 | 10/20/40 均精确 | 定 **20**（`BATCH_SIZE`），单测固定该常量 |
| 4 | §6「候选模型方向（实现时再实测定夺）」 | 选定 HY-MT1.5-1.8B-Q4_K_M，ModelScope 主源 + 两个镜像 | 写入 `models.json`（`hy_mt_1_8b_q4_k_m`） |
| 5 | §6 未提 `source` 字段怎么填 | `Source` 是封闭枚举，且 HY-MT 不属于任何已有家族 | 用 **`source: "other"`**，不新增变体 |
| 6 | §9 表 `commands/transcribe.rs` 行：「`total_steps` 改为按请求计算（4 或 5）」 | 实现为 `Steps::new(translating)` 携带 per-run `total` | 描述修正为 per-run 字段（原意一致） |
| 7 | §11「包体积…需实测」 | **+约 68 MB** | 风险从「未知」改为「已知量」 |
| 8 | §11「输出格式漂移」 | 编号行下 41/20/10 全部精确；漂移只出现在结构化输出上 | 风险收敛（但保留 §4 的校验 + 降级） |
| 9 | §3.2「多文件队列里能否复用同一个 server 留作后续优化」 | 未做 | **保持不变**（T6-5） |

---

## 10. Go / No-Go 判定

| 条件 | 判定 |
|------|------|
| 可离线、无 Python、按需下载 | ✅ 单二进制 + 3 个源；模型永不打包 |
| 打包体积可控 | ✅ 约 68 MB（+ 按需 1.06 GB 模型） |
| Metal 生效、内存可控 | ✅ Metal 确认，RSS ~1.4 GB |
| 速度可接受 | ✅ 有界、可预测、有批次进度；约 1 分钟 / 200 条 |
| 输出格式可控 | ✅ **但方案已换**：编号行 + 校验 + 降级（不是 spec 原定的 grammar） |
| 与 audio.cpp 共存 | ✅ 分进程，各自带 ggml，不链接进同一进程 |

**结论：Go**，附带两个条件：

1. 补一次**真实 SenseVoice** 的标签观察，关闭 T0-5 缺口（§6）。
2. 在真实 ≥30 min 素材上做**质量回归 + 人工抽读**（T6-2）；在此之前不宣称翻译质量达标。

---

## 11. 复现步骤

```bash
# 1. 取 llama-server + dylib（已落地为正式脚本，幂等）
./scripts/build/fetch-llamacpp.sh
#    平铺安装到 src-tauri/resources/bin/llama-server-<triple> 与 bin/lib*.dylib
#    校验: arm64 tarball 12,222,784 B / 6aac007f…f649

# 2. 取翻译模型（本机经 Clash 7897 代理；ModelScope 直连亦可）
HTTPS_PROXY=http://127.0.0.1:7897 \
curl -L -o HY-MT1.5-1.8B-Q4_K_M.gguf \
  "https://www.modelscope.cn/models/Tencent-Hunyuan/HY-MT1.5-1.8B-GGUF/resolve/master/HY-MT1.5-1.8B-Q4_K_M.gguf"
#    校验: 1,133,080,512 B / 4383ac0c…77c7

# 3. 起 server（参数与 translate/server.rs::server_args 一致）
llama-server --model HY-MT1.5-1.8B-Q4_K_M.gguf --host 127.0.0.1 --port 8080 \
  -c 4096 -ngl 99 --no-webui -np 1 -lv 1

# 4. 就绪
curl -s http://127.0.0.1:8080/health          # {"status":"ok"}

# 5. 一次批量请求（批次 20；编号契约见 §5.4）
curl -s http://127.0.0.1:8080/v1/chat/completions \
  -H 'Content-Type: application/json' \
  -d '{"messages":[{"role":"user","content":"将以下 20 条带编号的字幕逐条翻译为English (en)，逐条对应，保持编号（1. 2. 3. …），每行一条，不要合并、不要漏条、不要额外解释：\n\n1. …"}],
       "temperature":0.7,"top_k":20,"top_p":0.6,"repeat_penalty":1.05,"n_predict":1024,"cache_prompt":true}'
#    取 .choices[0].message.content；按 §5.3 的规则解析

# 6. 应用内等价物
cargo test --manifest-path src-tauri/Cargo.toml translate::   # 49 个翻译单测
#    缺二进制/模型时 `live_server_…` 打印 skip: 后通过
#    全量: cargo test --manifest-path src-tauri/Cargo.toml → 249 passed
```

PoC 的原始素材与脚本未入库（与 audio.cpp PoC 的处理一致）。
