# MonoSub 经验复盘：下一项目实战手册（去官方常识版）

> 这份文档只保留“不是官方入门里随处可见”的实战经验。默认你已经知道基础框架能力。

## 1. 先讲结论：最该复用的不是技术名词，而是工程分层

真正有效的是这三层职责边界：

1. 交互层：只管 UI、队列可视化、设置、状态展示。
2. 编排层：只管任务生命周期、并发、进程、路径、事件桥接。
3. 算法层：只管识别质量和输出结果。

关键收益：

- 算法升级不会拖慢 UI 迭代。
- UI 变更不会污染算法代码。
- 发布问题定位可以快速落到某一层。

## 1.1 设计风格与配色方案（可复用）

### 设计风格定位

- 主风格：Apple 气质 + 半透明玻璃层次 + 轻动效反馈
- 结构风格：顶部信息区 + 中部任务工作区 + 底部全局配置区
- 视觉目标：克制、清晰、状态优先，避免“炫技式”复杂视觉

### 配色方案（语义化 token）

- 主色 `primary`：Apple Green（`#34C759`）
- 强调色 `accent/teal`：Apple Blue/Teal（`#5AC8FA`）
- 成功 `success`：`#34C759`
- 警告 `warning`：`#FF9500`
- 危险 `danger`：`#FF3B30`
- 信息 `info`：`#007AFF`

### 中性色与层级

- 页面底色 `bg.page/card`：白底 + 浅灰分层（`#FFFFFF` / `#F5F5F7` / `#FAFAFA`）
- 文字层级：主文字 `#1D1D1F`，次级 `#86868B`，弱化 `#AEAEB2`
- 分割与边框：`#D2D2D7` / `#E8E8ED`

### 暗色与玻璃态策略

- 暗色不做“简单反色”，而是单独定义 glass 背景和边框透明度
- 玻璃层通过 `backdrop-filter + 半透明背景 + 细边框` 构建层次
- 关键容器使用 `glass-medium / glass-dark / glass-dark-strong` 区分深浅

### 形态与动效规范

- 圆角体系：6 / 10 / 14 / 18 / 20 / 24（强调“软边”桌面感）
- 阴影体系：轻悬浮 + 玻璃阴影（默认轻、悬停增强）
- 动效节奏：短时入场（约 0.2~0.3s）、按压反馈（scale 0.95~0.98）

### 下个项目可直接套用的视觉原则

1. 先定义语义 token，再写组件样式。
2. 先保证状态可读（空态/处理中/错误/完成），再做装饰。
3. 强调色只用于动作与状态，不用于大面积背景。
4. 暗色主题独立调优，不依赖自动反转。

## 1.2 技术、组件与插件清单（本项目实际使用）

### 前端技术栈

- React 18 + TypeScript + Vite
- Tailwind CSS + tailwindcss-animate
- Zustand（含 persist）
- i18next + react-i18next + browser language detector
- framer-motion（交互动效）
- lucide-react（图标）

### 桌面编排层技术栈

- Rust + tokio（异步并发）
- tracing / tracing-subscriber / tracing-appender（日志）
- reqwest + semver（版本检查与网络请求）
- lazy_static + uuid（全局任务与标识）

### 使用到的桌面插件

- dialog：文件/目录选择、确认弹窗
- fs：文件存在性检查等本地文件能力
- shell：启动 sidecar / 外部进程

### AI 引擎与媒体处理栈

- Python 3.9+（项目里实际使用 3.11 运行时）
- uv + pyproject（依赖与环境管理）
- FunASR（SenseVoice / Paraformer）
- torch / torchaudio（推理后端）
- ffmpeg / ffprobe（媒体处理与探测）

### 项目内关键 UI 组件（可复用）

- Header：窗口头部、进度摘要、主题入口
- Toolbar：导入、批量启动/停止、清空
- FileList / FileItem：队列展示、单任务操作、状态反馈
- BottomBar：输出目录/格式/语言与高级开关
- Settings：分类设置、环境修复、许可管理、更新检查

## 1.3 Indie Hacker 必做能力（商业化与系统化）

你提到的这三块非常关键，建议直接当作新项目默认基线。

### A. Pro 功能 License 管理与校验

建议最小闭环：

1. 本地保存：`licenseKey`、`activationId`、`deviceId`、最近校验时间、到期时间。  
2. 生命周期：激活 `activate` -> 校验 `validate` -> 失效/解绑 `deactivate`。  
3. 能力门控：Pro 开关动作前先 `ensureLicense()`，失败给用户明确动作（购买 / 打开设置）。  
4. 校验缓存：有效状态做短缓存（如 5 分钟），避免每次点击都打远程接口。  
5. 降级策略：校验失败时 Pro 能力关闭，但不影响免费功能主流程。

MonoSub 已验证的实现点：

- 前端统一 License 入口与状态缓存：`src/lib/license.ts`
- 后端做激活/校验/解绑命令：`src-tauri/src/commands/license.rs`
- UI 在设置页和关键开关位同步展示许可状态：`src/components/Settings.tsx`、`src/components/BottomBar.tsx`

### B. 软件版本自动检查（基于 GitHub Releases）

建议最小闭环：

1. 启动或“关于”页触发检查最新 release。  
2. 比较当前版本与最新 tag（语义化版本优先）。  
3. 有更新时展示版本号、发布时间、跳转 release 页面。  
4. 检查失败不阻塞主流程，只提示“检查失败，可稍后重试”。

MonoSub 已验证的实现点：

- 通过 GitHub latest release API 拉取并比较版本：`src-tauri/src/commands/update.rs`
- 设置页提供“检查更新 + 打开发布页”交互：`src/components/Settings.tsx`

### C. macOS 后台常驻 + 菜单栏/托盘唤醒

建议最小闭环：

1. 关闭窗口默认“隐藏不退出”。  
2. 菜单栏（或托盘）提供 `Show / Hide / Quit`。  
3. 点击图标可切换显示状态并聚焦窗口。  
4. Dock 或图标重新唤起时恢复主窗口到前台。  
5. 多语言时同步更新菜单项文案。

MonoSub 已验证的实现点：

- 关闭事件改为 hide：`src-tauri/src/main.rs`
- 托盘创建与点击行为：`src-tauri/src/tray.rs`
- 动态更新托盘菜单文案：`src-tauri/src/commands/tray.rs`、`src/App.tsx`

## 1.4 打包踩坑与当前解法（重点记录）

这部分是最容易在新项目里重复踩坑的，建议直接作为发布 SOP。

### A. Python 环境打包常见坑

1. 坑：Full 模式拷贝 venv 后，目标机运行报 `libpython*.dylib` 缺失。  
解法：打包阶段主动补齐 `libpython3.11.dylib`，并给 venv Python 注入 rpath。  
实现：`scripts/build/prepare-python-env.sh` 里的 `fix_python_dylib()`。

2. 坑：Lite 模式 sidecar 运行目录和资源目录不一致，启动器找不到 `main.py` 或 Python。  
解法：启动器按运行位置分支解析 `RESOURCES_DIR`（`Contents/MacOS` 与 `Contents/Resources/resources/bin`）。  
实现：`scripts/build/launcher-template.sh`。

3. 坑：Lite 首次安装依赖网络波动导致失败，用户误以为软件坏了。  
解法：首启安装阶段把安装日志透传成可读状态（`MSG/PROGRESS`），并保留 pip 兜底。  
实现：`scripts/build/launcher-template.sh`（uv 优先，pip fallback）。

4. 坑：引擎内部（第三方库）直接调用裸 `ffmpeg`/`ffprobe`，忽略外部路径。  
解法：在 Python 入口 monkey patch `subprocess.run`，把裸命令替换为环境变量路径。  
实现：`core-engine/main.py`。

### B. FFmpeg 打包常见坑

1. 坑：仅拷贝 ffmpeg 二进制，在无 Homebrew 的机器上运行失败（缺少 dylib 依赖）。  
解法：递归收集依赖 dylib，重写链接路径到 `@loader_path`，并重新签名。  
实现：`scripts/build/bundle-ffmpeg.sh`。

2. 坑：不同环境 ffmpeg 路径不一致，运行时找不到可执行文件。  
解法：统一路径解析优先级：环境变量 -> 开发资源 -> 打包资源 -> 系统常见路径 -> PATH。  
实现：`src-tauri/src/commands/video.rs` 的 `resolve_binary_path()`。

3. 坑：Python 子进程与主进程使用不同 ffmpeg 路径，行为不一致。  
解法：编排层统一向子进程注入 `MONOSUB_FFMPEG_PATH` / `MONOSUB_FFPROBE_PATH`。  
实现：`src-tauri/src/commands/transcribe.rs`（dev/sidecar 两条链路都注入）。

### C. 资源复制与权限坑

1. 坑：历史构建产物中只读文件（尤其 `uv`）导致下一次资源覆盖 `EACCES`。  
解法：构建前清理 target resources + 对旧 `uv` 先解锁权限再删除。  
实现：`src-tauri/build.rs`（`cleanup_stale_resources`）和 `scripts/build/build-macos-app.sh`。

2. 坑：版本号在多处定义，产物命名与应用内版本不一致。  
解法：打包前做版本同步，再生成 DMG 名称。  
实现：`scripts/build/build-macos-app.sh` + `scripts/utils/version-utils.sh`。

### D. 现在这套发布链路的推荐顺序

1. `prepare-python-env`（先决定 Lite/Full）  
2. `prepare-ffmpeg`（必要时做完整 dylib bundling）  
3. `prepare-engine`（复制/混淆引擎）  
4. `generate-launcher`（按模式生成启动器）  
5. 前端构建 + 客户端打包 + 签名  

对应总入口：`scripts/build/prepare-resources.sh`、`scripts/build/build-macos-app.sh`。

### E. 发布前检查清单（建议每次发版执行）

1. 在“未安装 Homebrew ffmpeg”的机器验证音视频转写。  
2. 验证 Lite 首启安装流程（含弱网场景）。  
3. 验证 Full 模式离线可运行。  
4. 验证关闭窗口后后台常驻、菜单栏唤醒与退出路径。  
5. 验证更新检查、License 激活/校验/解绑流程。  
6. 验证日志目录生成与关键错误可追踪。

## 2. MonoSub 里最值得抄作业的“非显然”实践

## 2.1 文件导入体验：先上屏，再补信息

做法：

- 文件先入队（占位缩略图 + 基本信息）。
- 元数据和缩略图异步回填。

价值：

- 批量导入时体感明显更快。
- 避免“必须等 ffprobe/ffmpeg 完成才能看到列表”的阻塞。

## 2.2 双层并发控制（不是只设一个上限）

做法：

1. UI 层控制“最多同时处理几个任务”（用户感知层）。
2. 引擎编排层再细分资源并发（例如音频提取与转写分开限流）。

价值：

- 既保证用户体验，又保护底层资源。
- 避免单层限流导致的抖动和抢占。

## 2.3 事件回调防 stale closure

做法：

- 事件监听里读取 ref 持有的最新状态，而不是读取旧闭包。

价值：

- 长任务 + 多事件场景下，状态不会错乱。
- 这是桌面端异步任务 UI 最常见的稳定性坑之一。

## 2.4 引擎协议化输出（可观测步骤而不是黑盒进度）

做法：

- 引擎输出结构化行协议（如步骤切换、总耗时、错误、成功路径）。
- 编排层只解析协议并转发 UI。

价值：

- 调试时可快速定位卡在“提取/加载/推理/写出”哪一步。
- 后续替换模型或重构引擎成本低。

## 2.5 异常路径的“自动降级链”

做法：

- 音频直通失败时自动回退到 WAV 转换重试。
- 缩略图硬件加速失败后回退软件解码。
- 高级音频滤镜失败后回退基础提取。

价值：

- 用户看见的是“慢一点但成功”，不是“直接失败”。
- 这类降级链比单一 happy path 重要得多。

## 2.6 缓存策略不是可选项

做法：

- 音频提取缓存、缩略图缓存都按源文件 mtime 判断新鲜度。

价值：

- 批处理反复重跑时节省大量时间。
- 避免“看似缓存，实际使用脏数据”。

## 2.7 依赖分发双轨（Lite/Full）

做法：

- Lite：首启安装依赖，包小。
- Full：预置完整环境，包大但稳定。

价值：

- 让“下载体积”和“首次可用性”可按场景取舍。
- 对桌面 AI 产品，这通常比单方案更现实。

## 2.8 可恢复发布流水线

做法：

- 打包脚本拆成 prepare-resources / prepare-python-env / prepare-engine / generate-launcher 等子步骤。
- 各步骤可单独重跑，失败可定位。

价值：

- CI/CD 和本地发布都更可维护。
- 发布问题不再是“一坨黑盒脚本”。

## 2.9 日志体系前置，而不是出事后补

做法：

- 日志按天滚动 + 保留窗口 + 结构化字段。

价值：

- 用户环境问题能靠日志回溯。
- 明显降低售后沟通成本。

## 3. 设计风格里真正有复用价值的部分

不重复“风格名词”，只说可迁移方法：

1. 先做 token 再做页面
- 颜色/圆角/阴影/字号/动效都先语义化。

2. 先做四态闭环
- 空态、处理中、错误态、完成态先完整。

3. 动画服务反馈
- 动画用于状态反馈，不用于装饰。

4. 暗色单独设计
- 不是简单反色，边框/阴影/层次都要重调。

## 4. 下个项目建议优先补强（高回报）

1. 类型约束更严格
- 把关键状态模型先做 strict 化。

2. 协议版本化
- 引擎输出协议标版本，避免以后字段漂移。

3. 错误码体系
- 统一错误码 + 诊断文案，便于支持和追踪。

4. 最小自动化测试闭环
- 至少覆盖：状态流、任务调度、引擎核心处理。

5. 发布期安全收口
- 发布配置里把开发期放宽项收紧。

## 5. 下一项目可直接复用的启动顺序

1. 先定义任务状态机（pending/processing/completed/error/skipped）。
2. 再定义引擎文本协议（步骤、耗时、成功、失败）。
3. 打通单任务闭环，再做批量。
4. 做双层限流与取消机制。
5. 做缓存与降级链。
6. 做日志与错误码。
7. 最后做 UI 精修和多语言扩展。

---

## 附：本手册依据的项目实现参考

- 状态与队列：`src/store/useStore.ts`
- 事件桥接与监听：`src/App.tsx`
- 批处理调度：`src/lib/transcription.ts`
- 导入异步回填：`src/lib/fileImport.ts`
- 任务编排与并发：`src-tauri/src/commands/transcribe.rs`
- 媒体处理与缓存：`src-tauri/src/commands/video.rs`
- 引擎协议与步骤追踪：`core-engine/main.py`
- 识别与降级处理：`core-engine/monosub_core/transcriber.py`、`core-engine/monosub_core/audio_extractor.py`
- 发布流水线：`scripts/build/*.sh`
