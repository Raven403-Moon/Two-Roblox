use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;
use anyhow::{anyhow, Result};
use reqwest::blocking::Client;
use serde::Deserialize;

const VERSION_API: &str =
    "https://clientsettings.roblox.com/v2/client-version/WindowsPlayer/channel/LIVE";
const OFFICIAL_INSTALLER_URL: &str = "https://www.roblox.com/download/client?os=win";

#[derive(Deserialize)]
struct VersionResponse {
    #[serde(rename = "clientVersionUpload")]
    client_version_upload: String,
}

pub struct RobloxUpdater;

impl RobloxUpdater {
    pub fn get_live_version() -> Result<String> {
        let client = Client::builder()
            .timeout(Duration::from_secs(10))
            .build()?;
        let resp = client.get(VERSION_API).send()?;
        if !resp.status().is_success() {
            return Err(anyhow!("공식 버전 조회 실패 (HTTP {})", resp.status()));
        }

        let data: VersionResponse = resp.json()?;
        Ok(data.client_version_upload)
    }

    pub fn find_local_player_exe(target_version: Option<&str>) -> Option<(String, String)> {
        let mut base_dirs = Vec::new();
        if let Ok(local) = env::var("LOCALAPPDATA") {
            base_dirs.push(PathBuf::from(local).join("Roblox").join("Versions"));
        }
        if let Ok(pf86) = env::var("PROGRAMFILES(X86)") {
            base_dirs.push(PathBuf::from(pf86).join("Roblox").join("Versions"));
        }

        let mut candidates: Vec<(std::time::SystemTime, String, String)> = Vec::new();

        for base in base_dirs {
            if !base.is_dir() {
                continue;
            }
            if let Ok(entries) = fs::read_dir(&base) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if !path.is_dir() {
                        continue;
                    }
                    let file_name = path.file_name().unwrap_or_default().to_string_lossy();
                    if !file_name.starts_with("version-") {
                        continue;
                    }

                    let exe_path = path.join("RobloxPlayerBeta.exe");
                    if exe_path.is_file() {
                        let ver_str = file_name.to_string();
                        if let Some(target) = target_version {
                            if ver_str == target {
                                return Some((ver_str, exe_path.to_string_lossy().to_string()));
                            }
                        }

                        let mtime = fs::metadata(&exe_path)
                            .and_then(|m| m.modified())
                            .unwrap_or(std::time::UNIX_EPOCH);

                        candidates.push((mtime, ver_str, exe_path.to_string_lossy().to_string()));
                    }
                }
            }
        }

        if candidates.is_empty() {
            return None;
        }

        candidates.sort_by(|a, b| b.0.cmp(&a.0));
        let (_, ver, exe) = candidates.remove(0);
        Some((ver, exe))
    }

    pub fn check_status() -> Result<(bool, Option<String>, String, Option<String>)> {
        let live_ver = Self::get_live_version()?;
        let local_info = Self::find_local_player_exe(Some(&live_ver));

        let (local_ver, exe_path) = match local_info {
            Some((v, p)) => (Some(v), Some(p)),
            None => {
                let fallback = Self::find_local_player_exe(None);
                match fallback {
                    Some((v, p)) => (Some(v), Some(p)),
                    None => (None, None),
                }
            }
        };

        let is_latest = local_ver.as_deref() == Some(&live_ver) && exe_path.is_some();
        Ok((is_latest, local_ver, live_ver, exe_path))
    }

    pub fn run_official_update() -> Result<String> {
        // 1회용 격리 임시 디렉터리 생성 (심볼릭 링크 공격 및 파일 변조 차단)
        let tmp_dir = tempfile::tempdir()?;
        let installer_path = tmp_dir.path().join("RobloxPlayerInstaller.exe");

        let client = Client::builder()
            .timeout(Duration::from_secs(45))
            .build()?;

        let mut resp = client.get(OFFICIAL_INSTALLER_URL).send()?;
        if !resp.status().is_success() {
            return Err(anyhow!("인스톨러 다운로드 실패 (HTTP {})", resp.status()));
        }

        let mut file = fs::File::create(&installer_path)?;
        std::io::copy(&mut resp, &mut file)?;
        drop(file);

        // 공식 인스톨러 실행
        let _ = Command::new(&installer_path).status()?;

        std::thread::sleep(Duration::from_secs(2));

        let live_ver = Self::get_live_version().ok();
        let local_info = Self::find_local_player_exe(live_ver.as_deref());

        if let Some((_, new_exe)) = local_info {
            Ok(new_exe)
        } else {
            Err(anyhow!("설치가 완료되었으나 새 RobloxPlayerBeta.exe를 찾지 못했습니다."))
        }
    }
}
