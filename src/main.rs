#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod crypto_store;
mod multi_instance;
mod roblox_api;
mod updater;
mod win_dpapi;

use anyhow::Result;
use app::RobloxLauncherApp;
use eframe::egui::Vec2;

fn main() -> Result<()> {
    // 윈도우 GUI 실행 옵션 설정
    let mut native_options = eframe::NativeOptions::default();
    native_options.viewport = egui::ViewportBuilder::default()
        .with_title("Two-Roblox")
        .with_inner_size(Vec2::new(1120.0, 720.0))
        .with_min_inner_size(Vec2::new(940.0, 600.0))
        .with_active(true);

    // 아이콘 로드 시도
    let icon_bytes = include_bytes!("../assets/icon.png");
    if let Ok(img) = image::load_from_memory(icon_bytes) {
        let rgba = img.to_rgba8();
        let (width, height) = rgba.dimensions();
        native_options.viewport = native_options.viewport.with_icon(egui::IconData {
            rgba: rgba.into_raw(),
            width,
            height,
        });
    }

    eframe::run_native(
        "Two-Roblox",
        native_options,
        Box::new(|cc| Ok(Box::new(RobloxLauncherApp::new(cc)))),
    )
    .map_err(|e| anyhow::anyhow!("GUI 런타임 오류: {:?}", e))?;

    Ok(())
}
