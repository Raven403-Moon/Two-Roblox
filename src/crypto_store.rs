use std::fs;
use std::path::Path;
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use pbkdf2::pbkdf2_hmac_array;
use sha2::Sha256;
use rand::RngCore;

use crate::win_dpapi::{dpapi_protect, dpapi_unprotect};

const PBKDF2_ITERATIONS: u32 = 480_000;
const SALT_LEN: usize = 16;
const NONCE_LEN: usize = 12;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Account {
    pub user_id: u64,
    pub username: String,
    pub display_name: String,
    pub cookie: String,
    pub group: String,
    pub memo: String,
    pub avatar_url: Option<String>,
    pub added_at: String,
    pub updated_at: String,
    #[serde(default)]
    pub saved_password: Option<String>,
}

pub struct AccountStore {
    filepath: String,
    key_cache_path: String,
    current_key: Option<[u8; 32]>,
    pub accounts: Vec<Account>,
}

impl AccountStore {
    pub fn new(filepath: &str) -> Self {
        Self {
            filepath: filepath.to_string(),
            key_cache_path: ".vault_key".to_string(),
            current_key: None,
            accounts: Vec::new(),
        }
    }

    pub fn exists(&self) -> bool {
        Path::new(&self.filepath).exists()
    }

    fn derive_key(password: &str, salt: &[u8]) -> [u8; 32] {
        pbkdf2_hmac_array::<Sha256, 32>(password.as_bytes(), salt, PBKDF2_ITERATIONS)
    }

    pub fn try_auto_unlock(&mut self) -> bool {
        if !self.exists() || !Path::new(&self.key_cache_path).exists() {
            return false;
        }

        let cipher_pw = match fs::read(&self.key_cache_path) {
            Ok(bytes) => bytes,
            Err(_) => return false,
        };

        let plain_pw = match dpapi_unprotect(&cipher_pw) {
            Ok(bytes) => match String::from_utf8(bytes) {
                Ok(s) => s,
                Err(_) => return false,
            },
            Err(_) => return false,
        };

        self.unlock(&plain_pw, false).is_ok()
    }

    pub fn unlock(&mut self, password: &str, remember: bool) -> Result<()> {
        if !self.exists() {
            let mut salt = [0u8; SALT_LEN];
            rand::thread_rng().fill_bytes(&mut salt);
            let key = Self::derive_key(password, &salt);
            self.current_key = Some(key);
            self.accounts = Vec::new();
            self.save_with_salt(&salt)?;

            if remember {
                let _ = self.save_dpapi_password(password);
            }
            return Ok(());
        }

        let raw = fs::read(&self.filepath)?;
        if raw.len() < SALT_LEN + NONCE_LEN {
            return Err(anyhow!("암호화 데이터 파일이 손상되었거나 너무 짧습니다."));
        }

        let salt = &raw[..SALT_LEN];
        let nonce = &raw[SALT_LEN..SALT_LEN + NONCE_LEN];
        let ciphertext = &raw[SALT_LEN + NONCE_LEN..];

        let key = Self::derive_key(password, salt);
        let cipher = Aes256Gcm::new_from_slice(&key)
            .map_err(|e| anyhow!("암호화 키 초기화 실패: {:?}", e))?;

        let decrypted = cipher
            .decrypt(Nonce::from_slice(nonce), ciphertext)
            .map_err(|_| anyhow!("비밀번호가 올바르지 않거나 복호화에 실패했습니다."))?;

        let accounts: Vec<Account> = serde_json::from_slice(&decrypted)
            .map_err(|e| anyhow!("계정 JSON 파싱 오류: {:?}", e))?;

        self.current_key = Some(key);
        self.accounts = accounts;

        if remember {
            let _ = self.save_dpapi_password(password);
        }

        Ok(())
    }

    fn save_with_salt(&self, salt: &[u8]) -> Result<()> {
        let key = self
            .current_key
            .as_ref()
            .ok_or_else(|| anyhow!("보관함이 잠금 해제되지 않았습니다."))?;

        let json_payload = serde_json::to_vec(&self.accounts)?;

        let mut nonce_bytes = [0u8; NONCE_LEN];
        rand::thread_rng().fill_bytes(&mut nonce_bytes);

        let cipher = Aes256Gcm::new_from_slice(key)
            .map_err(|e| anyhow!("AES-256-GCM 초기화 실패: {:?}", e))?;

        let ciphertext = cipher
            .encrypt(Nonce::from_slice(&nonce_bytes), json_payload.as_ref())
            .map_err(|e| anyhow!("암호화 실패: {:?}", e))?;

        let mut out = Vec::with_capacity(SALT_LEN + NONCE_LEN + ciphertext.len());
        out.extend_from_slice(salt);
        out.extend_from_slice(&nonce_bytes);
        out.extend_from_slice(&ciphertext);

        fs::write(&self.filepath, out)?;
        Ok(())
    }

    pub fn save(&self) -> Result<()> {
        let salt = if self.exists() {
            let raw = fs::read(&self.filepath)?;
            if raw.len() >= SALT_LEN {
                raw[..SALT_LEN].to_vec()
            } else {
                let mut s = vec![0u8; SALT_LEN];
                rand::thread_rng().fill_bytes(&mut s);
                s
            }
        } else {
            let mut s = vec![0u8; SALT_LEN];
            rand::thread_rng().fill_bytes(&mut s);
            s
        };

        self.save_with_salt(&salt)
    }

    fn save_dpapi_password(&self, password: &str) -> Result<()> {
        let protected = dpapi_protect(password.as_bytes())?;
        fs::write(&self.key_cache_path, protected)?;

        // Windows 파일 숨김 속성 부여
        unsafe {
            use std::ffi::OsStr;
            use std::os::windows::ffi::OsStrExt;
            let wide: Vec<u16> = OsStr::new(&self.key_cache_path)
                .encode_wide()
                .chain(std::iter::once(0))
                .collect();
            windows_sys::Win32::Storage::FileSystem::SetFileAttributesW(
                wide.as_ptr(),
                windows_sys::Win32::Storage::FileSystem::FILE_ATTRIBUTE_HIDDEN,
            );
        }

        Ok(())
    }

    #[allow(dead_code)]
    pub fn clear_remembered_password(&self) {
        if Path::new(&self.key_cache_path).exists() {
            let _ = fs::remove_file(&self.key_cache_path);
        }
    }

    pub fn reload(&mut self) -> Result<()> {
        if let Some(key) = self.current_key {
            let raw = fs::read(&self.filepath)?;
            if raw.len() >= SALT_LEN + NONCE_LEN {
                let nonce = &raw[SALT_LEN..SALT_LEN + NONCE_LEN];
                let ciphertext = &raw[SALT_LEN + NONCE_LEN..];
                let cipher = Aes256Gcm::new_from_slice(&key)
                    .map_err(|e| anyhow!("AES-256-GCM 초기화 실패: {:?}", e))?;
                let decrypted = cipher
                    .decrypt(Nonce::from_slice(nonce), ciphertext)
                    .map_err(|_| anyhow!("계정 보관함 복호화 실패"))?;
                self.accounts = serde_json::from_slice(&decrypted)
                    .map_err(|e| anyhow!("계정 JSON 파싱 실패: {:?}", e))?;
            }
        }
        Ok(())
    }

    pub fn update_cookie(&mut self, user_id: u64, new_cookie: &str) -> Result<()> {
        for acc in &mut self.accounts {
            if acc.user_id == user_id {
                acc.cookie = new_cookie.to_string();
                acc.updated_at = chrono_now_str();
                return self.save();
            }
        }
        Ok(())
    }

    pub fn upsert_account(
        &mut self,
        user_id: u64,
        username: &str,
        display_name: &str,
        cookie: &str,
        group: &str,
        avatar_url: Option<String>,
        saved_password: Option<String>,
    ) -> Result<()> {
        let now = chrono_now_str();

        for acc in &mut self.accounts {
            if acc.user_id == user_id {
                acc.username = username.to_string();
                acc.display_name = display_name.to_string();
                acc.cookie = cookie.to_string();
                if !group.is_empty() {
                    acc.group = group.to_string();
                }
                if avatar_url.is_some() {
                    acc.avatar_url = avatar_url;
                }
                if saved_password.is_some() {
                    acc.saved_password = saved_password;
                }
                acc.updated_at = now;
                return self.save();
            }
        }

        self.accounts.push(Account {
            user_id,
            username: username.to_string(),
            display_name: display_name.to_string(),
            cookie: cookie.to_string(),
            group: if group.is_empty() { "기본".to_string() } else { group.to_string() },
            memo: String::new(),
            avatar_url,
            added_at: now.clone(),
            updated_at: now,
            saved_password,
        });

        self.save()
    }

    pub fn remove_accounts(&mut self, user_ids: &[u64]) -> Result<()> {
        let set: std::collections::HashSet<u64> = user_ids.iter().cloned().collect();
        self.accounts.retain(|acc| !set.contains(&acc.user_id));
        self.save()
    }
}

fn chrono_now_str() -> String {
    // 경량 시간 문자열 생성
    use std::time::SystemTime;
    let dur = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default();
    format!("{}", dur.as_secs())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub auto_update_roblox: bool,
    pub auto_unlock: bool,
    pub last_place_id: String,
    pub last_job_id: String,
    pub launch_delay: f32,
    pub max_concurrent: usize,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            auto_update_roblox: true,
            auto_unlock: true,
            last_place_id: String::new(),
            last_job_id: String::new(),
            launch_delay: 3.0,
            max_concurrent: 10,
        }
    }
}

pub struct ConfigStore {
    filepath: String,
    pub config: AppConfig,
}

impl ConfigStore {
    pub fn new(filepath: &str) -> Self {
        let mut store = Self {
            filepath: filepath.to_string(),
            config: AppConfig::default(),
        };
        store.load();
        store
    }

    pub fn load(&mut self) {
        if let Ok(bytes) = fs::read(&self.filepath) {
            if let Ok(cfg) = serde_json::from_slice::<AppConfig>(&bytes) {
                self.config = cfg;
            }
        }
    }

    pub fn save(&self) -> Result<()> {
        let json = serde_json::to_string_pretty(&self.config)?;
        fs::write(&self.filepath, json)?;
        Ok(())
    }
}
