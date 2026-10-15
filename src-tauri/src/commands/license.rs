use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::Sha256;
use std::process::Command;
use std::time::Duration;

const POLAR_BASE_URL: &str = "https://polar.leix.dev";
const POLAR_ORGANIZATION_ID: &str = "6c7d88dd-4cae-4491-85c9-4dbb65170b12";
const POLAR_BENEFIT_ID: &str = "f288c189-f31b-4e0a-b849-1a0308420110";
const LICENSE_HMAC_KEY: &[u8] = b"ms_lic_v1_7f3a9c2e5b8d4f1a6e0c3b9d2f5a8e1c";

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivateLicenseRequest {
    pub key: String,
    pub organization_id: String,
    pub benefit_id: String,
    pub device_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidateLicenseRequest {
    pub key: String,
    pub organization_id: String,
    pub benefit_id: String,
    pub activation_id: Option<String>,
    pub device_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeactivateLicenseRequest {
    pub key: String,
    pub organization_id: String,
    pub activation_id: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivateLicenseResult {
    pub activated: bool,
    pub activation_id: Option<String>,
    pub status: String,
    pub message: String,
    pub expires_at: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidateLicenseResult {
    pub valid: bool,
    pub status: String,
    pub message: String,
    pub expires_at: Option<String>,
    pub activation_required: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeactivateLicenseResult {
    pub deactivated: bool,
    pub status: String,
    pub message: String,
}

fn license_state_path() -> Option<std::path::PathBuf> {
    let home = std::env::var("HOME").ok()?;
    Some(license_state_path_in(std::path::Path::new(&home)))
}

fn license_state_path_in(home: &std::path::Path) -> std::path::PathBuf {
    home.join(".monosub").join("license_state.json")
}

/// Decode a hex string the way Python's `bytes.fromhex` does: ASCII hex digits
/// only, with any ASCII whitespace ignored, and an even number of digits.
fn decode_hex(input: &str) -> Option<Vec<u8>> {
    let mut nibbles = Vec::with_capacity(input.len());
    for ch in input.chars() {
        if ch.is_ascii_whitespace() {
            continue;
        }
        nibbles.push(ch.to_digit(16)? as u8);
    }
    if nibbles.len() % 2 != 0 {
        return None;
    }
    Some(
        nibbles
            .chunks(2)
            .map(|pair| (pair[0] << 4) | pair[1])
            .collect(),
    )
}

fn hmac_hex(payload: &str) -> String {
    type HmacSha256 = Hmac<Sha256>;
    let mut mac = HmacSha256::new_from_slice(LICENSE_HMAC_KEY).expect("HMAC key error");
    mac.update(payload.as_bytes());
    mac.finalize()
        .into_bytes()
        .iter()
        .map(|b| format!("{:02x}", b))
        .collect()
}

/// HMAC-SHA256 stream cipher (CTR mode): XOR each 32-byte block with
/// HMAC(key, "enc:" || block_index_as_u32_be). Symmetric — same function encrypts and decrypts.
fn xor_crypt(data: &[u8]) -> Vec<u8> {
    type HmacSha256 = Hmac<Sha256>;
    let mut result = Vec::with_capacity(data.len());
    for (i, chunk) in data.chunks(32).enumerate() {
        let mut mac = HmacSha256::new_from_slice(LICENSE_HMAC_KEY).expect("HMAC key error");
        mac.update(b"enc:");
        mac.update(&(i as u32).to_be_bytes());
        let keystream = mac.finalize().into_bytes();
        for (j, b) in chunk.iter().enumerate() {
            result.push(b ^ keystream[j]);
        }
    }
    result
}

fn write_license_state(valid: bool, expires_at: Option<&str>, activation_id: Option<&str>) {
    let Some(path) = license_state_path() else { return };
    write_license_state_at(&path, valid, expires_at, activation_id);
}

/// Writes the state into an explicit `$HOME`, so the unit test can point at a
/// temporary directory instead of the developer's real licence.
#[cfg(test)]
fn write_license_state_in(
    home: &std::path::Path,
    valid: bool,
    expires_at: Option<&str>,
    activation_id: Option<&str>,
) {
    write_license_state_at(
        &license_state_path_in(home),
        valid,
        expires_at,
        activation_id,
    );
}

fn write_license_state_at(
    path: &std::path::Path,
    valid: bool,
    expires_at: Option<&str>,
    activation_id: Option<&str>,
) {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let validated_at = chrono::Utc::now().to_rfc3339();
    let expires_at_str = expires_at.unwrap_or("");
    let activation_id_str = activation_id.unwrap_or("");
    let payload = format!(
        "{}|{}|{}|{}",
        valid as u8, validated_at, expires_at_str, activation_id_str
    );
    let sig = hmac_hex(&payload);
    let state = json!({
        "valid": valid,
        "validated_at": validated_at,
        "expires_at": expires_at,
        "activation_id": activation_id,
        "sig": sig,
    });
    // Encrypt the JSON and store as hex — not human-readable
    let encrypted = xor_crypt(state.to_string().as_bytes());
    let hex: String = encrypted.iter().map(|b| format!("{:02x}", b)).collect();
    let _ = std::fs::write(path, hex);
}

fn clear_license_state() {
    if let Some(path) = license_state_path() {
        let _ = std::fs::remove_file(path);
    }
}

/// True when a previously validated Pro license is still trustworthy.
///
/// Decrypts `~/.monosub/license_state.json`, verifies the HMAC signature over
/// `1|validated_at|expires_at|activation_id`, requires `valid == true`, and
/// rejects a licence whose `expires_at` is in the past (spec §5.7).
///
/// Task: P1-13.
#[allow(dead_code)]
pub fn is_pro_licensed() -> bool {
    match std::env::var("HOME") {
        Ok(home) if !home.is_empty() => is_pro_licensed_in(std::path::Path::new(&home)),
        _ => false,
    }
}

/// Same check against an explicit `$HOME`, so the unit test can point at a
/// temporary directory instead of the developer's real licence.
///
/// Task: P1-13.
#[allow(dead_code, unused_variables)]
pub fn is_pro_licensed_in(home: &std::path::Path) -> bool {
    let Ok(raw) = std::fs::read_to_string(license_state_path_in(home)) else {
        return false;
    };
    let raw = raw.trim();
    // Legacy plaintext state is rejected — the user must re-validate.
    if raw.starts_with('{') {
        return false;
    }
    let Some(encrypted) = decode_hex(raw) else {
        return false;
    };
    let Ok(state) = serde_json::from_slice::<Value>(&xor_crypt(&encrypted)) else {
        return false;
    };
    if state.get("valid").and_then(Value::as_bool) != Some(true) {
        return false;
    }

    let validated_at = state
        .get("validated_at")
        .and_then(Value::as_str)
        .unwrap_or("");
    let expires_at_str = state
        .get("expires_at")
        .and_then(Value::as_str)
        .unwrap_or("");
    let activation_id_str = state
        .get("activation_id")
        .and_then(Value::as_str)
        .unwrap_or("");
    let stored_sig = state.get("sig").and_then(Value::as_str).unwrap_or("");
    let Some(stored_sig) = decode_hex(stored_sig) else {
        return false;
    };

    let payload = format!(
        "1|{}|{}|{}",
        validated_at, expires_at_str, activation_id_str
    );
    type HmacSha256 = Hmac<Sha256>;
    let Ok(mut mac) = HmacSha256::new_from_slice(LICENSE_HMAC_KEY) else {
        return false;
    };
    mac.update(payload.as_bytes());
    // Constant-time comparison of the stored signature (never `==`).
    if mac.verify_slice(&stored_sig).is_err() {
        return false;
    }

    // Only now is `expires_at_str` authenticated, so it may drive control flow
    // (spec §5.7). An empty value is a perpetual licence, matching
    // `write_license_state`'s `unwrap_or("")`. Anything unparseable is treated
    // as expired rather than as perpetual, so a corrupt date cannot grant Pro.
    if !expires_at_str.is_empty() {
        match chrono::DateTime::parse_from_rfc3339(expires_at_str) {
            Ok(expires) if expires.with_timezone(&chrono::Utc) > chrono::Utc::now() => {}
            _ => return false,
        }
    }

    true
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A throwaway `$HOME` under the system temp dir, removed on drop.
    struct TempHome(std::path::PathBuf);

    impl TempHome {
        fn new() -> Self {
            let dir =
                std::env::temp_dir().join(format!("monosub-license-test-{}", uuid::Uuid::new_v4()));
            std::fs::create_dir_all(&dir).expect("create temp home");
            Self(dir)
        }

        fn path(&self) -> &std::path::Path {
            &self.0
        }

        /// Encrypt an arbitrary state JSON exactly like `write_license_state_at`.
        fn write_state_json(&self, state_json: &str) {
            let path = license_state_path_in(self.path());
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).expect("create .monosub dir");
            }
            let hex: String = xor_crypt(state_json.as_bytes())
                .iter()
                .map(|b| format!("{:02x}", b))
                .collect();
            std::fs::write(path, hex).expect("write state");
        }
    }

    impl Drop for TempHome {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn accepts_state_written_by_write_license_state() {
        let home = TempHome::new();
        // Far-future expiry: this test asserts acceptance, so it must not start
        // failing as wall-clock time advances past a near-future date.
        write_license_state_in(
            home.path(),
            true,
            Some("2099-01-01T00:00:00+00:00"),
            Some("act-1"),
        );
        assert!(is_pro_licensed_in(home.path()));
    }

    /// Spec §5.7 requires the expiry to be honoured, not merely signed.
    #[test]
    fn rejects_expired_license() {
        let home = TempHome::new();
        // Correctly signed, `valid: true`, but the expiry has already passed.
        write_license_state_in(
            home.path(),
            true,
            Some("2020-01-01T00:00:00+00:00"),
            Some("act-1"),
        );
        assert!(!is_pro_licensed_in(home.path()));
    }

    /// An unparseable expiry must not be treated as perpetual.
    #[test]
    fn rejects_unparseable_expiry() {
        let home = TempHome::new();
        write_license_state_in(home.path(), true, Some("not-a-date"), Some("act-1"));
        assert!(!is_pro_licensed_in(home.path()));
    }

    #[test]
    fn accepts_state_without_optional_fields() {
        let home = TempHome::new();
        write_license_state_in(home.path(), true, None, None);
        assert!(is_pro_licensed_in(home.path()));
    }

    #[test]
    fn rejects_tampered_signature() {
        let home = TempHome::new();
        let validated_at = "2026-01-01T00:00:00+00:00";
        // Signature is genuine, but for the *old* expiry; the stored expiry was
        // changed afterwards, so the recomputed signature cannot match.
        let sig = hmac_hex(&format!(
            "1|{}|2027-01-01T00:00:00+00:00|act-1",
            validated_at
        ));
        home.write_state_json(
            &json!({
                "valid": true,
                "validated_at": validated_at,
                "expires_at": "2028-01-01T00:00:00+00:00",
                "activation_id": "act-1",
                "sig": sig,
            })
            .to_string(),
        );
        assert!(!is_pro_licensed_in(home.path()));
    }

    #[test]
    fn rejects_sig_of_the_right_length_but_wrong_value() {
        let home = TempHome::new();
        let validated_at = "2026-01-01T00:00:00+00:00";
        let sig = hmac_hex(&format!("1|{}||", validated_at));
        let flipped = format!("{}{}", if &sig[..1] == "0" { "1" } else { "0" }, &sig[1..]);
        home.write_state_json(
            &json!({
                "valid": true,
                "validated_at": validated_at,
                "expires_at": null,
                "activation_id": null,
                "sig": flipped,
            })
            .to_string(),
        );
        assert!(!is_pro_licensed_in(home.path()));
    }

    #[test]
    fn rejects_valid_false() {
        let home = TempHome::new();
        write_license_state_in(home.path(), false, None, None);
        assert!(!is_pro_licensed_in(home.path()));
    }

    #[test]
    fn rejects_missing_file() {
        let home = TempHome::new();
        assert!(!is_pro_licensed_in(home.path()));
    }

    #[test]
    fn rejects_legacy_plaintext_state() {
        let home = TempHome::new();
        let path = license_state_path_in(home.path());
        std::fs::create_dir_all(path.parent().expect("parent dir")).expect("create .monosub dir");
        std::fs::write(
            &path,
            r#"{"valid":true,"validated_at":"","expires_at":null,"activation_id":null,"sig":""}"#,
        )
        .expect("write legacy state");
        assert!(!is_pro_licensed_in(home.path()));
    }

    #[test]
    fn rejects_malformed_state() {
        let home = TempHome::new();
        let path = license_state_path_in(home.path());
        std::fs::create_dir_all(path.parent().expect("parent dir")).expect("create .monosub dir");

        // Bad hex (odd length).
        std::fs::write(&path, "abc").expect("write");
        assert!(!is_pro_licensed_in(home.path()));

        // Valid hex, but not JSON once decrypted.
        let hex: String = xor_crypt(b"not json")
            .iter()
            .map(|b| format!("{:02x}", b))
            .collect();
        std::fs::write(&path, hex).expect("write");
        assert!(!is_pro_licensed_in(home.path()));

        // Valid JSON, but the `valid` field is missing.
        home.write_state_json(r#"{"validated_at":"x"}"#);
        assert!(!is_pro_licensed_in(home.path()));
    }

    #[test]
    fn rejects_missing_state_dir() {
        // A path that does not exist under the temp dir — no `.monosub` created.
        let dir =
            std::env::temp_dir().join(format!("monosub-license-absent-{}", uuid::Uuid::new_v4()));
        assert!(!is_pro_licensed_in(&dir));
    }

    /// Cross-language contract pin (review item R2).
    ///
    /// The round-trip tests above write with `write_license_state_in` and read
    /// with `is_pro_licensed_in`, so they cannot detect a divergence in the
    /// payload string or the keystream layout — both sides would move together.
    /// These constants were produced by the ORIGINAL Python implementation
    /// (`core-engine/main.py::_hmac_hex` / `_xor_crypt`, key
    /// `ms_lic_v1_7f3a9c2e5b8d4f1a6e0c3b9d2f5a8e1c`), so if the Rust payload or
    /// cipher drifts, real v1 licence files stop verifying.
    #[test]
    fn matches_python_golden_signature_vector() {
        // python: _hmac_hex("1|2026-01-01T00:00:00+00:00||act-1")
        assert_eq!(
            license_signature("1|2026-01-01T00:00:00+00:00||act-1"),
            "8d66e9f06eca4092ca78ded2c8507214e673e9e5a1e4f7a43bff0dd8366bcbc7"
        );
    }

    #[test]
    fn accepts_python_produced_state_blob() {
        let home = TempHome::new();
        // Emitted by the Python writer for the state
        // {"valid":true,"validated_at":"2026-01-01T00:00:00+00:00",
        //  "expires_at":"2099-01-01T00:00:00+00:00",
        //  "activation_id":"act-1","sig":"16063678…b2f2"}
        // It is written verbatim (no Rust re-encryption), so this fails if the
        // Rust reader's cipher or signed payload ever diverges from Python's.
        const PYTHON_BLOB: &str = "51c75f8fa3595499778000aba3131aa6510cf109d4b6a3d79ff56893d762babdafa33af9c6c7d4eb22c8b39142bb95e50b1db43bb87b6ce4b05cd0480919959d8bf028fb91035678e255f0e61a356fac915220c426f8f3eb84f1216ea40354f2a0fa053ef3f72a6c46e4defef40c21ead6ad75261e502e06cfe4a3a235f10c5b775f312100ce2d4dcd5117fd0430d233fdb00e38038d422161ef52bef18ec600e23fe9d0f89a149bc93bc3cf1f135a7c75386e9aeade890baae6b3c5f8c285b8eefd9f";
        let path = license_state_path_in(home.path());
        std::fs::create_dir_all(path.parent().expect("parent dir")).expect("create .monosub dir");
        std::fs::write(&path, PYTHON_BLOB).expect("write python blob");

        assert!(is_pro_licensed_in(home.path()));
    }
}

/// Compiled-in signature key, exposed for the P1-13 unit test fixture.
#[allow(dead_code)]
pub(crate) fn license_signature(payload: &str) -> String {
    hmac_hex(payload)
}

fn build_endpoints(path: &str) -> Vec<String> {
    let base = POLAR_BASE_URL.trim_end_matches('/');
    let mut endpoints = vec![format!("{}/v1{}", base, path)];
    endpoints.push(format!("{}/api/v1{}", base, path));
    endpoints
}

fn get_value_by_path<'a>(value: &'a Value, path: &[&str]) -> Option<&'a Value> {
    let mut current = value;
    for segment in path {
        current = current.get(*segment)?;
    }
    Some(current)
}

fn extract_string(value: &Value, paths: &[&[&str]]) -> Option<String> {
    for path in paths {
        if let Some(v) = get_value_by_path(value, path) {
            if let Some(s) = v.as_str() {
                let trimmed = s.trim();
                if !trimmed.is_empty() {
                    return Some(trimmed.to_string());
                }
            }
        }
    }
    None
}

fn extract_error_message(body: &Value, fallback: &str) -> String {
    extract_string(
        body,
        &[
            &["detail"],
            &["message"],
            &["error"],
            &["errors", "0", "message"],
        ],
    )
    .unwrap_or_else(|| fallback.to_string())
}

fn is_activation_required(status: u16, body: &Value) -> bool {
    if status != 422 {
        return false;
    }

    let msg = extract_error_message(body, "").to_lowercase();
    msg.contains("activation") && msg.contains("required")
}

async fn post_polar(path: &str, payload: &Value) -> Result<(u16, Value), String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(12))
        .build()
        .map_err(|e| format!("Failed to initialize Polar API client: {}", e))?;

    let endpoints = build_endpoints(path);
    let mut last_error: Option<String> = None;

    for endpoint in endpoints {
        tracing::info!(endpoint = %endpoint, "Calling Polar API");
        let response = match client.post(&endpoint).json(payload).send().await {
            Ok(resp) => resp,
            Err(err) => {
                tracing::warn!(endpoint = %endpoint, error = %err, "Polar request failed");
                last_error = Some(err.to_string());
                continue;
            }
        };

        let status_code = response.status().as_u16();
        let body = response.json::<Value>().await.unwrap_or(Value::Null);

        if status_code == 404 && endpoint.contains("/v1/") && !endpoint.contains("/api/") {
            continue;
        }

        return Ok((status_code, body));
    }

    Err(match last_error {
        Some(err) => format!("Unable to reach Polar API: {}", err),
        None => "Unable to reach Polar API.".to_string(),
    })
}

fn validate_fixed_params(organization_id: &str, benefit_id: Option<&str>) -> Result<(), String> {
    if organization_id.trim() != POLAR_ORGANIZATION_ID {
        return Err("Invalid organization_id".to_string());
    }
    if let Some(benefit_id) = benefit_id {
        if benefit_id.trim() != POLAR_BENEFIT_ID {
            return Err("Invalid benefit_id".to_string());
        }
    }
    Ok(())
}

#[tauri::command]
pub async fn activate_license(
    request: ActivateLicenseRequest,
) -> Result<ActivateLicenseResult, String> {
    validate_fixed_params(&request.organization_id, Some(&request.benefit_id))?;

    let key = request.key.trim();
    let device_id = request.device_id.trim();
    if key.is_empty() || device_id.is_empty() {
        return Ok(ActivateLicenseResult {
            activated: false,
            activation_id: None,
            status: "invalid".to_string(),
            message: "License key and device ID are required.".to_string(),
            expires_at: None,
        });
    }

    let payload = json!({
        "key": key,
        "organization_id": POLAR_ORGANIZATION_ID,
        "label": format!("MonoSub-{}", &device_id.chars().take(12).collect::<String>()),
        "meta": {
            "device_id": device_id,
            "client": "monosub-desktop"
        }
    });

    let (status_code, body) =
        post_polar("/customer-portal/license-keys/activate", &payload).await?;
    if !(200..300).contains(&status_code) {
        let message = extract_error_message(
            &body,
            &format!("License activation failed ({})", status_code),
        );
        return Ok(ActivateLicenseResult {
            activated: false,
            activation_id: None,
            status: "invalid".to_string(),
            message,
            expires_at: extract_string(&body, &[&["license_key", "expires_at"], &["expires_at"]]),
        });
    }

    let activation_id = extract_string(
        &body,
        &[
            &["id"],
            &["activation_id"],
            &["license_key_activation", "id"],
        ],
    );

    if activation_id.is_none() {
        return Ok(ActivateLicenseResult {
            activated: false,
            activation_id: None,
            status: "error".to_string(),
            message: "License activation succeeded but activation ID was missing.".to_string(),
            expires_at: extract_string(&body, &[&["license_key", "expires_at"], &["expires_at"]]),
        });
    }

    Ok(ActivateLicenseResult {
        activated: true,
        activation_id,
        status: "activated".to_string(),
        message: "License activated on this device.".to_string(),
        expires_at: extract_string(&body, &[&["license_key", "expires_at"], &["expires_at"]]),
    })
}

#[tauri::command]
pub async fn validate_license(
    request: ValidateLicenseRequest,
) -> Result<ValidateLicenseResult, String> {
    validate_fixed_params(&request.organization_id, Some(&request.benefit_id))?;

    let key = request.key.trim();
    let device_id = request.device_id.trim();
    if key.is_empty() || device_id.is_empty() {
        return Ok(ValidateLicenseResult {
            valid: false,
            status: "invalid".to_string(),
            message: "License key and device ID are required.".to_string(),
            expires_at: None,
            activation_required: false,
        });
    }

    let activation_id = request
        .activation_id
        .as_ref()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    let mut payload = json!({
        "key": key,
        "organization_id": POLAR_ORGANIZATION_ID,
        "benefit_id": POLAR_BENEFIT_ID,
        "conditions": {
            "device_id": device_id
        }
    });

    if let Some(activation_id) = activation_id {
        payload["activation_id"] = Value::String(activation_id);
    }

    let (status_code, body) =
        post_polar("/customer-portal/license-keys/validate", &payload).await?;
    if !(200..300).contains(&status_code) {
        let activation_required = is_activation_required(status_code, &body);
        return Ok(ValidateLicenseResult {
            valid: false,
            status: if activation_required {
                "activation_required".to_string()
            } else {
                "invalid".to_string()
            },
            message: extract_error_message(
                &body,
                &format!("License validation failed ({})", status_code),
            ),
            expires_at: extract_string(&body, &[&["license_key", "expires_at"], &["expires_at"]]),
            activation_required,
        });
    }

    let status = extract_string(&body, &[&["status"], &["license_key", "status"]])
        .unwrap_or_else(|| "unknown".to_string());
    let normalized_status = status.to_lowercase();
    let valid = normalized_status == "granted" || normalized_status == "active";
    let expires_at = extract_string(&body, &[&["license_key", "expires_at"], &["expires_at"]]);

    write_license_state(valid, expires_at.as_deref(), request.activation_id.as_deref());

    Ok(ValidateLicenseResult {
        valid,
        status: status.clone(),
        message: if valid {
            "License is valid.".to_string()
        } else {
            format!("License status is \"{}\".", status)
        },
        expires_at,
        activation_required: false,
    })
}

#[tauri::command]
pub async fn deactivate_license(
    request: DeactivateLicenseRequest,
) -> Result<DeactivateLicenseResult, String> {
    validate_fixed_params(&request.organization_id, None)?;

    let key = request.key.trim();
    let activation_id = request.activation_id.trim();
    if key.is_empty() || activation_id.is_empty() {
        return Ok(DeactivateLicenseResult {
            deactivated: false,
            status: "invalid".to_string(),
            message: "License key and activation ID are required.".to_string(),
        });
    }

    let payload = json!({
        "key": key,
        "organization_id": POLAR_ORGANIZATION_ID,
        "activation_id": activation_id,
    });

    let (status_code, body) =
        post_polar("/customer-portal/license-keys/deactivate", &payload).await?;
    if !(200..300).contains(&status_code) {
        return Ok(DeactivateLicenseResult {
            deactivated: false,
            status: "invalid".to_string(),
            message: extract_error_message(
                &body,
                &format!("License deactivation failed ({})", status_code),
            ),
        });
    }

    clear_license_state();

    Ok(DeactivateLicenseResult {
        deactivated: true,
        status: "deactivated".to_string(),
        message: "License deactivated on this device.".to_string(),
    })
}

#[tauri::command]
pub async fn open_external_url(url: String) -> Result<(), String> {
    let target = url.trim();
    if target.is_empty() {
        return Err("URL is required".to_string());
    }

    tracing::info!(url = %target, "Opening external URL");

    #[cfg(target_os = "macos")]
    {
        Command::new("open")
            .arg(target)
            .spawn()
            .map_err(|e| format!("Failed to open URL: {}", e))?;
    }

    #[cfg(target_os = "windows")]
    {
        Command::new("cmd")
            .args(["/C", "start", "", target])
            .spawn()
            .map_err(|e| format!("Failed to open URL: {}", e))?;
    }

    #[cfg(target_os = "linux")]
    {
        Command::new("xdg-open")
            .arg(target)
            .spawn()
            .map_err(|e| format!("Failed to open URL: {}", e))?;
    }

    Ok(())
}
