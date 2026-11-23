use std::collections::HashSet;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::time::{Duration, Instant};

use eframe::egui::{
    self, Align, Button, Color32, FontFamily, FontId, Layout, Response, RichText, Rounding,
    ScrollArea, Stroke, TextEdit, TextStyle, Ui, Vec2, Window,
};
use regex::Regex;

use crate::crypto_store::{Account, AccountStore, ConfigStore};
use crate::multi_instance::{is_admin, restart_as_admin, InstanceInfo, MultiInstanceManager};
use crate::roblox_api::RobloxAPI;
use crate::updater::RobloxUpdater;

// 테마 색상 팔레트 (Python 현대적 Black, White, Blue 테마와 100% 일치)
const COLOR_BG_MAIN: Color32 = Color32::from_rgb(9, 10, 13);
const COLOR_BG_SIDEBAR: Color32 = Color32::from_rgb(15, 16, 21);
const COLOR_BG_HEADER: Color32 = Color32::from_rgb(12, 13, 18);
const COLOR_BG_CARD: Color32 = Color32::from_rgb(20, 22, 30);
const COLOR_BG_CARD_ROW: Color32 = Color32::from_rgb(18, 20, 27);
const COLOR_BORDER: Color32 = Color32::from_rgb(30, 34, 45);
const COLOR_BORDER_LIGHT: Color32 = Color32::from_rgb(37, 42, 54);

const COLOR_PRIMARY: Color32 = Color32::from_rgb(37, 99, 235); // #2563EB
const COLOR_PRIMARY_HOVER: Color32 = Color32::from_rgb(59, 130, 246); // #3B82F6
const COLOR_SKY_ACCENT: Color32 = Color32::from_rgb(56, 189, 248); // #38BDF8
const COLOR_DANGER: Color32 = Color32::from_rgb(239, 68, 68); // #EF4444
const COLOR_DANGER_BG: Color32 = Color32::from_rgb(40, 20, 24);
const COLOR_DANGER_BORDER: Color32 = Color32::from_rgb(127, 29, 29);

const COLOR_TEXT_WHITE: Color32 = Color32::from_rgb(255, 255, 255);
const COLOR_TEXT_MUTED: Color32 = Color32::from_rgb(148, 163, 184); // #94A3B8
const COLOR_TEXT_SUBTLE: Color32 = Color32::from_rgb(100, 116, 139);

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Tab {
    Accounts = 0,
    Monitor = 1,
    Settings = 2,
}

pub enum AppEvent {
    Log(String),
    Toast(String),
    VersionChecked {
        is_latest: bool,
        local_v: Option<String>,
        live_v: String,
        exe: Option<String>,
    },
    UpdateFinished {
        ok: bool,
        msg: String,
        new_exe: Option<String>,
    },
    AccountsChanged,
    #[allow(dead_code)]
    InstancesRefreshed(Vec<InstanceInfo>),
}

enum Modal {
    AddAccount {
        #[allow(dead_code)]
        is_cookie_mode: bool,
        group_input: String,
        cookie_input: String,
        status_msg: String,
        is_loading: bool,
    },
    LaunchOptions {
        job_id: String,
        delay: f32,
        max_n: usize,
    },
    ConfirmDelete,
}

pub struct RobloxLauncherApp {
    store: AccountStore,
    config_store: ConfigStore,
    multi_mgr: MultiInstanceManager,

    current_tab: Tab,
    unlocked: bool,
    is_new_vault: bool,
    master_pw_input: String,
    master_pw_confirm: String,
    remember_pw_input: bool,
    unlock_error: Option<String>,

    search_query: String,
    selected_group: String,
    selected_ids: HashSet<u64>,

    place_input: String,
    toast_msg: String,
    logs: Vec<String>,

    live_instances: Vec<InstanceInfo>,
    is_client_latest: bool,
    local_version: Option<String>,
    live_version: String,
    exe_path: Option<String>,
    is_updating_now: bool,

    modal: Option<Modal>,

    event_tx: Sender<AppEvent>,
    event_rx: Receiver<AppEvent>,
    last_poll: Instant,
}

impl RobloxLauncherApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // 1. 한국어 맑은 고딕(Malgun Gothic) 폰트 설정
        Self::setup_fonts(&cc.egui_ctx);

        // 2. 전역 비주얼 및 여백 스타일 설정
        Self::setup_styles(&cc.egui_ctx);

        let (tx, rx) = channel();

        let vault_file = if std::path::Path::new("accounts.enc").exists() {
            "accounts.enc"
        } else if std::path::Path::new("accounts_rs.enc").exists() {
            "accounts_rs.enc"
        } else {
            "accounts.enc"
        };

        let mut store = AccountStore::new(vault_file);
        let config_store = ConfigStore::new("config.json");
        let mut multi_mgr = MultiInstanceManager::new();

        let is_new_vault = !store.exists();
        let mut unlocked = false;

        // DPAPI 자동 언락 시도
        if !is_new_vault && config_store.config.auto_unlock {
            if store.try_auto_unlock() {
                unlocked = true;
            }
        }

        let _ = multi_mgr.prepare_for_launch();
        let place_val = config_store.config.last_place_id.clone();

        let app = Self {
            store,
            config_store,
            multi_mgr,
            current_tab: Tab::Accounts,
            unlocked,
            is_new_vault,
            master_pw_input: String::new(),
            master_pw_confirm: String::new(),
            remember_pw_input: true,
            unlock_error: None,
            search_query: String::new(),
            selected_group: "전체 그룹".to_string(),
            selected_ids: HashSet::new(),
            place_input: place_val,
            toast_msg: "준비되었습니다. 계정을 선택한 후 하단의 실행 버튼을 누르세요.".to_string(),
            logs: vec!["Two-Roblox 엔진이 준비되었습니다.".to_string()],
            live_instances: Vec::new(),
            is_client_latest: true,
            local_version: None,
            live_version: String::new(),
            exe_path: None,
            is_updating_now: false,
            modal: None,
            event_tx: tx,
            event_rx: rx,
            last_poll: Instant::now(),
        };

        app.check_version_async();
        app
    }

    /// Windows 시스템의 한글 폰트를 로드하여 egui에 등록
    fn setup_fonts(ctx: &egui::Context) {
        let mut fonts = egui::FontDefinitions::default();

        let win_dir = std::env::var("SystemRoot").unwrap_or_else(|_| "C:\\Windows".to_string());
        let font_candidates = [
            format!("{}\\Fonts\\malgun.ttf", win_dir),
            format!("{}\\Fonts\\malgunbd.ttf", win_dir),
            format!("{}\\Fonts\\gulim.ttc", win_dir),
            "C:\\Windows\\Fonts\\malgun.ttf".to_string(),
        ];

        for path in &font_candidates {
            if let Ok(font_bytes) = std::fs::read(path) {
                fonts.font_data.insert(
                    "korean_font".to_owned(),
                    egui::FontData::from_owned(font_bytes),
                );

                // Proportional 기본 글꼴 1순위로 지정
                fonts
                    .families
                    .entry(FontFamily::Proportional)
                    .or_default()
                    .insert(0, "korean_font".to_owned());

                // Monospace 기본 글꼴 1순위로 지정
                fonts
                    .families
                    .entry(FontFamily::Monospace)
                    .or_default()
                    .insert(0, "korean_font".to_owned());

                break;
            }
        }

        ctx.set_fonts(fonts);
    }

    /// 전역 모던 다크 테마 및 타이포그래피, 간격 설정
    fn setup_styles(ctx: &egui::Context) {
        let mut visuals = egui::Visuals::dark();
        visuals.override_text_color = Some(COLOR_TEXT_WHITE);
        visuals.window_fill = COLOR_BG_MAIN;
        visuals.panel_fill = COLOR_BG_SIDEBAR;
        visuals.faint_bg_color = COLOR_BG_SIDEBAR;
        visuals.extreme_bg_color = Color32::from_rgb(14, 16, 22);

        // 카드 및 비대화형 위젯
        visuals.widgets.noninteractive.bg_fill = COLOR_BG_CARD;
        visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, COLOR_BORDER);
        visuals.widgets.noninteractive.rounding = Rounding::same(8.0);

        // 기본 비활성 버튼
        visuals.widgets.inactive.bg_fill = Color32::from_rgb(22, 25, 35);
        visuals.widgets.inactive.bg_stroke = Stroke::new(1.0_f32, COLOR_BORDER);
        visuals.widgets.inactive.rounding = Rounding::same(6.0);

        // 호버 시 버튼
        visuals.widgets.hovered.bg_fill = Color32::from_rgb(31, 36, 50);
        visuals.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, COLOR_PRIMARY_HOVER);
        visuals.widgets.hovered.rounding = Rounding::same(6.0);

        // 활성/클릭 시 버튼
        visuals.widgets.active.bg_fill = Color32::from_rgb(18, 21, 30);
        visuals.widgets.active.bg_stroke = Stroke::new(1.0_f32, COLOR_PRIMARY);
        visuals.widgets.active.rounding = Rounding::same(6.0);

        // 텍스트 선택 색상
        visuals.selection.bg_fill = Color32::from_rgba_premultiplied(37, 99, 235, 100);
        visuals.selection.stroke = Stroke::new(1.0_f32, COLOR_PRIMARY_HOVER);

        ctx.set_visuals(visuals);

        // 텍스트 스타일 및 크기 설정
        let mut style = (*ctx.style()).clone();
        style.text_styles.insert(
            TextStyle::Heading,
            FontId::new(18.0, FontFamily::Proportional),
        );
        style.text_styles.insert(
            TextStyle::Name("Subheading".into()),
            FontId::new(15.0, FontFamily::Proportional),
        );
        style.text_styles.insert(
            TextStyle::Body,
            FontId::new(13.0, FontFamily::Proportional),
        );
        style.text_styles.insert(
            TextStyle::Button,
            FontId::new(13.0, FontFamily::Proportional),
        );
        style.text_styles.insert(
            TextStyle::Small,
            FontId::new(11.5, FontFamily::Proportional),
        );
        style.text_styles.insert(
            TextStyle::Monospace,
            FontId::new(12.0, FontFamily::Monospace),
        );

        style.spacing.item_spacing = Vec2::new(8.0, 8.0);
        style.spacing.button_padding = Vec2::new(12.0, 6.0);
        style.spacing.interact_size = Vec2::new(0.0, 32.0);

        ctx.set_style(style);
    }

    fn check_version_async(&self) {
        let tx = self.event_tx.clone();
        std::thread::spawn(move || match RobloxUpdater::check_status() {
            Ok((is_latest, local_v, live_v, exe)) => {
                let _ = tx.send(AppEvent::VersionChecked {
                    is_latest,
                    local_v,
                    live_v,
                    exe,
                });
            }
            Err(e) => {
                let _ = tx.send(AppEvent::Log(format!("버전 점검 오류: {}", e)));
            }
        });
    }

    fn run_official_update_async(&mut self) {
        if self.is_updating_now {
            return;
        }
        self.is_updating_now = true;
        self.multi_mgr.terminate_all();
        self.multi_mgr.release_locks();

        let tx = self.event_tx.clone();
        std::thread::spawn(move || {
            let _ = tx.send(AppEvent::Log("로블록스 공식 최신 버전 다운로드 및 자동 설치 시작...".to_string()));
            let _ = tx.send(AppEvent::Toast("최신 버전 자동 업데이트 중...".to_string()));
            match RobloxUpdater::run_official_update() {
                Ok(new_exe) => {
                    let _ = tx.send(AppEvent::UpdateFinished {
                        ok: true,
                        msg: "업데이트 성공".to_string(),
                        new_exe: Some(new_exe),
                    });
                }
                Err(e) => {
                    let _ = tx.send(AppEvent::UpdateFinished {
                        ok: false,
                        msg: e.to_string(),
                        new_exe: None,
                    });
                }
            }
        });
    }

    fn poll_events(&mut self) {
        while let Ok(evt) = self.event_rx.try_recv() {
            match evt {
                AppEvent::Log(msg) => {
                    self.logs.push(msg.clone());
                    self.toast_msg = msg;
                }
                AppEvent::Toast(msg) => {
                    self.toast_msg = msg;
                }
                AppEvent::VersionChecked {
                    is_latest,
                    local_v,
                    live_v,
                    exe,
                } => {
                    self.is_client_latest = is_latest;
                    self.local_version = local_v;
                    self.live_version = live_v;
                    self.exe_path = exe;
                }
                AppEvent::UpdateFinished { ok, msg, new_exe } => {
                    self.is_updating_now = false;
                    let _ = self.multi_mgr.prepare_for_launch();
                    if ok {
                        self.exe_path = new_exe;
                        self.logs.push("공식 최신 클라이언트 설치 완료".to_string());
                        self.toast_msg = "업데이트가 완료되었습니다.".to_string();
                        self.check_version_async();
                    } else {
                        self.logs.push(format!("자동 업데이트 실패: {}", msg));
                        self.toast_msg = format!("업데이트 실패: {}", msg);
                    }
                }
                AppEvent::AccountsChanged => {}
                AppEvent::InstancesRefreshed(list) => {
                    self.live_instances = list;
                }
            }
        }

        if self.last_poll.elapsed() >= Duration::from_millis(1500) {
            self.last_poll = Instant::now();
            let instances = self.multi_mgr.get_active_instances_info();
            self.live_instances = instances;
        }
    }

    fn parse_place_id(&self, raw: &str) -> Option<String> {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return None;
        }
        if trimmed.chars().all(|c| c.is_ascii_digit()) {
            return Some(trimmed.to_string());
        }
        let re1 = Regex::new(r"(?i)games/(\d+)").ok()?;
        if let Some(caps) = re1.captures(trimmed) {
            return caps.get(1).map(|m| m.as_str().to_string());
        }
        let re2 = Regex::new(r"(?i)placeid=(\d+)").ok()?;
        if let Some(caps) = re2.captures(trimmed) {
            return caps.get(1).map(|m| m.as_str().to_string());
        }
        None
    }

    fn launch_selected_async(&mut self) {
        let place_opt = self.parse_place_id(&self.place_input);
        let job_opt = if self.config_store.config.last_job_id.trim().is_empty() {
            None
        } else {
            Some(self.config_store.config.last_job_id.trim().to_string())
        };

        let delay = self.config_store.config.launch_delay;
        let max_n = self.config_store.config.max_concurrent;

        let accounts: Vec<Account> = self
            .store
            .accounts
            .iter()
            .filter(|a| self.selected_ids.contains(&a.user_id))
            .take(max_n)
            .cloned()
            .collect();

        if accounts.is_empty() {
            self.toast_msg = "실행할 계정을 먼저 선택하세요.".to_string();
            return;
        }

        self.launch_accounts_pipeline(accounts, place_opt, job_opt, delay);
    }

    fn launch_single_async(&mut self, acc: Account) {
        let place_opt = self.parse_place_id(&self.place_input);
        let job_opt = if self.config_store.config.last_job_id.trim().is_empty() {
            None
        } else {
            Some(self.config_store.config.last_job_id.trim().to_string())
        };

        self.launch_accounts_pipeline(vec![acc], place_opt, job_opt, 0.0);
    }

    fn launch_accounts_pipeline(
        &mut self,
        targets: Vec<Account>,
        place_id: Option<String>,
        job_id: Option<String>,
        delay: f32,
    ) {
        let auto_update = self.config_store.config.auto_update_roblox;
        let tx = self.event_tx.clone();

        self.toast_msg = format!("총 {}개 계정 실행을 시작합니다...", targets.len());
        self.logs.push(format!("총 {}개 계정 클라이언트 직접 실행 파이프라인 가동...", targets.len()));

        std::thread::spawn(move || {
            let mut final_exe: Option<String> = None;

            if auto_update {
                if let Ok((is_latest, _, _, cur_exe)) = RobloxUpdater::check_status() {
                    if !is_latest || cur_exe.is_none() {
                        let _ = tx.send(AppEvent::Log("최신 로블록스 버전 감지: 자동 무중단 업데이트 수행 중...".to_string()));
                        if let Ok(new_exe) = RobloxUpdater::run_official_update() {
                            final_exe = Some(new_exe);
                        }
                    } else {
                        final_exe = cur_exe;
                    }
                }
            }

            if final_exe.is_none() {
                if let Some((_, exe)) = RobloxUpdater::find_local_player_exe(None) {
                    final_exe = Some(exe);
                }
            }

            let exe = match final_exe {
                Some(e) => e,
                None => {
                    let _ = tx.send(AppEvent::Log("오류: RobloxPlayerBeta.exe를 찾을 수 없습니다.".to_string()));
                    return;
                }
            };

            let mut mgr = MultiInstanceManager::new();
            let _ = mgr.prepare_for_launch();

            for (idx, acc) in targets.iter().enumerate() {
                let uname = &acc.username;
                let ticket_res = RobloxAPI::get_auth_ticket(&acc.cookie);
                match ticket_res {
                    Ok(ticket) => {
                        match RobloxAPI::build_direct_cli_args(
                            &exe,
                            &ticket,
                            place_id.as_deref(),
                            job_id.as_deref(),
                            "ko_kr",
                        ) {
                            Ok(args) => match mgr.launch_client_directly(acc.user_id, uname, &args) {
                                Ok(pid) => {
                                    let _ = tx.send(AppEvent::Log(format!(
                                        "[{}/{}] {} 계정 실행 성공 (PID: {})",
                                        idx + 1,
                                        targets.len(),
                                        uname,
                                        pid
                                    )));
                                }
                                Err(e) => {
                                    let _ = tx.send(AppEvent::Log(format!(
                                        "[{}/{}] {} 프로세스 생성 실패: {}",
                                        idx + 1,
                                        targets.len(),
                                        uname,
                                        e
                                    )));
                                }
                            },
                            Err(e) => {
                                let _ = tx.send(AppEvent::Log(format!(
                                    "[{}/{}] {} CLI 인수 생성 오류: {}",
                                    idx + 1,
                                    targets.len(),
                                    uname,
                                    e
                                )));
                            }
                        }
                    }
                    Err(e) => {
                        let _ = tx.send(AppEvent::Log(format!(
                            "[{}/{}] {} 인증 티켓 발급 실패: {}. 쿠키를 재등록하세요.",
                            idx + 1,
                            targets.len(),
                            uname,
                            e
                        )));
                    }
                }

                if idx + 1 < targets.len() && delay > 0.0 {
                    std::thread::sleep(Duration::from_secs_f32(delay));
                }
            }

            let _ = tx.send(AppEvent::Toast("선택 계정 실행 작업이 완료되었습니다.".to_string()));
            let _ = tx.send(AppEvent::AccountsChanged);
        });
    }

    // ---------- 재사용 UI 버튼 헬퍼 ----------
    fn ui_primary_btn(ui: &mut Ui, text: &str, min_size: Vec2) -> Response {
        ui.add(
            Button::new(RichText::new(text).color(COLOR_TEXT_WHITE).strong())
                .fill(COLOR_PRIMARY)
                .stroke(Stroke::new(1.0_f32, COLOR_PRIMARY_HOVER))
                .min_size(min_size),
        )
    }

    fn ui_danger_btn(ui: &mut Ui, text: &str, min_size: Vec2) -> Response {
        ui.add(
            Button::new(RichText::new(text).color(Color32::from_rgb(248, 113, 113)).strong())
                .fill(COLOR_DANGER_BG)
                .stroke(Stroke::new(1.0_f32, COLOR_DANGER_BORDER))
                .min_size(min_size),
        )
    }
}

impl eframe::App for RobloxLauncherApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_events();

        // 1. 보안 보관함 잠금 해제 화면 (언락 전)
        if !self.unlocked {
            self.render_unlock_view(ctx);
            return;
        }

        // 2. 메인 UI 화면 (사이드바 + 상단 헤더 + 하단 런처 바 + 중앙 콘텐츠)
        self.render_sidebar(ctx);
        self.render_header(ctx);
        self.render_bottom_bar(ctx);
        self.render_content(ctx);

        // 3. 모달 오버레이 다이얼로그 렌더링
        self.render_modals(ctx);

        ctx.request_repaint_after(Duration::from_millis(500));
    }
}

// ---------- 세부 UI 렌더링 구현 ----------
impl RobloxLauncherApp {
    fn render_unlock_view(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(COLOR_BG_MAIN))
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(80.0);

                    let box_width = 400.0;
                    egui::Frame::none()
                        .fill(COLOR_BG_CARD)
                        .stroke(Stroke::new(1.0_f32, COLOR_BORDER_LIGHT))
                        .rounding(Rounding::same(10.0))
                        .inner_margin(24.0)
                        .show(ui, |ui| {
                            ui.set_width(box_width);
                            ui.set_min_width(box_width);

                            let title = if self.is_new_vault {
                                "새 마스터 비밀번호 설정"
                            } else {
                                "계정 보관함 잠금 해제"
                            };
                            ui.label(RichText::new(title).size(18.0).strong().color(COLOR_TEXT_WHITE));

                            ui.add_space(8.0);
                            let desc = if self.is_new_vault {
                                "계정 정보를 AES-256-GCM으로 안전하게 암호화하기 위해 사용할 마스터 비밀번호를 설정하세요."
                            } else {
                                "저장된 로블록스 계정을 불러오기 위해 마스터 비밀번호를 입력하세요."
                            };
                            ui.label(RichText::new(desc).size(12.0).color(COLOR_TEXT_MUTED));

                            ui.add_space(16.0);

                            ui.label(RichText::new("마스터 비밀번호:").size(12.5).color(COLOR_TEXT_MUTED));
                            let pw_edit = ui.add(
                                TextEdit::singleline(&mut self.master_pw_input)
                                    .password(true)
                                    .hint_text("비밀번호 입력")
                                    .desired_width(box_width),
                            );

                            if self.is_new_vault {
                                ui.add_space(10.0);
                                ui.label(RichText::new("비밀번호 확인:").size(12.5).color(COLOR_TEXT_MUTED));
                                ui.add(
                                    TextEdit::singleline(&mut self.master_pw_confirm)
                                        .password(true)
                                        .hint_text("비밀번호 재입력")
                                        .desired_width(box_width),
                                );
                            }

                            ui.add_space(12.0);
                            ui.checkbox(&mut self.remember_pw_input, "이 PC에서 마스터 비밀번호 기억 (Windows DPAPI 자동 언락)");

                            if let Some(ref err) = self.unlock_error {
                                ui.add_space(8.0);
                                ui.label(RichText::new(err).size(12.0).color(COLOR_DANGER).strong());
                            }

                            ui.add_space(20.0);
                            ui.horizontal(|ui| {
                                if ui.add(Button::new("종료").min_size(Vec2::new(80.0, 34.0))).clicked() {
                                    std::process::exit(0);
                                }

                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    let enter_pressed = pw_edit.lost_focus() && ctx.input(|i| i.key_pressed(egui::Key::Enter));
                                    let btn = Self::ui_primary_btn(ui, "확인", Vec2::new(100.0, 34.0));

                                    if btn.clicked() || enter_pressed {
                                        self.handle_unlock_attempt();
                                    }
                                });
                            });
                        });
                });
            });
    }

    fn handle_unlock_attempt(&mut self) {
        let pw = self.master_pw_input.trim();
        if pw.is_empty() {
            self.unlock_error = Some("비밀번호를 입력하세요.".to_string());
            return;
        }

        if self.is_new_vault {
            if pw != self.master_pw_confirm.trim() {
                self.unlock_error = Some("비밀번호 확인이 일치하지 않습니다.".to_string());
                return;
            }
        }

        match self.store.unlock(pw, self.remember_pw_input) {
            Ok(_) => {
                self.config_store.config.auto_unlock = self.remember_pw_input;
                let _ = self.config_store.save();
                self.unlocked = true;
                self.unlock_error = None;
                self.logs.push("계정 보관함 잠금 해제 완료".to_string());
            }
            Err(e) => {
                self.unlock_error = Some(format!("비밀번호가 올바르지 않습니다: {}", e));
            }
        }
    }

    fn render_sidebar(&mut self, ctx: &egui::Context) {
        egui::SidePanel::left("sidebar")
            .exact_width(210.0)
            .resizable(false)
            .frame(
                egui::Frame::none()
                    .fill(COLOR_BG_SIDEBAR)
                    .stroke(Stroke::new(1.0_f32, COLOR_BORDER)),
            )
            .show(ctx, |ui| {
                // 브랜드 헤더 영역
                ui.add_space(20.0);
                ui.horizontal(|ui| {
                    ui.add_space(16.0);
                    ui.label(
                        RichText::new("Two-Roblox")
                            .size(19.0)
                            .strong()
                            .color(COLOR_TEXT_WHITE),
                    );
                });

                ui.add_space(22.0);

                let nav_items = [
                    ("계정 목록", Tab::Accounts),
                    ("실행 모니터", Tab::Monitor),
                    ("설정 및 업데이트", Tab::Settings),
                ];

                for (name, tab) in nav_items {
                    let is_active = self.current_tab == tab;
                    let fill_color = if is_active {
                        Color32::from_rgb(26, 33, 50)
                    } else {
                        Color32::TRANSPARENT
                    };
                    let stroke = if is_active {
                        Stroke::new(1.0_f32, COLOR_PRIMARY_HOVER)
                    } else {
                        Stroke::NONE
                    };
                    let text_color = if is_active {
                        COLOR_SKY_ACCENT
                    } else {
                        Color32::from_rgb(203, 213, 225)
                    };

                    ui.horizontal(|ui| {
                        ui.add_space(10.0);
                        let btn = ui.add(
                            Button::new(RichText::new(name).size(13.5).strong().color(text_color))
                                .fill(fill_color)
                                .stroke(stroke)
                                .rounding(Rounding::same(6.0))
                                .min_size(Vec2::new(190.0, 38.0)),
                        );

                        if btn.clicked() {
                            self.current_tab = tab;
                        }
                    });

                    ui.add_space(6.0);
                }

                // 하단 관리자 권한 상태 카드
                ui.with_layout(Layout::bottom_up(Align::Center), |ui| {
                    ui.add_space(16.0);
                    egui::Frame::none()
                        .fill(COLOR_BG_CARD)
                        .stroke(Stroke::new(1.0_f32, COLOR_BORDER))
                        .rounding(Rounding::same(8.0))
                        .inner_margin(12.0)
                        .show(ui, |ui| {
                            ui.set_width(186.0);
                            let admin_active = is_admin();
                            let status_text = if admin_active {
                                "관리자 권한 활성"
                            } else {
                                "일반 사용자 모드"
                            };
                            let status_color = if admin_active {
                                COLOR_SKY_ACCENT
                            } else {
                                COLOR_TEXT_MUTED
                            };

                            ui.vertical_centered(|ui| {
                                ui.label(RichText::new(status_text).size(11.5).strong().color(status_color));
                                if !admin_active {
                                    ui.add_space(6.0);
                                    let btn = ui.add(
                                        Button::new(RichText::new("관리자 권한 실행").size(11.0))
                                            .fill(Color32::from_rgb(26, 30, 42))
                                            .min_size(Vec2::new(162.0, 28.0)),
                                    );
                                    if btn.clicked() {
                                        self.multi_mgr.release_locks();
                                        restart_as_admin();
                                        std::process::exit(0);
                                    }
                                }
                            });
                        });
                });
            });
    }

    fn render_header(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("header")
            .exact_height(58.0)
            .frame(
                egui::Frame::none()
                    .fill(COLOR_BG_HEADER)
                    .stroke(Stroke::new(1.0_f32, COLOR_BORDER)),
            )
            .show(ctx, |ui| {
                ui.horizontal_centered(|ui| {
                    ui.add_space(20.0);

                    let title = match self.current_tab {
                        Tab::Accounts => "계정 목록",
                        Tab::Monitor => "실행 모니터",
                        Tab::Settings => "설정 및 업데이트",
                    };
                    ui.label(RichText::new(title).size(17.0).strong().color(COLOR_TEXT_WHITE));

                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.add_space(20.0);

                        // + 계정 추가 파란색 주 버튼
                        let add_btn = Self::ui_primary_btn(ui, "+ 계정 추가", Vec2::new(116.0, 34.0));
                        if add_btn.clicked() {
                            self.modal = Some(Modal::AddAccount {
                                is_cookie_mode: true,
                                group_input: "기본".to_string(),
                                cookie_input: String::new(),
                                status_msg: "쿠키(.ROBLOSECURITY)를 입력하면 자동으로 사용자 정보와 아바타를 등록합니다.".to_string(),
                                is_loading: false,
                            });
                        }

                        // 로블록스 업데이트 배너 버튼 (새 버전 감지 시)
                        if !self.is_client_latest && !self.live_version.is_empty() {
                            ui.add_space(10.0);
                            let banner_text = format!("새 로블록스 버전 ({}) 업데이트 가능 (클릭)", self.live_version);
                            let banner_btn = ui.add(
                                Button::new(RichText::new(banner_text).size(12.0).strong().color(Color32::from_rgb(147, 197, 253)))
                                    .fill(Color32::from_rgb(23, 37, 84))
                                    .stroke(Stroke::new(1.0_f32, Color32::from_rgb(30, 64, 175)))
                                    .min_size(Vec2::new(0.0, 34.0)),
                            );
                            if banner_btn.clicked() {
                                self.run_official_update_async();
                            }
                        }
                    });
                });
            });
    }

    fn render_bottom_bar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::bottom("bottom_bar")
            .exact_height(64.0)
            .frame(
                egui::Frame::none()
                    .fill(COLOR_BG_SIDEBAR)
                    .stroke(Stroke::new(1.0_f32, COLOR_BORDER)),
            )
            .show(ctx, |ui| {
                ui.horizontal_centered(|ui| {
                    ui.add_space(16.0);

                    // 우측 고정 버튼 먼저 배치
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.add_space(16.0);

                        let launch_btn = Self::ui_primary_btn(ui, "선택 계정 실행", Vec2::new(136.0, 38.0));
                        if launch_btn.clicked() {
                            self.launch_selected_async();
                        }

                        ui.add_space(8.0);

                        let opt_btn = ui.add(
                            Button::new(RichText::new("실행 옵션").size(13.0))
                                .min_size(Vec2::new(94.0, 38.0)),
                        );
                        if opt_btn.clicked() {
                            self.modal = Some(Modal::LaunchOptions {
                                job_id: self.config_store.config.last_job_id.clone(),
                                delay: self.config_store.config.launch_delay,
                                max_n: self.config_store.config.max_concurrent,
                            });
                        }

                        // 나머지 공간 좌측: 선택 카운터 및 Place ID 입력창
                        ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                            let sel_text = format!("선택: {}개", self.selected_ids.len());
                            ui.label(RichText::new(sel_text).size(13.5).strong().color(COLOR_SKY_ACCENT));

                            ui.add_space(14.0);

                            let avail = (ui.available_width() - 16.0).max(120.0);
                            let place_edit = ui.add(
                                TextEdit::singleline(&mut self.place_input)
                                    .hint_text("Place ID 또는 게임 링크 (비워두면 로블록스 홈 화면 실행)")
                                    .desired_width(avail),
                            );
                            if place_edit.changed() {
                                self.config_store.config.last_place_id = self.place_input.clone();
                                let _ = self.config_store.save();
                            }
                        });
                    });
                });
            });
    }

    fn render_content(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(COLOR_BG_MAIN).inner_margin(20.0))
            .show(ctx, |ui| match self.current_tab {
                Tab::Accounts => self.render_accounts_tab(ui),
                Tab::Monitor => self.render_monitor_tab(ui),
                Tab::Settings => self.render_settings_tab(ui),
            });
    }

    fn render_accounts_tab(&mut self, ui: &mut Ui) {
        // 상단 툴바: 검색, 그룹 필터, 전체선택, 선택해제, 쿠키검증, 삭제
        ui.horizontal(|ui| {
            ui.add(
                TextEdit::singleline(&mut self.search_query)
                    .hint_text("계정명, 닉네임, 그룹 검색...")
                    .desired_width(260.0),
            );

            ui.add_space(6.0);

            let mut groups = vec!["전체 그룹".to_string()];
            let mut unique_groups: Vec<String> = self
                .store
                .accounts
                .iter()
                .map(|a| a.group.clone())
                .collect::<HashSet<_>>()
                .into_iter()
                .collect();
            unique_groups.sort();
            groups.extend(unique_groups);

            egui::ComboBox::from_id_salt("group_filter")
                .selected_text(&self.selected_group)
                .show_ui(ui, |ui| {
                    for g in groups {
                        ui.selectable_value(&mut self.selected_group, g.clone(), g);
                    }
                });

            ui.add_space(10.0);

            if ui.button("전체 선택").clicked() {
                for a in &self.store.accounts {
                    self.selected_ids.insert(a.user_id);
                }
            }

            if ui.button("선택 해제").clicked() {
                self.selected_ids.clear();
            }

            if ui.button("쿠키 검증").clicked() {
                self.validate_cookies_async();
            }

            let del_btn = Self::ui_danger_btn(ui, "삭제", Vec2::new(60.0, 32.0));
            if del_btn.clicked() {
                if !self.selected_ids.is_empty() {
                    self.modal = Some(Modal::ConfirmDelete);
                } else {
                    self.toast_msg = "삭제할 계정을 먼저 선택하세요.".to_string();
                }
            }
        });

        ui.add_space(14.0);

        // 계정 테이블 헤더 바
        egui::Frame::none()
            .fill(COLOR_BG_CARD_ROW)
            .stroke(Stroke::new(1.0_f32, COLOR_BORDER))
            .rounding(Rounding::same(6.0))
            .inner_margin(10.0)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.add_space(6.0);
                    ui.label(RichText::new("선택").size(12.0).strong().color(COLOR_TEXT_MUTED));
                    ui.add_space(16.0);
                    ui.label(RichText::new("계정 정보").size(12.0).strong().color(COLOR_TEXT_MUTED));

                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.add_space(8.0);
                        ui.label(RichText::new("작업").size(12.0).strong().color(COLOR_TEXT_MUTED));
                        ui.add_space(86.0);
                        ui.label(RichText::new("상태").size(12.0).strong().color(COLOR_TEXT_MUTED));
                        ui.add_space(36.0);
                        ui.label(RichText::new("그룹").size(12.0).strong().color(COLOR_TEXT_MUTED));
                    });
                });
            });

        ui.add_space(6.0);

        let query = self.search_query.trim().to_lowercase();
        let selected_grp = self.selected_group.clone();
        let filtered: Vec<Account> = self
            .store
            .accounts
            .iter()
            .filter(|a| {
                if selected_grp != "전체 그룹" && a.group != selected_grp {
                    return false;
                }
                if !query.is_empty() {
                    let blob = format!("{} {} {} {}", a.username, a.display_name, a.group, a.memo).to_lowercase();
                    return blob.contains(&query);
                }
                true
            })
            .cloned()
            .collect();

        let mut to_launch_single: Option<Account> = None;
        let mut to_terminate_uid: Option<u64> = None;

        ScrollArea::vertical()
            .max_height(ui.available_height() - 36.0)
            .show(ui, |ui| {
                if filtered.is_empty() {
                    ui.add_space(60.0);
                    ui.vertical_centered(|ui| {
                        ui.label(
                            RichText::new("등록된 계정이 없습니다. 상단의 '+ 계정 추가'를 눌러 계정을 등록하세요.")
                                .size(14.0)
                                .color(COLOR_TEXT_MUTED),
                        );
                    });
                }

                for acc in &filtered {
                    let uid = acc.user_id;
                    let mut checked = self.selected_ids.contains(&uid);
                    let is_running = self.multi_mgr.is_account_running(uid);

                    egui::Frame::none()
                        .fill(COLOR_BG_CARD)
                        .stroke(Stroke::new(1.0_f32, COLOR_BORDER))
                        .rounding(Rounding::same(8.0))
                        .inner_margin(10.0)
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                // 체크박스
                                if ui.checkbox(&mut checked, "").changed() {
                                    if checked {
                                        self.selected_ids.insert(uid);
                                    } else {
                                        self.selected_ids.remove(&uid);
                                    }
                                }

                                ui.add_space(8.0);

                                // 원형 아바타 이니셜 배지
                                let initial = if acc.username.len() >= 2 {
                                    &acc.username[..2]
                                } else {
                                    &acc.username
                                };

                                egui::Frame::none()
                                    .fill(Color32::from_rgb(37, 42, 56))
                                    .rounding(Rounding::same(18.0))
                                    .inner_margin(Vec2::new(8.0, 6.0))
                                    .show(ui, |ui| {
                                        ui.set_min_size(Vec2::new(36.0, 36.0));
                                        ui.vertical_centered(|ui| {
                                            ui.label(
                                                RichText::new(initial.to_uppercase())
                                                    .size(12.0)
                                                    .strong()
                                                    .color(Color32::from_rgb(203, 213, 225)),
                                            );
                                        });
                                    });

                                ui.add_space(6.0);

                                // 사용자명 및 닉네임
                                ui.vertical(|ui| {
                                    ui.label(
                                        RichText::new(&acc.username)
                                            .size(13.5)
                                            .strong()
                                            .color(COLOR_TEXT_WHITE),
                                    );
                                    ui.label(
                                        RichText::new(format!("@{}", acc.display_name))
                                            .size(11.5)
                                            .color(COLOR_TEXT_MUTED),
                                    );
                                });

                                // 우측 정렬: 작업(종료, 실행), 상태, 그룹
                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    ui.add_space(6.0);
                                    if Self::ui_danger_btn(ui, "종료", Vec2::new(48.0, 28.0)).clicked() {
                                        to_terminate_uid = Some(uid);
                                    }

                                    ui.add_space(4.0);
                                    if Self::ui_primary_btn(ui, "실행", Vec2::new(48.0, 28.0)).clicked() {
                                        to_launch_single = Some(acc.clone());
                                    }

                                    ui.add_space(20.0);

                                    // 상태 레이블
                                    if is_running {
                                        ui.label(
                                            RichText::new("[실행 중]")
                                                .size(12.0)
                                                .strong()
                                                .color(COLOR_SKY_ACCENT),
                                        );
                                    } else {
                                        ui.label(
                                            RichText::new("[대기]")
                                                .size(12.0)
                                                .color(COLOR_TEXT_SUBTLE),
                                        );
                                    }

                                    ui.add_space(24.0);

                                    // 그룹 태그
                                    egui::Frame::none()
                                        .fill(Color32::from_rgb(24, 32, 48))
                                        .rounding(Rounding::same(4.0))
                                        .inner_margin(Vec2::new(8.0, 4.0))
                                        .show(ui, |ui| {
                                            ui.label(
                                                RichText::new(&acc.group)
                                                    .size(11.5)
                                                    .color(Color32::from_rgb(147, 197, 253)),
                                            );
                                        });
                                });
                            });
                        });

                    ui.add_space(4.0);
                }
            });

        if let Some(acc) = to_launch_single {
            self.launch_single_async(acc);
        }

        if let Some(uid) = to_terminate_uid {
            self.multi_mgr.terminate_account(uid);
            self.toast_msg = format!("계정 ID {} 클라이언트를 종료했습니다.", uid);
        }

        ui.add_space(8.0);
        ui.label(RichText::new(&self.toast_msg).size(12.0).color(COLOR_TEXT_MUTED));
    }

    fn validate_cookies_async(&mut self) {
        let accounts = self.store.accounts.clone();
        let tx = self.event_tx.clone();

        self.logs.push("전체 계정 쿠키 유효성 검사 시작...".to_string());
        self.toast_msg = "쿠키 검사를 진행 중입니다...".to_string();

        std::thread::spawn(move || {
            let mut valid_cnt = 0;
            let mut invalid_cnt = 0;

            for acc in accounts {
                match RobloxAPI::validate_cookie(&acc.cookie) {
                    Ok(_) => valid_cnt += 1,
                    Err(e) => {
                        invalid_cnt += 1;
                        let _ = tx.send(AppEvent::Log(format!("쿠키 만료 확인 ({}) : {}", acc.username, e)));
                    }
                }
                std::thread::sleep(Duration::from_millis(350));
            }

            let msg = format!("쿠키 검사 완료: 유효 {}개 / 만료 {}개", valid_cnt, invalid_cnt);
            let _ = tx.send(AppEvent::Log(msg.clone()));
            let _ = tx.send(AppEvent::Toast(msg));
        });
    }

    fn render_monitor_tab(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            ui.label(
                RichText::new("현재 PC에서 실행 중인 모든 Roblox 클라이언트를 실시간 감시하고 제어합니다.")
                    .size(13.0)
                    .color(COLOR_TEXT_MUTED),
            );

            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if Self::ui_danger_btn(ui, "모든 Roblox 종료", Vec2::new(130.0, 32.0)).clicked() {
                    self.multi_mgr.terminate_all();
                    self.toast_msg = "모든 Roblox 프로세스를 종료했습니다.".to_string();
                    self.logs.push("실행 중인 모든 Roblox 프로세스 일괄 종료".to_string());
                }

                if Self::ui_primary_btn(ui, "모든 창 바둑판 정렬", Vec2::new(140.0, 32.0)).clicked() {
                    let cnt = MultiInstanceManager::tile_all_windows();
                    self.toast_msg = format!("Roblox 창 {}개를 화면에 바둑판 정렬했습니다.", cnt);
                    self.logs.push(format!("Roblox 창 {}개 바둑판 정렬 완료", cnt));
                }
            });
        });

        ui.add_space(14.0);

        // 인스턴스 테이블 헤더
        egui::Frame::none()
            .fill(COLOR_BG_CARD_ROW)
            .stroke(Stroke::new(1.0_f32, COLOR_BORDER))
            .rounding(Rounding::same(6.0))
            .inner_margin(10.0)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.add_space(10.0);
                    ui.label(RichText::new("계정 / 프로세스").size(12.0).strong().color(COLOR_TEXT_MUTED));

                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.add_space(10.0);
                        ui.label(RichText::new("창 제어").size(12.0).strong().color(COLOR_TEXT_MUTED));
                        ui.add_space(80.0);
                        ui.label(RichText::new("메모리").size(12.0).strong().color(COLOR_TEXT_MUTED));
                        ui.add_space(40.0);
                        ui.label(RichText::new("PID").size(12.0).strong().color(COLOR_TEXT_MUTED));
                    });
                });
            });

        ui.add_space(6.0);

        let mut to_kill_pid: Option<u32> = None;
        let mut to_focus_hwnd: Option<isize> = None;

        ScrollArea::vertical()
            .max_height(ui.available_height() - 20.0)
            .show(ui, |ui| {
                if self.live_instances.is_empty() {
                    ui.add_space(60.0);
                    ui.vertical_centered(|ui| {
                        ui.label(
                            RichText::new("현재 실행 중인 Roblox 프로세스가 없습니다.")
                                .size(14.0)
                                .color(COLOR_TEXT_MUTED),
                        );
                    });
                }

                for inst in &self.live_instances {
                    egui::Frame::none()
                        .fill(COLOR_BG_CARD)
                        .stroke(Stroke::new(1.0_f32, COLOR_BORDER))
                        .rounding(Rounding::same(8.0))
                        .inner_margin(10.0)
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.label(
                                    RichText::new(&inst.username)
                                        .size(13.5)
                                        .strong()
                                        .color(COLOR_TEXT_WHITE),
                                );

                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    ui.add_space(4.0);
                                    if Self::ui_danger_btn(ui, "종료", Vec2::new(50.0, 28.0)).clicked() {
                                        to_kill_pid = Some(inst.pid);
                                    }

                                    ui.add_space(4.0);
                                    if let Some(hwnd) = inst.hwnd {
                                        if ui.add(Button::new("창 활성화").min_size(Vec2::new(70.0, 28.0))).clicked() {
                                            to_focus_hwnd = Some(hwnd);
                                        }
                                    }

                                    ui.add_space(20.0);

                                    ui.label(
                                        RichText::new(format!("{} MB", inst.memory_mb))
                                            .size(12.5)
                                            .color(Color32::from_rgb(147, 197, 253)),
                                    );

                                    ui.add_space(30.0);

                                    ui.label(
                                        RichText::new(format!("{}", inst.pid))
                                            .size(12.5)
                                            .color(Color32::from_rgb(203, 213, 225)),
                                    );
                                });
                            });
                        });

                    ui.add_space(4.0);
                }
            });

        if let Some(hwnd) = to_focus_hwnd {
            MultiInstanceManager::focus_window(hwnd as _);
        }

        if let Some(pid) = to_kill_pid {
            let _ = MultiInstanceManager::terminate_pid(pid);
            self.live_instances.retain(|i| i.pid != pid);
            self.toast_msg = format!("PID {} 프로세스를 종료했습니다.", pid);
        }
    }

    fn render_settings_tab(&mut self, ui: &mut Ui) {
        ScrollArea::vertical().show(ui, |ui| {
            // 로블록스 클라이언트 및 자동 업데이트 카드
            egui::Frame::none()
                .fill(COLOR_BG_CARD)
                .stroke(Stroke::new(1.0_f32, COLOR_BORDER))
                .rounding(Rounding::same(8.0))
                .inner_margin(18.0)
                .show(ui, |ui| {
                    ui.label(
                        RichText::new("로블록스 클라이언트 및 자동 업데이트")
                            .size(15.0)
                            .strong()
                            .color(COLOR_TEXT_WHITE),
                    );
                    ui.add_space(10.0);

                    let local_v_str = self.local_version.as_deref().unwrap_or("미설치 또는 미감지");
                    let live_v_str = if self.live_version.is_empty() {
                        "확인 중..."
                    } else {
                        &self.live_version
                    };
                    let exe_str = self.exe_path.as_deref().unwrap_or("찾을 수 없음");

                    ui.label(RichText::new(format!("로컬 설치 버전: {}", local_v_str)).size(12.5).color(COLOR_TEXT_MUTED));
                    ui.label(RichText::new(format!("공식 최신 버전: {}", live_v_str)).size(12.5).color(COLOR_TEXT_MUTED));
                    ui.label(RichText::new(format!("실행 파일 경로: {}", exe_str)).size(12.5).color(COLOR_TEXT_MUTED));

                    ui.add_space(10.0);

                    if ui.checkbox(
                        &mut self.config_store.config.auto_update_roblox,
                        "실행 전 로블록스 최신 버전 자동 감지 및 무중단 업데이트 유지",
                    ).changed() {
                        let _ = self.config_store.save();
                    }

                    ui.add_space(12.0);

                    ui.horizontal(|ui| {
                        if ui.button("버전 다시 확인").clicked() {
                            self.check_version_async();
                        }

                        if Self::ui_primary_btn(ui, "공식 최신 버전 지금 설치", Vec2::new(170.0, 32.0)).clicked() {
                            self.run_official_update_async();
                        }

                        if ui.button("이미 켜진 창 지금 격리").clicked() {
                            match self.multi_mgr.isolate_existing_roblox_processes() {
                                Ok(msg) => {
                                    self.toast_msg = msg.clone();
                                    self.logs.push(msg);
                                }
                                Err(e) => {
                                    self.toast_msg = format!("격리 실패: {}", e);
                                    self.logs.push(format!("격리 오류: {}", e));
                                }
                            }
                        }
                    });
                });

            ui.add_space(14.0);

            // 보안 및 편의 설정 카드
            egui::Frame::none()
                .fill(COLOR_BG_CARD)
                .stroke(Stroke::new(1.0_f32, COLOR_BORDER))
                .rounding(Rounding::same(8.0))
                .inner_margin(18.0)
                .show(ui, |ui| {
                    ui.label(
                        RichText::new("보안 및 편의 설정")
                            .size(15.0)
                            .strong()
                            .color(COLOR_TEXT_WHITE),
                    );
                    ui.add_space(10.0);

                    if ui.checkbox(
                        &mut self.config_store.config.auto_unlock,
                        "이 PC에서 마스터 비밀번호 기억 (Windows DPAPI 자동 언락)",
                    ).changed() {
                        let _ = self.config_store.save();
                        if !self.config_store.config.auto_unlock {
                            self.store.clear_remembered_password();
                            self.logs.push("자동 로그인 캐시 삭제 완료".to_string());
                        }
                    }
                });

            ui.add_space(14.0);

            // 실시간 작업 로그 뷰어
            ui.label(RichText::new("실시간 작업 로그").size(14.0).strong().color(COLOR_TEXT_WHITE));
            ui.add_space(6.0);

            egui::Frame::none()
                .fill(Color32::from_rgb(12, 13, 18))
                .stroke(Stroke::new(1.0_f32, COLOR_BORDER))
                .rounding(Rounding::same(6.0))
                .inner_margin(12.0)
                .show(ui, |ui| {
                    ScrollArea::vertical().max_height(220.0).show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        for log in self.logs.iter().rev() {
                            ui.label(RichText::new(log).size(12.0).color(Color32::from_rgb(203, 213, 225)));
                        }
                    });
                });
        });
    }

    fn render_modals(&mut self, ctx: &egui::Context) {
        let mut close_modal = false;

        if let Some(ref mut m) = self.modal {
            match m {
                Modal::AddAccount {
                    is_cookie_mode: _,
                    group_input,
                    cookie_input,
                    status_msg,
                    is_loading,
                } => {
                    Window::new("로블록스 계정 추가")
                        .collapsible(false)
                        .resizable(false)
                        .fixed_size(Vec2::new(500.0, 360.0))
                        .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
                        .frame(
                            egui::Frame::none()
                                .fill(COLOR_BG_CARD)
                                .stroke(Stroke::new(1.0_f32, COLOR_BORDER_LIGHT))
                                .rounding(Rounding::same(10.0))
                                .inner_margin(20.0),
                        )
                        .show(ctx, |ui| {
                            ui.horizontal(|ui| {
                                ui.label(RichText::new("저장 그룹:").size(13.0).strong().color(COLOR_TEXT_MUTED));
                                ui.add(TextEdit::singleline(group_input).desired_width(140.0));
                            });

                            ui.add_space(10.0);
                            ui.label(RichText::new(&*status_msg).size(12.0).color(COLOR_TEXT_MUTED));

                            ui.add_space(10.0);
                            ui.label(
                                RichText::new("ROBLOSECURITY 쿠키 입력 (여러 줄 입력 시 일괄 등록 지원):")
                                    .size(12.5)
                                    .color(Color32::from_rgb(203, 213, 225)),
                            );
                            ui.add(
                                TextEdit::multiline(cookie_input)
                                    .hint_text("_|WARNING:-DO-NOT-SHARE-THIS.--Sharing-this-will-allow-someone-to-log-in-to-your-account...")
                                    .desired_rows(6)
                                    .desired_width(460.0),
                            );

                            ui.add_space(16.0);
                            ui.horizontal(|ui| {
                                if ui.button("닫기").clicked() {
                                    close_modal = true;
                                }

                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    let btn_text = if *is_loading { "추가 중..." } else { "계정 등록" };
                                    let add_btn = Self::ui_primary_btn(ui, btn_text, Vec2::new(100.0, 34.0));

                                    if add_btn.clicked() && !*is_loading {
                                        let raw_cookies = cookie_input.clone();
                                        let grp = group_input.clone();
                                        *is_loading = true;

                                        let tx = self.event_tx.clone();
                                        std::thread::spawn(move || {
                                            let mut added_cnt = 0;
                                            for line in raw_cookies.lines() {
                                                let c = line.trim().trim_matches('"');
                                                if c.is_empty() {
                                                    continue;
                                                }
                                                let pure_cookie = if c.contains(".ROBLOSECURITY=") {
                                                    c.split(".ROBLOSECURITY=").nth(1).unwrap_or(c).split(';').next().unwrap_or(c).trim()
                                                } else {
                                                    c
                                                };

                                                match RobloxAPI::validate_cookie(pure_cookie) {
                                                    Ok(info) => {
                                                        let thumb = RobloxAPI::get_user_thumbnail(info.id);
                                                        let _ = tx.send(AppEvent::Log(format!(
                                                            "새 계정 검증 성공: {} (@{})",
                                                            info.name, info.display_name
                                                        )));

                                                        let mut store = AccountStore::new("accounts.enc");
                                                        if store.try_auto_unlock() {
                                                            let _ = store.upsert_account(
                                                                info.id,
                                                                &info.name,
                                                                &info.display_name,
                                                                pure_cookie,
                                                                &grp,
                                                                thumb,
                                                            );
                                                            added_cnt += 1;
                                                        }
                                                    }
                                                    Err(e) => {
                                                        let _ = tx.send(AppEvent::Log(format!("쿠키 검증 실패: {}", e)));
                                                    }
                                                }
                                            }

                                            let _ = tx.send(AppEvent::Toast(format!("총 {}개 계정이 등록되었습니다.", added_cnt)));
                                            let _ = tx.send(AppEvent::AccountsChanged);
                                        });

                                        close_modal = true;
                                    }
                                });
                            });
                        });
                }
                Modal::LaunchOptions {
                    job_id,
                    delay,
                    max_n,
                } => {
                    Window::new("클라이언트 실행 고급 옵션")
                        .collapsible(false)
                        .resizable(false)
                        .fixed_size(Vec2::new(380.0, 240.0))
                        .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
                        .frame(
                            egui::Frame::none()
                                .fill(COLOR_BG_CARD)
                                .stroke(Stroke::new(1.0_f32, COLOR_BORDER_LIGHT))
                                .rounding(Rounding::same(10.0))
                                .inner_margin(20.0),
                        )
                        .show(ctx, |ui| {
                            ui.label(RichText::new("Job ID (특정 서버 인스턴스 GUID, 선택):").size(12.5).color(COLOR_TEXT_MUTED));
                            ui.add(TextEdit::singleline(job_id).desired_width(340.0));

                            ui.add_space(12.0);
                            ui.horizontal(|ui| {
                                ui.label(RichText::new("계정별 실행 간격:").size(12.5).color(COLOR_TEXT_MUTED));
                                ui.add(egui::DragValue::new(delay).speed(0.1).range(0.5..=30.0).suffix(" 초"));
                            });

                            ui.add_space(10.0);
                            ui.horizontal(|ui| {
                                ui.label(RichText::new("최대 동시 실행 수:").size(12.5).color(COLOR_TEXT_MUTED));
                                ui.add(egui::DragValue::new(max_n).speed(1).range(1..=100).suffix(" 개"));
                            });

                            ui.add_space(22.0);
                            ui.horizontal(|ui| {
                                if ui.button("취소").clicked() {
                                    close_modal = true;
                                }

                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    if Self::ui_primary_btn(ui, "확인", Vec2::new(90.0, 34.0)).clicked() {
                                        self.config_store.config.last_job_id = job_id.clone();
                                        self.config_store.config.launch_delay = *delay;
                                        self.config_store.config.max_concurrent = *max_n;
                                        let _ = self.config_store.save();
                                        self.logs.push(format!("실행 옵션 저장: 간격 {}초, 최대 {}개", delay, max_n));
                                        close_modal = true;
                                    }
                                });
                            });
                        });
                }
                Modal::ConfirmDelete => {
                    Window::new("계정 삭제 확인")
                        .collapsible(false)
                        .resizable(false)
                        .fixed_size(Vec2::new(340.0, 140.0))
                        .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
                        .frame(
                            egui::Frame::none()
                                .fill(COLOR_BG_CARD)
                                .stroke(Stroke::new(1.0_f32, COLOR_BORDER_LIGHT))
                                .rounding(Rounding::same(10.0))
                                .inner_margin(20.0),
                        )
                        .show(ctx, |ui| {
                            let cnt = self.selected_ids.len();
                            ui.label(
                                RichText::new(format!("선택한 {}개의 계정을 목록에서 완전히 삭제하시겠습니까?", cnt))
                                    .size(13.5)
                                    .color(COLOR_TEXT_WHITE),
                            );

                            ui.add_space(20.0);
                            ui.horizontal(|ui| {
                                if ui.button("취소").clicked() {
                                    close_modal = true;
                                }

                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    if Self::ui_danger_btn(ui, "삭제", Vec2::new(80.0, 32.0)).clicked() {
                                        let ids: Vec<u64> = self.selected_ids.iter().cloned().collect();
                                        for uid in &ids {
                                            self.multi_mgr.terminate_account(*uid);
                                        }
                                        let _ = self.store.remove_accounts(&ids);
                                        self.selected_ids.clear();
                                        self.logs.push(format!("{}개 계정 삭제 완료", ids.len()));
                                        self.toast_msg = format!("{}개 계정이 삭제되었습니다.", ids.len());
                                        close_modal = true;
                                    }
                                });
                            });
                        });
                }
            }
        }

        if close_modal {
            self.modal = None;
        }
    }
}
