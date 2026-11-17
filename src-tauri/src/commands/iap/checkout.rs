//! 自托管 checkout 授权（离线 Ed25519 验签）。
//!
//! 仅编译进「非商店」构建（DMG / NSIS / CLI）。授权串格式：
//!   base64url(payloadJSON) + "." + base64url(signature)
//! payload 由 checkout.leix.dev 用对应私钥签名；本模块内置公钥，离线验签。
//! 授权串落盘到 `<config_dir>/license.txt`；启动时读取恢复 Pro。

use std::path::PathBuf;

use base64::engine::general_purpose::URL_SAFE_NO_PAD as B64;
use base64::Engine as _;
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use super::{ProStatus, FREE_MAX_QUEUE_FILES};

/// checkout.leix.dev 对应私钥的公钥（raw 32 bytes，base64url）。
///
/// 构建期由 `include_str!` 从仓库内 `src-tauri/keys/checkout_pubkey.b64` 注入
/// （公钥非机密，随仓库提交，任意构建机——含 Windows——可直接编）。
/// 文件缺失或不是 32 字节公钥时，`build.rs` 与本文件末尾的单测都会失败，
/// 不会像占位符那样悄悄打进一个「永远验签失败」的包。
const CHECKOUT_PUBKEY_B64: &str = include_str!("../../../keys/checkout_pubkey.b64");

#[derive(Debug, Clone, Deserialize)]
struct LicensePayload {
    #[serde(default)]
    v: u32,
    product: String,
    version: String,
    email: String,
    #[serde(default)]
    name: Option<String>,
    issued_at: i64,
    #[serde(default)]
    expires_at: Option<i64>,
    #[serde(default)]
    order_id: Option<String>,
}

/// 前端展示用的授权信息（camelCase）。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LicenseInfo {
    pub product: String,
    pub version: String,
    pub email: String,
    pub name: Option<String>,
    pub issued_at: i64,
    pub expires_at: Option<i64>,
    pub order_id: Option<String>,
    /// 到期展示名：有姓名用姓名，否则用邮箱。
    pub licensed_to: String,
}

impl From<LicensePayload> for LicenseInfo {
    fn from(p: LicensePayload) -> Self {
        let licensed_to = p
            .name
            .clone()
            .filter(|n| !n.trim().is_empty())
            .unwrap_or_else(|| p.email.clone());
        LicenseInfo {
            product: p.product,
            version: p.version,
            email: p.email,
            name: p.name,
            issued_at: p.issued_at,
            expires_at: p.expires_at,
            order_id: p.order_id,
            licensed_to,
        }
    }
}

fn verifying_key() -> Result<VerifyingKey, String> {
    // The injected file is a standard 44-char base64 (one '=' pad); the license
    // strings themselves are unpadded, hence URL_SAFE_NO_PAD. Strip the pad so
    // both shapes decode with the same engine.
    let bytes = B64
        .decode(CHECKOUT_PUBKEY_B64.trim().trim_end_matches('='))
        .map_err(|e| format!("公钥解码失败: {e}"))?;
    let arr: [u8; 32] = bytes
        .as_slice()
        .try_into()
        .map_err(|_| "公钥长度不是 32 字节".to_string())?;
    VerifyingKey::from_bytes(&arr).map_err(|e| format!("公钥无效: {e}"))
}

/// 解析并验签授权串；过期视为无效。
fn verify_license(text: &str) -> Result<LicensePayload, String> {
    let text = text.trim();
    let (payload_part, sig_part) = text
        .split_once('.')
        .ok_or_else(|| "授权码格式不正确".to_string())?;
    let payload_bytes = B64.decode(payload_part).map_err(|_| "授权码 payload 解码失败".to_string())?;
    let sig_bytes = B64.decode(sig_part).map_err(|_| "授权码签名解码失败".to_string())?;
    let sig_arr: [u8; 64] = sig_bytes
        .as_slice()
        .try_into()
        .map_err(|_| "签名长度不是 64 字节".to_string())?;
    let sig = Signature::from_bytes(&sig_arr);
    let vk = verifying_key()?;
    vk.verify(&payload_bytes, &sig)
        .map_err(|_| "授权码签名校验失败（可能被篡改或不是本产品签发）".to_string())?;

    let payload: LicensePayload =
        serde_json::from_slice(&payload_bytes).map_err(|e| format!("授权码内容解析失败: {e}"))?;

    if let Some(exp) = payload.expires_at {
        let now = chrono_now_secs();
        if now > exp {
            return Err("授权已过期".to_string());
        }
    }
    Ok(payload)
}

fn chrono_now_secs() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn license_path(app: &AppHandle) -> Result<PathBuf, String> {
    let paths = crate::paths::AppPaths::from_app(app).map_err(|e| e.to_string())?;
    let dir = paths.config_dir.clone();
    std::fs::create_dir_all(&dir).map_err(|e| format!("创建配置目录失败: {e}"))?;
    Ok(dir.join("license.txt"))
}

fn read_license_text(app: &AppHandle) -> Option<String> {
    let path = license_path(app).ok()?;
    std::fs::read_to_string(path).ok().map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}

fn pro_status() -> ProStatus {
    ProStatus {
        supported: true,
        provider: "checkout".to_string(),
        status: "pro".to_string(),
        is_pro: true,
        max_queue_files: None,
        product: None,
        error: None,
    }
}

fn free_status() -> ProStatus {
    ProStatus {
        supported: true,
        provider: "checkout".to_string(),
        status: "free".to_string(),
        is_pro: false,
        max_queue_files: Some(FREE_MAX_QUEUE_FILES),
        product: None,
        error: None,
    }
}

/// 启动/刷新时根据磁盘授权决定状态。
pub fn status_from_disk(app: &AppHandle) -> ProStatus {
    match read_license_text(app) {
        Some(text) if verify_license(&text).is_ok() => pro_status(),
        _ => free_status(),
    }
}

/// 用户粘贴授权码激活：验签通过才落盘。
pub fn activate(app: &AppHandle, license: &str) -> Result<ProStatus, String> {
    verify_license(license)?;
    let path = license_path(app)?;
    std::fs::write(&path, license.trim()).map_err(|e| format!("保存授权失败: {e}"))?;
    Ok(pro_status())
}

/// 当前授权信息（无/无效/过期均返回 None）。
pub fn info(app: &AppHandle) -> Option<LicenseInfo> {
    let text = read_license_text(app)?;
    verify_license(&text).ok().map(LicenseInfo::from)
}

/// 打开内嵌 checkout 购买窗口。
pub fn open_checkout(app: &AppHandle, url: &str) -> Result<(), String> {
    let parsed = url::Url::parse(url).map_err(|e| format!("URL 不合法: {e}"))?;
    if let Some(existing) = app.get_webview_window("checkout") {
        let _ = existing.set_focus();
        return Ok(());
    }
    tauri::WebviewWindowBuilder::new(app, "checkout", tauri::WebviewUrl::External(parsed))
        .title("升级 MonoSub Pro")
        .inner_size(1000.0, 780.0)
        .build()
        .map_err(|e| format!("打开购买窗口失败: {e}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Guards the build-time injection: an unresolved placeholder, an empty
    /// file, or a truncated key fails here instead of silently shipping a
    /// binary that rejects every license at `verifying_key()`.
    #[test]
    fn embedded_checkout_pubkey_decodes_to_a_valid_32_byte_key() {
        let raw = B64
            .decode(CHECKOUT_PUBKEY_B64.trim().trim_end_matches('='))
            .expect("checkout public key must be valid base64url");
        assert_eq!(raw.len(), 32, "checkout public key must be raw 32 bytes");
        assert!(verifying_key().is_ok(), "checkout public key must parse as Ed25519");
    }
}
