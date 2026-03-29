mod app;
mod music;
mod texture;
mod ui;

use app::App;
use eframe::egui;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Apple Music Now Playing")
            .with_inner_size([340.0, 120.0])
            .with_resizable(false)
            .with_decorations(false)
            .with_always_on_top(),
        ..Default::default()
    };

    eframe::run_native(
        "Apple Music Now Playing",
        options,
        Box::new(|cc| Ok(Box::new(App::new(cc)))),
    )
}
