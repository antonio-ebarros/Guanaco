use ring::signature::{UnparsedPublicKey, ED25519};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

/// Chave pública oficial Ed25519 do Guanaco Pro (32 bytes).
/// A chave privada correspondente permanece exclusivamente com Antonio E. Barros (em `.keys/`).
const GUANACO_PRO_PUBKEY: [u8; 32] = [
    56, 90, 174, 234, 37, 209, 110, 127, 98, 206, 44, 36, 14, 153, 114, 204, 229, 79, 45, 115, 246,
    209, 152, 122, 198, 179, 19, 204, 77, 159, 93, 22,
];

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct LicensePayload {
    pub holder: String,
    #[serde(default)]
    pub email: String,
    pub tier: String,
    pub issued_at: String,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct LicenseStatus {
    pub active: bool,
    pub tier: String,
    pub holder: Option<String>,
    pub email: Option<String>,
    pub issued_at: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct StoredLicense {
    key: String,
}

impl Default for LicenseStatus {
    fn default() -> Self {
        Self {
            active: false,
            tier: "free".to_string(),
            holder: None,
            email: None,
            issued_at: None,
        }
    }
}

fn license_file(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir.join("license.json"))
}

fn decode_base64url(input: &str) -> Result<Vec<u8>, String> {
    let mut out = Vec::with_capacity(input.len() * 3 / 4);
    let mut buf: u32 = 0;
    let mut bits: u8 = 0;
    for b in input.bytes() {
        if b == b'=' {
            break;
        }
        let val: u32 = match b {
            b'A'..=b'Z' => (b - b'A') as u32,
            b'a'..=b'z' => (b - b'a' + 26) as u32,
            b'0'..=b'9' => (b - b'0' + 52) as u32,
            b'-' | b'+' => 62,
            b'_' | b'/' => 63,
            b' ' | b'\n' | b'\r' | b'\t' => continue,
            _ => return Err("Caractere inválido na chave de licença.".to_string()),
        };
        buf = (buf << 6) | val;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push(((buf >> bits) & 0xFF) as u8);
        }
    }
    Ok(out)
}

pub fn verify_license_key(raw_key: &str) -> Result<LicensePayload, String> {
    let cleaned: String = raw_key.chars().filter(|c| !c.is_whitespace()).collect();
    let body = cleaned
        .strip_prefix("GUANACO-PRO-")
        .ok_or("Formato inválido: a chave deve começar com GUANACO-PRO-.")?;
    let (payload_b64, sig_b64) = body
        .split_once('.')
        .ok_or("Formato inválido: assinatura ausente na chave.")?;
    if payload_b64.is_empty() || sig_b64.is_empty() {
        return Err("Chave de licença incompleta.".to_string());
    }

    let sig_bytes = decode_base64url(sig_b64)?;
    let pub_key = UnparsedPublicKey::new(&ED25519, &GUANACO_PRO_PUBKEY);
    pub_key
        .verify(payload_b64.as_bytes(), &sig_bytes)
        .map_err(|_| "Assinatura criptográfica inválida para esta chave Guanaco Pro.".to_string())?;

    let payload_bytes = decode_base64url(payload_b64)?;
    let payload: LicensePayload = serde_json::from_slice(&payload_bytes)
        .map_err(|_| "Dados da licença corrompidos.".to_string())?;
    if payload.tier != "pro" {
        return Err("Nível de licença desconhecido.".to_string());
    }
    Ok(payload)
}

pub fn is_pro_active(app: &AppHandle) -> bool {
    get_license_status_impl(app).active
}

fn get_license_status_impl(app: &AppHandle) -> LicenseStatus {
    let Ok(path) = license_file(app) else {
        return LicenseStatus::default();
    };
    let Ok(raw) = fs::read_to_string(path) else {
        return LicenseStatus::default();
    };
    let Ok(stored) = serde_json::from_str::<StoredLicense>(&raw) else {
        return LicenseStatus::default();
    };
    match verify_license_key(&stored.key) {
        Ok(p) => LicenseStatus {
            active: true,
            tier: "pro".to_string(),
            holder: Some(p.holder),
            email: if p.email.is_empty() { None } else { Some(p.email) },
            issued_at: Some(p.issued_at),
        },
        Err(_) => LicenseStatus::default(),
    }
}

#[tauri::command]
pub fn get_license_status(app: AppHandle) -> LicenseStatus {
    get_license_status_impl(&app)
}

#[tauri::command]
pub fn activate_license(app: AppHandle, key: String) -> Result<LicenseStatus, String> {
    let payload = verify_license_key(&key)?;
    let path = license_file(&app)?;
    let tmp = path.with_extension("json.tmp");
    let stored = StoredLicense {
        key: key.chars().filter(|c| !c.is_whitespace()).collect(),
    };
    let json = serde_json::to_string_pretty(&stored).map_err(|e| e.to_string())?;
    fs::write(&tmp, json).map_err(|e| e.to_string())?;
    fs::rename(&tmp, &path).map_err(|e| e.to_string())?;

    Ok(LicenseStatus {
        active: true,
        tier: "pro".to_string(),
        holder: Some(payload.holder),
        email: if payload.email.is_empty() {
            None
        } else {
            Some(payload.email)
        },
        issued_at: Some(payload.issued_at),
    })
}

#[tauri::command]
pub fn deactivate_license(app: AppHandle) -> Result<LicenseStatus, String> {
    if let Ok(path) = license_file(&app) {
        let _ = fs::remove_file(path);
    }
    Ok(LicenseStatus::default())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verifies_valid_ed25519_founder_key_and_rejects_tampering() {
        let key = "GUANACO-PRO-eyJob2xkZXIiOiJBbnRvbmlvIEUuIEJhcnJvcyIsImVtYWlsIjoiYW50b25pb0BndWFuYWNvLmFpIiwidGllciI6InBybyIsImlzc3VlZEF0IjoiMjAyNi0xMS0xOCJ9.ezvpmbuyR01W1VBwjLs1mzeysd-fS-nvSeG7h-ttuFobyoLIPB655zM-9xoXnVLNsbmb8XhSX10AxBpMvPmaBA";
        let payload = verify_license_key(key).expect("Chave oficial deve ser válida");
        assert_eq!(payload.holder, "Antonio E. Barros");
        assert_eq!(payload.tier, "pro");

        let tampered = "GUANACO-PRO-eyJob2xkZXIiOiJBbnRvbmlvIEUuIEJhcnJvcyIsImVtYWlsIjoiYW50b25pb0BndWFuYWNvLmFpIiwidGllciI6InBybyIsImlzc3VlZEF0IjoiMjAyNi0xMS0xOCJ9.AzvpmbuyR01W1VBwjLs1mzeysd-fS-nvSeG7h-ttuFobyoLIPB655zM-9xoXnVLNsbmb8XhSX10AxBpMvPmaBA";
        assert!(verify_license_key(tampered).is_err());
    }
}
