# MonoSub · Microsoft Store 免费下载 + Durable add-on 内购 — 实施记录

日期：2026-11-16 · 版本：2.1.0 · 平台：Windows / Microsoft Store (MSIX)

## 已确认的方案
- Windows 商店改为「免费下载 + Durable（非消耗型）add-on 解锁」
- 内购价 **$9.99 / ¥48** 一次性买断（沿用 macOS 方案）
- 免费版一次只能处理 **1 个文件**；Pro 解除限制
- **不做老用户 grandfather**（商店尚未卖出，无需保留）
- 仅 Store 构建（MSIX）启用付费门禁；NSIS 自装版 / DMG 仍然 `Direct`（无限）

## Partner Center 标识（已创建）
| 项 | 值 |
|---|---|
| 父应用 Store ID | `9P6WTCCRQTVW` |
| 父应用 identity Name | `BuyOnceLabs.monosub` |
| Publisher | `CN=3AFDD6AB-7CB4-4064-8C0B-7F48F2BC7DFA` |
| Add-on 类型 | Durable（非消耗型） |
| Add-on Store ID | `9PG23VS4QX3G` |
| Add-on offer token / Product ID | `monosub_pro` （代码里 `PRO_ADDON_OFFER_TOKEN`） |
| Add-on 草稿 submission | `1152921505702410957` |
| 基础价 | USD 9.99（"USD - United States"）已保存草稿 |
| 中国区 | 需覆盖为 ¥48（store-ops 处理中） |

## 代码改动
- `src-tauri/src/commands/iap/msstore.rs`（新）
  - `Windows.Services.Store.StoreContext` 持久型 add-on 读取/购买
  - 权益：`GetAppLicenseAsync().AddOnLicenses()` 优先（按 Store ID / offer token / SKU 匹配 `IsActive`），
    回退 `GetAssociatedStoreProductsAsync(["Durable"])` + `IsInUserCollection`
  - 价格/名称：关联 Durable 产品的 `Price().FormattedPrice()` / `Title()` / `Description()`
  - 购买：UI 线程 `run_on_main_thread` 里 `IInitializeWithWindow::Initialize(hwnd)` + `RequestPurchaseAsync(store_id)`，
    阻塞 worker 线程上 `.join()`，UI 线程不冻结
  - 无零售许可证（`0x803F6108/0x803F6107`，dev 侧载）→ 视为 **free**、仅告警、绝不崩溃
  - 无 grandfather
- `src-tauri/src/commands/iap/mod.rs`
  - 新增 `msstore` 模块与 `PRO_ADDON_OFFER_TOKEN` / `PRO_ADDON_STORE_ID` 常量
  - `unknown()`/`free()`/`store_provider()` 泛化，支持 macOS appstore 与 windows msstore
  - `init` / `refresh_and_emit` / `pro_product` / `pro_purchase` / `pro_restore` 增加 windows 分支（sync 函数经 spawn_blocking 调用）
- `src-tauri/Cargo.toml`
  - feature `msstore = ["dep:windows","dep:windows-collections","dep:windows-future"]`
  - `[target.'cfg(target_os="windows")'.dependencies]`：`windows = "=0.62.2"`（与 tauri 0.62.2 一致，HWND 同型）+
    `windows-collections = "=0.3.2"` + `windows-future = "=0.3.2"`；features: Services_Store / Foundation /
    Foundation_Collections / Win32_Foundation / Win32_UI_Shell / Win32_System_WinRT
- `scripts/release/package.mjs`（Windows/MSIX 分支）：`tauri build --no-bundle --features msstore`
  - `scripts/release/installer.mjs`（NSIS 自装）不加该 feature → 保持 Direct
- `src/lib/iap.ts`：`ProProvider` 增加 `'microsoft_store'`
- `src/components/ProUpgradeDialog.tsx`：Family Sharing 卖点仅 `app_store` 显示（MS 商店无 Family Sharing）
- 版本：`tauri.conf.json` / `package.json` / `src-tauri/Cargo.toml` 全部 2.0.2 → **2.1.0**

## 验证（本机实测）
- `cargo check --lib --features msstore` → exit 0
- `cargo check --lib`（默认 Direct 构建）→ exit 0
- `cargo test --lib --features msstore iap::` → 5 passed（含 msstore 状态码/无证码 单测）
  - 全量 `cargo test --lib --features msstore` 有 116 个 **与本改动无关** 的用例因本机 `%TEMP%` 权限（os error 5 拒绝访问）失败；IAP 用例全过
- `bun run check`（tsc）→ exit 0；`bun run check:i18n` → 369 keys 对齐；`bun run lint` → 0 error
- `node scripts\release\package.mjs --skip-resources` → **MSIX 构建并校验通过**
  - 产物：`dist\monosub_2.1.0.0_x64.msix`（约 1.46 GB，未签名，商店重签）
  - 校验：架构 x64 / 版本 2.1.0.0 / identity BuyOnceLabs.monosub / PFN BuyOnceLabs.monosub_3jbgsq2x4ngec / 运行时二进制齐全 / execution alias 正常
  - 二进制内已含 `9PG23VS4QX3G` / `microsoft_store` 字符串 → msstore 代码已编入

## 仍需人工 / 待办（提交前）
- Partner Center：add-on 中国区 ¥48 覆盖、Store listing、Properties、Age ratings、Save draft（store-ops）
- 父应用 Submission 3：Pricing 改为 **Free**、上传 `monosub_2.1.0.0_x64.msix`、更新商店描述（store-ops）
- 两个 submission 一起 **Submit for certification**（Lead 最终确认后执行）
- **注意**：应用从「收费」改「免费」后不可逆（微软规则）；但尚未卖出，符合预期

## 已知限制 / UNVERIFIED
- 真实购买/恢复端到端 **无法在本机验证**：需商店重签的 MSIX + 已发布 add-on + 零售许可证；dev 侧载无零售证 → 代码判为 free（不回崩溃）。
- CLI（`monosub-cli.exe`）仍为 Direct 不拦，属既有设计（GUI 队列才执行单文件门禁）。
- 若 ShopContext 在真实商店安装态异常，会降级为 free（不崩溃），可点「恢复授权」重查。
- 参考 API 调研：`win-iap-api-research.md`
