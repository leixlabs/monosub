# URL 导入（B 站）— 任务清单

> 设计见 [url-import-design.md](./url-import-design.md)（v2，下称 spec），实测记录见 spec §9。
> 首版**只做 B 站**；YouTube 推迟（spec §10），本清单不含 YouTube 任务。
> 约定：每个任务独立可提交；「依赖」列出必须先完成的任务；完成时勾选，并在「状态」中写清证据。
> 粒度：S ≤ 半天，M ≈ 1 天，L ≈ 2–3 天。

---

## 0. 决策基线

| 项 | 决定 | spec |
|----|------|------|
| 站点 | 仅 B 站（`www./m.bilibili.com/video/BV…|av…`、`b23.tv`）；拒绝番剧、课程、列表页 | §1, §2 |
| 二进制 | build 时下载官方 `yt-dlp_macos`，钉住 2026.08.19 + SHA256，放到 `resources/bin/yt-dlp-<triple>`；可用 `MONOSUB_YTDLP_PATH` 覆盖 | §6.1 |
| 常量参数 | `--ignore-config --no-playlist --newline --no-progress --ffmpeg-location <bundled>` | §6.2 |
| 探测 | 一次 Resolve；info dict 取「第一个是对象且含 `id` 的 JSON 行」 | §4.1 |
| 语言 | `select()` 返回唯一 `Pick {tag, kind}`；人工轨优先于 `ai-zh`；`zh` / `zh-tw` 互不回落；`auto` 直接降级 | §4.2 |
| 字幕内容 | 优先用 info dict 内联的 `data`；否则 `--load-info-json` 单独下载这一条；不用 `--convert-subs` | §2, §9.11 |
| 命中判定 | 解析出非空 SRT，或磁盘上有非空的 `<id>.<tag>.srt`；**不看退出码** | §4.3 |
| 降级 | `-f ba[ext=m4a]/ba/b` 下载到 `<app_cache_dir>/url-import/<id>/`，入队前下载完 | §5.3 |
| 输出 | `urlImportOutputDir`（默认 `~/Downloads/MonoSub`），`custom` 模式用自定义目录；文件名 `<title> [<id>]` | §5.4, §5.5 |
| 翻译 | 两条路径一致，共用同一个翻译写出函数 | §5.6 |
| 登录 | 默认未配置；412 → `URL_IMPORT_NEEDS_COOKIES` 引导选择浏览器；「需登录才有字幕」→ 照常转录并提示 | §4.5 |
| IPC | `parse_url_input` / `start_url_import`（立即返回）/ `cancel_url_import`；事件 `url_import_*`；`transcribe_*` 与 `STEP_NAMES` **一律不动** | §5.9 |

---

## Phase U0 — PoC

| ID | 任务 | 规模 | 依赖 | 验收 | 状态 |
|----|------|------|------|------|------|
| U0-1 | B 站字幕列表形态（含排除 `danmaku`） | S | — | 记入 spec | ✅ spec §9.1（人工 CC 字幕；AI 字幕见 U0-10） |
| U0-3 | 一次拿到 info dict 的调用方式 | M | — | 一条命令 | ✅ spec §9.3 / §9.4（Python 版；随包二进制由 U0-9 复测） |
| U0-4 | 无匹配语言时的退出码与落盘物 | S | — | 结论 | ✅ spec §9.5：退出码 0，只写出 `.info.json` |
| U0-5 | B 站无 cookie 时的行为 | S | — | 结论 | ✅ spec §9.6：412（是否稳定复现由 U0-9 复测） |
| U0-7 | 降级下载的形态与进度模板 | M | — | 命令行 + 输出格式 | ✅ spec §9.8 |
| U0-8 | SRT 成品形态 | S | — | 结论 | ✅ spec §9.9 |
| U0-9 | **用随包二进制复测**：PATH 中去掉 ffmpeg / deno，加 `--ignore-config`。确认：①`--list-subs --print-json` 与 `-j` 哪个组合能拿到含 `subtitles` 的 info dict（§4.1）；②`subtitles[].data` 是否内联了 SRT（§9.11）；③412 是否稳定复现；④`-f ba[ext=m4a]/ba/b` 的产物；⑤多 P 视频不带 `p` 时的 id / 标题形态（§4.4）；⑥`b23.tv` 能否被直接跟随 | M | U1-1 | 结论与命令记入 spec §9，常量据此固化 | ⬜ |
| U0-10 | `ai-zh` 视频：登录与未登录各测一次；确认 `ai-en` 等是否为机翻；确认 `need_login_subtitle` 的 warning 原文 | S | U1-1 | 记入 spec §9；据此确认或修改 §4.2 规则 3 | ⬜ |

> v1 的 U0-2 / U0-6（YouTube）移到 spec §10.2，不再列在这里。

---

## Phase U1 — 打包与纯逻辑

| ID | 任务 | 规模 | 依赖 | 验收 | 状态 |
|----|------|------|------|------|------|
| U1-1 | `scripts/build/fetch-ytdlp.sh`：下载 `yt-dlp_macos` 2026.08.19 → `src-tauri/resources/bin/yt-dlp-<triple>`，校验 SHA256 `0f192b7e…f7b202`，`chmod +x`；幂等；同时下载 `LICENSE` 与 `THIRD_PARTY_LICENSES.txt` | S | — | 重复运行无副作用；校验失败时退出码非零 | ⬜ |
| U1-2 | `prepare-resources.sh` 调用 U1-1（放在 llama.cpp 之后），更新头注释中的资源清单和 MANIFEST | S | U1-1 | 一条命令备齐全部资源 | ⬜ |
| U1-3 | 许可：`resources/licenses/yt-dlp-LICENSE` + `yt-dlp-THIRD_PARTY_LICENSES.txt`，`THIRD-PARTY-NOTICES.md` 增加一段 | S | U1-1 | 文件就位 | ⬜ |
| U1-4 | `site/ytdlp.rs`：`resolve_ytdlp_path` 复用 `resolve_bundled_binary` + `MONOSUB_YTDLP_PATH`；常量参数集中定义；纯函数 `build_argv(stage, &opts)` | S | U1-1 | 单测：每个阶段的 argv 都带全常量参数；不含 `--no-warnings` / `--convert-subs`；URL 是最后一个独立元素；`cookies_browser` 为 `None` 时不出现 `--cookies-from-browser` | ⬜ |
| U1-5 | `site/ytdlp.rs`：spawn + 按行读 stdout + 排空 stderr（环形缓冲）+ `kill_on_drop` + 关闭 stdin。优先把读循环从 `engine/cli.rs` 抽成公用函数 | M | U1-4 | 实跑能拿到 info dict 行；`engine` 原有测试仍全部通过 | ⬜ |
| U1-6 | `site/url.rs::parse_input`：按空格 / 换行 / 逗号切分；规范化（去掉跟踪参数，**保留 `p`**）；去重；白名单。错误类型 `unsupported_site` / `unsupported_kind`（`bangumi` / `cheese` / `space` / `favlist`）/ `invalid` | M | — | 单测：混合分隔符、重复项、`m.bilibili.com`、`av` 号、`?p=3`、带 `spm_id_from` 的 URL、`b23.tv`、YouTube 链接 → `unsupported_site`、番剧 → `unsupported_kind` | ⬜ |
| U1-7 | `site/lang.rs::select(OutputLanguage, &SiteMedia) -> Option<Pick>`，按 spec §4.2 的表实现 | S | — | 单测：`match` 穷尽、不写 `_`；`zh` 与 `zh-tw` 互不回落；`auto` → `None`；人工轨优先于 `ai-zh`；`ai-en` 不被 `en` 接受；`danmaku` 永不入选；输出唯一一条；`source_lang` 正确 | ⬜ |
| U1-8 | `site/srt.rs`：SRT 文本 → `Vec<subtitle::Segment>` | M | — | 单测：标准块、多行正文、末尾无空行、BOM、`\r\n`、空文件、超过 1 小时的时间码、非严格的序号、正文为空的块 | ⬜ |
| U1-9 | `site/naming.rs`：标题清洗 + `output_path(dir, title, id, ext)` | S | — | 单测：`/ \ : * ? " < > |`、控制字符、首尾的 `.` 和空格、120 字符截断不切断多字节字符、清洗后为空 → `[<id>]` | ⬜ |
| U1-10 | `site/progress.rs`：解析 `PROG <done> <total> <est> <status>` | S | — | 单测：`NA` → 不确定进度；优先用 total、其次 est；忽略非 `PROG` 行 | ⬜ |
| U1-11 | `site/error.rs`：`ImportError` + stderr 分类（`NeedsCookies` / `Unavailable` / `MultiPart` / `Network` / `RateLimited` / `BinaryMissing`）+ 检测「需登录才有字幕」的 warning；匹配串集中成常量 | S | — | 单测用 fixture stderr，每一类各一条；`URL_IMPORT_NEEDS_COOKIES` sentinel 有常量 | ⬜ |
| U1-12 | 把 `output_path_for` 从 `commands/transcribe.rs` 移到 `subtitle/` 作为公用函数 | S | — | 原有测试原样通过 | ⬜ |
| U1-13 | 把 `translate_and_write` 中与 Tauri 无关的部分抽成共享函数（spec §5.6）；`resolve_translation` 改为可共用 | M | — | 转录路径的行为不变（原有测试通过）；新函数可在没有 `AppHandle` 时测试写出路径 | ⬜ |

---

## Phase U2 — Resolve + 字幕命中路径（后端）

| ID | 任务 | 规模 | 依赖 | 验收 | 状态 |
|----|------|------|------|------|------|
| U2-1 | `site/resolve.rs`：Resolve 输出 → `SiteMedia { id, title, duration, subs: BTreeMap<tag, SubTrack{ext, data?}> }`；多 P 判定（URL 无 `p` 且 id 匹配 `_p\d+$` → `MultiPart`） | M | U1-5, U0-9 | 单测用 fixture：表格行 + info dict 行；像 `123` 这样的 JSON 行不会被误当成 info dict；`subtitles` 为 null 或缺失时不 panic；多 P 形态 | ⬜ |
| U2-2 | `start_url_import` 骨架：注册 `IMPORT_TASKS`、`Semaphore(3)`、立即返回、发 `url_import_progress`、收敛时发 complete / error；临时目录 `Drop` guard | M | U2-1 | 单测：任务表的注册与注销；Drop 后临时目录不存在 | ⬜ |
| U2-3 | Fetch：`pick.data` 非空就直接用；否则 `--load-info-json <tmp>/info.json --skip-download --write-subs --sub-langs <escape(tag)> --sub-format srt/best -o <tmp>/%(id)s.%(ext)s`，判定磁盘上是否有 `<id>.<tag>.srt` 且非空 | M | U1-7, U2-1 | 单测：argv 构造与判定函数；手测：人工字幕视频、`ai-zh` 视频各一个 | ⬜ |
| U2-4 | Materialize：`srt::parse` → 对每个格式调 `subtitle::write(output_path_for(...))`；处理 `skip_existing`；解析结果为空 → 降级 | M | U1-8, U1-9, U1-12, U2-3 | 单测：`srt,txt` 产出两个文件；文件名为 `<title> [<id>].srt`；`skip_existing` 时不写并标记 `skipped` | ⬜ |
| U2-5 | 翻译：请求开启翻译时，在 Resolve **之前**调用 `resolve_translation`（模型未安装 → `MODEL_NOT_INSTALLED:<id>`）；命中后用 `Pick.source_lang` 调共享翻译函数，写出 `<stem>.<lang>.<ext>` | M | U1-13, U2-4 | 单测：源语言等于目标语言时跳过；手测：`zh` 字幕翻成 `en` 后多出 `.en.srt` | ⬜ |
| U2-6 | 设置项 `urlImportOutputDir`（默认 `~/Downloads/MonoSub`，不存在时创建）；前端按 spec §5.4 算出 `output_dir` 传入；Settings 增加目录选择 | S | — | 单测（Rust）：目录不存在时创建；`bun run check` 通过 | ⬜ |
| U2-7 | `RateLimited`：退避重试 1–2 次，字幕阶段仍失败则降级 | S | U1-11, U2-3 | 单测：429 分类为可重试，耗尽后走降级而不是报错 | ⬜ |

---

## Phase U3 — 降级路径（音频 → 转录）

| ID | 任务 | 规模 | 依赖 | 验收 | 状态 |
|----|------|------|------|------|------|
| U3-1 | `download_audio`：`-f ba[ext=m4a]/ba/b` + `--progress-template`，下载到 `<app_cache_dir>/url-import/<id>/`，进度发 `downloading_audio` | M | U1-5, U1-10 | 手测：无字幕视频下载出 m4a，进度条在走 | ⬜ |
| U3-2 | 缓存复用：已有非空的 `<id>.*` 媒体文件就跳过下载（排除 `.part` / `.ytdl` / `.info.json` / `.json` / `.srt`） | S | U3-1 | 单测：用临时目录构造各种残留文件 | ⬜ |
| U3-3 | `Queued { path, title, duration, login_hint }`；`login_hint` 来自 U1-11 的 warning 检测 | S | U3-1, U1-11 | 单测：stderr 含该 warning 时 `login_hint=true` | ⬜ |
| U3-4 | 前端入队：收到 `Queued` 后走既有的 `fileImport`（metadata + thumbnail），设置 `name = <title> [<id>].<ext>` 和新增的可选字段 `VideoFile.outputDir`；`getOutputPath` 优先用 `outputDir` | S | U3-3, U2-6 | 手测：字幕写到 `~/Downloads/MonoSub/<title> [<id>].srt`，不写进缓存目录 | ⬜ |
| U3-5 | 自动开始：勾选了「自动开始」且当前没有批处理在跑 → `processQueueBatch()`（纯前端） | S | U3-4 | 手测 | ⬜ |
| U3-6 | `cancel_url_import`：abort 并等待 → yt-dlp 被杀 → 临时目录和 `.part` 被清理；已完成的缓存音频保留 | S | U2-2, U3-1 | 手测：下载中取消后 `pgrep yt-dlp` 为空，`.part` 不存在 | ⬜ |
| U3-7 | 「优先本地识别」开关：跳过 Select，直接降级（spec §7-2） | S | U3-1 | 单测：开关为真时不调用 `select` | ⬜ |

---

## Phase U4 — 前端

| ID | 任务 | 规模 | 依赖 | 验收 | 状态 |
|----|------|------|------|------|------|
| U4-1 | `src/lib/urlImport.ts`：调用 `parse_url_input` → 逐条 `start_url_import`；监听 `url_import_*`，更新 store | S | U1-6, U2-2 | `bun run check` 通过 | ⬜ |
| U4-2 | `useStore.ts`：非持久化切片 `urlImports: Record<taskId, UrlImportState>`；持久化的 settings 增加 `urlImportOutputDir`、`bilibiliCookiesBrowser`（默认 `''` = 未配置）、`preferLocalRecognition`。**需要**把 `PERSIST_VERSION` 升到 5，迁移逻辑只补默认值、不删任何字段 | S | — | 用一份 v4 数据迁移后字段齐全；刷新后不残留进行中的导入 | ⬜ |
| U4-3 | `UrlImportDialog`：多行输入、非法项逐条标红、语言与输出目录只读展示、「自动开始」复选框 | M | U4-1 | tauri-pilot 手测 | ⬜ |
| U4-4 | Toolbar 增加「从链接添加」入口（`Link` 图标，`data-testid="toolbar-import-url"`） | S | U4-3 | 手测 | ⬜ |
| U4-5 | `UrlImportItem`（`FileList` 顶部）：resolving / downloading_audio / writing / translating 各阶段的文案和进度（含不确定进度）；命中时显示语言与「人工 / AI」徽标 +「在 Finder 中显示」；失败时显示错误 +「重试」；`Queued` 后移除该项 | M | U4-1 | 手测三类路径 | ⬜ |
| U4-6 | 登录引导：`URL_IMPORT_NEEDS_COOKIES` 时行内出现浏览器选择器（Chrome / Edge / Firefox / Safari）+ 隐私说明（Safari 需要完全磁盘访问权限，Chrome / Edge 会弹钥匙串授权）→ 写入设置并重试；`login_hint` 时在转录行显示提示 | M | U4-5, U3-3 | 手测：未登录 → 选择浏览器 → 重试成功 | ⬜ |
| U4-7 | Settings：URL 导入分组（输出目录、B 站 cookies 来源、优先本地识别） | S | U4-2 | 手测；取值与 `--cookies-from-browser` 一一对应 | ⬜ |
| U4-8 | i18n：`en.json` / `zh-CN.json` 同步新增 `urlImport.*` 与相关的 `errors.*` | S | U4-3..7 | `bun run check:i18n` 通过 | ⬜ |
| U4-9 | `src-tauri/tests/pilot/smoke.toml`：打开面板 → 输入 YouTube 链接和番剧链接 → 断言逐条错误文案；输入合法的 B 站链接 → 断言「导入」按钮可用（**不联网**） | S | U4-3 | `tauri-pilot run` 通过且不依赖外网 | ⬜ |

---

## Phase U5 — 收尾

| ID | 任务 | 规模 | 依赖 | 验收 | 状态 |
|----|------|------|------|------|------|
| U5-1 | 更新 `AGENTS.md`：目录树增加 `site/`；IPC 表增加 3 个命令和 3 个事件；环境变量增加 `MONOSUB_YTDLP_PATH`；资源脚本增加 `fetch-ytdlp.sh`；bin 清单增加 yt-dlp；新增「URL 导入」小节；persist 版本改为 v5 | S | U2..U4 | 文档与代码一致 | ⬜ |
| U5-2 | 真实巡检：人工字幕视频 / `ai-zh` 视频 / 无字幕视频 / 未登录 / 多 P（带 `p` 与不带 `p`）/ 私有或已删除 / 非法站点 / 超过 1 小时的长视频，各跑一次 | M | U2..U4 | 结果记入 spec §9.10 | ⬜ |
| U5-3 | `sign-app.sh` + 公证：确认签名后 `yt-dlp-<triple>` 在打包的 app 中可执行 | S | U1-2 | 在打包的 app 里成功完成一次导入 | ⬜ |
| U5-4 | （可选）yt-dlp 自更新检查 | M | U1-1 | — | ⬜ |

---

## 每阶段完成前

`bun run test:rust`、`bun run lint:rust`、`bun run check`、`bun run lint`、`bun run check:i18n` 全部通过。
需要真实二进制或网络的测试，在缺失时必须打印 `skip: <原因>` 并通过（项目既有约定）。
