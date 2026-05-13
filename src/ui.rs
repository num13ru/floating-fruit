use std::time::Duration;

use eframe::egui::{self, RichText};

use crate::app::App;
use crate::music::{PlayerCommand, PlayerState};

const COVER_SIZE: f32 = 72.0;
const COVER_ROUNDING: f32 = 0.0;
const SHELL_ROUNDING: f32 = 0.0;
const SHELL_MARGIN: f32 = 6.0;
const ERROR_TEXT_COLOR: egui::Color32 = egui::Color32::from_rgb(255, 166, 176);
const CONTROL_COLOR: egui::Color32 = egui::Color32::from_gray(218);
const CONTROL_HOVER_COLOR: egui::Color32 = egui::Color32::WHITE;
const CONTROL_PRESSED_COLOR: egui::Color32 = egui::Color32::from_gray(170);
const CONTROL_FONT_SIZE: f32 = 16.0;
const CONTROL_PADDING: egui::Vec2 = egui::vec2(11.0, 7.0);
const CONTROL_ROUNDING: f32 = 0.0;
const PROGRESS_HEIGHT: f32 = 20.0;
const SHIMMER_WIDTH: f32 = 40.0;
const SHIMMER_PERIOD: f64 = 1.0;
const SHIMMER_ALPHA: u8 = 48;
const TIME_FONT_SIZE: f32 = 14.0;
const TIME_MARGIN_BOTTOM: f32 = 5.0;
const TIME_MARGIN_X: f32 = 14.0;
const SCROLL_SPEED: f32 = 30.0;
const SCROLL_PAUSE: f64 = 4.0;
const SCROLL_REPAINT: Duration = Duration::from_millis(50);

impl eframe::App for App {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        egui::Color32::TRANSPARENT.to_normalized_gamma_f32()
    }

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

        ui.visuals_mut().window_fill = egui::Color32::TRANSPARENT;
        ui.visuals_mut().panel_fill = egui::Color32::TRANSPARENT;
        ui.visuals_mut().widgets.noninteractive.bg_fill = egui::Color32::TRANSPARENT;
        ui.visuals_mut().override_text_color = Some(egui::Color32::WHITE);

        let mut pending_command: Option<PlayerCommand> = None;
        let shell_rect = ui.max_rect().shrink(SHELL_MARGIN);

        paint_liquid_glass(ui, shell_rect);
        ui.scope_builder(
            egui::UiBuilder::new().max_rect(shell_rect.shrink2(egui::vec2(12.0, 11.0))),
            |ui| {
                ui.set_clip_rect(shell_rect);

                ui.horizontal(|ui| {
                    let cover_size = egui::vec2(COVER_SIZE, COVER_SIZE);
                    let has_track = self.track.is_some();
                    let sense = if has_track {
                        egui::Sense::click()
                    } else {
                        egui::Sense::hover()
                    };
                    let (cover_rect, cover_response) = ui.allocate_exact_size(cover_size, sense);

                    paint_cover_backplate(ui, cover_rect);
                    if let Some(texture) = &self.cover_texture {
                        let uv =
                            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
                        ui.painter()
                            .image(texture.id(), cover_rect, uv, egui::Color32::WHITE);
                    } else {
                        ui.painter().rect_filled(
                            cover_rect,
                            COVER_ROUNDING,
                            rgba(255, 255, 255, 30),
                        );
                        ui.painter().text(
                            cover_rect.center(),
                            egui::Align2::CENTER_CENTER,
                            "♪",
                            egui::FontId::proportional(28.0),
                            egui::Color32::from_rgba_unmultiplied(240, 248, 255, 190),
                        );
                    }
                    paint_cover_gloss(ui, cover_rect);

                    if has_track && cover_response.hovered() {
                        let alpha = if cover_response.is_pointer_button_down_on() {
                            46
                        } else {
                            28
                        };
                        ui.painter().rect_filled(
                            cover_rect,
                            COVER_ROUNDING,
                            egui::Color32::from_white_alpha(alpha),
                        );
                    }

                    if cover_response.clicked() && has_track {
                        self.reveal_in_app();
                    }

                    ui.add_space(8.0);

                    ui.vertical(|ui| {
                        ui.add_space(8.0);

                        if let Some(track) = &self.track {
                            scrolling_label(
                                ui,
                                &track.title,
                                egui::FontId::proportional(18.0),
                                egui::Color32::WHITE,
                            );
                            ui.add_space(4.0);
                            scrolling_label(
                                ui,
                                &track.artist,
                                egui::FontId::proportional(14.0),
                                egui::Color32::from_rgba_unmultiplied(226, 238, 248, 218),
                            );
                            ui.add_space(2.0);
                            scrolling_label(
                                ui,
                                &track.album,
                                egui::FontId::proportional(12.0),
                                egui::Color32::from_rgba_unmultiplied(210, 226, 238, 170),
                            );
                            ui.add_space(4.0);

                            let play_pause_label = match track.state {
                                PlayerState::Playing => "⏸",
                                _ => "▶",
                            };

                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = 4.0;

                                if control_button(ui, "⏮").clicked() {
                                    pending_command = Some(PlayerCommand::Previous);
                                }
                                if control_button(ui, play_pause_label).clicked() {
                                    pending_command = Some(PlayerCommand::PlayPause);
                                }
                                if control_button(ui, "⏭").clicked() {
                                    pending_command = Some(PlayerCommand::Next);
                                }
                            });
                        } else if let Some(err) = &self.last_error {
                            ui.label(
                                RichText::new("Apple Music unavailable")
                                    .strong()
                                    .size(18.0)
                                    .color(egui::Color32::WHITE),
                            );
                            ui.add_space(4.0);
                            ui.label(RichText::new(err).size(12.0).color(ERROR_TEXT_COLOR));
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
                                    .color(egui::Color32::from_rgba_unmultiplied(
                                        226, 238, 248, 205,
                                    )),
                            );
                        }
                    });
                });
            },
        );

        if let Some(cmd) = pending_command {
            self.execute_command(cmd);
        }

        if let Some(track) = &self.track
            && track.duration > 0.0
        {
            let full_rect = shell_rect;
            let duration = track.duration;
            let base_position = self.effective_position();

            let bar_top = full_rect.bottom() - PROGRESS_HEIGHT - 1.0;
            let bar_rect = egui::Rect::from_min_size(
                egui::pos2(full_rect.left() + 1.0, bar_top),
                egui::vec2(full_rect.width(), PROGRESS_HEIGHT),
            );

            let bar_response = ui.interact(
                bar_rect,
                ui.id().with("seekbar"),
                egui::Sense::click_and_drag(),
            );

            let pointer_fraction = bar_response
                .interact_pointer_pos()
                .map(|pos| ((pos.x - bar_rect.left()) / bar_rect.width()).clamp(0.0, 1.0));

            let is_interacting = bar_response.is_pointer_button_down_on() || bar_response.dragged();
            let display_position = if is_interacting {
                pointer_fraction.map_or(base_position, |f| f as f64 * duration)
            } else {
                base_position
            };
            let fraction = (display_position / duration).clamp(0.0, 1.0) as f32;

            if (bar_response.clicked() || bar_response.drag_stopped())
                && let Some(f) = pointer_fraction
            {
                self.seek(f as f64 * duration);
            }

            let fill_color = if bar_response.is_pointer_button_down_on() {
                rgba(118, 191, 242, 190)
            } else if bar_response.hovered() {
                rgba(207, 239, 255, 212)
            } else {
                rgba(172, 222, 255, 170)
            };

            ui.painter()
                .rect_filled(bar_rect, 0.0, rgba(255, 255, 255, 24));

            let fill_width = bar_rect.width() * fraction;
            let fill_rect =
                egui::Rect::from_min_size(bar_rect.min, egui::vec2(fill_width, PROGRESS_HEIGHT));
            ui.painter().rect_filled(fill_rect, 0.0, fill_color);
            if fill_width > 4.0 {
                let gleam_rect = egui::Rect::from_min_size(
                    fill_rect.min + egui::vec2(0.0, 2.0),
                    egui::vec2(fill_width, 5.0),
                );
                ui.painter().rect_filled(
                    gleam_rect,
                    0.0,
                    egui::Color32::from_rgba_unmultiplied(255, 255, 255, 56),
                );
            }

            if self.pending_seek_position.is_some() && fill_width > 0.0 {
                let time = ui.ctx().input(|i| i.time);
                let phase = ((time % SHIMMER_PERIOD) / SHIMMER_PERIOD) as f32;
                let shimmer_center = fill_rect.left() + phase * fill_width;
                let shimmer_left = (shimmer_center - SHIMMER_WIDTH / 2.0).max(fill_rect.left());
                let shimmer_right = (shimmer_center + SHIMMER_WIDTH / 2.0).min(fill_rect.right());

                if shimmer_right > shimmer_left {
                    let shimmer_rect = egui::Rect::from_x_y_ranges(
                        shimmer_left..=shimmer_right,
                        fill_rect.y_range(),
                    );
                    ui.painter().rect_filled(
                        shimmer_rect,
                        0.0,
                        egui::Color32::from_white_alpha(SHIMMER_ALPHA),
                    );
                }

                ui.ctx().request_repaint();
            }

            let time_text = format!(
                "{} / {}",
                format_time(display_position),
                format_time(duration),
            );
            let font = egui::FontId::proportional(TIME_FONT_SIZE);
            let time_color = rgba(232, 242, 250, 178);
            let galley = ui.painter().layout_no_wrap(time_text, font, time_color);
            let text_pos = egui::pos2(
                full_rect.left() + TIME_MARGIN_X,
                bar_top - galley.size().y - TIME_MARGIN_BOTTOM,
            );
            ui.painter().galley(text_pos, galley, time_color);
        }
    }
}

fn paint_liquid_glass(ui: &mut egui::Ui, rect: egui::Rect) {
    let painter = ui.painter();
    let shadow_rect = rect.translate(egui::vec2(0.0, 3.0));
    painter.rect_filled(shadow_rect, SHELL_ROUNDING, rgba(0, 0, 0, 76));
    painter.rect_filled(rect, SHELL_ROUNDING, rgba(28, 32, 38, 168));
    painter.rect_stroke(
        rect,
        SHELL_ROUNDING,
        egui::Stroke::new(1.0, rgba(255, 255, 255, 66)),
        egui::StrokeKind::Inside,
    );

    let top_glow = egui::Rect::from_min_max(
        rect.min + egui::vec2(12.0, 4.0),
        egui::pos2(rect.right() - 12.0, rect.top() + 34.0),
    );
    painter.rect_filled(top_glow, 0.0, rgba(255, 255, 255, 36));

    let lower_glow = egui::Rect::from_min_max(
        egui::pos2(rect.left() + 18.0, rect.bottom() - 42.0),
        egui::pos2(rect.right() - 18.0, rect.bottom() - 8.0),
    );
    painter.rect_filled(
        lower_glow,
        0.0,
        egui::Color32::from_rgba_unmultiplied(110, 176, 210, 22),
    );
}

fn paint_cover_backplate(ui: &mut egui::Ui, rect: egui::Rect) {
    ui.painter().rect_filled(
        rect.expand(2.0),
        0.0,
        egui::Color32::from_rgba_unmultiplied(255, 255, 255, 32),
    );
}

fn paint_cover_gloss(ui: &mut egui::Ui, rect: egui::Rect) {
    let gloss_rect = egui::Rect::from_min_max(
        rect.min + egui::vec2(4.0, 4.0),
        egui::pos2(rect.right() - 4.0, rect.top() + rect.height() * 0.42),
    );
    ui.painter().rect_filled(
        gloss_rect,
        COVER_ROUNDING,
        egui::Color32::from_rgba_unmultiplied(255, 255, 255, 34),
    );
    ui.painter().rect_stroke(
        rect,
        COVER_ROUNDING,
        egui::Stroke::new(
            1.0,
            egui::Color32::from_rgba_unmultiplied(255, 255, 255, 70),
        ),
        egui::StrokeKind::Inside,
    );
}

fn rgba(r: u8, g: u8, b: u8, a: u8) -> egui::Color32 {
    egui::Color32::from_rgba_unmultiplied(r, g, b, a)
}

fn control_button(ui: &mut egui::Ui, label: &str) -> egui::Response {
    let font = egui::FontId::proportional(CONTROL_FONT_SIZE);
    let base_galley = ui
        .painter()
        .layout_no_wrap(label.to_string(), font.clone(), CONTROL_COLOR);
    let desired_size = base_galley.size() + CONTROL_PADDING * 2.0;

    let (rect, response) = ui.allocate_exact_size(desired_size, egui::Sense::click());

    if ui.is_rect_visible(rect) {
        let text_color = if response.is_pointer_button_down_on() {
            CONTROL_PRESSED_COLOR
        } else if response.hovered() {
            CONTROL_HOVER_COLOR
        } else {
            CONTROL_COLOR
        };

        if response.hovered() {
            let bg_alpha = if response.is_pointer_button_down_on() {
                25
            } else {
                12
            };
            ui.painter().rect_filled(
                rect,
                CONTROL_ROUNDING,
                egui::Color32::from_white_alpha(bg_alpha),
            );
        }

        let galley = if text_color == CONTROL_COLOR {
            base_galley
        } else {
            ui.painter()
                .layout_no_wrap(label.to_string(), font, text_color)
        };

        let text_pos = rect.center() - galley.size() / 2.0;
        ui.painter().galley(text_pos, galley, text_color);
    }

    response
}

fn scrolling_label(ui: &mut egui::Ui, text: &str, font: egui::FontId, color: egui::Color32) {
    let max_width = ui.available_width();
    let galley = ui.painter().layout_no_wrap(text.to_string(), font, color);
    let text_width = galley.size().x;

    if text_width <= max_width {
        let (rect, _) = ui.allocate_exact_size(galley.size(), egui::Sense::hover());
        ui.painter().galley(rect.min, galley, color);
        return;
    }

    let overflow = text_width - max_width;
    let offset = ping_pong_offset(ui.ctx().input(|i| i.time), overflow);

    let (rect, _) =
        ui.allocate_exact_size(egui::vec2(max_width, galley.size().y), egui::Sense::hover());
    let text_pos = egui::pos2(rect.left() - offset, rect.top());
    ui.painter()
        .with_clip_rect(rect)
        .galley(text_pos, galley, color);
    ui.ctx().request_repaint_after(SCROLL_REPAINT);
}

fn ping_pong_offset(time: f64, overflow: f32) -> f32 {
    let scroll_dur = overflow as f64 / SCROLL_SPEED as f64;
    let cycle = SCROLL_PAUSE + scroll_dur + SCROLL_PAUSE + scroll_dur;
    let t = time % cycle;

    if t < SCROLL_PAUSE {
        0.0
    } else if t < SCROLL_PAUSE + scroll_dur {
        ((t - SCROLL_PAUSE) / scroll_dur) as f32 * overflow
    } else if t < 2.0 * SCROLL_PAUSE + scroll_dur {
        overflow
    } else {
        let back = t - 2.0 * SCROLL_PAUSE - scroll_dur;
        (1.0 - (back / scroll_dur) as f32) * overflow
    }
}

fn format_time(seconds: f64) -> String {
    let total = seconds.max(0.0) as u64;
    let m = total / 60;
    let s = total % 60;
    format!("{m}:{s:02}")
}
