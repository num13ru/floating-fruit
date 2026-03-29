use std::{
    path::PathBuf,
    time::{Duration, Instant},
};

use eframe::egui::{self, TextureHandle};

use crate::{music, texture};

pub(crate) const POLL_EVERY: Duration = Duration::from_secs(1);

pub struct App {
    pub(crate) last_poll: Instant,
    pub(crate) track: Option<music::TrackInfo>,
    pub(crate) cover_texture: Option<TextureHandle>,
    pub(crate) last_loaded_art_path: Option<PathBuf>,
    pub(crate) last_error: Option<String>,
}

impl App {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        Self {
            last_poll: Instant::now() - POLL_EVERY,
            track: None,
            cover_texture: None,
            last_loaded_art_path: None,
            last_error: None,
        }
    }

    pub(crate) fn poll_if_needed(&mut self, ctx: &egui::Context) {
        if self.last_poll.elapsed() < POLL_EVERY {
            ctx.request_repaint_after(POLL_EVERY - self.last_poll.elapsed());
            return;
        }

        self.last_poll = Instant::now();

        match music::query() {
            Ok(Some(track)) => {
                let art_changed = track.artwork_path != self.last_loaded_art_path;
                self.track = Some(track.clone());
                self.last_error = None;

                if art_changed {
                    self.cover_texture = None;
                    self.last_loaded_art_path = track.artwork_path.clone();

                    if let Some(path) = &track.artwork_path {
                        match texture::load_from_file(ctx, path) {
                            Ok(tex) => self.cover_texture = Some(tex),
                            Err(err) => {
                                self.last_error =
                                    Some(format!("Failed to load artwork: {err:#}"));
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
