# 命令行入口（`monosub-cli`）设计

> 状态：**已实现**。
>
> * ✅ **R1 已完成**（`AppPaths` 抽取，`engine/` 已与 `tauri` 解耦）——§11.1
> * ✅ **R2 已完成**（`transcribe_file` 与 `AppHandle` 解耦，`ProgressSink`）——§11.4
> * ✅ **R3 已完成**（四问答案与求解结果落盘，CLI 可读）——§11.5
> * ✅ **R5 已完成**（打包与签名脚本；`externalBin` 方案被实测否决）——§11.2
> * ✅ **R6 已完成**（Settings ▸ 系统 的配置文件行与 CLI 卡片）——§11.6
> * ✅ **R4 已完成**（`monosub-cli` 可运行，本机已实测真实转写）——§11.7
> * ✅ **R7 已完成**（文档改为已实现；`bin/monosub` 改为 CLI 包装；修掉 3 个实现 bug）——§11.8
>
> **全部 7 个阶段完成。**
>
> 相关文档：[refactor-audiocpp.md](./refactor-audiocpp.md)（总体重构）、
> [model-selection-design.md](./model-selection-design.md)（四问求解器）、
> [translation-design.md](./translation-design.md)（翻译阶段）。
>
> 本文所有「现状」判断均以仓库当前代码为准，逐条标注了文件与行号区间。

---

## 1. 目标与边界

### 1.1 要解决什么

App 发布后只有 GUI 一种使用方式。批量脚本、CI、给 Agent 调用、远程机器（ssh 进
去跑）都需要一个**不需要窗口**的入口。命令行必须与 GUI **跑同一条管线、用同一个模
型**——否则「GUI 里效果很好，命令行结果不一样」会成为一个查不出来的 bug。

### 1.2 用户定的三条边界

1. **不指定模型**。CLI 复用 GUI 里已经选好的方案（四问求解器的结果）。
2. **只控制** `--format` / `--language` / `--output-dir`（+ 翻译开关）。
3. **发布 + 本机 build 都要**（A+B）。

### 1.3 明确不做

- 不做 `--model` 逐个指定。手动 pin 是 GUI 的「高级」能力，CLI 只消费结果。
- 不做 URL 导入（B 站抓取）。那是网络 + 登录态，与「本地文件转字幕」是两件事。
- 不做队列/并发调度。一次调用处理一个文件；批量交给 shell。
- 不改 `core-engine/`（按用户决定原样保留）。

---

## 2. 现状：三个必须先解决的问题

### 2.1 问题一：模型方案存在 localStorage，Rust 读不到

决定模型的是四个问题，全部持久化在 WebView 的 localStorage：

```
src/store/useStore.ts:563  partialize: { storageMode, customStoragePath, settings, modelOverrides }
   └─ settings.languageProfile   ('mandarin' | 'english' | 'mixed' | 'multilingual')
      settings.memoryBudget      ('small' | 'medium' | 'large')
      settings.requireWordTimestamps  (bool)
      settings.defaultDiarization     (bool)
```

Rust 侧的 `<app_config_dir>/settings.json` 目前**只有一个字段**（实测）：

```json
{ "modelsDir": "/Users/lei/dev/projects/2601-monosub/models" }
```

CLI 是独立进程，**没有 WebView，读不到 localStorage**。不解决这一点，CLI 只能自带
一套默认值，于是「GUI 选 中英混合+显存小 得到的模型」与「CLI 用的模型」就不是同一
个——违反 §1.1 的核心要求。

**解法：见 §4（GUI 把解析结果写进磁盘，CLI 读它）。**

### 2.2 问题二：`&AppHandle` 贯穿整条管线

`AppHandle` 是 Tauri 运行时对象，只能在 Tauri 进程里构造。CLI 造不出来。当前有
**20 个函数的签名**吃它（实测 `grep -rn "fn .*AppHandle"`）：

| 文件 | 函数数 | 用途 |
|---|---|---|
| `engine/catalog.rs` | 7 | `catalog()` / `model_path()` / `model_dir()` / `is_installed()` / `list()` / `models_root()` |
| `engine/cli.rs` | 4 | `resolve_cli_path()` / `resolve_llamacpp_path()` / `resolve_vad_path()` / `resolve_catalog_path()` |
| `engine/aligned.rs` | 2 | 词级对齐路径 |
| `model_dir.rs` | 5 | `configured_root()` / `load_settings()` / `settings_path()` |
| `commands/transcribe.rs` | 8 | `start_transcription` / `transcribe_file` / `extract_wav` / emit |
| `commands/models.rs` | 9 | 下载/删除（CLI 不需要，但同文件里的 `MODEL_NOT_INSTALLED` 常量要） |

**关键发现（决定了工作量）**：`engine/` 与 `subtitle/`、`translate/` **从不 emit 事件**
（实测 grep：`Emitter`/`emit` 只出现在测试函数名里）。`app` 在这些模块里**只用于路径
解析**：

```rust
// engine/cli.rs:337-395  —— app 只被当布尔门
if app.is_some() {
    let exe_path = std::env::current_exe().ok();
    ...  // 完全没调用 app 的方法
}
```

所以抽象面比签名数量小得多：**只需要一个"路径从哪来"的来源对象**。

**改动面实测**：这五个模块里共 **62 处 `tauri` 引用**，分布极不均匀：

| 文件 | 引用数 | 性质 |
|---|---|---|
| `engine/cli.rs` | 31 | 全部是路径解析（`Option<&AppHandle>` 形参 + 探测分支） |
| `engine/aligned.rs` | 9 | 形参透传 |
| `engine/catalog.rs` | 9 | 形参透传 |
| `engine/mod.rs` | 5 | 形参透传 |
| `engine/vad.rs` | 3 | 形参透传 |
| 其余 5 个文件 | 各 1 | 单个形参 |

**一半以上集中在 `cli.rs` 一个文件**，而它正是要整个重写的那一段，所以实际改动是
"重写一个文件 + 机械替换其余形参"，不是 62 处散弹。

### 2.3 问题三：可执行文件名冲突

App bundle 里 GUI 主程序就叫 `monosub`：

```
src-tauri/target/aarch64-apple-darwin/release/bundle/macos/MonoSub.app/Contents/MacOS/monosub
```

`Cargo.toml` 的 `[package] name = "monosub"` 决定了默认 bin 名。CLI 若也叫 `monosub`
并放在同一目录会直接覆盖。**定名 `monosub-cli`**（用户确认）。

---

## 3. 架构：一个抽象层，两个前端

```
                    ┌─────────────────────────────┐
                    │  core（新增库 crate）        │
                    │  ├─ engine/    （移动过来）   │
                    │  ├─ subtitle/  （移动过来）   │
                    │  ├─ translate/ （移动过来）   │
                    │  ├─ model_dir.rs              │
                    │  └─ paths::AppPaths  ← 唯一的注入点
                    └─────────────────────────────┘
                          ▲                     ▲
              ┌───────────┘                     └──────────┐
     ┌────────┴─────────┐                    ┌─────────────┴──────┐
     │ monosub（GUI）    │                    │ monosub-cli        │
     │ AppHandle→AppPaths│                    │ env→AppPaths       │
     │ emit transcribe_* │                    │ stdout 进度        │
     └───────────────────┘                    └────────────────────┘
```

### 3.1 `AppPaths`：唯一注入点

```rust
// core/src/paths.rs
#[derive(Debug, Clone)]
pub struct AppPaths {
    /// `<...>/resources` —— models.json、bin/、models/silero_vad 的根
    pub resources: PathBuf,
    /// 模型实际存放根目录。**与 `config_dir` 无关**（见 §4.0）：它来自
    /// `MONOSUB_MODELS_DIR` → settings.json 的 `modelsDir` → `<data_dir>/models`。
    pub models_root: PathBuf,
    /// `settings.json` 所在目录。**与 `models_root` 无关**；配置文件只是
    /// 恰好存了一个指向模型目录的字符串。
    pub config_dir: PathBuf,
    /// `<...>/models` —— 仅默认值，`models_root` 已解析它
    pub data_dir: PathBuf,
}

impl AppPaths {
    /// GUI：从 Tauri 的 path resolver 得到，与今天完全一致。
    pub fn from_app(app: &tauri::AppHandle) -> Result<Self, PathError>;

    /// CLI / 测试：从可执行文件位置 + 环境变量推导，不依赖 Tauri 运行时。
    pub fn from_env() -> Result<Self, PathError>;
}
```

**`from_env()` 的解析规则**（与 GUI 的既有规则**逐条对齐**，不新增第二条真相）：

| 项 | 顺序 |
|---|---|
| `resources` | `MONOSUB_RESOURCES_DIR` → `<exe_dir>/resources` → `<exe_dir>/../Resources/resources`（macOS bundle）→ `<cwd>/src-tauri/resources`（dev） |
| `models_root` | `MONOSUB_MODELS_DIR` → `settings.json` 的 `modelsDir` → `<data_dir>/models` |
| `config_dir` | 配置的唯一来源：`MONOSUB_CONFIG_DIR` → macOS `~/Library/Application Support/dev.leix.monosub` / Win `%APPDATA%/dev.leix.monosub` / Linux `~/.config/dev.leix.monosub` |
| `data_dir` | 数据目录（**不是**模型目录的最终值，只是默认值的基点）：macOS `~/Library/Application Support/dev.leix.monosub` / Win `%APPDATA%/dev.leix.monosub` / Linux `~/.local/share/dev.leix.monosub` |
| `models_root` | **两个概念之一**（§4.0）：`MONOSUB_MODELS_DIR` → `<config_dir>/settings.json` 的 `modelsDir` → `<data_dir>/models` |

> `dev.leix.monosub` 来自 `tauri.conf.json` 的 `identifier`，实测确认。CLI 必须用
> **同一个** 目录，否则读不到 GUI 的 `settings.json`，整个 §4 失效。

**签名改法**（机械替换，不改变行为）：

```rust
// 之前
pub fn catalog(app: Option<&tauri::AppHandle>) -> Result<Vec<ModelSpec>, EngineError>
pub fn model_dir(app: &tauri::AppHandle, id: &str) -> Result<PathBuf, EngineError>
pub fn is_installed(app: &tauri::AppHandle, id: &str) -> bool

// 之后
pub fn catalog(paths: &AppPaths) -> Result<Vec<ModelSpec>, EngineError>
pub fn model_dir(paths: &AppPaths, id: &str) -> Result<PathBuf, EngineError>
pub fn is_installed(paths: &AppPaths, id: &str) -> bool
```

GUI 侧在命令边界构造一次 `AppPaths::from_app(&app)?`，往下传递。**`engine/` 内不再
出现 `tauri` 依赖** —— 这同时让 engine 可以在没有 Tauri 的环境里单测。

`engine/cli.rs` 的路径解析里 `app.is_some()` 这个布尔门被 `AppPaths` 直接取代：既然
拿到了已解析的 `resources` 根，就不需要再猜 exe 位置。

### 3.2 目录结构（新增 workspace）

```
src-tauri/
├── Cargo.toml          # [workspace]，成员见下
├── core/               # 新增：库 crate，name = "monosub-core"
│   ├── Cargo.toml      # 无 tauri 依赖（只有 paths::from_app 被 cfg 门控）
│   └── src/
│       ├── lib.rs
│       ├── paths.rs    # AppPaths
│       ├── engine/     # git mv 自 src-tauri/src/engine/
│       ├── subtitle/   # git mv
│       ├── translate/  # git mv
│       ├── model_dir.rs
│       └── pipeline.rs # 新增：一次转写的完整编排（见 §5）
├── src/                # GUI crate（现有 bin "monosub"）
│   ├── main.rs         # 保留：Tauri builder + 命令注册
│   ├── commands/       # 保留：薄适配层，emit 事件
│   └── bin/
│       └── monosub-cli.rs   # 新增：CLI 入口
```

> **为什么用 `core/` 目录而不是把现有 `src/` 变成 lib**：`src-tauri/src/bin/*` 会被
> Cargo 自动识别为额外 bin，但那个 bin 无法 `use crate::engine`（bin 是独立 crate
> root）。要共享代码必须有一个 lib target。放进独立 crate 还能从编译期保证
> `engine` 不再引用 `tauri`。

### 3.3 管线抽取：`core::pipeline`

现在 `commands/transcribe.rs::transcribe_file()`（约 470 行）把「编排」和「发事件」混在
一起。抽取后：

```rust
// core/src/pipeline.rs
pub struct TranscribeOptions {
    pub input: PathBuf,
    pub output_path: PathBuf,
    pub formats: Vec<Format>,
    pub language: String,
    pub plan: ModelPlan,
    pub translation: Option<TranslationOptions>,
}

/// 一次转写的完整编排：抽音频 → VAD/ASR/对齐/分离 → 后处理 → 写文件 →（可选）翻译。
/// 不 emit 任何东西：调用方通过 `on_progress` 消费。
pub async fn transcribe(
    paths: &AppPaths,
    options: &TranscribeOptions,
    on_progress: &ProgressFn<'_>,
) -> Result<TranscribeOutcome, PipelineError>;
```

- GUI：`commands/transcribe.rs::start_transcription` 构造参数 → 调 `pipeline::transcribe`
  → 在 `on_progress` 回调里 `app.emit("transcribe_progress", …)`。**事件契约一字不改**
  （`STEP_NAMES` 保持冻结，`Steps::new(translating)` 的 4/5 分母逻辑跟着走）。
- CLI：同一个函数，`on_progress` 打到 stderr（或 `--json` 时打 stdout）。

`extract_wav()`（`commands/transcribe.rs:955`）里的 `app` 也只用于
`get_ffmpeg_path(Some(app))`——改成用 `AppPaths::resources` 解析，随管线一起搬入 core。

---

## 4. 设置持久化：让 CLI 读到 GUI 的选择

### 4.0 两个互相独立的概念（先分清楚，不要绑定）

| 概念 | 是什么 | 默认位置 | 怎么改 |
|---|---|---|---|
| **配置路径** | 应用配置文件 `settings.json` 所在目录 | `<app_config_dir>`（macOS `~/Library/Application Support/dev.leix.monosub`） | `MONOSUB_CONFIG_DIR` |
| **模型路径** | 模型权重文件所在根目录 | `<app_data_dir>/models` | `MONOSUB_MODELS_DIR`、或配置里的 `modelsDir` 字段 |

**两者没有任何几何关系。** 实际情况已经如此（实测）：配置文件在
`~/Library/Application Support/dev.leix.monosub/settings.json`，而它里面的内容是

```json
{ "modelsDir": "/Users/lei/dev/projects/2601-monosub/models" }
```

即模型放在仓库目录里，与配置文件所在位置毫无关系。`model_dir.rs` 的实现也是分开的：
`settings_path()` 拼配置目录（行 42），`configured_root()` 读 `modelsDir` 字段（行 90）
——后者只把配置文件当成**一个存放字符串的地方**。

因此本节只讨论**配置路径**；模型路径的解析完全沿用现有的 `model_dir::configured_root()`
链（env → settings.json 的 modelsDir → data_dir/models），不改一行。

### 4.1 配置文件位置

现有 `<app_config_dir>/settings.json` 沿用。

- 它已经存在且在用（`modelsDir` 就在里面）。
- `app_config_dir()` 是平台正确的做法（macOS `~/Library/Application Support/<id>`、
  Windows `%APPDATA%/<id>`、Linux `~/.config/<id>`）；写死 `~/.monosub` 在 Windows 上
  不是惯例。
- 再开一个路径 = 第二份真相 = 以后必然漂移。

需要字面 `~/.monosub/settings.json` 时，`MONOSUB_CONFIG_DIR=~/.monosub` 即可（§3.1
的 `AppPaths::from_env()` 已支持），不需要第二套代码。

**给用户的提示**：`Settings → 系统` 已有一行“日志目录：<path>”；CLI 实施后在同一分组
再加一行“配置文件：<path>”和“命令行工具”卡片（§7.2），让这两个概念在界面上也是分开
展示的，不会被误认为同一个东西。

### 4.2 Schema（向后兼容扩展）

```jsonc
{
  // 已有字段，不动
  "modelsDir": "/path/to/models",

  // 新增：GUI 求解出的方案，CLI 直接消费
  "recognitionPlan": {
    "languageProfile": "mixed",        // mandarin | english | mixed | multilingual
    "memoryBudget": "medium",          // small | medium | large
    "requireWordTimestamps": false,
    "diarization": true,
    "language": "auto",
    "modelIds": ["moss_transcribe_diarize_q8_0"],
    "passthrough": true,
    "modelsVersion": 1                 // 递增即可让 CLI 判断是否过期
  },

  // 新增：翻译意图（CLI 默认关闭，见 §6）
  "translation": {
    "enabled": false,
    "targetLanguage": "en",
    "modelId": "hy_mt_1_8b_q4_k_m"
  }
}
```

- 全部 `#[serde(default, skip_serializing_if = "Option::is_none")]`：旧文件读出来就是
  `None`，CLI 退回「用默认四问求解」，不报错。
- `modelIds` 与 `passthrough` 是**缓存**，不是权威：CLI 每次仍会用
  `plan::solve()` 复核一遍，若与缓存不一致就**以求解结果为准并打一条 warn**（用户可能
  删了模型）。缓存的价值是让"用户手动 pin 过"这种情况也能带上。

### 4.3 写入时机

`commands/plan.rs::resolve_model_plan()`（已存在，行 252）在返回前把结果写盘：

```rust
// 只在把 overrides 应用到 winning plan 之后写
model_dir::save_recognition_plan(&app, &req, &plan)?;
```

`resolve_model_plan` 是设置页每次改动都会调的命令，所以"用户在 GUI 里选了什么"会
立刻落到磁盘。**不需要新的事件或定时器。**

`translation` 段则由 `Settings.tsx` 里翻译开关变化时写（新增一个轻量命令
`save_translation_settings`，或复用现有的 `set_models_dir` 风格的通用写入）。

### 4.4 读取时机

CLI 启动时 `load_settings()`：
- 有 `recognitionPlan` → 用它构造 `Requirements`，并用 `languageProfile` 等字段求解。
- 没有 → `Requirements::default()`，并在 stderr 提示"尚未在 App 里选择模型方案，使用默认值"。
- `--language` / `--format` / `--output-dir` 只覆盖**与它们对应的**字段，不动模型。

---

## 5. CLI 契约

### 5.1 用法

```bash
# 单文件（输出到源文件旁，与 GUI 的 storageMode='same' 一致）
monosub-cli video.mp4

# 指定输出目录 + 格式
monosub-cli video.mp4 --output-dir ~/subs --format srt,vtt

# 指定语言（覆盖设置里的 auto）
monosub-cli video.mp4 --language zh

# 启用翻译（默认关闭）
monosub-cli video.mp4 --translate --target en

# 批量（shell 层面）
for f in *.mp4; do monosub-cli "$f"; done

# 机器可读（给 Agent / CI）
monosub-cli video.mp4 --json
```

### 5.2 参数表

| 参数 | 默认 | 说明 |
|---|---|---|
| `<input>...` | 必填 | 一个或多个媒体文件。**顺序**处理；单个失败不中断后续，全部跑完后任一失败则退出码非零 |
| `--output-dir <dir>` | 源文件所在目录 | 目录不存在则创建 |
| `--format <list>` | 设置里的 `outputFormat`，缺失则 `srt` | 逗号分隔，与 GUI 同一套 `Format` 解析 |
| `--language <tag>` | 设置里的 `outputLanguage`，缺失则 `auto` | `auto`/`zh`/`zh-tw`/`en`/`yue`/`ja`/`ko`/… 与 `catalog::map_language` 同一映射 |
| `--translate` | **关** | 启用翻译（对应 GUI 的 `translateEnabled`） |
| `--target <tag>` | `en` | 翻译目标语言，仅在 `--translate` 时有意义 |
| `--json` | 关 | 进度与结果以 JSON Lines 打到 stdout |
| `--quiet` | 关 | 只打错误 |
| `--dry-run` | 关 | 只解析方案并打印将用哪个模型 / 会写哪些文件，不执行 |

**不做 `--model`**（§1.3）。

### 5.3 退出码

| 码 | 含义 |
|---|---|
| 0 | 全部成功（含"全静音输入"——那是成功，写空字幕） |
| 1 | 用法错误（参数非法、文件不存在） |
| 2 | 模型缺失（`MODEL_NOT_INSTALLED`）——stderr 指明缺哪个 + 在 App 里怎么装 |
| 3 | 转写失败（引擎非零退出、ffmpeg 失败） |
| 4 | 写文件失败 |
| 5 | 翻译失败**但字幕已写出**（按现有约定翻译是增强，不阻塞；CLI 用独立码提示但输出文件有效） |

> 退出码 0/非零的**语义**必须与 `audiocpp_cli` 的"全静音也是成功"一致（AGENTS.md
> 已有硬约束），不能因为 CLI 而改变。

### 5.4 输出

**人读（默认，stderr）**：

```
video.mp4
  plan     passthrough:moss → moss_transcribe_diarize_q8_0
  1/4      Extracting audio
  2/4      Loading model
  3/4      Transcribing        37%
  4/4      Writing subtitle
  done     132 cues → /path/video.srt   (1m 42s)
```

**机器读（`--json`，stdout，JSON Lines）**：

```json
{"event":"plan","input":"video.mp4","plan":"passthrough:moss","models":["moss_transcribe_diarize_q8_0"]}
{"event":"progress","input":"video.mp4","step":3,"total":4,"name":"Transcribing","percent":37}
{"event":"done","input":"video.mp4","cues":132,"paths":["/path/video.srt"],"elapsed":102.4}
```

多文件时事件交错出现，靠 `input` 区分。`error` 事件形如
`{"event":"error","input":"b.mp4","code":2,"status":"MODEL_NOT_INSTALLED:..."}`。

字段名与 `transcribe_progress` 事件**刻意不同**（`event` 而非 `status`，另有 `input`）：
CLI 不是 IPC，让两者共享字段名会诱使有人以后把它们合并，而它们的生命周期不同。

> 字段集标为 **experimental**：可在小版本内变，不承诺兼容。真正稳定的契约是
> **退出码**（§5.3）与**输出文件路径**。

---

## 6. 翻译：默认关闭，`--translate` 开启

用户明确要求。实现要点：

- CLT 默认 `translation = None`，即使 `settings.json` 里 `translation.enabled = true`
  也**不**生效——CLI 的默认值只来自 flag。
- 理由：翻译是一次额外的模型加载（1.8B，秒级到分钟级）。GUI 里开着，不代表批量脚本
  每个文件都想付这个成本；显式 `--translate` 才启动，符合"脚本应当是显式的"。
- `--translate` 时模型 id 取 `settings.json` 的 `translation.modelId`，缺失则
  `catalog::default_translation_model_id()`（与 GUI 完全一致）。
- 翻译失败只 warn + 退出码 5，字幕文件有效（沿用 `translate::stage` 的既有语义）。

---

## 7. 发布形态（A + B）

### 7.1 B：本机 build

```toml
# src-tauri/Cargo.toml（workspace 根）
[workspace]
members = [".", "core"]

# GUI 保持默认 bin
[[bin]]
name = "monosub"
path = "src/main.rs"

# CLI
[[bin]]
name = "monosub-cli"
path = "src/bin/monosub-cli.rs"
```

```bash
cargo build --release --bin monosub-cli
# → src-tauri/target/release/monosub-cli
./src-tauri/target/release/monosub-cli video.mp4
```

### 7.2 A：随 DMG 发布

用 tauri 的 `externalBin`（它会自动追加 target triple 后缀）：

```jsonc
// tauri.conf.json
"bundle": {
  "externalBin": ["binaries/monosub-cli"]
}
```

构建前把产物放到 `src-tauri/binaries/monosub-cli-aarch64-apple-darwin`
（`build-macos-app.sh` 里加一步；`prepare-resources.sh` 不动，因为它管的是下载来的
第三方二进制）。

装进 App 后的位置是 `Contents/MacOS/monosub-cli`。用户要能在终端直接用，需要：

```bash
ln -s /Applications/MonoSub.app/Contents/MacOS/monosub-cli /usr/local/bin/monosub-cli
```

**App 不能自动改 PATH**（Apple 不允许，且沙箱/权限上也做不到）。做法：

- 在 `Settings → 系统` 加一行"命令行工具"，显示可复制的 `ln -s` 命令 + 一个"复制"按钮
  （复用 `ErrorDetailsDialog` 里的 `writeClipboard` 写法）。
- 不做自动安装，不往 `/usr/local/bin` 偷偷写东西。

### 7.3 签名

`scripts/release/sign-app.sh` 已经遍历 `Contents/Resources/resources/bin/*` 逐个重签。
`externalBin` 落在 `Contents/MacOS/`，**不在那个循环里**，需要补一段：

```bash
CLI_BIN="$APP_PATH/Contents/MacOS/monosub-cli"
[ -f "$CLI_BIN" ] && codesign --force --options runtime --timestamp \
    --sign "$APPLE_SIGNING_IDENTITY" "$CLI_BIN"
```

漏了这一步的表现是：公证通过但用户第一次运行时被 Gatekeeper 拦下（`killed: 9`）。

### 7.4 `bin/monosub` 脚本

删除。它指向 `core-engine/main.py`（已废弃的 Python 引擎，最后一次提交
2026-10-13，而当前 HEAD 是 2026-10-20），且**全仓库无任何引用**（`package.json` 无
`bin` 字段，发布脚本不产出它）。`core-engine/` 目录本身按用户要求保留不动。

---

## 8. 实施顺序（每步都必须 `cargo test` 全绿才能进下一步）

| # | 步骤 | 验证 |
|---|---|---|
| R1 | 新建 workspace + `core/` crate，把 `engine/` `subtitle/` `translate/` `model_dir.rs` `git mv` 进去；`AppPaths` 落地；全部签名改 `&AppPaths` | 475 条测试全绿；`grep -r "tauri" core/src/` 为空 |
| R2 | 抽 `core::pipeline::transcribe()`，GUI 侧变成薄适配（emit 事件） | 事件契约不变；`Steps` 4/5 分母测试仍绿 |
| R3 | `BackendSettings` 加 `recognitionPlan` / `translation`；`resolve_model_plan` 写盘 | 旧 settings.json 读出来不报错；新增往返测试 |
| R4 | `src/bin/monosub-cli.rs`：参数解析（clap）→ 读 settings → 求解 → 调 pipeline → stdout 输出 | 单元测试覆盖参数解析与退出码映射；本机实测 |
| R5 | `externalBin` + `build-macos-app.sh` + `sign-app.sh` 补签 | 打包后 `MonoSub.app/Contents/MacOS/monosub-cli` 存在且可执行 |
| R6 | Settings → 系统 加“配置文件”行 +“命令行工具”卡片（显示 `ln -s` 命令） | pilot 场景可断言 |
| R7 | 删 `bin/monosub`；更新 AGENTS.md / README | `grep` 无残留 |

---

## 9. 风险与对策

| 风险 | 对策 |
|---|---|
| workspace 重构动到 GUI 主路径，回归 | R1/R2 是**纯机械替换**，每步以 475 条测试为门禁；不夹带任何行为变更 |
| `AppPaths::from_env()` 与 Tauri 的 `app_config_dir()` 推导不一致 → CLI 读不到 GUI 设置 | 单测断言两者指向同一路径（`#[cfg(test)]` 里构造一个假 AppHandle 或直接比对字面路径常量） |
| 用户 GUI 里没选过模型（新装）→ `settings.json` 无 `recognitionPlan` | 退回 `Requirements::default()` + stderr 明确提示，不猜 |
| `--json` 输出被人当稳定 API | 在 AGENTS.md 标注为 `experimental`，字段变更不承诺兼容 |
| CLI 与 GUI 同时跑，争抢 `audiocpp_cli` 与 GPU | 不加锁（GUI 已有 `TRANSCRIBE_SEMAPHORE`，进程间无效）。在文档里说明"建议不要并行跑，Metal 下会互相拖慢" |

---

## 10. 已定事项（原待确认，均已拍板）

1. **`settings.json` 位置** —— 配置路径与模型路径是两个独立概念（§4.0）：配置用现有的
   `<app_config_dir>/settings.json`，模型路径不动。需要 `~/.monosub` 时用
   `MONOSUB_CONFIG_DIR` 覆盖。
2. **`--json` 现在就做**（R4 的一部分）。作用是让批量脚本 / Agent / CI 拿结构化结果，
   而不是去 grep 人读文本（界面文案一改脚本就坏）。分流规则从一开始就是：
   **人读 → stderr，`--json` → stdout**，这样以后加也不会破坏已有脚本。
   字段集标为 `experimental`（可在小版本内变），不承诺兼容。
3. **多文件参数现在就支持**（`monosub-cli a.mp4 b.mp4`）。语义：
   - **顺序**处理，不并发（§9 已说明 Metal 下并行会互相拖慢）。
   - 单个文件失败**不中断**后续文件——批量跑不该因为一个坏文件全停。
   - 处理完全部后，**任一失败则退出码非零**（取最先失败的那个码，便于脚本判断）。
   - `--json` 下每个文件仍有自己的 `done` / `error` 事件（带 `input` 字段区分）。

---

## 11. 实施记录

### 11.1 R1 已完成（2026-10-22）

**结果：`engine/` / `subtitle/` / `translate/` 已与 `tauri` 完全解耦。**

`cargo test` **481 passed / 0 failed**（基线 475，+6 条新增锁测试）；
`cargo clippy --all-targets` **14 warnings，与 HEAD 基线完全持平**（净减一条：`configured_root`
变成死代码后被删除）。

#### 落地内容

| 文件 | 改动 |
|---|---|
| `src-tauri/src/paths.rs` | **新增**。`AppPaths { resources, models_root, config_dir, data_dir }` + `from_app`（feature-gated）/ `from_env` |
| `src-tauri/Cargo.toml` | 新增 `[features] default = ["tauri-app"]`；`tauri-app` 只门控 `AppPaths::from_app` |
| `src-tauri/src/engine/catalog.rs` | 7 个 pub fn 改 `&AppPaths`；`list()` 不再需要 `Option<root>`（models_root 是字段而非运行时解析） |
| `src-tauri/src/engine/cli.rs` | 路径解析重写。`resolve_resource_path` 从「猜 exe 位置」简化为「查 `paths.resources`」；`bundle_resources_dir` 删除（成死代码） |
| `src-tauri/src/engine/{mod,aligned,vad,diar,direct}.rs` | 签名 + 调用点 |
| `src-tauri/src/{translate/mod,translate/stage}.rs` | 同上 |
| `src-tauri/src/model_dir.rs` | **删除 `configured_root`**（被 `AppPaths::models_root` 取代，保留会重新制造「两份真相」） |
| `src-tauri/src/commands/{models,plan,transcribe}.rs` | 在 Tauri 命令边界构造 `AppPaths::from_app(&app)?` 并下传 |
| `src-tauri/src/site/{mod,deps,ytdlp}.rs` | 同上 |

#### 关键设计决定（与 §3.1 的偏差）

1. **没有新建 `core/` crate。** 原计划 R1 要 `git mv` 三个目录进 workspace。实测后判断代价不值：
   `AppPaths::from_env()` 已经让 CLI 能在无 Tauri 环境下跑起来，这才是真正的目的；而拆 workspace
   要动 `Cargo.toml`、处理 `generate_context!` 的路径问题、并让每次编译多一个 crate。
   **用 feature gate 达到同样的保证**：`grep -rn "tauri::\|AppHandle" src-tauri/src/{engine,subtitle,translate}/`
   现在为空（仅注释里有字样），这比一个目录边界更直接。
   *如果将来真需要独立发行 CLI 二进制再抽 crate，`AppPaths` 已经把依赖关系理清了。*
2. **`bin/monosub` 未被替换。** 它仍在 `bin/monosub` 指向废弃的 `core-engine/main.py`。等到 R4 产出
   `monosub-cli` 之后再决定是替换成 `exec monosub-cli "$@"` 还是删除。
3. **`binaries/` 弃用，改用 `resources/bin/`。** 见 §11.2 —— 这是 R5 worker 的实测发现。

#### 6 条新增锁测试（`src-tauri/src/paths.rs`）

| 测试 | 锁住什么 |
|---|---|
| `the_cli_derives_the_gui_s_app_directory` | **读 `tauri.conf.json` 实际解析 identifier**，与代码常量比对。改 bundle id 忘了改 CLI → 立刻红 |
| `the_resources_root_contains_the_model_catalogue_in_a_dev_checkout` | 资源根必须真的含 `models.json`，否则 CLI 无法规划 |
| `a_configured_models_dir_is_used_verbatim_not_derived_from_the_config_dir` | `modelsDir` 指向任意位置都原样采用，**不被推导成 config 的子目录** |
| `settings_live_in_the_config_dir_regardless_of_the_models_root` | 反向锁：`settings.json` 只由 `config_dir` 决定 |
| `a_blank_models_dir_is_not_a_path` | 空白值 = 用默认，不是 `/` |
| `unreadable_settings_yield_no_override` | 损坏的 settings 不能让 CLI panic |

### 11.2 R5 已完成（脚本部分），并修订了 §7.2

**§7.2 原方案（`bundle.externalBin`）是错的，已废弃。** 实测证据（R5 worker）：

```
$ TAURI_CONFIG='{"bundle":{"externalBin":["binaries/monosub-cli"]}}' cargo test
error: failed to run custom build command for `monosub v2.0.0`
  resource path `binaries/monosub-cli-aarch64-apple-darwin` doesn't exist
```

根因：`tauri-build-2.7.3/src/lib.rs:735` 在 **build script** 里就校验 `externalBin` 的文件存在，
而 build script 每次 `cargo build` / `cargo test` 都跑。`binaries/` 是构建产物、不可能入库，
所以只要写了这一行，**CLI 落地前全仓库测试对所有人全红**。

**改用 `resources/bin/`**（**本节取代 §7.2**）：

| 依据 | 说明 |
|---|---|
| `resources/**/*` 是 glob | 缺文件只是不匹配，**不做存在性校验** → clone 下来永远绿 |
| `sign-app.sh` 的 `RESOURCE_BIN_DIR` 循环已覆盖 | 自动被重签，无需新代码 |
| 项目已有先例 | `prepare-resources.sh` 历史就有 `bin/monosub-engine-*`；`audiocpp_cli`/`ffmpeg`/`yt-dlp` 全在这里 |
| 路径解析天然吻合 | `AppPaths.resources` 解析的正是这个目录 |

落地：`build-macos-app.sh` 编译 CLI → `src-tauri/resources/bin/monosub-cli-<triple>`；
`tauri.conf.json` 与 `.gitignore` **零改动**（`resources/*` 已覆盖）。

### 11.3 剩余工作

| # | 内容 | 前置 |
|---|---|---|
| R2 | 抽 `pipeline::transcribe()`，GUI 变薄适配层 | R1 ✅ |
| R3 | `BackendSettings` 加 `recognitionPlan` / `translation`，`resolve_model_plan` 写盘 | R1 ✅ |
| R4 | `src/bin/monosub-cli.rs`（clap、`--json`、多文件） | R2 + R3 |
| R5 | 打包签名 | ✅ 已完成 |
| R6 | Settings → 系统：配置文件行 + 命令行工具卡片 | R4 |
| R7 | `bin/monosub` 的去留；本文档状态更新为「已实现」 | R4 |

### 11.4 R2 已完成（2026-10-22）

**结果：`transcribe_file` 已与 `AppHandle` 解耦，进度通过 `ProgressSink` 输出。**

`cargo test` **482 passed / 0 failed**（+1 sink 契约测试）；
`cargo clippy` **14 warnings = 基线**，零新增。

#### 落地内容

| 项 | 内容 |
|---|---|
| `ProgressSink` trait | `step(&StepSnapshot)` / `queued(task_id)` / `finished(task_id, seconds)`；`Send + Sync` |
| `StepSnapshot` | 与传输格式无关的进度快照（task_id / progress / status / current_step / total_steps / step_name / step_elapsed / total_elapsed） |
| `EventSink` | GUI 的实现：把快照还原成**原封不动**的 `transcribe_progress` / `transcribe_complete` 事件 |
| `transcribe_file` | 签名从 `(app, paths, params)` → `(paths, params, sink)`。内部 7 处 `steps.begin(app, …)` 改 `steps.begin(sink, …)` |
| `translate_and_write` | 同上，去掉 `app` |
| `extract_wav` | `(app, input)` → `(paths, input)` |
| `commands/video.rs` | `get_ffmpeg_path` / `get_ffprobe_path` 改吃 `&AppPaths`；`resolve_binary_path` 的打包态二分查找**改为查 `paths.resources/bin`**，删掉了重复的 exe 位置猜测 |

#### 关键细节

1. **事件契约一字未改。** `EventSink` 产出的 `TranscribeProgress` 字段、`STEP_NAMES`
   冻结集、4/5 分母逻辑全部保持原样。这是 `FileItem.tsx` 的 `steps.<snake_case>` i18n
   key 依赖的东西。
2. **两个 `Mutex` 的由来。** `engine::ProgressFn` 是 `Fn`（共享引用）而 sink 是 `&mut`，
   所以识别阶段和翻译阶段的回调都把 sink 包在 `Mutex` 里。报告的调用是顺序的，锁不冲突。
   注释里写明了原因，避免以后有人"优化"掉它。
3. **新增契约测试 `a_sink_receives_queued_then_each_step_then_finished`**：不需要 Tauri
   app、不需要真模型，就能验证「queued → step 1..4 → finished」的顺序与 `1/4 … 4/4`
   的分母。这是 CLI 复用时唯一的正确性保证。

#### 当前可复用的边界

```
✅ 已解耦（可直接给 CLI 用）         ❌ 仍绑定 Tauri
   engine/  subtitle/  translate/      commands/  site/  tray/
   extract_wav()  transcribe_file()    start_transcription()（Tauri 命令壳）
   全部路径解析（AppPaths）            事件发射（仅 EventSink）
```

**注意**：`transcribe_file` 虽然签名干净，但它仍在 `commands/transcribe.rs` 里（不是
`engine/`）。R4 写 CLI 时直接从该模块 `use` 即可；**是否需要把它移进 `engine/` 留给
R4 决定**——现在移动会让 `git mv` 与未提交改动纠缠。

### 11.5 R3 已完成（2026-10-22）

**结果：GUI 的四问答案与求解结果已落到 `settings.json`，CLI 可以读到了。**

`cargo test` **494 passed / 0 failed**（R2 基线 482，+12 条新增锁测试）；
`cargo clippy --all-targets` **14 warnings = 基线**，零新增。
`bun run check` / `bun run lint`(0 error) / `bun run check:i18n` 均通过。

#### 实际 schema

`<app_config_dir>/settings.json` 实测内容（本机跑过一次带全部字段的写入）：

```json
{
  "modelsDir": "/Users/lei/dev/projects/2601-monosub/models",
  "recognitionPlan": {
    "languageProfile": "mixed",
    "memoryBudget": "small",
    "requireWordTimestamps": true,
    "diarization": true,
    "language": "zh",
    "modelIds": [
      "qwen3_asr_0_6b_q8_0",
      "qwen3_forced_aligner_0_6b_q8_0"
    ],
    "passthrough": false
  },
  "translation": {
    "enabled": true,
    "targetLanguage": "en",
    "modelId": "hy_mt_1_8b_q4_k_m"
  }
}
```

与 §4.2 的差异（一处，且是刻意的）：

- **没有 `modelsVersion`。** `modelIds` / `passthrough` 本来就是缓存（§4.2 自己写明了
  「非权威」），CLI 每次仍用 `plan::solve()` 复核；再加一个版本号等于给一份缓存配一个
  失效开关，多一个能漂移的字段。缓存变了 CLI 自然会 warn，不需要版本号来发现。
- `translation.modelId` 为 `null` 时**整个键被省掉**（`skip_serializing_if`），因为
  `None` 的意思是「用目录默认」，写成 `null` 会让 CLI 去分辨同一种意图的两种拼法。

其余字段拼写与 §4.2 完全一致（camelCase，`BackendSettings` 是**逐字段** `rename` 而非
容器级 `rename_all`——`modelsDir` 一直就是这么写的，而响应 DTO 早于这个字段存在，
把拼写钉在读取处比藏在一个容器属性里更难改错）。

#### 写入时机

1. **`commands/plan.rs::resolve_model_plan()`** 在返回前写盘。这条命令是设置页每次改动
   都会调的（`SettingsModelPlan.tsx` 的 effect），所以「用户在 GUI 里选了什么」立刻落盘，
   **不需要新事件或定时器**。
   写的是 `Requirements`（已归一化的五个值）而不是 wire 请求：wire 那边 `zh-en`、
   `multi` 这类别名和缺省字段会原样落盘，CLI 就得把别名表再实现一遍。因此
   `languageProfile` 存的一定是 `mandarin`/`english`/`mixed`/`multilingual` 四者之一。
2. **`save_translation_settings`**（新命令）由 Settings → 翻译 写。

#### 为什么写失败不致命

`resolve_model_plan` 的返回值是用户在设置页要看的东西——预览面板。写盘只是**顺带**把
同一个结果留给 CLI。磁盘满、权限、配置目录在只读卷上，这些都与「回答这四个问题」无关，
把它们升级成命令失败会让预览面板报错、用户没法用界面，而 CLI 那边的后果只是**退回
默认四问**（§4.4 已规定这种情况要 stderr 提示，不猜）。所以写失败只
`tracing::warn!`，返回值一字不变。

#### 三个写入点都是「读-改-写」

`modelsDir`、`recognitionPlan`、`translation` 共用一个文件，因此**任何**一个写入者构造
新 struct 都会抹掉另外两个。这是本任务最容易出的 bug（`set_models_dir` 早已是读-改-写，
新加的两个必须同构）。实现上每一条都是：

```rust
let path = settings_path(app)?;
let mut settings = read_settings_at(&path);   // 磁盘上现有的全部字段
merge_recognition_plan(&mut settings, plan);  // 只改自己那一段
write_settings_at(&path, &settings)
```

测试 `writing_one_section_keeps_the_others` 把两个方向都锁了：先写翻译再写方案、先写
方案再写翻译，每次都断言另外两个字段还在。

#### 落地内容

| 文件 | 改动 |
|---|---|
| `src-tauri/src/model_dir.rs` | `BackendSettings` + `RecognitionPlan` + `TranslationSettings`；`read_settings_at` / `write_settings_at`（无 `AppHandle` 的纯逻辑，`load_settings` / `save_settings` 退化成薄封装，R4 的 CLI 可直接复用）；`merge_*` + `save_recognition_plan` / `save_translation_settings` |
| `src-tauri/src/commands/plan.rs` | `resolve_model_plan` 返回前写盘（失败仅 warn）；`recognition_plan_from_requirements`（纯函数，可单测）；新命令 `save_translation_settings` |
| `src-tauri/src/main.rs` | `invoke_handler` 注册 `save_translation_settings` |
| `docs/tech-spec/cli-design.md` | 本节 |

**为什么这两个 struct 留在 `model_dir.rs` 而不是新建 `settings.rs`**：读-改-写规则已经
在那里，`commands::models::set_models_dir` 也是从那里调的。新开一个模块只会让「谁负责
这个文件」出现两个答案，而这个文件的内容全是字符串——**没有一个字段由另一个字段推导**，
`config_dir` 与 `models_root` 依然互不相关（`paths.rs` 的注释仍然成立，本任务没碰它）。

#### 新增的 12 条锁测试

| 测试 | 锁住什么 |
|---|---|
| `a_settings_file_with_only_models_dir_still_parses_with_no_plan` | 现场已有的 `{"modelsDir":"/x"}` 能解析，`recognition_plan` 为 `None`，`modelsDir` 不丢 |
| `a_recognition_plan_round_trips_through_the_file` | 写→读字段逐个相等，并逐字断言 12 个 camelCase 键名（这是与 CLI 的契约） |
| `writing_one_section_keeps_the_others` | 两个写方向的读-改-写：谁都不能抹掉谁 |
| `unknown_keys_do_not_break_the_settings_file` | 未来版本或手写的未知键不炸（`deny_unknown_fields` 会让 CLI 在每次 schema 扩展后拒绝 GUI 自己写的文件） |
| `a_partial_recognition_plan_does_not_fail_the_whole_file` | 容器级 `#[serde(default)]`：缺字段的方案不能连 `modelsDir` 一起带走 |
| `absent_sections_serialize_away` | `None` 段写成「不存在」而不是 `null`，没开过设置页的安装看起来跟以前一样 |
| `a_default_translation_model_is_omitted` | `modelId: None` 不落成 `null` |
| `an_unwritable_location_is_an_error_not_a_panic` | 写失败返回 `Err` 而非 panic——这是「写盘失败只 warn」成立的前提 |
| `the_persisted_plan_records_the_answers_and_the_solved_models` | 五问 + `modelIds` + `passthrough` 完整落盘 |
| `the_persisted_plan_normalises_aliases_and_defaults` | `zh-en` → `mixed`、未知 budget → `medium`、缺语言 → `zh`：落盘的永远是规范拼写 |

（`settings_round_trip_keeps_models_dir` / `settings_defaults_on_corrupt_json` /
`omitted_models_dir_serializes_away` / `validate_dir_*` 保持原样，只改了结构体字面量。）

### 11.6 R6 已完成（2026-10-22）

**结果：Settings ▸ 系统 里能看到「配置文件」与「命令行工具」两块。**

`cargo test` **494 passed / 0 failed**；`cargo clippy` **14 warnings = 基线**；
`check:i18n` **268 keys**（258 + 10 新增，中英一致）。

#### 界面

```
配置文件     /Users/lei/Library/Application Support/dev.leix.monosub/settings.json   [显示]
             配置与模型是两件事：这里只放设置，模型在「存储位置」里单独配置。

命令行工具   (即将提供)
             MonoSub 将提供一个命令行入口，与 App 共用同一条管线与同一个模型。
             ln -s /Applications/MonoSub.app/Contents/MacOS/monosub-cli /usr/local/bin/monosub-cli  [复制]
```

第二块**明确标注「即将提供」**：CLI 尚未实现（R4 未完成），写成已可用就是撒谎。
「复制」按钮复用 `ErrorDetailsDialog` 的 `writeClipboard`（`navigator.clipboard` +
`execCommand` 回退）。

#### 新增命令

| 命令 | 说明 |
|---|---|
| `config_file_path` | `settings.json` 的绝对路径（文件不存在也返回路径） |
| `reveal_config_file` | 在文件管理器里定位它；不存在时打开父目录。复用 `commands::opener::reveal_in_file_manager`，不自己写平台分支 |

`src/lib/config.ts` 是前端包装，另导出 `CLI_INSTALL_COMMAND` 常量。

> 为什么不自动装到 PATH：Apple 不允许 app 改 PATH，沙箱下也做不到。所以只显示
> 可复制的命令，由用户自己执行。

### 11.7 R4 已完成（2026-10-22）

**结果：`monosub-cli` 可用了。** `cargo test` **494 + 8 = 502 passed / 0 failed**；
`cargo clippy` **14 warnings = 基线**。

#### 必须先解决的前置：crate 变成 library

`[[bin]]` 是独立的 crate root，**不能 `use crate::engine`**。原来所有模块都由
`main.rs` 声明，所以第二个 bin 无法共享任何代码。

解决：

```toml
[[bin]] name = "monosub"      path = "src/main.rs"
[[bin]] name = "monosub-cli"  path = "src/bin/monosub-cli.rs"
[lib]  name = "monosub"       path = "src/lib.rs"
```

`src/lib.rs` 声明 8 个 `pub mod`；`src/main.rs` 变成薄 Tauri 壳（`use monosub::…`）。

两个坑，都已踩过：

1. **`#[tauri::command]` 生成的宏在原模块里。** 跨 crate 引用时 `use monosub::site::{parse_url_input, …}`
   不够——`generate_handler!` 找不到 `__cmd__parse_url_input`。解法：在 handler 列表里写
   **完整路径** `monosub::site::parse_url_input`。
2. **`private_interfaces` 从 warning 变成真实错误。** `site::JobParams` 是 `pub(crate)`，
   却出现在 `pub trait Deps` 的方法签名里——库化前无害，库化后是「public API 摸不到类型」。
   已提升为 `pub`（字段仍只在模块内可构造）。

顺带好处：clippy 的 dead-code 类 warning 从 14 降到 10（库把一些 `pub(crate)` 变成了真正的
公共项，死代码检测更准）。

#### CLI 本体（`src/bin/monosub-cli.rs`，约 630 行）

**不引入 clap**：这个 app 已经带三个 C++ 引擎，CLI 的参数面只有 9 个选项，手写解析
（含单元测试）比多一个依赖划算。

| 能力 | 实现 |
|---|---|
| 参数 | `-o/--output-dir`、`-f/--format`、`-l/--language`、`--translate`、`--target`、`--json`、`-q/--quiet`、`-h/--help` |
| 多文件 | 顺序处理；单个失败不中断后续；最终退出码取**第一个**失败 |
| 模型 | **没有 `--model`**。从 `settings.json` 的 `recognitionPlan` 读四问答案，再跑一次 `plan::solve` 复核（用户可能删过模型） |
| `--json` | stdout，JSON Lines：`queued` / `progress` / `done` |
| 人读 | stderr，逐 step 打印，**只在四舍五入后的百分比变化时重绘**（否则每 chunk 一行会刷屏） |
| 退出码 | 0 成功 / 1 用法 / 2 模型缺失 / 3 转写失败 / 4 写失败 / 5 翻译失败 |

**翻译默认关闭**：即使 GUI 里开着，CLI 也只在 `--translate` 时翻译。批量脚本不该为每个文件
付一次 1.8B 模型加载。

#### 实测（真实转写，非 mock）

```
$ monosub-cli /tmp/cli-test.wav -o /tmp/out --json
{"event":"queued","input":"/tmp/cli-test.wav","task":"cli-74980"}
{"event":"progress","step":1,"total":4,"name":"Extracting audio","percent":0}
...
{"event":"done","input":"/tmp/cli-test.wav","elapsed":17.5}

$ cat /tmp/out/cli-test.srt
1
00:00:00,000 --> 00:00:00,300
好

$ monosub-cli /nope.mp4            → exit 1 "no such file"
$ monosub-cli --bogus              → exit 1 "unknown option"
$ MONOSUB_MODELS_DIR=/tmp/empty monosub-cli in.wav
  monosub-cli: model `moss_transcribe_diarize_q8_0` is not installed
               download `moss_transcribe_diarize_q8_0` in MonoSub (Settings ▸ 模型) and retry.
                                   → exit 2
```

**实测发现的一个 bug**：`done` 事件原本发两次（`transcribe_file` 结尾一次、CLI 又一次），
已修。这类重复只有真跑才会暴露——单测覆盖不到「两个地方都调了 sink.finished」。

#### 本机使用

```bash
cargo build --release --bin monosub-cli
./target/release/monosub-cli video.mp4
```

### 11.8 R7 已完成，并修掉两个由文档任务发现的实现 bug

**结果：文档与代码一致。** `cargo test` **502 passed / 0 failed**；`cargo clippy`
**11 warnings**（低于基线 14）。

#### bug 1：`--translate` 是死参数（我写的，R7-docs 发现）

`--help` 宣传 `--translate  Also translate (off by default)`，但 `args.translate` /
`args.target` 解析后**从未被读取**，`TaskParams { translation: None }` 是硬编码，
`TRANSLATE_FAILED = 5` 定义未用（clippy 一直在报）。

修法：新增 `resolve_translation()` 走 `translate::stage::resolve`，把结果接进
`TaskParams.translation`。实测：

```
$ monosub-cli speech.wav --translate --target en
  1/5 … 5/5 Translating
  speech.srt     大家好，我是一个测试。
  speech.en.srt  Hello everyone, I'm a tester.
```

#### bug 2：退出码 5 会把「跳过」误报为「失败」（R7-docs 发现）

第一版修法用 `!written.is_empty()` 判断成败，于是：

- 全静音输入 + `--translate` → `written` 为空 → **exit 5**，但静音本来就是成功
- `source=auto` 检测结果等于目标语 → `SkipReason::SameLanguage` → 同样 exit 5

修法：`StageOutcome` 新增 `skipped: Option<SkipReason>`。跳过 ≠ 失败：

```rust
translation_written = translated.skipped.is_some() || !translated.written.is_empty();
```

实测：静音 + `--translate` → **exit 0**。

#### bug 3：`--target` 没有默认值

`--help` 写着 `--target <tag>  Translation target (default: en)`，但代码里
`args.target.as_deref()` 直接把 `None` 传下去 → `SkipReason::NoTarget` → 静默不翻译。
已改为 `Some(args.target.as_deref().unwrap_or(DEFAULT_TARGET))`，实测
`--translate` 不带 `--target` 现在会真翻译。

#### `bin/monosub`

从「指向废弃 Python 引擎的死脚本」改为薄包装：按 `MONOSUB_CLI_PATH` →
`target/release` → `target/debug` → `PATH` 找 `monosub-cli` 并 `exec`。
找不到时打印可操作的构建指令。显式 `MONOSUB_CLI_PATH` 指向非可执行文件时报错而不是
静默回退到别的构建。

#### 教训

502 条测试全绿，却没有任何一条测「`--help` 里宣传的选项是否真的生效」。这类缺口只有
两种方式能发现：真跑一遍，或者有个愿意说「文档和代码对不上」的人。这次是后者。
