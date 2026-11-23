use std::fs;
use std::path::PathBuf;
use std::process::{Child, Command};
use std::time::{Duration, Instant};
use anyhow::{anyhow, Result};
use serde_json::Value;
use tungstenite::{connect, Message};

pub struct WebLoginManager;

impl WebLoginManager {
    /// 설치된 Chromium 기반 브라우저(Edge, Chrome, Brave) 탐색
    pub fn find_browser() -> Option<String> {
        let candidates = [
            r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe",
            r"C:\Program Files\Microsoft\Edge\Application\msedge.exe",
            r"C:\Program Files\Google\Chrome\Application\chrome.exe",
            r"C:\Program Files (x86)\Google\Chrome\Application\chrome.exe",
            r"C:\Program Files\BraveSoftware\Brave-Browser\Application\brave.exe",
        ];

        for path in candidates {
            if std::path::Path::new(path).exists() {
                return Some(path.to_string());
            }
        }

        if let Ok(local) = std::env::var("LOCALAPPDATA") {
            let edge_local = format!(r"{}\Microsoft\Edge\Application\msedge.exe", local);
            if std::path::Path::new(&edge_local).exists() {
                return Some(edge_local);
            }
            let chrome_local = format!(r"{}\Google\Chrome\Application\chrome.exe", local);
            if std::path::Path::new(&chrome_local).exists() {
                return Some(chrome_local);
            }
        }

        None
    }

    /// 공식 로그인 웹 창을 독립 앱 모드로 띄우고 완료 시까지 세션 쿠키를 자동 감지
    pub fn start_web_login_flow<F>(on_status: F) -> Result<Option<String>>
    where
        F: Fn(&str),
    {
        let browser_path = Self::find_browser()
            .ok_or_else(|| anyhow!("PC에 설치된 Microsoft Edge 또는 Chrome을 찾을 수 없습니다."))?;

        let rand_num = rand::random::<u16>() % 1000;
        let port = 9300 + rand_num;
        let temp_dir_name = format!("TwoRoblox_WebLogin_{}_{}", std::process::id(), rand_num);
        let profile_dir: PathBuf = std::env::temp_dir().join(temp_dir_name);

        let _ = fs::create_dir_all(&profile_dir);

        on_status("공식 Roblox 로그인 웹 창을 실행합니다...");

        // Chromium 앱 모드로 전용 로그인 창 생성 (주소창, 탭 없는 깔끔한 독립 창)
        let mut child: Child = Command::new(&browser_path)
            .arg("--app=https://www.roblox.com/login")
            .arg(format!("--user-data-dir={}", profile_dir.display()))
            .arg(format!("--remote-debugging-port={}", port))
            .arg("--no-first-run")
            .arg("--no-default-browser-check")
            .arg("--window-size=520,740")
            .spawn()
            .map_err(|e| anyhow!("브라우저 창 실행 실패: {:?}", e))?;

        on_status("로그인 창이 열렸습니다. 아이디/비밀번호 입력 및 2FA, 캡차를 완료하세요.");

        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_millis(800))
            .build()?;

        let json_list_url = format!("http://127.0.0.1:{}/json/list", port);
        let start_time = Instant::now();
        let timeout = Duration::from_secs(300); // 최대 5분 대기

        let mut captured_cookie: Option<String> = None;

        while start_time.elapsed() < timeout {
            // 사용자가 창을 닫았는지 확인
            if let Ok(Some(_)) = child.try_wait() {
                on_status("사용자가 로그인 창을 닫았습니다.");
                break;
            }

            // CDP 대상 페이지 목록 확인
            if let Ok(resp) = client.get(&json_list_url).send() {
                if let Ok(val) = resp.json::<Value>() {
                    if let Some(arr) = val.as_array() {
                        // roblox.com 페이지 또는 기본 page 검색
                        let target_ws = arr.iter().find_map(|t| {
                            let url_str = t.get("url").and_then(|u| u.as_str()).unwrap_or("");
                            let ws = t.get("webSocketDebuggerUrl").and_then(|w| w.as_str())?;
                            if url_str.contains("roblox.com") || url_str.contains("login") {
                                Some(ws.to_string())
                            } else {
                                None
                            }
                        }).or_else(|| {
                            arr.iter().find_map(|t| {
                                t.get("webSocketDebuggerUrl").and_then(|w| w.as_str()).map(|s| s.to_string())
                            })
                        });

                        if let Some(ws_url) = target_ws {
                            if let Ok(Some(cookie)) = Self::query_cookie_via_cdp(&ws_url) {
                                captured_cookie = Some(cookie);
                                on_status("로그인 성공 확인! 보안 세션 쿠키를 성공적으로 획득했습니다.");
                                break;
                            }
                        }
                    }
                }
            }

            std::thread::sleep(Duration::from_millis(1000));
        }

        // 브라우저 프로세스 종료 및 임시 디렉터리 정리
        let _ = child.kill();
        let _ = child.wait();
        std::thread::sleep(Duration::from_millis(300));
        let _ = fs::remove_dir_all(&profile_dir);

        Ok(captured_cookie)
    }

    /// CDP WebSocket 연결을 통해 .ROBLOSECURITY 쿠키 조회
    fn query_cookie_via_cdp(ws_url: &str) -> Result<Option<String>> {
        let (mut socket, _) = connect(ws_url).map_err(|e| anyhow!("CDP 연결 실패: {:?}", e))?;

        // 소켓 읽기 타임아웃 600ms 설정
        if let tungstenite::stream::MaybeTlsStream::Plain(ref s) = socket.get_ref() {
            let _ = s.set_read_timeout(Some(Duration::from_millis(600)));
        }

        let cmd = serde_json::json!({
            "id": 1,
            "method": "Network.getCookies",
            "params": {
                "urls": ["https://www.roblox.com"]
            }
        });

        socket.send(Message::Text(cmd.to_string()))?;

        // 응답 수신 대기 (최대 3개 메시지 확인)
        for _ in 0..3 {
            match socket.read() {
                Ok(Message::Text(text)) => {
                    if let Ok(parsed) = serde_json::from_str::<Value>(&text) {
                        if let Some(cookies) = parsed.pointer("/result/cookies").and_then(|c| c.as_array()) {
                            for c in cookies {
                                if c.get("name").and_then(|n| n.as_str()) == Some(".ROBLOSECURITY") {
                                    if let Some(val) = c.get("value").and_then(|v| v.as_str()) {
                                        let trimmed = val.trim();
                                        if !trimmed.is_empty() {
                                            let _ = socket.close(None);
                                            return Ok(Some(trimmed.to_string()));
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                _ => break,
            }
        }

        let _ = socket.close(None);
        Ok(None)
    }
}
