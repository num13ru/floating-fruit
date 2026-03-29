use std::{
    fs,
    path::PathBuf,
    process::Command,
    time::{Duration, Instant},
};

use anyhow::{Context, Result, anyhow};
use eframe::egui::{self, ColorImage, RichText, TextureHandle};

const POLL_EVERY: Duration = Duration::from_secs(1);
const FIELD_SEP: char = '\u{001f}';

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Apple Music Now Playing")
            .with_inner_size([340.0, 96.0])
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

#[derive(Debug, Clone, PartialEq)]
struct TrackInfo {
    state: String,
    title: String,
    artist: String,
    artwork_path: Option<PathBuf>,
}

struct App {
    last_poll: Instant,
    track: Option<TrackInfo>,
    cover_texture: Option<TextureHandle>,
    last_loaded_art_path: Option<PathBuf>,
    last_error: Option<String>,
}

impl App {
    fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        Self {
            last_poll: Instant::now() - POLL_EVERY,
            track: None,
            cover_texture: None,
            last_loaded_art_path: None,
            last_error: None,
        }
    }

    fn poll_if_needed(&mut self, ctx: &egui::Context) {
        if self.last_poll.elapsed() < POLL_EVERY {
            ctx.request_repaint_after(POLL_EVERY - self.last_poll.elapsed());
            return;
        }

        self.last_poll = Instant::now();

        match query_music() {
            Ok(Some(track)) => {
                let art_changed = track.artwork_path != self.last_loaded_art_path;
                self.track = Some(track.clone());
                self.last_error = None;

                if art_changed {
                    self.cover_texture = None;
                    self.last_loaded_art_path = track.artwork_path.clone();

                    if let Some(path) = &track.artwork_path {
                        match load_texture_from_file(ctx, path) {
                            Ok(texture) => self.cover_texture = Some(texture),
                            Err(err) => {
                                self.last_error = Some(format!("Failed to load artwork: {err:#}"))
                            }
                        }
                    }
                }
            }
            Ok(None) => {
                self.track = None;
                self.cover_texture = None;
                self.last_loaded_art_path = None;
                self.last_error = None;
            }
            Err(err) => {
                self.track = None;
                self.cover_texture = None;
                self.last_loaded_art_path = None;
                self.last_error = Some(format!("{err:#}"));
            }
        }

        ctx.request_repaint_after(POLL_EVERY);
    }
}

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

        ui.visuals_mut().widgets.noninteractive.bg_fill = egui::Color32::from_rgb(24, 24, 28);
        ui.visuals_mut().override_text_color = Some(egui::Color32::WHITE);

        ui.horizontal(|ui| {
            if let Some(texture) = &self.cover_texture {
                ui.add(egui::Image::new(texture).fit_to_exact_size(egui::vec2(72.0, 72.0)));
            } else {
                let (rect, _) =
                    ui.allocate_exact_size(egui::vec2(72.0, 72.0), egui::Sense::hover());
                ui.painter()
                    .rect_filled(rect, 8.0, egui::Color32::from_rgb(45, 45, 52));
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
                            .color(egui::Color32::from_rgb(255, 140, 140)),
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

fn query_music() -> Result<Option<TrackInfo>> {
    let base_path = std::env::temp_dir().join("am_now_playing_artwork");
    let base_path_str = base_path
        .to_str()
        .ok_or_else(|| anyhow!("Temp path is not valid UTF-8"))?;

    let script = format!(
        r#"
if application "Music" is not running then
    return "stopped"
end if

set sep to character id 31
set outputBase to "{output_base}"

tell application "Music"
    if player state is stopped then
        return "stopped"
    end if

    set t to current track
    set trackState to (player state as text)
    set trackTitle to (name of t as text)
    set trackArtist to (artist of t as text)
    set artPath to ""

    try
        if (count of artworks of t) > 0 then
            tell artwork 1 of t
                if format is JPEG picture then
                    set imgExt to ".jpg"
                else
                    set imgExt to ".png"
                end if
            end tell

            set rawData to (get raw data of artwork 1 of t)
            set artPath to outputBase & imgExt

            set fileRef to open for access (POSIX file artPath) with write permission
            set eof fileRef to 0
            write rawData to fileRef
            close access fileRef
        end if
    on error
        try
            close access fileRef
        end try
        set artPath to ""
    end try

    set AppleScript's text item delimiters to sep
    return {{trackState, trackTitle, trackArtist, artPath}} as text
end tell
"#,
        output_base = escape_applescript_string(base_path_str),
    );

    let output = Command::new("osascript")
        .arg("-e")
        .arg(script)
        .output()
        .context("Failed to run osascript")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow!("osascript failed: {}", stderr.trim()));
    }

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();

    if stdout == "stopped" || stdout.is_empty() {
        return Ok(None);
    }

    let parts: Vec<&str> = stdout.split(FIELD_SEP).collect();
    if parts.len() != 4 {
        return Err(anyhow!("Unexpected AppleScript output: {:?}", stdout));
    }

    let artwork_path = if parts[3].trim().is_empty() {
        None
    } else {
        Some(PathBuf::from(parts[3].trim()))
    };

    Ok(Some(TrackInfo {
        state: parts[0].trim().to_string(),
        title: parts[1].trim().to_string(),
        artist: parts[2].trim().to_string(),
        artwork_path,
    }))
}

fn load_texture_from_file(ctx: &egui::Context, path: &PathBuf) -> Result<TextureHandle> {
    let bytes = fs::read(path).with_context(|| format!("Failed reading {}", path.display()))?;
    let image = image::load_from_memory(&bytes)
        .with_context(|| format!("Failed decoding {}", path.display()))?
        .to_rgba8();

    let size = [image.width() as usize, image.height() as usize];
    let pixels = image.into_vec();
    let color_image = ColorImage::from_rgba_unmultiplied(size, &pixels);

    Ok(ctx.load_texture("album_art", color_image, egui::TextureOptions::LINEAR))
}

fn escape_applescript_string(input: &str) -> String {
    input.replace('\\', "\\\\").replace('"', "\\\"")
}
