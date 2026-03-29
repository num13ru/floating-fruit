mod app;
mod music;
mod texture;
mod ui;

use app::App;
use eframe::egui;

const APP_NAME: &str = "Floating Fruit — Apple Music Widget";

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(APP_NAME)
            .with_inner_size([340.0, 150.0])
            .with_resizable(false)
            .with_decorations(false)
            .with_always_on_top(),
        ..Default::default()
    };

    eframe::run_native(
        APP_NAME,
        options,
        Box::new(|cc| Ok(Box::new(App::new(cc)))),
    )
}
