#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod format;
mod model;
mod platform;
mod sampler;
mod theme;

use app::TrontopApp;
use eframe::egui;

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Trontop")
            .with_inner_size([1280.0, 760.0])
            .with_min_inner_size([940.0, 600.0]),
        renderer: eframe::Renderer::Wgpu,
        ..Default::default()
    };

    eframe::run_native(
        "Trontop",
        options,
        Box::new(|cc| Ok(Box::new(TrontopApp::new(cc)))),
    )
}
