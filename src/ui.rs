use eframe::egui::{self, RichText};

use crate::app::App;

const COVER_SIZE: f32 = 72.0;
const COVER_ROUNDING: f32 = 8.0;
const BG_COLOR: egui::Color32 = egui::Color32::from_rgb(24, 24, 28);
const PLACEHOLDER_COLOR: egui::Color32 = egui::Color32::from_rgb(45, 45, 52);
const ERROR_TEXT_COLOR: egui::Color32 = egui::Color32::from_rgb(255, 140, 140);

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.poll_if_needed(ui.ctx());

        let drag_response = ui.interact(
            ui.max_rect(),
            ui.id().with("window_drag_area"),
            egui::Sense::click_and_drag(),
        );

        if drag_response.drag_started() {
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);
        }

        ui.visuals_mut().widgets.noninteractive.bg_fill = BG_COLOR;
        ui.visuals_mut().override_text_color = Some(egui::Color32::WHITE);

        ui.horizontal(|ui| {
            if let Some(texture) = &self.cover_texture {
                let size = egui::vec2(COVER_SIZE, COVER_SIZE);
                ui.add(egui::Image::new(texture).fit_to_exact_size(size));
            } else {
                let size = egui::vec2(COVER_SIZE, COVER_SIZE);
                let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
                ui.painter()
                    .rect_filled(rect, COVER_ROUNDING, PLACEHOLDER_COLOR);
                ui.painter().text(
                    rect.center(),
                    egui::Align2::CENTER_CENTER,
                    "♪",
                    egui::FontId::proportional(28.0),
                    egui::Color32::LIGHT_GRAY,
                );
            }

            ui.add_space(8.0);

            ui.vertical(|ui| {
                ui.add_space(8.0);

                if let Some(track) = &self.track {
                    ui.label(
                        RichText::new(&track.title)
                            .strong()
                            .size(18.0)
                            .color(egui::Color32::WHITE),
                    );
                    ui.add_space(4.0);
                    ui.label(
                        RichText::new(&track.artist)
                            .size(14.0)
                            .color(egui::Color32::from_gray(190)),
                    );
                    ui.add_space(4.0);
                    ui.label(
                        RichText::new(format!("State: {}", track.state))
                            .size(11.0)
                            .color(egui::Color32::from_gray(130)),
                    );
                } else if let Some(err) = &self.last_error {
                    ui.label(
                        RichText::new("Apple Music unavailable")
                            .strong()
                            .size(18.0)
                            .color(egui::Color32::WHITE),
                    );
                    ui.add_space(4.0);
                    ui.label(
                        RichText::new(err)
                            .size(12.0)
                            .color(ERROR_TEXT_COLOR),
                    );
                } else {
                    ui.label(
                        RichText::new("Nothing playing")
                            .strong()
                            .size(18.0)
                            .color(egui::Color32::WHITE),
                    );
                    ui.add_space(4.0);
                    ui.label(
                        RichText::new("Open Music.app and start playback")
                            .size(13.0)
                            .color(egui::Color32::from_gray(180)),
                    );
                }
            });
        });
    }
}
