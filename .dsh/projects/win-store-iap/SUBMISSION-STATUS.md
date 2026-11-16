# MonoSub 2.1.0 Microsoft Store「免费 + Durable 内购」— 最终状态

更新时间：2026-11-16 12:08 (Asia/Shanghai) · 结果：**已提交认证（两笔），通过后自动发布**

## 目标（已确认方案）
- Windows 商店改「免费下载 + Durable（非消耗型）add-on 解锁」
- 内购 $9.99 / ¥48 一次性买断；免费版一次处理 1 个文件；不做 grandfather
- 仅 Store（MSIX）构建启用门禁；NSIS/DMG 保持 Direct

## 代码（已完成并实测）
- 新增 `src-tauri/src/commands/iap/msstore.rs`（Windows.Services.Store 持久型 add-on：读取/购买/HWND+UI 线程）
- `src-tauri/src/commands/iap/mod.rs` 增加 msstore 分支（provider=microsoft_store，无 grandfather）
- `Cargo.toml`：`msstore` feature + `windows = "=0.62.2"` / `windows-collections` / `windows-future`
- `scripts/release/package.mjs`：MSIX 构建加 `--features msstore`；`installer.mjs`(NSIS) 不加 → Direct
- 前端：`src/lib/iap.ts` 增加 `microsoft_store`；`ProUpgradeDialog` Family Sharing 仅 app_store 显示
- 版本 2.0.2 → **2.1.0**（tauri.conf.json / package.json / Cargo.toml）
- 验证：`cargo check --lib --features msstore` 及默认均 exit 0；IAP 单测 5 passed；`bun run check/lint/check:i18n` 通过；`package.mjs` 产出并校验 `dist\monosub_2.1.0.0_x64.msix`（1.46 GB，含 msstore 符号）

## Partner Center（已完成）
| 项 | 值 |
|---|---|
| 父应用 Store ID | `9P6WTCCRQTVW` |
| Add-on Store ID / token | `9PG23VS4QX3G` / `monosub_pro` |
| Add-on 价格 | 基础 USD 9.99 + 中国区 CNY ¥48（市场组，已保存生效）|
| Add-on 文案 | en「MonoSub License」/ zh「MonoSub 授权」+ 描述 |

### 提交结果
- **父应用 Submission 3**：Pricing=Free（$0/¥0），Packages=`monosub_2.1.0.0_x64.msix` Validated，Submission options 已补 runFullTrust 说明 → **In certification（Pre-processing）**
  - URL: https://partner.microsoft.com/en-us/dashboard/products/9P6WTCCRQTVW/submissions/1152921505702302395
- **Add-on Submission 1**：全部 Complete → **In certification（Pre-processing）**
  - URL: https://partner.microsoft.com/en-us/dashboard/products/9PG23VS4QX3G/submissions/1152921505702410957
- 两者均为「通过认证后自动发布」（Publish as soon as it passes certification）。认证通常数小时、最长 3 个工作日。

## 交接备注 / 已知项
- 商店「从付费改免费」不可逆（已确认，尚未卖出，符合预期）。
- 父应用 Store 描述文案仍沿用线上（未改），如需标注「免费+内购」可后续在 Store listings 更新（不影响本次提交）。
- 真实购买/恢复端到端需商店重签 + 零售许可证，本机无法验证；dev 侧载无零售证时代码降级为 free、不崩溃。
- CLI（monosub-cli.exe）仍 Direct 不拦（既有设计，GUI 队列才执行单文件门禁）。
- 回滚：草稿在认证前可 "Cancel certification"；代码 `git checkout` 或移除 msstore feature。
