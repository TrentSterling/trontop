#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod diagnostics;
mod disk_activity;
mod export;
mod failure;
mod format;
mod gpu_activity;
mod gpu_sensors;
mod icons;
mod inventory;
mod model;
mod platform;
mod process_icons;
mod sampler;
mod service_control;
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
    let failures = std::sync::Arc::new(failure::Recorder::new(
        trontop_state_directory().map(|root| root.join(failure::LOG_NAME)),
    ));
    failures.clone().install();
    let result = run();
    record_run_failure(&failures, &result);
    result
}

fn record_run_failure(failures: &failure::Recorder, result: &eframe::Result) {
    if let Err(error) = result {
        let kind = match error {
            eframe::Error::AppCreation(_) => failure::Kind::AppCreation,
            eframe::Error::Winit(_) => failure::Kind::WindowSystem,
            eframe::Error::WinitEventLoop(_) => failure::Kind::EventLoop,
            eframe::Error::Wgpu(_) => failure::Kind::Graphics,
        };
        failures.record(kind, None);
    }
}

fn run() -> eframe::Result {
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
    let directory = trontop_state_directory()?;
    let _ = std::fs::create_dir_all(&directory);
    Some(directory.join("state-v2.ron"))
}

fn trontop_state_directory() -> Option<std::path::PathBuf> {
    let root = std::env::var_os("LOCALAPPDATA").map(std::path::PathBuf::from)?;
    root.is_absolute().then(|| root.join("Trontop"))
}
