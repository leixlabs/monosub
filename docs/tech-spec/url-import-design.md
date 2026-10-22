# URL 导入：从 B 站直接取字幕，或在缺字幕时转成转录任务

> 状态：**设计稿 v2（未实现）**。首版**只支持 B 站**；YouTube 推迟，已知约束记录在 §10。
> v1 的评审结论已全部并入本稿（变更摘要见 §11）。
> §9 的实测用的是本机 **Python 版** yt-dlp 2026.02.21，PATH 中有 `deno` / `ffmpeg`，
> 也没有屏蔽用户配置；这与随包分发的二进制环境不同。所以 §9 中凡是会被这些差异
> 影响的结论都标为「待复测」，统一由 U0-9 用**随包二进制**重跑。
> 相关：[refactor-audiocpp.md](./refactor-audiocpp.md)、
> [model-selection-design.md](./model-selection-design.md)、
> [translation-design.md](./translation-design.md)、
> [url-import-tasks.md](./url-import-tasks.md)

---

## 1. 目标

用户在应用里粘贴**一个或多个** B 站链接（空格、换行或逗号分隔），程序：

1. 用 **yt-dlp** 取该视频在**当前设置语言**下的字幕；
2. **有字幕** → 把字幕转成用户选的输出格式，写到 URL 导入的输出目录（§5.4）；
   若开启了翻译，同样产出 `<stem>.<lang>.<ext>`（§5.6）；
3. **没有该语言字幕** → 下载音频，**转成普通转录任务**，走现有 `engine/`（audio.cpp）
   的识别、后处理、写出和可选翻译整条流水线。

失败语义沿用翻译的既有约定：**能降级就降级，不能降级才失败**。一个链接失败不影响其他链接。

### 非目标

- 不做通用下载器：只取字幕，或取回音频用于识别。
- 不处理播放列表、合集、多 P 整体导入（`--no-playlist` 恒定）：一个 URL 对应一个视频（一个分 P）。
- 不支持番剧、课程（`/bangumi/`、`/cheese/`）以及空间页、收藏夹等列表页：
  这些多为付费或 DRM 内容，或者本身就是列表。
- 不引入 Python 运行时。yt-dlp 用 **build 时下载好的对应平台官方预编译文件**（§6.1），
  与 `audiocpp_cli` / `llama-server` 同等对待：随包分发、`kill_on_drop`、子进程隔离。
- 首版不做 YouTube（§10）。
- 登录方式只支持 `--cookies-from-browser`：不做 OAuth，不做账号密码。

---

## 2. 关键决策（先看这张表）

| 项 | 决定 | 依据 |
|----|------|------|
| 站点范围 | **仅 B 站**：`www./m.bilibili.com/video/BV…|av…`、`b23.tv` 短链 | YouTube 需要额外的 JS 运行时、代理、机翻轨识别，单独成期（§10） |
| URL 解析 | Rust 纯函数 `site::url::parse_input`，经 `parse_url_input` 命令给前端用 | 前端没有测试运行器；规则放在 Rust 里才能单测，也只有一个真值来源 |
| 语言选择 | **严格匹配当前设置语言**，绝不拿别的语言或机翻顶替 | 用户要求「根据设置中的语言来选择」。拿 `zh-Hans` 顶 `zh-Hant`、拿 `ai-en`（AI 翻译轨）顶英文，都会悄悄交付错语言的成品（§4.2） |
| 字幕选择 | Rust 根据 Resolve 得到的 info dict 选出**唯一一条** `Pick { tag, kind }` | 不把候选列表交给 `--sub-langs` 再事后挑：一次只取一条，结果可预测 |
| 探测 | Resolve **一次** yt-dlp 调用，拿到完整 info dict（含 `subtitles`） | §4.1 |
| 字幕内容 | 优先直接用 info dict 里 `subtitles[tag][].data`（B 站提取器已转好的 SRT 文本）；没有 `data` 时才用 `--load-info-json` 单独下载这一条 | B 站提取器调用 `json2srt` 把 SRT 文本内联进 `data`（§9.11），省去第二次网络请求，也不需要 ffmpeg 转换 |
| 不做格式转换 | **不用** `--convert-subs`；下载时使用 `--sub-format srt/best` | 转换要依赖 ffmpeg，而 B 站原生就是 SRT |
| 降级触发 | `select()` 返回 `None`，或该条字幕取不到 / 解析为空 | 判定基于结构化数据和磁盘文件，**不看退出码**（无匹配语言时退出码为 0，§9.5） |
| 降级动作 | `-f ba[ext=m4a]/ba/b` 下载到**用户配置的下载目录**（与字幕同目录、同 stem），下载完成后**入队**为普通文件 | 媒体是用户的素材，不是中间产物；与字幕同目录才不会“清缓存时顺手删了下载” |
| 音频下载时机 | **入队前**完成；入队的 `path` 永远是一个真实存在的本地文件 | 现有转录命令和元数据、缩略图命令都无需改动（§5.7） |
| 输出目录 | 新增设置项 `urlImportOutputDir`（默认 `~/Downloads/MonoSub`）；`storageMode='custom'` 时用自定义目录；**媒体与字幕共用这个目录** | 一个导入的产物是一个「视频 + 字幕」，分开存会让用户搞不清东西在哪、清缓存时丢东西（§5.4） |
| 文件名 | `<sanitized title> [<id>].<ext>` | id 让文件名天然唯一：不同视频不会撞名，同一 URL 重导入得到同一路径，和本地文件一样走 `skipExistingSubtitle`（§5.5） |
| 翻译 | 两条路径一致：字幕命中路径**同样**按设置翻译 | 同一个开关在两条路径上表现不同是坏体验。命中路径已有 `Vec<Segment>` 和确定的源语言标签（§5.6） |
| B 站登录 | `--cookies-from-browser <browser>`，**默认未配置**；第一次遇到 412 或「需登录才有字幕」时，在行内引导用户选择浏览器，并持久化 | 无 cookie 时整条请求会 412（§9.6）；就算不 412，匿名请求的字幕列表也是空的（§9.12）。默认关闭是为了不在用户不知情时读浏览器数据，但必须给出一条能走通的引导 |
| 失败粒度 | 每个 URL 独立成败，逐 URL 给出错误文案 | 批量粘贴 5 个链接，不该因为第 3 个失败就丢掉另外 4 个 |
| 取消 | 导入阶段可取消（abort tokio task，`kill_on_drop` 杀掉 yt-dlp）；入队后就是普通文件 | 与 `cancel_transcription` 同一套语义（§5.8） |

---

## 3. 整体流程

```
用户粘贴 "url1 url2 url3"（空格/换行/逗号分隔）
        │
        ▼ invoke parse_url_input(text) —— Rust 纯函数：切分 + 规范化 + 去重 + 白名单
        │   ├─ 不支持的站点 / 番剧 / 列表页 ──→ 立即逐条报错，不建 job
        │   └─ 重复 URL ─────────────────────→ 去重（提示条数）
        ▼
┌── 每个 URL 一个异步 job（start_url_import，立即返回，结果走事件）──────┐
│                                                                        │
│  A. Resolve   yt-dlp <常量参数> --skip-download --list-subs --print-json│
│                     [--cookies-from-browser B] <url>                   │
│        ├─ 412 ─────────────────→ 失败：URL_IMPORT_NEEDS_COOKIES（引导登录）│
│        ├─ 私有 / 删除 / 网络 ──→ 失败，逐条报错（不可降级）            │
│        ├─ 多 P 且 URL 未指定 p ─→ 失败：请贴具体分 P 链接（§4.4）      │
│        ↓ SiteMedia { id, title, duration, subs: {tag → SubTrack} }     │
│          + stderr 是否出现「需登录才有字幕」→ login_hint                │
│                                                                        │
│  B. Select    select(setting_lang, &SiteMedia) → Option<Pick>（§4.2）   │
│        ├─ Some(pick) ──┐                                               │
│        └─ None ────────┼──────────────────────────────┐                │
│  C. Fetch    pick.data 有内容 → 直接用                 │                │
│              否则 --load-info-json + --write-subs      │                │
│                   --sub-langs <pick.tag> --sub-format srt/best         │
│        ├─ 得到非空 SRT ──→ D. Materialize（§5.4）       │                │
│        └─ 取不到 / 为空 ─────────────────────────────→ │                │
│                                                       ▼                │
│  E. Fallback（§5.3）  下载音频到输出目录 → url_import_complete(Queued) │
│                       → 前端入队 → 现有队列启动转录                     │
└────────────────────────────────────────────────────────────────────────┘
        │
        ▼  D. Materialize：SRT → Vec<Segment> → 每个所选格式写一份
           → （开启翻译时）translate → <stem>.<lang>.<ext>
```

### 3.1 和现有管线的关系

| 阶段 | 字幕命中路径 | 未命中路径 |
|------|--------------|------------|
| 音频抽取（`extract_wav`） | 不跑 | 跑（输入是 yt-dlp 下回的音频） |
| audio.cpp 识别 | 不跑 | 跑 |
| `subtitle::postprocess` | 不跑（平台字幕已经切好句） | 跑 |
| 写出 | `subtitle::write`（同一套 writer） | 同左 |
| 翻译 | **按设置跑**（复用共享的翻译写出函数，§5.6） | 按设置跑 |
| 事件 | `url_import_*` | 导入阶段发 `url_import_*`，入队后发 `transcribe_*` |

> 字幕命中路径**不经过** `start_transcription`。它没有 WAV、没有模型、没有 plan，
> 硬塞进去会污染那条命令的语义：`TranscribeRequest.path` 是音频，字幕命中时它不存在。

---

## 4. 探测与语言匹配

### 4.1 Resolve：一次调用拿全信息

```
yt-dlp --ignore-config --no-playlist --newline --no-progress
       --skip-download --list-subs --print-json
       [--cookies-from-browser <browser>]
       <url>
```

- 必须带 `--list-subs`：yt-dlp 的字幕提取是惰性的，只在 `writesubtitles` 或
  `listsubtitles` 打开时才执行。只带 `--print-json` 时，B 站的 `subtitles` 是 `{}`（§9.4）。
- stdout 先输出可读表格，最后一行是 info dict（§9.3）。**解析规则**：逐行
  `serde_json::from_str::<Value>`，取**第一个是对象且含字符串字段 `id`** 的行。
  不能只看「能被解析」：表格里像 `123` 这样的单元格本身就是合法 JSON。
- `--print-json` 是 yt-dlp 的隐藏别名，官方选项是 `-j`。`-j` 隐含 quiet，可能吞掉
  `--list-subs` 的表格（我们并不需要表格），也可能影响字幕的惰性提取。哪个组合可用，
  **由 U0-9 复测决定**，结果固化成常量，并用单测钉住 argv。
- stderr 同时排空并保留一个环形缓冲：用于错误分类（§5.9）和 `login_hint` 检测（§4.5）。

### 4.2 语言匹配（严格白名单，输出唯一一条）

B 站字幕全部挂在 `subtitles` 下（不在 `automatic_captions`），有两类：

- **人工 CC 字幕**：`zh-CN` / `zh-Hans` / `zh-Hant` / `zh-HK` / `en-US` / `ja` / `ko` / …（§9.1）
- **AI 字幕**：`ai-zh`（AI 语音识别），以及 `ai-en` / `ai-ja` / … 等（**推断**为由
  `ai-zh` 机翻而来，U0-10 确认）。新视频最常见的反而是这一类。

`danmaku`（xml 弹幕）不是字幕，永远排除。

```rust
// site/lang.rs —— 纯函数，重点单测
pub enum SubKind { Human, Ai }
pub struct Pick { pub tag: String, pub kind: SubKind, pub source_lang: &'static str }
pub fn select(setting: OutputLanguage, media: &SiteMedia) -> Option<Pick>;
```

| 设置值 | 候选（按优先级，**显式枚举**，`match` 穷尽，不写 `_`） | `source_lang`（喂给翻译） |
|--------|------------------------------------------------------|---------------------------|
| `auto` | 空集 → `None` → 走转录 | — |
| `zh` | `zh-Hans`、`zh-CN`、`zh`，然后 `ai-zh` | `zh` |
| `zh-tw` | `zh-Hant`、`zh-TW`、`zh-HK` | `zh-tw` |
| `en` | `en`、`en-US`、`en-GB` | `en` |
| `ja` | `ja`、`ja-JP` | `ja` |
| `ko` | `ko`、`ko-KR` | `ko` |
| `es` | `es`、`es-ES`、`es-419` | `es` |
| `de` | `de`、`de-DE` | `de` |
| `fr` | `fr`、`fr-FR` | `fr` |
| `pt` | `pt`、`pt-BR`、`pt-PT` | `pt` |

规则（全部是踩过的坑或产品承诺）：

1. **`zh` 与 `zh-tw` 互不回落**：宁可降级去转录，也不交付一个用了繁体字的「简体」
   文件（`translate/language.rs` 里同样区分 `zh` 和 `zh-Hant`）。`ai-zh` 是简体，
   所以只出现在 `zh` 一行。
2. **人工轨优先于 AI 轨**：同一行内，所有人工标签都排在 `ai-*` 之前。
3. **只接受与原声同语言的 AI 轨**：`ai-zh` 接受；`ai-en` / `ai-ja` 等推断为机翻，
   **不接受**。如果 U0-10 证明 `ai-<x>` 在 `<x>` 是原声语言的视频上就是识别结果，
   再按「原声语言」放开（这需要判断原声语言，首版不做）。
4. **auto 不取字幕**：`auto` 的语义是「让引擎自己识别」，而取字幕必须先说清要哪种语言。
   把 `auto` 解释成「平台提供的第一条字幕」，会把机翻轨当成原文。
5. 选择结果只有**一条**。下载时 `--sub-langs` 只传这一个标签。yt-dlp 把这个值当正则，
   所以标签需要 `regex::escape`，例如 `zh-Hans` 中的 `-` 安全，`.` 就要转义。

### 4.3 「有没有字幕」的判定

- 走 `data` 的情况：`select()` 命中，并且 `parse_srt(data)` 产出至少一个 cue，即为命中。
- 走下载的情况：不要看退出码（无匹配时为 **0**，§9.5），也不要解析 stdout 文案。
  判定只有一条：**下载目录下是否存在 `<id>.<pick.tag>.srt` 且非空**。
- 以上都不满足 → 降级。

### 4.4 多 P

带 `--no-playlist` 且 URL 没有 `?p=` 时，B 站提取器**静默返回 P1**，并把 id 改成
`<BV>_p1`、给标题追加 ` p01 <分P名>`（读源码所得，U0-9 实测确认）。规则：

- URL 带 `p=N` → 正常处理该分 P（id 为 `<BV>_pN`）。
- URL 没有 `p`，但 info dict 的 `id` 匹配 `_p\d+$` → 报错「这是多 P 视频，请粘贴具体分 P 的链接」。
  这样既不悄悄只转 P1，也不悄悄展开成几十个任务。

### 4.5 登录与 cookies

| 现象 | 来源 | 处理 |
|------|------|------|
| Resolve 报 `HTTP Error 412` | §9.6 | 失败，状态 `URL_IMPORT_NEEDS_COOKIES`。行内显示「需要登录 B 站」，并给一个浏览器选择器（Chrome / Edge / Firefox / Safari）。选定后写入设置，**重试该行** |
| stderr 出现 `Subtitles are only available when logged in` | §9.12（yt-dlp 只发 warning，字幕列表为空） | **不失败**：照常降级去转录，但 `Queued` 结果带 `login_hint: true`，UI 提示「登录后可直接获取平台字幕」 |

- 检测靠匹配 stderr 文本。这是 §4.3「不解析文案」的**唯一例外**，理由是 yt-dlp 不做
  本地化。这两条匹配串作为常量集中定义，并配 fixture 单测。
- 隐私：设置项默认「未配置」，并附说明「会读取所选浏览器中 bilibili.com 的 cookie，
  只用于本机请求」。用 Safari 时 macOS 需要给 MonoSub 授予「完全磁盘访问权限」；
  用 Chrome / Edge 时会弹出钥匙串授权。这两种情况的文案都要写明。

---

## 5. 实现方案

### 5.1 新模块：`src-tauri/src/site/`

```
src-tauri/src/site/
├── mod.rs        # UrlImportRequest → job；事件；IMPORT_TASKS；取消；并发上限
├── url.rs        # parse_input：切分、规范化、去重、白名单（纯函数）
├── ytdlp.rs      # 二进制解析、常量参数、build_argv（纯函数）、spawn + 按行读 + stderr 环形缓冲
├── resolve.rs    # info dict → SiteMedia；多 P 判定（纯函数）
├── lang.rs       # select(OutputLanguage, &SiteMedia) -> Option<Pick>（纯函数）
├── srt.rs        # SRT 文本 → Vec<subtitle::Segment>（纯函数）
├── naming.rs     # 标题清洗 + 输出路径推导（纯函数）
├── progress.rs   # 下载进度模板行解析（纯函数）
└── error.rs      # ImportError + stderr 分类 → 用户可读状态
```

与 `engine/`、`translate/` 平级。`engine/cli.rs::resolve_bundled_binary` 已是
`pub(crate)`，直接复用（新增 `MONOSUB_YTDLP_PATH` 覆盖），**不复制**路径解析规则。
stdout/stderr 的读循环优先从 `engine/cli.rs` 抽成公用函数；抽不动时再照抄一份。

### 5.2 前端交互

Toolbar 在「添加文件」旁新增「从链接添加」（`Link` 图标，`data-testid="toolbar-import-url"`），
弹出面板：

```
┌─────────────────────────────────────────────┐
│ 从 B 站链接导入                              │
│ ┌─────────────────────────────────────────┐ │
│ │ https://www.bilibili.com/video/BV1xx    │ │
│ │ https://b23.tv/xxxx                     │ │
│ └─────────────────────────────────────────┘ │
│ 一行一个，或用空格分隔                        │
│ 字幕语言：中文（跟随设置）                    │
│ 下载到：~/Downloads/MonoSub（跟随设置）        │
│ [☑ 转录完成后自动开始]                        │
│                            [取消] [导入]     │
└─────────────────────────────────────────────┘
```

- 语言和输出目录只显示、不在面板里改，避免出现第二个真值来源；文案注明「在设置中修改」。
- 点「导入」：先 `parse_url_input`，非法项在面板内逐条标红；合法项逐条调用
  `start_url_import`，然后关闭面板。
- **队列表示**：导入中的 URL 存在独立的 store 切片 `urlImports: Record<taskId, UrlImportState>`
  （非持久化），在 `FileList` 顶部用新组件 `UrlImportItem` 渲染（视觉上沿用 `FileItem` 的骨架）。
  它们**不进** `files`，因此不参与「全部开始」和批处理计数。
  - 解析中：`Loader2` + 「获取信息…」
  - 下载音频：进度条（总量未知时显示不确定进度）
  - 字幕命中：「已获取字幕（中文 · 人工 / AI）」+「在 Finder 中显示」
  - 降级入队：从 `urlImports` 移除，走既有 `fileImport` 入队（§5.3），变成普通 pending 行
  - 失败：错误文案 +「重试」；需要登录时显示浏览器选择器
- 转录阶段复用现有 `steps.*` 与 `transcribe_progress`，**不新增第二套进度组件**。

### 5.3 降级路径（无字幕 → 转录）

```
yt-dlp <常量参数> -f "ba[ext=m4a]/ba/b"
       --progress-template "download:PROG %(progress.downloaded_bytes)s %(progress.total_bytes)s %(progress.total_bytes_estimate)s %(progress.status)s"
       --ffmpeg-location <bundled ffmpeg>
       [--cookies-from-browser <browser>]
       --paths home:<输出目录>
       --paths temp:<app_cache_dir>/url-import/<id>
       -o "<sanitized title> [<id>].%(ext)s"
       <url>
```

- `ba[ext=m4a]/ba/b`：B 站实测直接给出 m4a（§9.8）。`m4a` 已在 `AUDIO_EXTENSIONS` 里，
  缩略图走既有的音频图标分支。`--ffmpeg-location` 只是保险：`b` 回退到音视频分离的
  格式时需要合并。
- **媒体写在用户目录，和字幕同一个目录**：`--paths home:` 指向 §5.4 的输出目录，
  `-o` 用的是与字幕完全相同的 stem（`<sanitized title> [<id>]`，见 §5.5），所以成品是
  `~/Downloads/MonoSub/<title> [<id>].m4a`，旁边就是同名 `.srt`。
  一个 URL 导入的产物是一个「视频 + 字幕」，把它们分到两个目录（缓存 + 输出）会让用户
  删缓存时顺手丢掉他以为已经下载好的素材。
- **缓存目录只放中间态**：`--paths temp:` 指向 `<app_cache_dir>/url-import/<id>/`，
  只有未完成的 `.part` 和 `.ytdl` 这类文件落在这里。yt-dlp 自己在下载完成后把成品从
  temp **移动**到 home（实测日志 `[MoveFiles] Moving file … to …`），所以成品只存在一份，
  不需要我们再搬一次，也不会出现「缓存里一份、用户目录一份」。
- `-o` 必须是**相对路径**：实测绝对路径的 `-o` 会让 yt-dlp 完全忽略 `--paths`。
  `--paths` 的值是字面路径（实测 `%(id)s` 不会被替换），`-o` 里的 `%` 则按模板规则处理，
  所以标题里本来就有的 `%` 要写成 `%%`，否则会被当成模板字段（`site::ytdlp::media_output_template`）。
- **复用**：先查输出目录里本视频的成品 `<stem>.<ext>`（非空、排除 `.part` / `.ytdl` /
  `.info.json` / `.json` / `.srt`），命中就直接入队、不再调用 yt-dlp。匹配按 `<stem>.` 前缀
  而不是 `Path::file_stem`：标题本身可能带点（`v1.2 final [BV1]`）。
- 下载完成后发 `url_import_complete { outcome: Queued { path, title, duration, login_hint } }`。
  前端调用**既有**的 `fileImport` 入队流程（`get_video_metadata` + `generate_thumbnail`，
  这样 `size` 等字段的来源与本地文件一致），只额外设置两个字段：
  - `name` = `<sanitized title> [<id>].<ext>`（§5.5）
  - `outputDir` = URL 导入的输出目录（`VideoFile` 新增的可选字段，§5.4）
  - `diarization` = `settings.defaultDiarization`（与本地导入一致）
- 自动开始：面板勾选了「自动开始」（默认勾选）且当前没有批处理在跑时，前端调用
  `processQueueBatch()`。这是纯前端行为，后端请求里**没有** `auto_start`。

### 5.4 输出目录与落盘

**输出目录**（两条路径共用，由前端算好后传入；**媒体与字幕都写到这里**）：

| `storageMode` | URL 导入的输出目录 |
|---------------|--------------------|
| `same` | `settings.urlImportOutputDir`（默认 `~/Downloads/MonoSub`，不存在时自动创建） |
| `custom` | `customStoragePath` |

- 这个目录同时也是下载目录：降级路径的媒体直接用这个目录（§5.3），文件名与字幕同 stem，
  两者就放在一起。设置项文案因此是「下载/保存目录」而不是「字幕保存位置」。
- 降级路径：`VideoFile.outputDir` 存在时，`getOutputPath` 用它替代 `file.path` 的目录。
  这是前端唯一的改动，`TranscribeRequest` **不变**。识别产物也落在这个目录。（此时
  媒体已经是本目录里的真实文件，`outputDir` 只是为了与 `storageMode='custom'` 保持一致。）
- 字幕命中路径：

```
SRT 文本 → site::srt::parse → Vec<subtitle::Segment>
        → 对每个所选格式：subtitle::write(output_path_for(primary, i, fmt), fmt)
        → （开启翻译时）共享翻译写出函数（§5.6）
```

- 复用 `subtitle::write` 全套 writer（srt/vtt/txt/md/json），时间轴与文本**原样**保留。
- **不跑** `postprocess`：平台字幕已经切好句，按 50 字 / 10 秒重新切会破坏 UP 主的时间轴。
- 多格式沿用 `output_path_for` 的规则（第一个格式用 primary 路径，其余只换扩展名）。
  这个函数要从 `commands/transcribe.rs` 移到 `subtitle/` 下作为公用函数，以免两处规则漂移。

### 5.5 文件名

- 形如 `<sanitized title> [<id>].<ext>`，与 yt-dlp 默认模板同构。
- 清洗规则（`site::naming`，纯函数）：
  - `/ \ : * ? " < > |` 和控制字符替换为 `_`；
  - 合并连续空白；去掉首尾的空格和 `.`；
  - 按**字符**截断到 120（UTF-8 字节数保持在 255 以内）；
  - 清洗后为空则只用 `[<id>]`。
- 已存在同名文件：与本地文件规则一致。`skipExistingSubtitle` 开启则跳过（行状态为
  「已存在，已跳过」），否则覆盖。因为文件名带 id，「同名」只可能是同一个视频。

### 5.6 翻译（两条路径一致）

- 把 `commands/transcribe.rs::translate_and_write` 中与 Tauri 无关的部分抽成共享函数，
  入参是 `(segments, source_tag, &TranslationPlan, output, formats, on_progress)`，返回写出的路径。
  转录路径和 URL 字幕路径都调它；`resolve_translation`（模型是否已安装等检查）同样共用。
- 字幕命中路径的源语言是**确定的**：`Pick.source_lang`。因此 `should_translate` 能准确判断
  「源语言等于目标语言就跳过」，不需要按文字脚本猜测语言。
- 失败语义不变：翻译是增强，失败只记 warning；原字幕已经写好，这一行仍然算成功。
  翻译模型未安装时，在 Resolve **之前**就返回 `MODEL_NOT_INSTALLED:<id>`，与转录路径一致。
- 进度：`url_import_progress { stage: "translating" }`。

### 5.7 与转录命令的衔接

降级路径**不修改** `TranscribeRequest`。音频在入队前已经下载好，`path` 永远指向真实文件，
所以 `extract_wav` / `get_video_metadata` / `generate_thumbnail` 都不需要改。

> 备选方案 B（未采用）：在 `transcribe_file` 里加一步「若 path 是 URL 则先下载」。
> 否决原因：这会把网络 IO 塞进离线转录命令，使它在无网环境下语义分裂，`TaskParams`
> 还要多带一个「延迟下载」状态。

### 5.8 取消

- 每个 job 注册进 `IMPORT_TASKS: HashMap<task_id, TaskEntry>`，与转录的 `TASKS` 分开。
  并发上限用 `Semaphore(3)`，与转录用的信号量相互独立。
- `cancel_url_import(task_id)`：abort 并等待 tokio task 结束；yt-dlp 子进程随
  `kill_on_drop` 被杀；`Drop` guard 清理 job 的临时目录。
- **只作用于导入阶段**。`Queued` 之后这个 job 已经结束，后续取消就是 `cancel_transcription`
  或从队列移除文件。已完成的媒体**不删**（它在输出目录里，缓存里本来就没有成品），
  下次导入同一 URL 直接复用；`.part` / `.ytdl` / `.temp` 则清理掉。
- 清理只扫「临时目录 + 缓存目录」，**从不扫输出目录**：用户拿到的成品不会因为一次取消而消失。

### 5.9 命令与事件（IPC）

| 命令 | 入参 | 返回 |
|------|------|------|
| `parse_url_input` | `text: String` | `Vec<ParsedUrl>`，`ParsedUrl = { input, url?, error? }`；`error ∈ {unsupported_site, unsupported_kind, invalid}` |
| `start_url_import` | `request: UrlImportRequest` | `()`：注册后立即返回，结果走事件 |
| `cancel_url_import` | `task_id` | `()` |

```rust
struct UrlImportRequest {
    task_id: String,
    url: String,                       // 已经过 parse_url_input 规范化
    language: String,                  // settings.outputLanguage
    formats: String,                   // settings.outputFormat，例如 "srt,txt"
    output_dir: String,                // 前端按 §5.4 算好
    skip_existing: bool,               // settings.skipExistingSubtitle
    cookies_browser: Option<String>,   // None = 未配置；取值 chrome/edge/firefox/safari
    translate_enabled: Option<bool>,   // 以下三项与 TranscribeRequest 同名同义
    target_language: Option<String>,
    translation_model: Option<String>,
}
```

事件（独立命名空间）：

| 事件 | 载荷 | 用途 |
|------|------|------|
| `url_import_progress` | `{task_id, stage, progress?}`，`stage ∈ {resolving, fetching_subtitles, downloading_audio, writing, translating}` | 行内进度；`progress` 缺省表示不确定进度 |
| `url_import_complete` | `{task_id, outcome}`，`outcome = Subtitle { paths, translated_paths, tag, kind, skipped } \| Queued { path, title, duration, login_hint }` | 该 URL 收敛 |
| `url_import_error` | `{task_id, status}` | 该 URL 失败。`status` 是 sentinel（`URL_IMPORT_NEEDS_COOKIES`、`MODEL_NOT_INSTALLED:<id>`）、i18n key 或原样消息 |

错误分类（`site::error`）按 yt-dlp 的 `ERROR:` 行做匹配，与 §4.5 同理，属于「解析文案」的例外：

| 类别 | 依据 | 可降级？ |
|------|------|----------|
| `NeedsCookies` | `HTTP Error 412` | 否（引导登录） |
| `Unavailable` | 私有、已删除、地区限制 | 否 |
| `MultiPart` | §4.4 | 否 |
| `Network` | 连接 / 超时 | 否（可重试） |
| `RateLimited` | `HTTP Error 429` | 退避重试 1–2 次；字幕下载阶段仍失败则降级去转录 |
| `BinaryMissing` | spawn 失败 | 否 |

`steps.*` 与 `transcribe_*` 的冻结契约**一律不动**；**不要**为 URL 路径往 `STEP_NAMES` 里加条目。

---

## 6. yt-dlp 的打包与调用

### 6.1 二进制

- 新增 `scripts/build/fetch-ytdlp.sh`：build 时下载官方 release 里**对应平台**的预编译文件，
  钉版本 + SHA256，落到 `src-tauri/resources/bin/yt-dlp-<target-triple>`，由
  `prepare-resources.sh` 调度。
  - macOS：`yt-dlp_macos`（Universal，arm64 和 x86_64 共用同一个文件）。
  - 当前钉选：**2026.08.19**，`yt-dlp_macos` SHA256
    `0f192b7ec147ab6288885d6351d9ab67367640029b4377576ef46dd79cf7b202`，
    37,146,048 字节（约 35 MB，取自 release 页的 `SHA2-256SUMS` 与 HTTP 头）。
- 直接作为普通可执行文件使用，**与其他 bin 一样**由 `sign-app.sh` 签名。
- 许可：随包附上 release 发布的 `LICENSE`（Unlicense）和 `THIRD_PARTY_LICENSES.txt`，
  放在 `resources/licenses/`，并更新 `THIRD-PARTY-NOTICES.md`。
- **必须内置**：B 站的提取逻辑随版本变化很快，依赖用户自装的 yt-dlp 会造成「版本不对」
  这类无从排查的问题。保留 `MONOSUB_YTDLP_PATH` 覆盖，方便调试和热修。首版不做自更新。

### 6.2 调用约定（硬性）

每次调用都带的常量参数（集中定义在 `site/ytdlp.rs`，单测钉住）：

| 参数 | 原因 |
|------|------|
| `--ignore-config` | 用户的 `~/.config/yt-dlp/config` 可能改输出模板、格式、字幕选项，会让基于文件的判定失效 |
| `--no-playlist` | 一个 URL 只处理一个视频 |
| `--newline` | 否则进度用 `\r` 刷新，按行读会读成一整行 |
| `--no-progress` | Resolve / 字幕阶段不需要进度；音频阶段换成 `--progress-template` |
| `--ffmpeg-location <bundled>` | 只有需要合并时才会用到。显式指定可以避免误用系统上的 ffmpeg |

- **不要**加 `--no-warnings`：412、需登录、地区限制等需要上报的信息就在警告里。
- 音频阶段额外带一对 `--paths home:<输出目录>` / `--paths temp:<缓存>` 和相对路径的
  `-o`（§5.3）。两个值都是常量形状、单 argv 元素；`home` 不能省，否则成品会落回缓存目录。
- stdout 按行读，同时排空 stderr；子进程 `kill_on_drop(true)`；`stdin` 关闭。
- URL 作为**单一 argv 元素**传入，不经 shell。
- 首版不需要 JS 运行时，也不需要代理（这两项都是 YouTube 的需求，见 §10）。

---

## 7. 取舍与开放问题

1. **`auto` 语言直接走转录**：更保守、更可解释（§4.2 规则 4）。
2. **平台字幕的质量档位**：AI 字幕（`ai-zh`）是 B 站自己的 ASR，与本应用的本地识别不是同一个
   质量档位。提供「优先本地识别」开关（默认关闭）：开启后跳过 Select，直接降级去转录。
   这也是需要 `words.json` 边车、统一术语的用户的出口。
3. **`words.json` 边车**：字幕命中路径没有词级数据，不写边车。
4. **`ai-<x>` 是否放开**：见 §4.2 规则 3，取决于 U0-10。
5. **长视频**：超过 1 小时的视频，字幕和音频的体积以及 writer 耗时待实测（U5-2）。

---

## 8. 分阶段

| 阶段 | 内容 | 出口条件 |
|------|------|----------|
| U0 | v1 PoC（§9）+ **用随包二进制复测**（U0-9 ~ U0-11） | 所有「待复测」项都有结论，常量固化 |
| U1 | 打包 + `site/` 纯函数（url/lang/srt/naming/progress/error）+ ytdlp spawn | `cargo test` 覆盖全部纯函数 |
| U2 | Resolve + Select + Fetch + 落盘 + 翻译（字幕命中路径）+ 输出目录设置 | B 站人工字幕视频和 `ai-zh` 视频各一个，端到端可用 |
| U3 | 降级路径：音频下载 + 复用 + 入队 + 取消 + 多 P + 登录提示 | 无字幕视频跑通完整转录 |
| U4 | 前端面板 + `UrlImportItem` + cookies 引导 + i18n | `bun run check:i18n` 通过；pilot smoke 增加离线用例 |
| U5 | 文档、真实巡检、可选项 | §9.10 的待测项全部收敛 |
| (后续) | YouTube（§10） | 单独立项 |

---

## 9. 实测记录（v1：Python 版 yt-dlp 2026.02.21，macOS arm64）

> 用例：B 站 `BV1GJ411x7h7`（老视频，有大量人工 CC 字幕；**不代表**新视频常见的
> `ai-zh` 情形）。YouTube 部分移到 §10.2 作为参考。

### 9.1 B 站字幕列表

```
$ yt-dlp --list-subs --skip-download --cookies-from-browser chrome <bili>
Language Formats
danmaku  xml
zh-CN    srt
zh-Hans  srt
zh-Hant  srt
zh-HK    srt
en-US    srt
ja       srt
ko       srt
de-DE    srt
ru       srt
iw       srt
ca       srt
ase      srt
```

- 该视频只有人工 CC 字幕。~~B 站没有自动生成字幕~~ **更正**：B 站的 AI 字幕以 `ai-zh` /
  `ai-en` 等标签出现在 `subtitles` 下（第三方工具与 yt-dlp 源码均可佐证），U0-10 实测。
- `danmaku` 是 xml 弹幕，必须排除。

### 9.3 `--list-subs --print-json` 同时使用

先输出可读表格，最后一行是完整 info dict。**待复测**（U0-9）：随包二进制下的行为，
以及 `-j` 能否替代。

### 9.4 只用 `--print-json`（不带 `--list-subs`）

B 站的 `subtitles` 为 `{}`。原因是字幕提取的惰性条件（§4.1）。

### 9.5 无匹配语言的行为

```
$ yt-dlp --write-subs --sub-langs "xx-QQ" --write-info-json …
[info] There are no subtitles for the requested languages
$ echo $?
0
```

→ **退出码 0**，只写出 info json。判定必须看文件，不能看退出码。

### 9.6 B 站无 cookie

```
ERROR: [BiliBili] 1GJ411x7h7: Unable to download JSON metadata: HTTP Error 412: Precondition Failed
```

连 Resolve 都失败；加 `--cookies-from-browser chrome` 后立即成功。
（shell 中 `echo $?` 因管道显示 0，yt-dlp 自身是以非零码退出的。）**待复测**（U0-9）：
412 是否稳定复现。yt-dlp 会自动设置 `buvid3` 等 cookie，412 也可能只是间歇性风控。

### 9.8 降级下载（纯音频）

```
$ yt-dlp -f ba/b --newline --progress-template "download:PROG …" …
[download] Destination: /tmp/dl2/BV1GJ411x7h7/BV1GJ411x7h7.m4a
PROG 1024 5408198 NA downloading BV1GJ411x7h7
…
PROG 5408198 5408198 NA finished BV1GJ411x7h7
```

`total_bytes` 在该源上已知；其他情况可能只有 `total_bytes_estimate`，用 `NA` 兜底为不确定进度。

### 9.9 SRT 成品形态

标准 SRT（序号 / `HH:MM:SS,mmm --> …` / 正文 / 空行）。B 站原生就是 srt。

### 9.10 待实测（实现阶段补齐）

- U0-9：随包二进制（PATH 中无 ffmpeg / deno，`--ignore-config`）复测 §9.3 / §9.6 / §9.8 / §9.11 / §4.4。
- U0-10：`ai-zh` 视频，在登录和未登录两种状态下各测一次；`ai-en` 等的真实含义。
- `b23.tv` 短链能否被 yt-dlp 直接跟随（generic 提取器跟随重定向）；不能的话由
  `parse_url_input` 先做一次 HEAD 解析。
- 超过 1 小时的视频：字幕和音频体积，writer 耗时。

### 9.11 字幕内容内联在 info dict 中（读源码，待复测）

`yt_dlp/extractor/bilibili.py::_get_subtitles` 对每条字幕执行
`{'ext': 'srt', 'data': self.json2srt(...)}`：SRT 文本直接放进 `data`，没有 `url`。
`-j` 的 `sanitize_info` 不会过滤掉 `data`。所以 Resolve 返回的 info dict 理应**已经包含**
字幕全文，字幕命中路径只需一次网络调用。U0-9 确认；如果拿不到，就回退到
`--load-info-json` 下载这一条（§3 C）。

### 9.12 未登录时的字幕（读源码 + 第三方记录，待复测）

`_get_subtitles` 在 `data.need_login_subtitle` 为真时只调用 `report_warning(
'Subtitles are only available when logged in. …')`，字幕列表为空，**不报错**。
第三方工具（2026-06）的记录是：匿名请求在所有测试过的视频上都返回空字幕列表。
→ 不登录时实际上**永远**走转录；这正是 §4.5 提示的依据。

### 9.13 `--paths home:/temp:` 分离成品与中间态（随包 2026.08.19 实测）

用一个本地 HTTP 源（`python3 -m http.server`）测完，与 B 站无关的机制都直接适用：

```
$ yt-dlp -f ba/b -P home:/out -P temp:/cache -o "Title [BV1].%(ext)s" <url>
[download] Destination: /cache/Title [BV1].m4a.part      ← 下载中只写在 temp
[MoveFiles] Moving file "/cache/Title [BV1].m4a" to "/out/Title [BV1].m4a"
```

四条实测结论，都是 §5.3 那条命令的前提：

1. **成品只存在一份**：yt-dlp 自己在下载完成后从 `temp` 移动到 `home`；不需要我们再搬一次，
   也不会出现“缓存一份 + 用户目录一份”。
2. **`-o` 必须是相对路径**：绝对路径的 `-o` 会让 yt-dlp **完全忽略** `--paths`，
   `.part` 直接写进目标目录。
3. **`--paths` 的值是字面路径**：写成 `/out/%(id)s` 会真的建一个叫 `%(id)s` 的目录；
   而 `-o` 里的 `%` 照模板规则处理，标题里的 `%` 需要写成 `%%`。
4. **开工前会先查已有文件**：同名的零字节文件也会被当成“已下载”（`has already been
   downloaded`，退码 0），所以复用判定必须自己检查 `len > 0`，不能依赖 yt-dlp。

---

## 10. 推迟：YouTube（已知约束，供后续立项）

### 10.1 必须解决的问题

1. **JS 运行时**：yt-dlp 自 2025.11.12 起，完整支持 YouTube 需要外部 JS 运行时
   （deno ≥ 2.3 / node ≥ 22 / quickjs）。v1 PoC 能成功，可能只是因为本机有
   `/opt/homebrew/bin/deno`。需要决定是随包附带运行时，还是接受降级。
2. **机翻轨识别**：对 ASR 轨，yt-dlp 会把每个翻译目标语言以**裸标签**放进
   `automatic_captions`（附带 `tlang` 参数）。真正的原声轨只有 `<lang>-orig`。
   规则：只有 `X-orig` 存在时，自动轨 `X` 才可接受。人工轨的翻译命名为
   `<target>-<source>`（例：`en-zh-CN` = 从 `zh-CN` 翻成英文；v1 稿的写法反了）。
3. **人工 / 自动分开选**：同时传 `--write-subs` 和 `--write-auto-subs` 时，yt-dlp 会把两份
   列表合并（同一标签人工优先）。请求的标签没有人工轨时，会悄悄拿到机翻自动轨。
   必须在 Rust 里先选定 `(tag, Human|Auto)`，再只开对应的那个开关。§4.2 的 `Pick` 已经预留了 `kind`。
4. **格式**：YouTube 原生提供 `srt`，用 `--sub-format srt/best` 即可，不需要 `--convert-subs`。
5. **代理**：中国大陆用户访问 YouTube 需要代理，需要新增 `--proxy` 设置项。
6. **音频格式**：`ba` 常常是 `webm` / `opus`，需要先用 `ba[ext=m4a]`，或者把它们加进 `AUDIO_EXTENSIONS`。
7. **URL 形态**：`youtu.be`、`/shorts/`、`/live/`、`music.youtube.com`。
8. **429**：通配语言会触发（§10.2）；选出唯一一条之后风险已经很低。

### 10.2 v1 实测（Python 版 yt-dlp，PATH 有 deno）

- `aircAruvnKk`：人工字幕 30 条（`vtt, srt, ttml, srv3, srv2, srv1, json3`），
  自动字幕 4837 条（`lang × src` 的机翻组合，外加裸标签）。
- `--list-subs --print-json` 同样成立；只用 `--print-json` 时的结果可能与之不一致。
- `--sub-langs "en.*"` 触发了 34 条字幕的下载，第一条成功后立即 `HTTP Error 429`。

---

## 11. v2 变更摘要（相对 v1 评审）

- 范围收窄为 B 站；YouTube 的约束全部移到 §10。
- yt-dlp 使用 build 时下载的官方平台二进制（钉住 2026.08.19 + SHA256），不考虑它的内部打包方式。
- 去掉 `--convert-subs`，不再依赖 ffmpeg 转换；字幕优先取 info dict 内联的 `data`。
- 语言选择改为输出唯一 `Pick { tag, kind }` 的 `select()`；新增 `ai-zh` 规则；各语言分行；去掉 `<src>` 占位。
- info dict 解析规则收紧（要求是对象且含 `id`）；增加 `--ignore-config` 和 `--ffmpeg-location`。
- 输出目录：新增 `urlImportOutputDir`（默认 `~/Downloads/MonoSub`），不再把字幕写进缓存目录。
- 文件名统一为 `<title> [<id>]`，并定义了清洗规则；同名时的处理与 `skipExistingSubtitle` 对齐。
- 字幕命中路径也翻译，与转录路径共用同一个翻译写出函数。
- 统一「入队前下载音频」的说法；`start_url_import` 立即返回、结果走事件；去掉 `auto_start`
  字段和 `resolve_url_sites`；新增 `parse_url_input`（URL 解析放在 Rust）。
- 登录：412 → `URL_IMPORT_NEEDS_COOKIES` 引导选择浏览器；「需登录才有字幕」→ 照常转录并提示。
- 多 P 判定改为看 id 的 `_pN` 后缀；取消只作用于导入阶段。
- 队列表示：独立的 `urlImports` 切片和 `UrlImportItem` 组件，不进 `files`。
