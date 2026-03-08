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
    let dir = std::path::Path::new(&home).join(".monosub");
    Some(dir.join("license_state.json"))
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
    let _ = std::fs::write(&path, hex);
}

fn clear_license_state() {
    if let Some(path) = license_state_path() {
        let _ = std::fs::remove_file(path);
    }
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
