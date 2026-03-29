mod app;
mod music;
mod texture;
mod ui;

use std::sync::Arc;

use app::App;
use eframe::egui;

const APP_NAME: &str = "Floating Fruit — Apple Music Widget";

fn main() -> eframe::Result<()> {
    let icon = eframe::icon_data::from_png_bytes(include_bytes!("../assets/app-icon.png"))
        .expect("Failed to load app icon");

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(APP_NAME)
            .with_inner_size([340.0, 150.0])
            .with_resizable(false)
            .with_decorations(false)
            .with_always_on_top()
            .with_icon(Arc::new(icon)),
        ..Default::default()
    };

    eframe::run_native(
        APP_NAME,
        options,
        Box::new(|cc| Ok(Box::new(App::new(cc)))),
    )
}
