use std::time::Duration;

use eframe::egui::{self, RichText};

use crate::app::App;
use crate::music::{PlayerCommand, PlayerState};

const COVER_SIZE: f32 = 72.0;
const COVER_ROUNDING: f32 = 8.0;
const BG_COLOR: egui::Color32 = egui::Color32::from_rgb(24, 24, 28);
const PLACEHOLDER_COLOR: egui::Color32 = egui::Color32::from_rgb(45, 45, 52);
const ERROR_TEXT_COLOR: egui::Color32 = egui::Color32::from_rgb(255, 140, 140);
const CONTROL_COLOR: egui::Color32 = egui::Color32::from_gray(200);
const CONTROL_HOVER_COLOR: egui::Color32 = egui::Color32::WHITE;
const CONTROL_PRESSED_COLOR: egui::Color32 = egui::Color32::from_gray(140);
const CONTROL_FONT_SIZE: f32 = 16.0;
const CONTROL_PADDING: egui::Vec2 = egui::vec2(10.0, 6.0);
const CONTROL_ROUNDING: f32 = 4.0;
const PROGRESS_HEIGHT: f32 = 20.0;
const PROGRESS_TRACK_COLOR: egui::Color32 = egui::Color32::from_gray(40);
const PROGRESS_FILL_COLOR: egui::Color32 = egui::Color32::from_rgb(200, 205, 215);
const PROGRESS_FILL_PRESSED_COLOR: egui::Color32 = egui::Color32::from_rgb(255, 140, 40);
const PROGRESS_FILL_SEEK_COLOR: egui::Color32 = egui::Color32::from_rgb(150, 35, 35);
const PROGRESS_FILL_HOVER_INTENSITY: f32 = 1.35;
const PROGRESS_FILL_SPEED: f32 = 1.0;
const PROGRESS_FILL_SEEK_SPEED: f32 = 2.5;
/// Time constant for easing the lava fill toward its target state.
const FILL_TRANSITION_DURATION: f32 = 0.3;
const TIME_FONT_SIZE: f32 = 14.0;
const TIME_COLOR: egui::Color32 = egui::Color32::from_gray(140);
const TIME_MARGIN_BOTTOM: f32 = 2.0;
const TIME_MARGIN_X: f32 = 4.0;
const SCROLL_SPEED: f32 = 30.0;
const SCROLL_PAUSE: f64 = 4.0;
const SCROLL_REPAINT: Duration = Duration::from_millis(50);

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

        let mut pending_command: Option<PlayerCommand> = None;
        let mut reveal_requested = false;

        let track_snapshot = self.track.clone();
        let last_error_snapshot = self.last_error.clone();
        let cover_texture_snapshot = self.cover_texture.clone();
        let has_track = track_snapshot.is_some();
        let cache = &mut self.galley_cache;

        ui.horizontal(|ui| {
            let cover_size = egui::vec2(COVER_SIZE, COVER_SIZE);
            let sense = if has_track {
                egui::Sense::click()
            } else {
                egui::Sense::hover()
            };
            let (cover_rect, cover_response) = ui.allocate_exact_size(cover_size, sense);

            if let Some(texture) = &cover_texture_snapshot {
                let uv = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
                ui.painter()
                    .image(texture.id(), cover_rect, uv, egui::Color32::WHITE);
            } else {
                ui.painter()
                    .rect_filled(cover_rect, COVER_ROUNDING, PLACEHOLDER_COLOR);
                ui.painter().text(
                    cover_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    "♪",
                    egui::FontId::proportional(28.0),
                    egui::Color32::LIGHT_GRAY,
                );
            }

            if has_track && cover_response.hovered() {
                let alpha = if cover_response.is_pointer_button_down_on() {
                    25
                } else {
                    12
                };
                ui.painter().rect_filled(
                    cover_rect,
                    COVER_ROUNDING,
                    egui::Color32::from_white_alpha(alpha),
                );
            }

            if cover_response.clicked() && has_track {
                reveal_requested = true;
            }

            ui.add_space(8.0);

            ui.vertical(|ui| {
                ui.add_space(8.0);

                if let Some(track) = &track_snapshot {
                    scrolling_label(
                        ui,
                        &mut *cache,
                        &track.title,
                        egui::FontId::proportional(18.0),
                        egui::Color32::WHITE,
                    );
                    ui.add_space(4.0);
                    scrolling_label(
                        ui,
                        &mut *cache,
                        &track.artist,
                        egui::FontId::proportional(14.0),
                        egui::Color32::from_gray(190),
                    );
                    ui.add_space(2.0);
                    scrolling_label(
                        ui,
                        &mut *cache,
                        &track.album,
                        egui::FontId::proportional(12.0),
                        egui::Color32::from_gray(150),
                    );
                    ui.add_space(4.0);

                    let play_pause_label = match track.state {
                        PlayerState::Playing => "⏸",
                        _ => "▶",
                    };

                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 4.0;

                        if control_button(ui, &mut *cache, "⏮").clicked() {
                            pending_command = Some(PlayerCommand::Previous);
                        }
                        if control_button(ui, &mut *cache, play_pause_label).clicked() {
                            pending_command = Some(PlayerCommand::PlayPause);
                        }
                        if control_button(ui, &mut *cache, "⏭").clicked() {
                            pending_command = Some(PlayerCommand::Next);
                        }
                    });
                } else if let Some(err) = &last_error_snapshot {
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
                            .color(egui::Color32::from_gray(180)),
                    );
                }
            });
        });

        if reveal_requested {
            self.reveal_in_app();
        }

        if let Some(cmd) = pending_command {
            self.execute_command(cmd);
        }

        let draw_seekbar = self.track.as_ref().is_some_and(|t| t.duration > 0.0);

        if draw_seekbar {
            let duration = self.track.as_ref().unwrap().duration;
            let full_rect = ui.max_rect();
            let base_position = self.effective_position();

            let bar_top = full_rect.bottom() - PROGRESS_HEIGHT;
            let bar_rect = egui::Rect::from_min_size(
                egui::pos2(full_rect.left(), bar_top),
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

            ui.painter()
                .rect_filled(bar_rect, 0.0, PROGRESS_TRACK_COLOR);

            let fill_width = bar_rect.width() * fraction;
            let fill_rect =
                egui::Rect::from_min_size(bar_rect.min, egui::vec2(fill_width, PROGRESS_HEIGHT));

            if fill_width > 0.0 {
                let seeking = self.pending_seek_position.is_some();
                let pressed = bar_response.is_pointer_button_down_on();
                let hovered = bar_response.hovered();

                let target_color = if seeking {
                    rgb_norm(PROGRESS_FILL_SEEK_COLOR)
                } else if pressed {
                    rgb_norm(PROGRESS_FILL_PRESSED_COLOR)
                } else {
                    rgb_norm(PROGRESS_FILL_COLOR)
                };
                let target_intensity = if seeking {
                    0.85
                } else if hovered {
                    PROGRESS_FILL_HOVER_INTENSITY
                } else {
                    1.0
                };
                let target_speed = if seeking {
                    PROGRESS_FILL_SEEK_SPEED
                } else {
                    PROGRESS_FILL_SPEED
                };

                // Smoothly ease toward the target state over FILL_TRANSITION_DURATION.
                let now = ui.ctx().input(|i| i.time) as f32;
                let dt = self
                    .fill_last_frame
                    .map_or(0.016, |t| (now - t).clamp(0.0, 0.1));
                self.fill_last_frame = Some(now);
                let lerp = 1.0 - (-dt / FILL_TRANSITION_DURATION).exp();
                for (i, target) in target_color.iter().enumerate() {
                    self.fill_color[i] += (target - self.fill_color[i]) * lerp;
                }
                self.fill_intensity += (target_intensity - self.fill_intensity) * lerp;
                self.fill_speed += (target_speed - self.fill_speed) * lerp;

                let time = now;
                crate::progressbar::paint_fill(
                    ui,
                    fill_rect,
                    self.fill_color,
                    time,
                    self.fill_intensity,
                    self.fill_speed,
                    self.fill_resource.clone(),
                );
                ui.ctx().request_repaint();
            }

            let time_text = format!(
                "{} / {}",
                format_time(display_position),
                format_time(duration),
            );
            let font = egui::FontId::monospace(TIME_FONT_SIZE);
            let galley = self.cached_galley(ui.painter(), &time_text, font, TIME_COLOR);
            let text_pos = egui::pos2(
                full_rect.left() + TIME_MARGIN_X,
                bar_top - galley.size().y - TIME_MARGIN_BOTTOM,
            );
            ui.painter().galley(text_pos, galley, TIME_COLOR);
        }
    }
}

fn control_button(
    ui: &mut egui::Ui,
    cache: &mut crate::app::GalleyCache,
    label: &str,
) -> egui::Response {
    let font = egui::FontId::proportional(CONTROL_FONT_SIZE);
    let base_galley =
        crate::app::cached_galley(cache, ui.painter(), label, font.clone(), CONTROL_COLOR);
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

        let galley = crate::app::cached_galley(cache, ui.painter(), label, font, text_color);

        let text_pos = rect.center() - galley.size() / 2.0;
        ui.painter().galley(text_pos, galley, text_color);
    }

    response
}

fn scrolling_label(
    ui: &mut egui::Ui,
    cache: &mut crate::app::GalleyCache,
    text: &str,
    font: egui::FontId,
    color: egui::Color32,
) {
    let max_width = ui.available_width();
    let galley = crate::app::cached_galley(cache, ui.painter(), text, font, color);
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

fn rgb_norm(c: egui::Color32) -> [f32; 4] {
    [
        c.r() as f32 / 255.0,
        c.g() as f32 / 255.0,
        c.b() as f32 / 255.0,
        c.a() as f32 / 255.0,
    ]
}

fn format_time(seconds: f64) -> String {
    let total = seconds.max(0.0) as u64;
    let m = total / 60;
    let s = total % 60;
    format!("{m}:{s:02}")
}
