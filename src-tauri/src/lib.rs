use hmac::{Hmac, KeyInit, Mac};
use serde::{Deserialize, Serialize};
use sha1::Sha1;
use sha2::{Sha256, Sha512};
use std::fs;
use std::sync::Mutex;
use std::time::SystemTime;
use tauri::{AppHandle, Manager};
use totp_rs::{Algorithm, Secret, TOTP};

type HmacSha1 = Hmac<Sha1>;
type HmacSha256 = Hmac<Sha256>;
type HmacSha512 = Hmac<Sha512>;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
enum OtpType {
    Totp,
    Hotp,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Account {
    id: String,
    issuer: String,
    label: String,
    secret: String,
    digits: usize,
    period: u64,
    algorithm: String,
    #[serde(default = "default_otp_type")]
    otp_type: OtpType,
    #[serde(default)]
    counter: u64,
}

fn default_otp_type() -> OtpType {
    OtpType::Totp
}

#[derive(Debug, Clone, Serialize)]
struct AccountWithCode {
    id: String,
    issuer: String,
    label: String,
    code: String,
    digits: usize,
    period: u64,
    seconds_remaining: u64,
    otp_type: String,
    counter: u64,
}

struct AppState {
    accounts: Mutex<Vec<Account>>,
}

fn storage_path(app: &AppHandle) -> std::path::PathBuf {
    let dir = app
        .path()
        .app_data_dir()
        .expect("failed to resolve app data dir");
    fs::create_dir_all(&dir).ok();
    dir.join("accounts.json")
}

fn load_accounts(app: &AppHandle) -> Vec<Account> {
    let path = storage_path(app);
    if path.exists() {
        let data = fs::read_to_string(&path).unwrap_or_default();
        serde_json::from_str(&data).unwrap_or_default()
    } else {
        Vec::new()
    }
}

fn save_accounts(app: &AppHandle, accounts: &[Account]) {
    let path = storage_path(app);
    if let Ok(data) = serde_json::to_string_pretty(accounts) {
        fs::write(path, data).ok();
    }
}

fn parse_algorithm(alg: &str) -> Algorithm {
    match alg.to_uppercase().as_str() {
        "SHA256" => Algorithm::SHA256,
        "SHA512" => Algorithm::SHA512,
        _ => Algorithm::SHA1,
    }
}

fn decode_secret(secret: &str) -> Result<Vec<u8>, String> {
    let padded = pad_base32(secret);
    Secret::Encoded(padded)
        .to_bytes()
        .map_err(|e| format!("Invalid secret: {}", e))
}

fn pad_base32(s: &str) -> String {
    let mut s = s.to_string();
    let remainder = s.len() % 8;
    if remainder != 0 {
        s.extend(std::iter::repeat('=').take(8 - remainder));
    }
    s
}

/// Generate HOTP code for a given counter using raw HMAC
fn generate_hotp(secret: &[u8], counter: u64, digits: usize, algorithm: Algorithm) -> String {
    let counter_bytes = counter.to_be_bytes();

    let hmac_result = match algorithm {
        Algorithm::SHA1 => {
            let mut mac = HmacSha1::new_from_slice(secret).expect("HMAC accepts any key length");
            mac.update(&counter_bytes);
            mac.finalize().into_bytes().to_vec()
        }
        Algorithm::SHA256 => {
            let mut mac =
                HmacSha256::new_from_slice(secret).expect("HMAC accepts any key length");
            mac.update(&counter_bytes);
            mac.finalize().into_bytes().to_vec()
        }
        Algorithm::SHA512 => {
            let mut mac =
                HmacSha512::new_from_slice(secret).expect("HMAC accepts any key length");
            mac.update(&counter_bytes);
            mac.finalize().into_bytes().to_vec()
        }
    };

    let offset = (hmac_result.last().unwrap() & 0x0f) as usize;
    let binary = ((hmac_result[offset] as u32 & 0x7f) << 24)
        | ((hmac_result[offset + 1] as u32) << 16)
        | ((hmac_result[offset + 2] as u32) << 8)
        | (hmac_result[offset + 3] as u32);

    let otp = binary % 10u32.pow(digits as u32);
    format!("{:0>width$}", otp, width = digits)
}

fn generate_totp_code(account: &Account) -> Result<(String, u64), String> {
    let algorithm = parse_algorithm(&account.algorithm);
    let secret_bytes = decode_secret(&account.secret)?;

    let totp = TOTP::new_unchecked(
        algorithm,
        account.digits,
        1,
        account.period,
        secret_bytes,
        Some(account.issuer.clone()),
        account.label.clone(),
    );

    let time = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_err(|e| format!("Time error: {}", e))?
        .as_secs();

    let code = totp.generate(time);
    let elapsed = time % account.period;
    let remaining = account.period - elapsed;

    Ok((code, remaining))
}

fn generate_hotp_code(account: &Account) -> Result<String, String> {
    let algorithm = parse_algorithm(&account.algorithm);
    let secret_bytes = decode_secret(&account.secret)?;
    Ok(generate_hotp(
        &secret_bytes,
        account.counter,
        account.digits,
        algorithm,
    ))
}

#[tauri::command]
fn get_all_codes(state: tauri::State<'_, AppState>) -> Result<Vec<AccountWithCode>, String> {
    let accounts = state.accounts.lock().map_err(|e| e.to_string())?;
    let mut results = Vec::new();

    for account in accounts.iter() {
        let (code, remaining) = if account.otp_type == OtpType::Hotp {
            match generate_hotp_code(account) {
                Ok(code) => (code, 0u64),
                Err(e) => (format!("ERR: {}", e), 0),
            }
        } else {
            match generate_totp_code(account) {
                Ok((code, rem)) => (code, rem),
                Err(e) => (format!("ERR: {}", e), 0),
            }
        };

        let type_str = match account.otp_type {
            OtpType::Hotp => "hotp",
            OtpType::Totp => "totp",
        };

        results.push(AccountWithCode {
            id: account.id.clone(),
            issuer: account.issuer.clone(),
            label: account.label.clone(),
            code,
            digits: account.digits,
            period: account.period,
            seconds_remaining: remaining,
            otp_type: type_str.to_string(),
            counter: account.counter,
        });
    }

    Ok(results)
}

/// Advance HOTP counter and return the new code
#[tauri::command]
fn next_hotp_code(
    id: String,
    app: AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<String, String> {
    let mut accounts = state.accounts.lock().map_err(|e| e.to_string())?;
    let account = accounts
        .iter_mut()
        .find(|a| a.id == id)
        .ok_or("Account not found")?;

    if account.otp_type != OtpType::Hotp {
        return Err("Not an HOTP account".to_string());
    }

    account.counter += 1;
    let code = generate_hotp_code(account)?;
    save_accounts(&app, &accounts);
    Ok(code)
}

/// Clean Ente Auth metadata and normalize the URI before parsing
fn clean_otpauth_uri(uri: &str) -> String {
    let uri = uri.trim().to_string();

    let base_and_query: Vec<&str> = uri.splitn(2, '?').collect();
    if base_and_query.len() < 2 {
        return uri;
    }

    let base = base_and_query[0];
    let query = base_and_query[1];

    let known_params = [
        "secret", "issuer", "algorithm", "digits", "period", "counter",
    ];

    let filtered: Vec<&str> = query
        .split('&')
        .filter(|param| {
            let key = param.split('=').next().unwrap_or("");
            known_params.contains(&key)
        })
        .collect();

    format!("{}?{}", base, filtered.join("&"))
}

#[tauri::command]
fn add_account(
    issuer: String,
    label: String,
    secret: String,
    digits: Option<usize>,
    period: Option<u64>,
    algorithm: Option<String>,
    otp_type: Option<String>,
    counter: Option<u64>,
    app: AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<String, String> {
    let clean_secret = secret.replace(' ', "").to_uppercase();

    decode_secret(&clean_secret)?;

    let otype = match otp_type.as_deref() {
        Some("hotp") => OtpType::Hotp,
        _ => OtpType::Totp,
    };

    let account = Account {
        id: format!(
            "{}",
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_millis()
        ),
        issuer,
        label,
        secret: clean_secret,
        digits: digits.unwrap_or(6),
        period: period.unwrap_or(30),
        algorithm: algorithm.unwrap_or_else(|| "SHA1".to_string()),
        otp_type: otype,
        counter: counter.unwrap_or(0),
    };

    let id = account.id.clone();
    let mut accounts = state.accounts.lock().map_err(|e| e.to_string())?;
    accounts.push(account);
    save_accounts(&app, &accounts);

    Ok(id)
}

#[tauri::command]
fn parse_otpauth_uri(uri: String) -> Result<serde_json::Value, String> {
    let cleaned = clean_otpauth_uri(&uri);
    let is_hotp = cleaned
        .to_lowercase()
        .starts_with("otpauth://hotp/");

    if is_hotp {
        // Parse HOTP URI manually since totp-rs only handles TOTP
        let url = url::Url::parse(&cleaned)
            .map_err(|e| format!("Invalid URI: {}", e))?;

        let path = url.path().trim_start_matches('/');
        let path_decoded = urlencoding::decode(path)
            .map_err(|e| format!("Decode error: {}", e))?;

        let (issuer_from_path, label) = if let Some(pos) = path_decoded.find(':') {
            (
                path_decoded[..pos].to_string(),
                path_decoded[pos + 1..].to_string(),
            )
        } else {
            (String::new(), path_decoded.to_string())
        };

        let mut secret = String::new();
        let mut issuer = issuer_from_path;
        let mut digits = 6usize;
        let mut period = 30u64;
        let mut algorithm = "SHA1".to_string();
        let mut counter = 0u64;

        for (key, value) in url.query_pairs() {
            match key.as_ref() {
                "secret" => secret = value.to_uppercase(),
                "issuer" => issuer = value.to_string(),
                "digits" => digits = value.parse().unwrap_or(6),
                "period" => period = value.parse().unwrap_or(30),
                "algorithm" => algorithm = value.to_uppercase(),
                "counter" => counter = value.parse().unwrap_or(0),
                _ => {}
            }
        }

        if secret.is_empty() {
            return Err("No secret found in URI".to_string());
        }

        Ok(serde_json::json!({
            "issuer": issuer,
            "label": label,
            "secret": secret,
            "digits": digits,
            "period": period,
            "algorithm": algorithm,
            "otp_type": "hotp",
            "counter": counter,
        }))
    } else {
        let totp = TOTP::from_url(&cleaned)
            .map_err(|e| format!("Invalid otpauth URI: {}", e))?;

        Ok(serde_json::json!({
            "issuer": totp.issuer.unwrap_or_default(),
            "label": totp.account_name,
            "secret": Secret::Raw(totp.secret).to_encoded().to_string(),
            "digits": totp.digits,
            "period": totp.step,
            "algorithm": format!("{:?}", totp.algorithm),
            "otp_type": "totp",
            "counter": 0,
        }))
    }
}

#[tauri::command]
fn copy_to_clipboard(text: String) -> Result<(), String> {
    let mut clipboard = arboard::Clipboard::new().map_err(|e| format!("Clipboard error: {}", e))?;
    clipboard
        .set_text(text)
        .map_err(|e| format!("Failed to copy: {}", e))
}

#[tauri::command]
fn delete_account(
    id: String,
    app: AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let mut accounts = state.accounts.lock().map_err(|e| e.to_string())?;
    accounts.retain(|a| a.id != id);
    save_accounts(&app, &accounts);
    Ok(())
}

#[tauri::command]
fn verify_account(
    id: String,
    state: tauri::State<'_, AppState>,
) -> Result<serde_json::Value, String> {
    let accounts = state.accounts.lock().map_err(|e| e.to_string())?;
    let account = accounts
        .iter()
        .find(|a| a.id == id)
        .ok_or("Account not found")?;

    let secret_bytes = decode_secret(&account.secret)?;
    let time = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_err(|e| format!("Time error: {}", e))?
        .as_secs();

    let (code, remaining) = if account.otp_type == OtpType::Totp {
        let r = generate_totp_code(account)?;
        (r.0, r.1)
    } else {
        (generate_hotp_code(account)?, 0)
    };

    Ok(serde_json::json!({
        "stored_secret": account.secret,
        "secret_length_bytes": secret_bytes.len(),
        "algorithm": account.algorithm,
        "digits": account.digits,
        "period": account.period,
        "otp_type": format!("{:?}", account.otp_type),
        "unix_time": time,
        "current_code": code,
        "seconds_remaining": remaining,
    }))
}

#[cfg(target_os = "macos")]
#[allow(deprecated)]
fn set_macos_dock_icon() {
    unsafe {
        use cocoa::appkit::NSImage;
        use cocoa::base::nil;
        use cocoa::foundation::NSData;
        use objc::{msg_send, sel, sel_impl};

        let icon_bytes = include_bytes!("../icons/128x128@2x.png");
        let data = NSData::dataWithBytes_length_(
            nil,
            icon_bytes.as_ptr() as *const std::ffi::c_void,
            icon_bytes.len() as u64,
        );
        let icon = NSImage::initWithData_(NSImage::alloc(nil), data);
        let app: cocoa::base::id = msg_send![
            cocoa::appkit::NSApp(),
            setApplicationIconImage: icon
        ];
        let _ = app;
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let accounts = load_accounts(&app.handle());
            app.manage(AppState {
                accounts: Mutex::new(accounts),
            });

            #[cfg(target_os = "macos")]
            set_macos_dock_icon();

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_all_codes,
            add_account,
            delete_account,
            parse_otpauth_uri,
            next_hotp_code,
            copy_to_clipboard,
            verify_account,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_secret() -> Vec<u8> {
        use sha2::Digest;
        Sha256::digest(b"test-only-seed").to_vec()
    }

    fn test_secret_base32() -> String {
        data_encoding::BASE32_NOPAD
            .encode(&test_secret()[..10])
            .to_uppercase()
    }

    #[test]
    fn test_totp_generates_correct_length() {
        let secret = test_secret();
        let totp = TOTP::new_unchecked(
            Algorithm::SHA1, 6, 1, 30,
            secret, None, String::new(),
        );
        let code = totp.generate(1_700_000_000);
        assert_eq!(code.len(), 6);
        assert!(code.chars().all(|c| c.is_ascii_digit()));
    }

    #[test]
    fn test_totp_deterministic() {
        let secret = test_secret();
        let totp = TOTP::new_unchecked(
            Algorithm::SHA1, 6, 1, 30,
            secret, None, String::new(),
        );
        let code_a = totp.generate(1_700_000_000);
        let code_b = totp.generate(1_700_000_000);
        assert_eq!(code_a, code_b);
    }

    #[test]
    fn test_totp_and_hotp_agree() {
        let secret = test_secret();
        let totp = TOTP::new_unchecked(
            Algorithm::SHA1, 6, 1, 30,
            secret.clone(), None, String::new(),
        );
        let time: u64 = 1_700_000_000;
        let counter = time / 30;
        let code_hotp = generate_hotp(&secret, counter, 6, Algorithm::SHA1);
        assert_eq!(totp.generate(time), code_hotp);
    }

    #[test]
    fn test_hotp_8_digits() {
        let secret = test_secret();
        let code = generate_hotp(&secret, 0, 8, Algorithm::SHA1);
        assert_eq!(code.len(), 8);
        assert!(code.chars().all(|c| c.is_ascii_digit()));
    }

    #[test]
    fn test_hotp_different_algorithms() {
        let secret = test_secret();
        let sha1 = generate_hotp(&secret, 1, 6, Algorithm::SHA1);
        let sha256 = generate_hotp(&secret, 1, 6, Algorithm::SHA256);
        let sha512 = generate_hotp(&secret, 1, 6, Algorithm::SHA512);
        assert_ne!(sha1, sha256);
        assert_ne!(sha256, sha512);
    }

    #[test]
    fn test_pad_base32() {
        assert_eq!(pad_base32("AAAAAAAA"), "AAAAAAAA");   // 8 chars, no pad
        assert_eq!(pad_base32("AAAAAAAAAAAAAAAA"), "AAAAAAAAAAAAAAAA"); // 16 chars
        assert_eq!(pad_base32("AAA"), "AAA=====");         // 3 chars -> pad to 8
        assert_eq!(pad_base32("AAAAA"), "AAAAA===");       // 5 chars -> pad to 8
    }

    #[test]
    fn test_decode_secret_roundtrip() {
        let b32 = test_secret_base32();
        let decoded = decode_secret(&b32);
        assert!(decoded.is_ok());
        assert!(!decoded.unwrap().is_empty());
    }

    #[test]
    fn test_decode_secret_invalid() {
        let result = decode_secret("!!!INVALID!!!");
        assert!(result.is_err());
    }
}
