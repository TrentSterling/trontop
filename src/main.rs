#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod diagnostics;
mod format;
mod gpu_activity;
mod gpu_sensors;
mod icons;
mod model;
mod platform;
mod process_icons;
mod sampler;
mod shutdown;
mod startup;
mod storage_sensors;
mod theme;
mod tray;
mod widgets;
mod windows_metrics;

use app::TrontopApp;
use eframe::egui;

fn main() -> eframe::Result {
    let persistence_path = trontop_state_path();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Trontop")
            .with_icon(tray::window_icon())
            .with_inner_size([1280.0, 760.0])
            .with_min_inner_size([1040.0, 640.0])
            .with_decorations(false),
        renderer: eframe::Renderer::Wgpu,
        centered: true,
        persist_window: false,
        persistence_path,
        ..Default::default()
    };

    eframe::run_native(
        "Trontop",
        options,
        Box::new(|cc| Ok(Box::new(TrontopApp::new(cc)))),
    )
}

fn trontop_state_path() -> Option<std::path::PathBuf> {
    let root = std::env::var_os("LOCALAPPDATA").map(std::path::PathBuf::from)?;
    let directory = root.join("Trontop");
    let _ = std::fs::create_dir_all(&directory);
    Some(directory.join("state-v2.ron"))
}
