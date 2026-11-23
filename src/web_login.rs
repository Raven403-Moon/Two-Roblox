use std::time::{Duration, Instant};
use anyhow::Result;
use wry::{Rect, WebView, WebViewBuilder};
use wry::raw_window_handle::HasWindowHandle;

pub struct EmbeddedWebLogin {
    pub webview: WebView,
    pub status: String,
    pub last_cookie: Option<String>,
    pub last_check: Instant,
    pub last_bounds: Option<(i32, i32, u32, u32)>,
}

impl EmbeddedWebLogin {
    pub fn new<W: HasWindowHandle>(
        window: &W,
        x: f64,
        y: f64,
        w: f64,
        h: f64,
    ) -> Result<Self> {
        let bounds = Rect {
            position: wry::dpi::Position::Logical(wry::dpi::LogicalPosition::new(x, y)),
            size: wry::dpi::Size::Logical(wry::dpi::LogicalSize::new(w, h)),
        };

        let webview = WebViewBuilder::new()
            .with_url("https://www.roblox.com/login")
            .with_bounds(bounds)
            .build_as_child(window)?;

        Ok(Self {
            webview,
            status: "공식 로그인 화면에서 로그인해 주세요. (세션 자동 감지 대기 중)".to_string(),
            last_cookie: None,
            last_check: Instant::now(),
            last_bounds: Some((x as i32, y as i32, w as u32, h as u32)),
        })
    }

    pub fn set_bounds(&mut self, x: f64, y: f64, w: f64, h: f64) {
        let ix = x as i32;
        let iy = y as i32;
        let iw = w as u32;
        let ih = h as u32;

        if let Some((lx, ly, lw, lh)) = self.last_bounds {
            if lx == ix && ly == iy && lw == iw && lh == ih {
                return;
            }
        }

        self.last_bounds = Some((ix, iy, iw, ih));
        let _ = self.webview.set_bounds(Rect {
            position: wry::dpi::Position::Logical(wry::dpi::LogicalPosition::new(x, y)),
            size: wry::dpi::Size::Logical(wry::dpi::LogicalSize::new(w, h)),
        });
    }

    pub fn set_visible(&self, visible: bool) {
        let _ = self.webview.set_visible(visible);
    }

    pub fn reload(&self) {
        let _ = self.webview.load_url("https://www.roblox.com/login");
    }

    pub fn clear_session(&mut self) {
        if let Ok(cookies) = self.webview.cookies() {
            for c in cookies {
                let _ = self.webview.delete_cookie(&c);
            }
        }
        let _ = self.webview.load_url("https://www.roblox.com/login");
        self.last_cookie = None;
        self.status = "세션을 초기화했습니다. 새 계정으로 로그인해 주세요.".to_string();
    }

    /// 쿠키 폴링: .ROBLOSECURITY 쿠키를 감지하면 반환
    pub fn poll_cookie(&mut self) -> Option<String> {
        if self.last_check.elapsed() < Duration::from_millis(500) {
            return None;
        }
        self.last_check = Instant::now();

        if let Ok(cookies) = self.webview.cookies() {
            for c in cookies {
                if c.name() == ".ROBLOSECURITY" {
                    let val = c.value().trim().to_string();
                    if !val.is_empty() && self.last_cookie.as_deref() != Some(&val) {
                        self.last_cookie = Some(val.clone());
                        return Some(val);
                    }
                }
            }
        }

        None
    }
}
