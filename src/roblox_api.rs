use std::time::{SystemTime, UNIX_EPOCH};
use anyhow::{anyhow, Result};
use rand::Rng;
use regex::Regex;
use reqwest::blocking::Client;
use reqwest::header::{HeaderMap, HeaderValue, CONTENT_TYPE, ORIGIN, REFERER, USER_AGENT};
use serde::{Deserialize, Serialize};

const DEFAULT_USER_AGENT: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserAuthenticatedResponse {
    pub id: u64,
    pub name: String,
    #[serde(rename = "displayName")]
    pub display_name: String,
}

#[derive(Debug)]
#[allow(dead_code)]
pub enum LoginResult {
    Success {
        cookie: String,
        user_info: UserAuthenticatedResponse,
    },
    ChallengeRequired {
        challenge_type: String,
        message: String,
    },
    Failed {
        message: String,
    },
}

pub struct RobloxAPI;

impl RobloxAPI {
    fn create_client() -> Result<Client> {
        Client::builder()
            .timeout(std::time::Duration::from_secs(12))
            .build()
            .map_err(|e| anyhow!("HTTP 클라이언트 생성 오류: {:?}", e))
    }

    pub fn validate_cookie(cookie: &str) -> Result<UserAuthenticatedResponse> {
        let cookie_clean = cookie.trim().trim_matches('"');
        if cookie_clean.is_empty() {
            return Err(anyhow!("쿠키가 비어 있습니다."));
        }

        let client = Self::create_client()?;
        let resp = client
            .get("https://users.roblox.com/v1/users/authenticated")
            .header(USER_AGENT, DEFAULT_USER_AGENT)
            .header("Cookie", format!(".ROBLOSECURITY={}", cookie_clean))
            .send()?;

        if resp.status().as_u16() != 200 {
            return Err(anyhow!("쿠키가 유효하지 않거나 만료되었습니다 (HTTP {})", resp.status()));
        }

        let info: UserAuthenticatedResponse = resp.json()?;
        Ok(info)
    }

    pub fn get_user_thumbnail(user_id: u64) -> Option<String> {
        let client = Self::create_client().ok()?;
        let url = format!(
            "https://thumbnails.roblox.com/v1/users/avatar-headshot?userIds={}&size=150x150&format=Png&isCircular=true",
            user_id
        );

        let resp = client
            .get(&url)
            .header(USER_AGENT, DEFAULT_USER_AGENT)
            .send()
            .ok()?;

        if resp.status().is_success() {
            #[derive(Deserialize)]
            struct ThumbnailData {
                data: Option<Vec<ThumbItem>>,
            }
            #[derive(Deserialize)]
            struct ThumbItem {
                state: Option<String>,
                #[serde(rename = "imageUrl")]
                image_url: Option<String>,
            }

            let val: ThumbnailData = resp.json().ok()?;
            if let Some(items) = val.data {
                if let Some(first) = items.into_iter().next() {
                    if first.state.as_deref() == Some("Completed") {
                        return first.image_url;
                    }
                }
            }
        }
        None
    }

    #[allow(dead_code)]
    pub fn login_with_credentials(username: &str, password: &str) -> Result<LoginResult> {
        let client = Self::create_client()?;
        let login_url = "https://auth.roblox.com/v2/login";

        let mut headers = HeaderMap::new();
        headers.insert(USER_AGENT, HeaderValue::from_static(DEFAULT_USER_AGENT));
        headers.insert(REFERER, HeaderValue::from_static("https://www.roblox.com/login"));
        headers.insert(ORIGIN, HeaderValue::from_static("https://www.roblox.com"));
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));

        let payload = serde_json::json!({
            "ctype": "Username",
            "cvalue": username.trim(),
            "password": password
        });

        // 1. CSRF 토큰 획득
        let r1 = client.post(login_url).headers(headers.clone()).json(&payload).send()?;
        let csrf = match r1.headers().get("x-csrf-token") {
            Some(v) => v.to_str()?.to_string(),
            None => {
                return Err(anyhow!("CSRF 토큰을 수신하지 못했습니다 (HTTP {})", r1.status()));
            }
        };

        headers.insert("x-csrf-token", HeaderValue::from_str(&csrf)?);

        // 2. 로그인 시도
        let r2 = client.post(login_url).headers(headers).json(&payload).send()?;

        if r2.status().as_u16() == 200 {
            // Set-Cookie 헤더에서 .ROBLOSECURITY 파싱
            for cookie_header in r2.headers().get_all("set-cookie") {
                if let Ok(s) = cookie_header.to_str() {
                    if let Some(idx) = s.find(".ROBLOSECURITY=") {
                        let sub = &s[idx + 15..];
                        let cookie_val = sub.split(';').next().unwrap_or("").trim().to_string();
                        if !cookie_val.is_empty() {
                            if let Ok(info) = Self::validate_cookie(&cookie_val) {
                                return Ok(LoginResult::Success {
                                    cookie: cookie_val,
                                    user_info: info,
                                });
                            }
                        }
                    }
                }
            }
            return Ok(LoginResult::Failed {
                message: "로그인은 성공했으나 보안 쿠키를 추출하지 못했습니다.".to_string(),
            });
        }

        if let Some(challenge_type) = r2.headers().get("rblx-challenge-type") {
            let c_type = challenge_type.to_str().unwrap_or("Unknown").to_string();
            return Ok(LoginResult::ChallengeRequired {
                challenge_type: c_type,
                message: "2단계 인증(2FA) 또는 보안 캡차가 요구되는 계정입니다.".to_string(),
            });
        }

        #[derive(Deserialize)]
        struct ErrorItem {
            message: Option<String>,
        }
        #[derive(Deserialize)]
        struct ErrorResponse {
            errors: Option<Vec<ErrorItem>>,
        }

        let err_msg = if let Ok(err_data) = r2.json::<ErrorResponse>() {
            err_data
                .errors
                .and_then(|mut v| v.pop())
                .and_then(|e| e.message)
                .unwrap_or_else(|| "아이디 또는 비밀번호가 올바르지 않습니다.".to_string())
        } else {
            "로그인에 실패했습니다.".to_string()
        };

        Ok(LoginResult::Failed { message: err_msg })
    }

    pub fn get_auth_ticket(cookie: &str) -> Result<String> {
        let client = Self::create_client()?;
        let cookie_clean = cookie.trim().trim_matches('"');
        let mut headers = HeaderMap::new();
        headers.insert(USER_AGENT, HeaderValue::from_static(DEFAULT_USER_AGENT));
        headers.insert(REFERER, HeaderValue::from_static("https://www.roblox.com/"));
        headers.insert(ORIGIN, HeaderValue::from_static("https://www.roblox.com"));
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        headers.insert("Cookie", HeaderValue::from_str(&format!(".ROBLOSECURITY={}", cookie_clean))?);

        let url = "https://auth.roblox.com/v1/authentication-ticket/";
        let r1 = client.post(url).headers(headers.clone()).send()?;
        let csrf = match r1.headers().get("x-csrf-token") {
            Some(v) => v.to_str()?.to_string(),
            None => {
                return Err(anyhow!("인증 티켓 CSRF 토큰 획득 실패 (HTTP {})", r1.status()));
            }
        };

        headers.insert("x-csrf-token", HeaderValue::from_str(&csrf)?);
        let r2 = client.post(url).headers(headers).send()?;

        if let Some(ticket) = r2.headers().get("rbx-authentication-ticket") {
            Ok(ticket.to_str()?.to_string())
        } else {
            Err(anyhow!("인증 티켓 발급 실패 (HTTP {})", r2.status()))
        }
    }

    pub fn build_direct_cli_args(
        exe_path: &str,
        auth_ticket: &str,
        place_id: Option<&str>,
        job_id: Option<&str>,
        locale: &str,
    ) -> Result<Vec<String>> {
        let mut rng = rand::thread_rng();
        let tracker_id: u64 = rng.gen_range(10_000_000_000..99_999_999_999);
        let launch_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();

        // 티켓 정제 (개행문자 및 인젝션 방어)
        let clean_ticket = auth_ticket.replace(['\r', '\n', '"', ' '], "");

        let mut args = vec![
            exe_path.to_string(),
            "--app".to_string(),
            "-a".to_string(),
            "https://www.roblox.com/Login/Negotiate.ashx".to_string(),
            "-t".to_string(),
            clean_ticket,
            "-b".to_string(),
            tracker_id.to_string(),
            format!("--launchtime={}", launch_time),
            "--rloc".to_string(),
            locale.to_string(),
            "--gloc".to_string(),
            locale.to_string(),
        ];

        if let Some(pid_raw) = place_id {
            let pid = pid_raw.trim();
            if !pid.is_empty() {
                // Place ID 화이트리스트 정규식 검증 (순수 숫자만 허용)
                let num_re = Regex::new(r"^\d+$").unwrap();
                if !num_re.is_match(pid) {
                    return Err(anyhow!("Place ID는 순수 숫자 형식이어야 합니다."));
                }

                let join_url = if let Some(jid_raw) = job_id {
                    let jid = jid_raw.trim();
                    if !jid.is_empty() {
                        let uuid_re = Regex::new(r"^[0-9a-zA-Z\-_]+$").unwrap();
                        if !uuid_re.is_match(jid) {
                            return Err(anyhow!("Job ID는 영숫자 및 하이픈 형식이어야 합니다."));
                        }
                        let encoded_jid = urlencoding::encode(jid);
                        format!(
                            "https://assetgame.roblox.com/game/PlaceLauncher.ashx?request=RequestGameJob&browserTrackerId={}&placeId={}&gameId={}&isPlayTogetherGame=false",
                            tracker_id, pid, encoded_jid
                        )
                    } else {
                        format!(
                            "https://assetgame.roblox.com/game/PlaceLauncher.ashx?request=RequestGame&browserTrackerId={}&placeId={}&isPlayTogetherGame=false",
                            tracker_id, pid
                        )
                    }
                } else {
                    format!(
                        "https://assetgame.roblox.com/game/PlaceLauncher.ashx?request=RequestGame&browserTrackerId={}&placeId={}&isPlayTogetherGame=false",
                        tracker_id, pid
                    )
                };

                args.push("-j".to_string());
                args.push(join_url);
            }
        }

        Ok(args)
    }
}
