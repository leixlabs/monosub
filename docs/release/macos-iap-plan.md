# MonoSub macOS Apple IAP 改造计划

> 日期：2026-11-13
> 状态：已实施（代码完成；App Store Connect 商品与 capability 仍需人工配置）
> 范围：Mac App Store 版（`leixlabs.monosub`）改为「免费下载 + Apple 内购」
> 不在本阶段范围：Windows / Microsoft Store、macOS GitHub DMG 直装版

---

## 1. 结论

Mac App Store 版从当前的「免费应用」改为：

- **下载免费**：App Store 标价保持 **Free**；
- **内购解锁 Pro**：一个非消耗型（Non-Consumable）内购，价格 **$9.99 / ¥48**，与当前 Windows 商店价一致；
- **免费版限制**：一次只能添加/处理 **1 个文件**的转录任务；
- **Pro 版**：解除单文件限制，可一次添加多个文件批量执行；
- **不做订阅**：一次性买断，永久有效；
- **Family Sharing**：开启，家庭成员可共享 Pro；
- **已上架老用户**：继续免费使用 Pro 能力（grandfathering），不因升级到 2.1.0 被限制为单文件；
- **直装版 / Windows**：最终也采用同样的「单文件免费、多文件付费」限制，但购买走后续自建的 checkout service；**本次先不改动**，当前仍保持现有行为。

产品形态是「免费单文件工具 + 一次性买断的批量能力」，不是一个空壳 App 加内购；免费版必须能完整完成单文件转录，这是通过 App Review 的前提。

---

## 2. 现状审计

### 2.1 当前定价与渠道

| 渠道 | 当前形态 | 价格 | 备注 |
|---|---|---|---|
| Microsoft Store（Windows） | 付费应用 | **$9.99 / ¥48 一次买断** | `README.md`、`landing_page` 已是此口径 |
| macOS GitHub Releases | 免费直装 | 免费 | 当前全功能 |
| Mac App Store | 免费应用，尚无内购 | 免费 | `docs/release/mas-app-store.md` §8 写的是「价格：免费」 |
| macOS 旧 Polar 授权 | 已移除 | — | `src-tauri/src/commands/external.rs` 头部注释说明 Pro 门禁已删除 |

本次确定的 macOS 内购价 = **$9.99 / ¥48**，与当前商店定价一致；不是旧的 `docs/business/store-pricing-assessment.md` 里 $29.99 / ¥128 的历史评估价。

### 2.2 当前代码没有付费门禁

- `src-tauri/src/commands/external.rs` 明确记录「Pro gate was removed」；
- `src-tauri/src/commands/mod.rs` 中没有 `license` / `purchase` / `iap` 模块；
- `src/` 与 `src-tauri/src/` 中没有 StoreKit / IAP / product / entitlement 逻辑；
- 所有功能（批量、翻译、说话人分离、词级时间戳、URL 导入、CLI）当前均免费。

### 2.3 MAS 打包链路已就绪，但没有 IAP 能力

已有：

- `src-tauri/tauri.mas.conf.json`（`minimumSystemVersion` 13.0、sandbox entitlements、Info.mas.plist；13.0 是 `AppTransaction` 的最低系统版本）；
- `src-tauri/entitlements/MonoSub.mas.entitlements`（app-sandbox、network client、user-selected read-write、bookmarks）；
- `scripts/release/mas.mjs`（构建 → 由内向外签名 → 校验 → productbuild 出 `.pkg`）；
- `docs/release/mas-app-store.md` 全流程；
- `.mas-build-number` 当前值为 **5**；build 5 已上传（`.workbuddy/memory/2026-11-03.md` 记录 build 4 → 5，build 4 已转 READY FOR INTERNAL TESTING，build 3 曾被 App Review 拒绝）。

缺：

- App Store Connect 的 IAP 商品记录与本地化；
- App ID 的 **In-App Purchase capability**（需要启用并重新生成 distribution profile）；
- StoreKit 2 的读取/购买/恢复实现；
- 免费版单文件限制与升级 UI；
- App Store 版打包时的 `appstore` 编译开关。

### 2.4 App Store 当前状态

- 当前版本：`2.0.2`；
- `.mas-build-number` = 5，说明 build 5 已上传；
- build 3 曾 `REJECTED BY APP REVIEW`，build 4 已处理通过（READY FOR INTERNAL TESTING），build 5 处理状态需在 App Store Connect 再确认；
- 本次改造应作为新版本（建议 `2.1.0`）提交，不复用 2.0.2 的审核版本。

> 注意：如果 2.0.2 已经在审核中，需要先从审核中移除或等审核结束，再提交 2.1.0 + IAP；IAP 不能事后追加到已提交的二进制里。

### 2.5 关键代码位置

| 位置 | 作用 |
|---|---|
| `src-tauri/src/commands/transcribe.rs` | `start_transcription`、`TASKS` 注册表、并发 semaphore；适合做后端兜底门禁 |
| `src/lib/fileImport.ts` | `addPathsToQueue`：拖拽 / 文件选择 / URL 导入最终都经这里入队；适合做「单文件限制」前端入口 |
| `src/App.tsx` | 拖拽监听、启动队列、事件订阅 |
| `src/components/Toolbar.tsx` / `FileList.tsx` | 添加文件按钮与拖拽入口 |
| `src/components/BottomBar.tsx` | 开始/全部开始、队列控制 |
| `src/components/Settings.tsx` | 设置面板；可增加 Pro/恢复购买入口 |
| `src/store/useStore.ts` | `files` 队列、`addFiles`、并发设置 |
| `scripts/release/mas.mjs` | MAS 构建入口；需要加 `--features appstore` |
| `src-tauri/tauri.mas.conf.json` | MAS 构建配置；最低系统版本 12.0 |

---

## 3. 产品规则（已确认）

### 3.1 免费版

- App Store 下载免费；
- 可以完整使用：单文件转录、翻译、说话人分离、词级时间戳、所有导出格式、模型下载；
- **一次只能添加 1 个文件**到转录队列；当前文件完成后，可以继续添加下一个；
- 不能一次拖入多个文件、不能一次选择多个文件、不能同时启动第二个转录任务；
- URL 导入同样按「队列中最多 1 个未完成文件」计。

### 3.2 Pro 版

- 非消耗型内购，**$9.99 / ¥48**，一次买断，无订阅、无续费；
- 解除单文件限制：可一次添加任意数量文件并批量执行；
- 沿用现有并发数（1–3）设置；
- Pro 状态绑定 Apple ID，可在其他 Mac 上用同一 Apple ID 恢复购买；
- **开启 Family Sharing**：购买者可与其家庭共享组成员共享 Pro（App Store Connect 的 IAP 设置中开启）；
- **老用户保留 Pro**：2.1.0 在 Mac App Store 上架之前已经下载过 MonoSub 的用户，即使没有购买 IAP，也继续视为 Pro（详见 §4.5 的 grandfathering 规则）。

### 3.3 边界规则

| 场景 | 行为 |
|---|---|
| 免费用户已有 1 个文件在队列中，再拖入 1 个或多个 | 只接受第 1 个，其余被拦截；弹出 Pro 升级对话框，说明「免费版一次处理一个文件」 |
| 免费用户已有 1 个文件正在处理，再点添加 | 立即弹升级对话框，不进队列 |
| 免费用户已有完成/跳过的文件，再添加 | 允许；限制只计算 `pending` + `processing` 的文件 |
| 免费用户同时拖入 5 个文件 | 只入队 1 个，剩余 4 个不静默丢弃：对话框明确显示「已添加 1 个，Pro 可一次添加全部」 |
| 免费用户启动 URL 导入，同时已有 1 个未完成文件 | 阻止并弹升级对话框；不允许先导入再补票 |
| 免费用户升级后 | 当前已入队的 1 个文件继续；之后可以继续添加多个 |
| Pro 用户退款 / 撤销 | 已进入队列的任务不强制中断；后续新增文件按免费规则限制 |
| 已上架老用户（升级前已下载） | 视为 Pro，保持无限批量；通过 `AppTransaction` 判定，见 §4.5 |
| 从旧版本升级且队列已有多个文件 | 不丢弃用户已添加的文件；只阻止新增，待队列清空后执行免费限制（详见 §9 风险） |
| 直装版 / Windows | 本阶段无限制；未来接 checkout service 后再启用同样限制 |

### 3.4 非目标

- 不改动现有转录、翻译、说话人分离、URL 导入的功能逻辑；
- 不做订阅、不做消耗型点数、不做账号体系；
- 不做服务端收据校验（非消耗型、纯本地应用，StoreKit 2 的 JWS 验证足够）；
- 不处理与 IAP 无关的 App Review 风险（`macOSPrivateApi`、URL 导入 5.2.3、浏览器 cookies 等，另行处理）。

---

## 4. 技术方案

### 4.1 StoreKit 2 接入方式

推荐 **`storekit-rs` 0.5.2（MIT OR Apache-2.0，macOS 12+）** 作为 Rust 侧的 StoreKit 2 绑定：

- 它自带 Swift bridge，覆盖 `Product.products(for:)`、`Product.purchase()`、`Transaction.currentEntitlements`、`Transaction.updates`、`AppStore.sync()`；
- `storekit-rs` 本身支持 macOS 12；但 `AppTransaction`（grandfathering）从 macOS 13 才可用，因此 MAS 包最低系统版本定为 13.0。
- 需要构建机安装 **Xcode 26.4+ SDK**（本机 Xcode 26.6 / SDK 26.5，满足）；
- 版本较新（0.5.x，下载量不大），建议在仓库中 **vendor / 锁定精确版本**，不要用宽泛的 `^` 范围；如果后续发现缺陷，替换成仓库内的轻量 Swift bridge。

备选：

| 方案 | 优点 | 问题 |
|---|---|---|
| `tauri-plugin-iap` 0.10.0-rc.10 + JS API | 接入快，封装好购买/恢复/产品查询 | macOS 13+，需要提升最低系统版本；`restorePurchases` 没有调用 `AppStore.sync()`，恢复购买需要 fork/patch；还是 pre-release |
| 仓库内自写 Swift bridge | 完全可控，macOS 12+ | 需要自己维护 Swift/FFI 代码，工作量更大 |
| StoreKit 1 (`objc2-store-kit`) | ObjC API 可用 | 需要手写 receipt 校验，非 StoreKit 2 推荐路径 |

### 4.2 模块边界

新增 `src-tauri/src/commands/iap/`（或 `commands/iap.rs` + `commands/entitlement.rs`）：

```
src-tauri/src/commands/iap/
├── mod.rs          # ProStatus / ProState / 对外命令 / 静态缓存
├── provider.rs     # EntitlementProvider trait：AppStore / Direct / 未来 Checkout
├── grandfather.rs  # AppTransaction 老用户判定（仅 App Store）
└── storekit.rs     # storekit-rs 薄封装（仅 macOS + appstore feature）
```

核心类型：

```rust
pub const PRO_PRODUCT_ID: &str = "leixlabs.monosub.pro";

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ProStatus {
    pub supported: bool,
    pub provider: String,          // "app_store" | "app_store_grandfathered" | "direct" | "checkout"
    pub status: String,            // "unknown" | "checking" | "free" | "pro" | "pending" | "error"
    pub is_pro: bool,
    /// 免费版为 Some(1)，Pro / 直装版为 None（无限）
    pub max_queue_files: Option<usize>,
    pub product: Option<ProProduct>,
    pub error: Option<String>,
}
```

### 4.3 状态机

```
unknown ──refresh()──> checking ──> free
                              └──> pro
free ──purchase()──> pending ──(StoreKit update)──> pro
free ──restore()──> checking ──> pro / free
free ──grandfathered──> pro（provider = app_store_grandfathered）
pro ──revoked/refund──> free
error ──retry──> checking
```

- 启动时 `unknown`，`bootstrapEntitlement()` 调 `pro_status` / `pro_refresh`；MAS 构建同时检查 `AppTransaction`，老用户直接判为 `pro`；
- 免费用户仍可立即做单文件转录；只有「添加第二个文件」时等待状态确认；
- Pro 缓存只保存在内存中（`RwLock<ProStatus>`），**不把 `is_pro` 布尔值写进 `settings.json`**；StoreKit 是唯一事实来源，避免用户改本地设置解锁；
- StoreKit 2 的 `Transaction.currentEntitlements` 在离线时也能读取本地已缓存的签名交易。

### 4.4 命令 / IPC

新增 Tauri commands（注册到 `src-tauri/src/main.rs` 的 `generate_handler!`）：

| 命令 | 作用 |
|---|---|
| `pro_status` | 返回当前内存快照，不发起网络请求 |
| `pro_refresh` | 查询 `Transaction.currentEntitlements`，更新状态并 emit `pro-status-changed` |
| `pro_product` | 读取 StoreKit product 的本地化名称/描述/价格（`displayPrice`） |
| `pro_purchase` | 发起购买；返回 `purchased` / `cancelled` / `pending` / `failed` |
| `pro_restore` | 调 `AppStore.sync()` 后重新查询当前权益 |
| `pro_dev_set_state` | **仅 debug 构建**：切换 fake provider 的 free/pro/pending/error 状态，用于无购买测试 |
| `pro_dev_clear_state` | **仅 debug 构建**：退出 FakeProvider，恢复真实 StoreKit provider |

事件：

- 后端 → 前端：`pro-status-changed`，payload 为 `ProStatus`；
- 购买成功、恢复成功、撤销/退款、事务更新都通过同一个事件更新 UI。

### 4.5 StoreKit 实现要点

1. **启动监听**：app 启动时在独立线程启动 `Transaction.updates()` 循环；
2. **验证交易**：只读 `VerificationResult::payload_value()`，`unverified` 一律不授予 Pro；
3. **Finish 时机**：确认已更新内存状态后再 `transaction.finish()`；如果进程在 finish 前崩溃，下次启动 `currentEntitlements` 会再次给出该交易；
4. **恢复购买**：先 `AppStore.sync()`，再枚举 `currentEntitlements`，过滤 `PRO_PRODUCT_ID`；
5. **退款/撤销**：`revocation_date.is_some()` 的 transaction 不算有效权益；`Transaction.updates` 会推送状态变化，下一次刷新也会纠正；
6. **主线程约束**：`Product.purchase()` / `AppStore.sync()` 必须从非主线程调用（Tauri async command 的 tokio worker 或 `spawn_blocking`），否则 StoreKit 会返回 `NotSupported`；
7. **未配置商品**：`Product.products(for:)` 返回空数组时返回 `PRODUCT_UNAVAILABLE`，UI 显示「价格暂不可用」，不让用户点进购买；
8. **不读取收据文件**：StoreKit 2 的 JWS 验证由系统完成，不需要 `appStoreReceiptURL` / 收据解析；
9. **老用户 grandfathering**：通过 `storekit_rs::AppTransaction::shared()`（必要时 `AppTransaction::refresh()`）读取 `original_app_version` 与 `original_purchase_date`；满足任一条件即视为老用户 Pro：
   - `original_app_version` 语义化版本 **低于 2.1.0**（例如 `2.0.2`、`2.0.1`）；
   - 或 `original_purchase_date` 早于 **2.1.0 的 App Store 上架日期**（该日期以 App Store Connect 实际发布时间为准，写在常量 `GRANDFATHER_CUTOFF` 里）。
   
   判定使用 `VerificationResult::payload_value()` 的已验证结果；第一次判定成功后把 `original_app_version` / `original_purchase_date` / `grandfathered = true` 缓存到 `settings.json`（仅作为离线兜底，不作为安全边界）。`provider` 设为 `app_store_grandfathered`，UI 显示「老用户保留 Pro」，不再显示购买按钮。
   
   注意：`AppTransaction` 可能受网络/StoreKit 缓存影响；如果首次启动拿不到结果，先按 `free` 展示但不要立即阻止老用户继续批量（以已缓存的老用户标记为准），下一次刷新补齐。

### 4.6 渠道判定（关键）

不能用 `is_sandboxed()` 区分 MAS 和本地沙盒测试包，因为本地 `release:pkg:sandbox` 也是沙盒。用 **Cargo feature**：

```toml
[features]
default = ["tauri-app"]
tauri-app = []
appstore = []          # 只有 MAS 构建启用

[target.'cfg(target_os = "macos")'.dependencies]
storekit-rs = { version = "=0.5.2", optional = true }
```

构建时：

- `scripts/release/mas.mjs`：给 `tauri build` 加 `--features appstore`；
- 普通 `bun tauri build`（DMG）：不启用 `appstore`，`iap` 模块返回 `provider: "direct", is_pro: true, max_queue_files: null`；
- 将来 Windows / 直装版接 checkout 时，新增 `checkout` feature 或运行时 provider 配置，不改 UI 和业务门禁代码。

> 如果 Cargo 对「optional target-specific dependency + feature」的组合有解析问题，可改为独立的 workspace crate `iap`，由 `appstore` feature 通过 path dependency 引入；不改变上面的模块边界。

### 4.7 后端兜底门禁

`start_transcription` 在最前面（在 `ensure_models_root_usable` 之前即可）增加：

```
if !iap::is_pro() {
    let active = TASKS.lock().await.values()
        .filter(|entry| !entry.handle.is_finished())
        .count();
    if active >= 1 {
        emit_error(&app, &task_id, "proRequiredBatch");
        return Ok(task_id);
    }
}
```

目的：即使前端被绕过（devtools、旧前端、URL 导入竞态），免费用户也不能同时跑第二个转录任务。返回的错误状态是一个 i18n key，前端映射到升级提示。

### 4.8 前端集成

新增 `src/lib/iap.ts` / `src/store/useEntitlement.ts`：

```ts
export type ProStatus = {
  supported: boolean;
  provider: 'app_store' | 'app_store_grandfathered' | 'direct' | 'checkout';
  status: 'unknown' | 'checking' | 'free' | 'pro' | 'pending' | 'error';
  isPro: boolean;
  maxQueueFiles: number | null;  // free: 1; pro/direct: null
  product?: { id: string; displayName: string; description: string; displayPrice: string };
  error?: string;
};
```

- `App.tsx` 启动时 `bootstrapEntitlement()`，并 `listen('pro-status-changed')`；
- `fileImport.ts::addPathsToQueue` 增加 `maxQueueFiles` 检查：
  - 只统计 `pending` / `processing` 的文件；
  - 超出时截断 `uniqueFiles`，返回 `blockedByPro: number`；
  - 不静默丢弃：调用方弹出 `ProUpgradeDialog`；
- 所有入口统一：
  - `Toolbar` 的文件选择；
  - `FileList` 的拖拽；
  - `App.tsx` 的窗口拖拽；
  - `UrlImportDialog` 的「开始导入」；
  - `ClipboardPrompt` 的「一键导入」；
- `BottomBar` 的「全部开始」在免费用户存在多个 `pending` 时（旧版本遗留队列）提示升级，但不破坏已入队文件；
- `ProUpgradeDialog`：
  - 标题「解锁批量转录 / Unlock batch transcription」；
  - 正文「未授权版一次处理一个文件；获得 MonoSub 授权后可一次添加多个文件并批量执行」；
  - 价格从 `pro_product.displayPrice` 读取，不用前端硬编码；
  - 主按钮「获取授权 / Get License」，次按钮「恢复授权 / Restore License」，文字按钮「以后再说 / Not now」；
  - 小字「一次性买断，无订阅。付款由 Apple 处理。」
- Settings 增加「MonoSub 授权」卡片（放在 `system` 分组或独立 `account` 分组）：
  - free：显示单文件限制 + 升级按钮；
  - pro：显示「已解锁无限批量转录」；
  - `app_store_grandfathered`：显示「老用户已获得授权 — 无限批量转录已包含」；
  - `provider = app_store` 时始终显示「恢复购买」；`app_store_grandfathered` 不显示购买按钮；
  - `provider = direct` 时不显示任何购买 UI（直装版/Windows 当前无内购）。

### 4.9 i18n

新增 en / zh-CN 双语文案，至少覆盖：

- `pro.title` / `pro.subtitle`
- `pro.freeLimit`
- `pro.upgrade`
- `pro.restore`
- `pro.restoring`
- `pro.pending`
- `pro.cancelled`
- `pro.purchaseFailed`
- `pro.productUnavailable`
- `pro.oneTime`
- `pro.grandfathered`
- `pro.familySharing`
- `errors.proRequiredBatch`

`bun run check:i18n` 必须保持 0 差异。

### 4.10 为什么不使用外部支付链接

App Store 版的数字功能只能走 Apple IAP（Guideline 3.1.1）。不在 App Store 包里放 GitHub Releases、官网购买、Paddle/Polar/自建 checkout 的购买入口；直装版与 Windows 的 checkout 留在各自渠道。模型许可证外链、隐私政策链接等非购买链接不受影响。

---

## 5. 代码改动清单

| 文件 | 改动 | 预估 |
|---|---|---|
| `src-tauri/Cargo.toml` | 增加 `appstore` feature；macOS target 引入 `storekit-rs = "=0.5.2"`（optional） | 0.5h |
| `src-tauri/src/commands/iap/mod.rs`（新） | `ProStatus` / 状态机 / 命令 | 1–2d |
| `src-tauri/src/commands/iap/storekit.rs`（新） | StoreKit 2 封装、交易监听、购买/恢复 | 2–3d |
| `src-tauri/src/commands/iap/grandfather.rs`（新） | `AppTransaction` 老用户判定、`GRANDFATHER_CUTOFF`、离线缓存 | 1d |
| `src-tauri/src/commands/iap/provider.rs`（新） | `EntitlementProvider` trait；Direct provider；debug-only fake states 在 `mod.rs` | 0.8d |
| `src-tauri/src/commands/mod.rs` | 导出 `iap` 模块 | 0.1h |
| `src-tauri/src/main.rs` | 注册 `pro_status` / `pro_refresh` / `pro_product` / `pro_purchase` / `pro_restore`；启动交易监听 | 0.5d |
| `src-tauri/src/commands/transcribe.rs` | `start_transcription` 增加免费单任务兜底门禁；错误状态 `proRequiredBatch` | 0.5d |
| `src-tauri/src/site/mod.rs` | `start_url_import` 可选：若当前有活动转录任务则拒绝（兜底） | 0.2d |
| `src/lib/iap.ts`（新） | IPC 包装 + `ProStatus` 类型 | 0.5d |
| `src/store/useEntitlement.ts`（新） | 全局 entitlement store、启动刷新、事件监听 | 0.5d |
| `src/lib/fileImport.ts` | `addPathsToQueue` 增加 `maxQueueFiles` 限制，返回 `blockedByPro` | 0.5d |
| `src/App.tsx` | 启动 entitlement、拖拽入口处理升级弹窗 | 0.3d |
| `src/components/ProUpgradeDialog.tsx`（新） | 升级/恢复购买对话框 | 1d |
| `src/components/ProSettingsCard.tsx`（新） | Settings 的 Pro 状态卡片 + debug 状态按钮 | 0.5d |
| `src/components/Toolbar.tsx` / `FileList.tsx` | 处理 `blockedByPro` 并弹窗 | 0.3d |
| `src/components/BottomBar.tsx` | 旧队列多文件场景的升级提示 | 0.2d |
| `src/components/UrlImportDialog.tsx` / `ClipboardPrompt.tsx` | 免费单文件限制 | 0.3d |
| `src/components/Settings.tsx` | Pro 状态卡片、恢复购买 | 0.5d |
| `src/i18n/locales/en.json` / `zh-CN.json` | 新增 i18n key | 0.3d |
| `scripts/release/mas.mjs` | 构建参数加 `--features appstore`；可选校验 profile 的 In-App Purchase capability | 0.3d |
| `src-tauri/tauri.mas.conf.json` | `minimumSystemVersion: 13.0`（AppTransaction 要求）；直装版仍保持原系统要求 | 0.1h |
| `README.md` / `README_CN.md` / `landing_page` | Mac 定价/获取方式改为「App Store 免费下载 + Pro ¥48/$9.99」 | 0.5d |
| `docs/release/mas-app-store.md` | 补 IAP 章节（商品、capability、审核备注、测试） | 0.3d |
| `docs/release/macos-iap-plan.md` | 本计划 | — |

预计编码 + 构建 **8–11 个工作日**，加 App Store 审核 3–7 天。

---

## 6. App Store Connect 配置

### 6.1 账号与协议

- [ ] App Store Connect → **Business**：Paid Applications Agreement 处于 Active；
- [ ] 银行账户、税务信息完整（否则 IAP 无法在 sandbox 测试）；
- [ ] 如符合条件，确认 App Store Small Business Program 状态（影响抽成比例）。

### 6.2 App ID 与 provisioning profile

- [ ] developer.apple.com → Certificates, Identifiers & Profiles → Identifiers → `leixlabs.monosub` → Edit；
- [ ] 启用 **In-App Purchase** capability；
- [ ] 重新生成 **Mac App Store Connect** distribution profile（旧的 profile 在 capability 变更后会失效）；
- [ ] 更新 `src-tauri/mas/distribution.provisionprofile`；
- [ ] 若使用 dev profile，同样重新生成；
- [ ] `mas.mjs` 可在签名前检查 profile 是否包含 In-App Purchase 能力/相关 entitlement；失败时直接报错，避免上传后才被 ITMS 退回。

### 6.3 创建 IAP 商品

App Store Connect → 我的 App → MonoSub → **In-App Purchases** → `+`：

| 字段 | 值 |
|---|---|
| Type | **Non-Consumable** |
| Reference Name | `MonoSub 授权` |
| Product ID | **`leixlabs.monosub.pro`**（必须与 `PRO_PRODUCT_ID` 逐字一致） |
| Price | 基础价 **$9.99**（US）；中国区单独覆盖 **¥48** |
| Availability | 所有目标店面；中国区需确认 App 的 ICP 备案状态 |
| Family Sharing | **开启**（Enable Family Sharing） |
| Tax Category | 按 Apple 默认 / 软件类；按账号后台提示填写 |

> Family Sharing 开启后，StoreKit 的 `Transaction.currentEntitlements` 会返回 `ownership_type = familyShared` 的有效交易；权益判定只认「已验证 + 未撤销」，不需要区分购买者本人还是家庭成员。

本地化（按工作区策略，English (U.S.) 为主语言，简体中文为副语言）：

- en-US：
  - Display Name: `MonoSub License`
  - Description: `Unlock unlimited batch transcription. One-time license, no subscription.`
- zh-Hans：
  - Display Name: `MonoSub 授权`
  - Description: `解锁无限批量转录。一次买断授权，无订阅。`

Review 截图：

- 1280×800 或 2880×1800；
- 展示免费限制 + 升级对话框，或 Settings 里的 Pro 卡片；
- 要能让审核员一眼看出「免费版单文件、Pro 批量」的区别。

### 6.4 提交版本

- [ ] App 价格设为 **Free**（不是 paid）；
- [ ] 版本号 `2.1.0`，build 号由 `.mas-build-number` 自动递增；
- [ ] 在版本页面的 **In-App Purchases** 区域选中 `MonoSub Pro`，随版本一起提交；
- [ ] App 信息 / 截图 / 描述更新：明确「Free download with optional MonoSub Pro in-app purchase」；
- [ ] 隐私政策 URL：按项目约定使用 `https://leix.dev/app/monosub/privacy/en` / `/zh`；
- [ ] 主语言按工作区约定为 **English (U.S.)**，简体中文为副语言；如果现有 App 记录是中文主语言，先在 App Information 检查能否调整，不能调整则按 Apple 支持流程处理。

### 6.5 审核备注模板

```
MonoSub converts local video/audio files into subtitles entirely on-device.

Pricing model: the app is free to download. A single non-consumable in-app
purchase, "MonoSub Pro" ($9.99 / ¥48), unlocks unlimited batch transcription.
The free version can process one file at a time. All other features
(transcription, translation, speaker separation, word timestamps, exports) work
for a single file without purchase.

To test:
1. Add one media file and start transcription — this works in the free version.
2. Add a second file while the first is queued — the upgrade screen appears.
3. Purchase MonoSub Pro in the sandbox; the queue accepts multiple files.
4. Settings → MonoSub Pro → Restore Purchases restores the purchase.

No account or login is required.
```

---

## 7. 构建与发版

### 7.1 本地构建

```bash
# MAS 构建（含 IAP）
MAS_SIGN_IDENTITY="3rd Party Mac Developer Application: Lei Zhang (RF4CPKUXQ7)" \
MAS_INSTALLER_IDENTITY="3rd Party Mac Developer Installer: Lei Zhang (RF4CPKUXQ7)" \
MAS_PROFILE=src-tauri/mas/distribution.provisionprofile \
bun run release:mas
```

`scripts/release/mas.mjs` 的 build 调用改为：

```js
['build', '--config', 'src-tauri/tauri.mas.conf.json', '--bundles', 'app', '--features', 'appstore']
```

### 7.2 开发阶段：不购买怎么测

结论：**开发阶段默认用假 IAP provider；真实 StoreKit 只放到沙盒/TestFlight 验证**。不需要也不应该让开发者用自己的 Apple ID 真买。

> 本机非 App Store 构建（没有 `appstore` feature）的「使用真实 Provider」是 `DirectProvider`，不是 StoreKit；它不会发起 Apple 内购。只有 `appstore` 构建里的真实 provider 才会调用 `Product.purchase()`。

#### A. 日常开发：Debug 假 provider（主力）

在 `iap/provider.rs` 增加一个只在 debug 构建编译的 `FakeProvider`：

- 不访问 StoreKit，不联网，不弹购买窗；
- 用 Tauri command `pro_dev_set_state` 在运行时切换：
  - `free`：免费状态，单文件限制生效；
  - `pro`：Pro 状态，无限批量；
  - `pending`：购买等待中；
  - `error`：StoreKit 错误态。
- `pro_dev_set_state` 会临时把当前 provider 切到 `FakeProvider`；配套 `pro_dev_clear_state` 恢复到真实 StoreKit provider，便于同一台机器上切换「假状态测 UI」和「真实 sandbox 测 StoreKit」；
- 仅 `#[cfg(debug_assertions)]` 注册命令，**release / MAS 包里没有这个命令**；
- Settings ▸ System ▸ Developer 只在 debug 构建显示「IAP Test State」按钮组；
- 这样可以用 `tauri-pilot` 或手工把「添加第 2 个文件 → 弹升级框 → 切 Pro → 再添加成功」整条链路跑通，而完全不碰 Apple 服务器。

#### B. StoreKit 本地测试环境（可选，验证 Swift bridge）

Apple 官方的 StoreKit Testing in Xcode 可以在不连 App Store 的情况下模拟商品、购买、退款和 Ask to Buy。但本项目不是 Xcode 工程，Tauri 进程无法直接挂 Xcode Scheme 的 StoreKit Configuration。

如果要把 StoreKit API 本身也测到，加一个轻量的测试宿主（不进主构建）：

- 用 Xcode 建一个最小的 macOS App / XCTest target，只链接 `storekit-rs` 的 Swift bridge（或直接链接 StoreKit）；
- 放一个 `StoreKitConfig.storekit`，用 `SKTestSession` 模拟：
  - `Product.products(for:)` 返回商品；
  - `Product.purchase()` 成功、取消、Pending；
  - `Transaction.currentEntitlements` / `AppTransaction`；
  - 退款、Ask to Buy、Family Sharing。
- 它测的是 StoreKit bridge 的 API 行为，不是整个 Tauri UI；主 App 的 UI/门禁仍由 A 覆盖。

#### C. App Store Sandbox / TestFlight（真实 StoreKit，但不花钱）

最终上线前必须用 Apple 的 sandbox 环境验证真实 StoreKit 路径：

- App Store Connect → Users and Access → Sandbox → 创建 **Sandbox Apple Account**；
- macOS 上 App Store → Settings 登录该 sandbox 账号；
- 用 TestFlight（推荐）或 MAS signed 测试包运行；
- sandbox 交易**不会真扣款**，但会走真实的 StoreKit 购买弹窗、收据、JWS 验证、恢复购买和退款流程；
- 测试项：product 加载、购买成功、取消、Pending（Ask to Buy）、恢复购买、重装恢复、离线、退款/撤销、Family Sharing；
- sandbox 账号可按店面切换，验证美国 `$9.99` 与中国 `¥48` 的显示价格。

#### D. 测试矩阵

| 方式 | 需要 App Store Connect | 需要真实付款 | 覆盖范围 | 使用阶段 |
|---|---|---|---|---|
| Rust 单元测试 + FakeProvider | 否 | 否 | 状态机、单文件限制、grandfather 判定 | 每次提交 |
| Debug `pro_dev_set_state` | 否 | 否 | 升级弹窗、队列限制、Settings 卡片、i18n | 日常 UI 开发 |
| StoreKit Test 宿主 + `.storekit` | 否 | 否 | Swift bridge / 商品 / 购买 / 恢复 API | bridge 改动时 |
| Sandbox / TestFlight | 是 | 否 | 真实 StoreKit 全链路 | 提审前 |
| App Review sandbox | 是 | 否 | 审核员路径 | 提交后 |

> 关键原则：**`FakeProvider` 绝不能进 release**；`pro_dev_set_state` 只在 `debug_assertions` 下编译。别用环境变量开关，因为环境变量泄漏到生产会造成「免费解锁」。

### 7.3 本地测试包

- `bun run release:pkg:sandbox` 目前用于沙盒行为测试；它没有 distribution profile，**StoreKit 购买不能用它验证**；
- 如需在本地验证 UI 的免费/Pro 状态，用上面的 Debug `pro_dev_set_state`；
- 真正的 StoreKit 沙盒测试：用 `release:mas --no-pkg` 签出的 `MonoSub.app`，或通过 TestFlight。

### 7.4 StoreKit 沙盒测试步骤

1. App Store Connect → Users and Access → Sandbox → 创建 Sandbox Apple Account；
2. Mac 上 App Store → Settings → 登录 Sandbox 账号（macOS 11.1+ 的路径见 Apple 文档）；
3. 安装 MAS signed 测试包或 TestFlight build；
4. 测试：product 加载、购买成功、取消、Pending（Ask to Buy）、恢复购买、重新安装后恢复、离线状态、退款/撤销；
5. 切换沙盒账号的国家/地区，验证 $9.99 与 ¥48 的显示价格。

### 7.5 发版流程

1. 版本号升到 `2.1.0`（`tauri.conf.json`、`package.json`、`Cargo.toml` 同步）；
2. `bun run release:mas` 出包；
3. Transporter 上传 `.pkg`；
4. App Store Connect 选择 build，绑定 IAP，提交审核；
5. 审核通过后发布；README / landing page 同步更新；
6. 记录 `.workbuddy/memory/2026-11-13.md` 或当天文件（构建号、审核结果、IAP 商品 ID）。

---

## 8. 测试与验收

### 8.1 自动化

- `cargo test --lib`：新增 `iap` 状态机、免费单文件限制、provider 切换、grandfather 版本比较、FakeProvider 购买/恢复路径测试；
- `cargo clippy --all-targets`：无新增告警；
- `bun run check` / `bun run lint` / `bun run check:i18n`：全过；
- `bun run build`：前端构建；
- `bun run release:mas`：签名、pkg、profile 检查全过。

### 8.2 手工验收

| # | 场景 | 预期 |
|---|---|---|
| 1 | 免费版启动 | 单文件可完整转录，所有导出格式可用 |
| 2 | 免费版添加第 2 个文件 | 只添加 1 个，弹出升级对话框，价格正确显示 `$9.99` / `¥48` |
| 3 | 购买成功 | 状态变为 Pro，队列可添加多个文件，重启后仍为 Pro |
| 4 | 购买取消 | 不报错、不扣款，继续保持免费 |
| 5 | 购买 Pending | 显示「等待批准」，不授予 Pro；批准后自动变 Pro |
| 6 | 恢复购买 | `AppStore.sync()` 后恢复 Pro；重装后可恢复 |
| 7 | 退款/撤销 | 下次刷新变回 free；已入队任务不被强杀 |
| 8 | 离线启动（已购买） | `currentEntitlements` 仍返回 Pro，可批量 |
| 9 | 非 App Store 直装版 | 不显示购买 UI，无单文件限制（本阶段不变） |
| 10 | Windows 版 | 完全不变 |
| 11 | 中文 / 英文 / 中文（台湾）界面 | 文案、价格本地化正确 |
| 12 | 沙盒日志 | 无 crash、无 StoreKit 错误、无 sandbox deny |
| 13 | 老用户升级（`original_app_version = 2.0.2`） | `provider = app_store_grandfathered`，显示「老用户保留 Pro」，可批量，无购买按钮 |
| 14 | 新用户首次下载 2.1.0 | `original_app_version = 2.1.0`，仍为 free，只能单文件 |
| 15 | 家庭共享成员购买 | `ownership_type = familyShared` 的有效交易授予 Pro，可批量 |

### 8.3 App Review 验收

- 免费版能独立完成一次转录，不是空壳；
- 升级对话框在添加第二个文件时出现；
- 购买、恢复购买路径可用；
- 无外部购买链接；
- 审核备注包含测试步骤；
- App Store 页面明确标注「Free」+「In-App Purchases」。

---

## 9. 风险与对策

| 风险 | 影响 | 对策 |
|---|---|---|
| `storekit-rs` 较新、下载量小 | 构建/运行时可能有坑 | 锁定 `=0.5.2` 并 vendor；先做独立 spike 验证购买/恢复；保留自写 Swift bridge 备选 |
| Xcode 26.4+ 要求 | 其他构建机可能失败 | 只在 MAS 构建启用 `appstore`；普通 DMG 构建不编译 StoreKit；文档注明 Xcode 版本 |
| App Review 认为免费版功能不足 | 拒审 2.1/4.2 | 免费版保留完整单文件转录 + 全部导出格式；审核备注说明 |
| 已有免费用户不满 | 差评/退款 | 已确认：已上架老用户通过 `AppTransaction` grandfathering 继续免费 Pro；新用户才按免费单文件规则 |
| 直装版全功能免费 | App Store 用户觉得不公平 | 本阶段先不动；后续 checkout 上线后统一限制；README 明确渠道差异 |
| 用户手动改本地状态 | 绕过付费 | 不把 `is_pro` 写入 settings；所有门禁以 StoreKit `currentEntitlements` 为准；后端兜底 |
| 中国区 ICP 备案 | App/ IAP 无法在中国区销售 | 先确认备案状态；未备案则先只开海外店面 |
| 退款/家庭共享 | 收入/权益变化 | StoreKit `revocation_date` / `Transaction.updates` 处理；Family Sharing 已确认开启，家庭共享交易按有效权益处理 |
| App Store 当前 build 5 审核状态 | 重复提交/版本冲突 | 先查 App Store Connect；必要时撤回 2.0.2，重新提交 2.1.0 |
| 非消耗型 IAP 被审核当作「解锁完整版」 | 需说明免费版价值 | 免费版单文件完整可用，升级卖的是批量能力，不是功能阉割 |

### 回滚

- 代码回滚：移除 `appstore` feature 或让 provider 回到 `Direct`，App Store 版变回全功能免费；但已发布的 IAP 商品仍在 App Store Connect，需要手动下架；
- 构建回滚：`mas.mjs` 去掉 `--features appstore` 即可恢复旧行为；
- 数据：不需要迁移用户数据；StoreKit 交易本身不可删除，但代码可以暂时忽略。

---

## 10. 已确认决策

1. **定价**：$9.99 / ¥48，非消耗型、一次性买断。
2. **免费版边界**：一次只能添加/处理 1 个文件；单文件功能完整。
3. **Family Sharing**：**开启**。
4. **已上架老用户**：**继续免费**（grandfathering），通过 `AppTransaction` 判定；老用户显示「老用户保留 Pro」，不显示购买按钮。
5. **直装版 / Windows**：最终同限制，但本次不动；后续接自建 checkout service 再启用。
6. **商品 ID**：`leixlabs.monosub.pro`（若 App Store Connect 已有命名规范，以实际为准，但必须与代码常量一致）。
7. **审核备注语言**：英文为主，中文为辅。
8. **最低系统版本**：MAS 包保持 **macOS 13.0**（`AppTransaction` 从 macOS 13 才可用，老用户 grandfathering 依赖它）；直装版不受此限制。

### 仍需在实施时补齐（非阻塞）

- `GRANDFATHER_CUTOFF` 的具体日期：以 2.1.0 在 App Store 实际发布时间为准；如果无法精确取得，可用版本号 `< 2.1.0` 作为主判据。
- App Store Connect 当前的 App 主语言是否可改为 English (U.S.)（按工作区策略）；如果不可改，按 Apple 支持流程处理。
- 中国区 ICP 备案状态，决定 2.1.0 是否先只开海外店面。

---

## 11. 建议执行顺序

| 阶段 | 内容 | 产出 |
|---|---|---|
| P0（0.5d） | App Store Connect 检查：build 5 状态、Paid Apps 协议、App ID 开启 In-App Purchase、重新生成 profile | 后台就绪 |
| P1（2–3d） | StoreKit spike：`storekit-rs` 接入 Tauri，能读取 product、完成 sandbox 购买/恢复 | 可运行 demo |
| P2（2d） | `iap` 模块 + 后端 `start_transcription` 门禁 + 单文件限制 | 后端行为正确 |
| P3（2d） | 前端 `useEntitlement`、`ProUpgradeDialog`、Settings Pro 卡片、i18n | UI 完整 |
| P4（1d） | MAS 构建参数、签名/profile 校验、版本号 2.1.0 | 可上传 pkg |
| P5（1–2d） | TestFlight/sandbox 全场景测试、App Store 元数据、提交审核 | 提交 |
| P6（3–7d） | App Review 往返、修复、发布 | 上线 |

---

## 11.1 实施记录（2026-11-13）

- Rust：新增 `src-tauri/src/commands/iap/`（`mod.rs` / `storekit.rs` / `grandfather.rs` / `provider.rs`）；`start_transcription` 与 `start_url_import` 增加免费单任务门禁；`BackendSettings` 增加 `proGrandfathered` 缓存。
- 前端：新增 `src/lib/iap.ts`、`src/store/useEntitlement.ts`、`ProUpgradeDialog.tsx`、`ProSettingsCard.tsx`；`addPathsToQueue` 单文件限制、各添加入口升级提示、Settings ▸ 系统 Pro 卡片、Debug 状态按钮。
- 构建：`storekit-rs = "=0.5.2"` 只在 `appstore` feature 下编译；`scripts/release/mas.mjs` 传入 `--features appstore`；`build.rs` 补 Swift runtime rpath。
- 验证：`cargo test --lib` 639 passed / 1 ignored；`cargo test --lib --features appstore` 642 passed / 1 ignored；`bun run check`、`bun run check:i18n`（369 keys）、`bun run lint`（0 error）、`bun run build`、`bun run tauri build --config src-tauri/tauri.mas.conf.json --bundles app --features appstore` 均通过；`bun scripts/release/mas.mjs --skip-build --no-pkg` 签名通过（CFBundleVersion 6）。
- 仍需人工：App Store Connect 创建 `leixlabs.monosub.pro`、App ID 开启 In-App Purchase capability 并重新生成 distribution profile；上架前用 TestFlight / Sandbox 验证真实购买、恢复、退款、Family Sharing。

## 12. 一句话交接

> 免费下载，单文件随便用；¥48 / $9.99 一次买断，解锁无限批量转录，开启 Family Sharing；已上架老用户继续免费。StoreKit 2 非消耗型 IAP，只在 MAS 构建启用；直装版和 Windows 当前不动，等自建 checkout service。
