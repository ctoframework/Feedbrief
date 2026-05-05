#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod feeds;
mod fetcher;
mod llm;
mod pipeline;
mod progress;
mod storage;

use eframe::egui;

fn main() -> Result<(), eframe::Error> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1200.0, 850.0])
            .with_min_inner_size([900.0, 600.0])
            .with_title("TechBrief"),
        ..Default::default()
    };
    eframe::run_native(
        "TechBrief",
        options,
        Box::new(|cc| Ok(Box::new(app::TechBriefApp::new(cc)))),
    )
}
