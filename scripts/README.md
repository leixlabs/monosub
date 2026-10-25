# scripts/

发布一条 MonoSub 版本的全部自动化：拉取运行时二进制、构建产物、校验、上传到
GitHub Releases。

## 一种语言，两个平台

这里所有脚本都是 Node（ESM，`>=20`，`package.json` 本来就要求）。不再有
bash / PowerShell 的分裂。

以前是 macOS 一套 bash、Windows 一套 PowerShell。那带来每个流程的两份拷贝、
一个让 bash 入口把控制权交给 PowerShell 的桥，以及一类只存在于接缝处的 bug：
传给 PowerShell 的 POSIX 路径、bash 执行不了的 `.cmd` shim、被 `-Command` 吃掉
的 `-File`。Node 在两个平台上都已经是这个项目的硬性依赖，所以脚本改为在内部
分支：

```js
const platform = hostPlatform();   // 'macos' | 'windows'，其余直接 die
if (platform === 'macos')   ...
else if (platform === 'windows') ...
```

两个平台真的需要不同逻辑时，各自一个函数放在同一个文件里；需要相同逻辑时
（校验和、资源清单、release notes、版本自增）就是共享的，只调一次。

## 目录结构

```
scripts/
├── lib/                    共享代码 + 几个独立小工具
│   ├── log.mjs             日志（只在 TTY 上着色）
│   ├── platform.mjs        宿主平台、target triple、.exe 后缀
│   ├── paths.mjs           项目根、dist/、resources/、暂存目录、tauri CLI
│   ├── shell.mjs           run / capture / which，处理 Windows 的 .cmd
│   ├── fs.mjs              拷贝、chmod、UTF-8（无 BOM）写入、临时目录
│   ├── net.mjs             带重试的下载、SHA-256 校验
│   ├── archive.mjs         zip 读取/解包（纯 JS）+ tar.gz
│   ├── version.mjs         版本读取 / 同步 / 自增
│   ├── xml.mjs             AppxManifest 的最小化编辑
│   ├── bump-version.mjs    独立工具：自增版本号
│   ├── check-i18n-keys.mjs 独立工具：i18n key 检查
│   └── clean-temp-files.mjs 独立工具：清理历史遗留
│
├── release/                产出可发布的产物 —— 只在本机跑，不联网
│   ├── release.mjs         入口：读版本 → 打包 → 核对 dist/
│   ├── package.mjs         构建产物（macOS → DMG，Windows → MSIX）
│   ├── prepare-resources.mjs 拉取随包的运行时二进制
│   ├── verify.mjs          校验产物（DMG / MSIX）
│   └── bundle-ffmpeg.mjs   macOS 专用：重链 ffmpeg 及其 dylib
│
└── publish/
    └── publish.mjs         dist/ → GitHub Releases —— 从不构建
```

`build/` 曾经和 `release/` 分开，实际上两者只是同一个流程的两段（打包 / 核对
产物），所以合成一个 `release/`。真正独立的是 `publish/`：它只上传，不构建。

## 两步

发布故意是两个命令，因为构建要十几分钟，上传只是一次 HTTP 调用：

```bash
node scripts/release/release.mjs      # 1. 构建 + 打包 + 校验 → dist/
node scripts/publish/publish.mjs      # 2. 把 dist/ 传上 GitHub Releases
```

构建失败不该在 GitHub 上留下半成品 release；上传失败（网络、权限、release 已存在）
也不该逼你重新构建。两步都能单独重跑，中间只靠 `dist/` 衔接。

两步都不自增版本号。release tag 必须等于打进 App 的版本号 —— App 的「检查更新」
拿自己的 `CARGO_PKG_VERSION` 跟 tag 比 —— 所以自增发生在发布**成功之后**
（`publish.mjs` 调 `lib/bump-version.mjs`，可用 `--skip-bump` 关掉）。
在构建前自增，就是当初 `tag v2.0.0` 配 `MonoSub-2.0.1.dmg` 的成因。

## 命令一览

`package.json` 里的封装：

```
bun run release           # 读版本 → 打包 → 核对 dist/
bun run release:package   # 只打包（不核对 dist/）
bun run release:resources # 只拉运行时二进制
bun run release:verify    # 校验一个已有产物
bun run publish           # node scripts/publish/publish.mjs
bun run publish:macos     # ... --platform macos
bun run publish:msix      # ... --platform windows
```

每个脚本都认 `-h` / `--help`：

| 脚本 | 参数 |
|------|------|
| `release/release.mjs` | `--skip-build`（`--skip-package` 是旧名）只核对 `dist/` 已有产物 |
| `release/package.mjs` | `--skip-resources` 复用 `resources/`；`--sign` 额外产出 dev-signed 副本（仅 Windows） |
| `release/prepare-resources.mjs` | 无参数，幂等 |
| `release/verify.mjs` | `<path-to-dmg-or-msix>`，失败即非零退出 |
| `publish/publish.mjs` | `--version` `--platform` `--recreate` `--add-assets` `--skip-bump` `--draft` `--asset` `--notes-file` |
| `lib/bump-version.mjs` | 无参数；发布成功后由 `publish.mjs` 调用 |

产物命名（两边都不一样，别凭印象）：

- macOS → `dist/MonoSub-<version>.dmg`
- Windows → `dist/monosub_<version>.0_x64.msix`
  （`*.signed.msix` 只是本地 `Add-AppxPackage` 用的副本，不算发布产物）

## Windows 只走 CUDA

CPU 推理太慢，不能出货，所以 Windows 包拉的是两个引擎的 **CUDA** 构建，里面
没有 CPU 版本：

| 引擎 | 包 | 落位 |
|------|-----|------|
| audio.cpp v0.9.1 | `...-windows-x64-cuda13.3.zip` + 同名 `cudart` 包 | `bin\audiocpp_cli\` |
| llama.cpp b11696 | `...-win-cuda-13.4-x64.zip` + 同名 `cudart` 包 | `bin\llama-server\` |

每个引擎放**各自的目录**：两者都带一个 `ggml-cuda.dll`，却是针对不同 CUDA
toolkit 编的，平铺在一个 `bin\` 里时后装的会静默覆盖先装的。Windows 解析进程
导入时先查可执行文件所在目录，所以 `.exe` 挨着自己的 DLL 就够了 —— 不需要 PATH
或 manifest。`engine::cli::find_executable` 会再探一层 `<dir>/<binary-name>/`，
`Backend::platform_default()` 在 Windows 上返回 `Cuda`，都是同一件事的两半。

CUDA 13.x 意味着 Pascal 及更早（GTX 10 系以下）被排除 —— NVIDIA 在 CUDA 13 里
移除了这些架构 —— 而它是覆盖 Blackwell（RTX 50 系，sm_120）的版本，CUDA 12.x
不覆盖。机器上没有 NVIDIA 显卡时，`prepare-resources.mjs` 只警告不中止。

macOS 不受影响：那边 audio.cpp 是静态链接，llama 的 dylib 本来就平铺在 `bin/`，
没有冲突，后端仍是 `metal`。

## 加一个平台

1. 给需要的脚本加分支 —— `platform.mjs` 已经会拒绝未知宿主，三个 release 脚本
   遇到不认识的平台会 `die` 并给出明确信息，而不是跑错的那半边。
2. `prepare-resources.mjs` 末尾那段共享尾巴（Silero VAD、yt-dlp 许可证、
   `MANIFEST.txt`）保持共享。
3. 在 `release.mjs` 和 `publish.mjs` 里加上该平台的产物扩展名。

## 约定

- **一切都 pin。** 每个下载都带 SHA-256。空 pin 表示「还没算」，会跳过校验，
  而不是让一个本该可行的构建失败。
- **幂等。** 产物已匹配时重跑是空操作，所以构建可以每次都调拉取脚本。
- **失败要响。** 缺 DLL、校验和不匹配都中止；绝不让一个只在用户运行时才坏的
  包发出去。
- **不要手改 `resources/MANIFEST.txt`** —— 它是生成的。
