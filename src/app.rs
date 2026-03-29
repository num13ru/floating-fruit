use std::{
    path::PathBuf,
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

use anyhow::Result;
use eframe::egui::{self, TextureHandle};

use crate::{music, texture};

pub(crate) const POLL_EVERY: Duration = Duration::from_secs(1);
const POLL_CHECK: Duration = Duration::from_millis(50);
const COMMAND_SETTLE: Duration = Duration::from_millis(200);

type PollResult = Result<Option<music::TrackInfo>>;

pub struct App {
    pub(crate) track: Option<music::TrackInfo>,
    pub(crate) cover_texture: Option<TextureHandle>,
    pub(crate) last_loaded_art_path: Option<PathBuf>,
    pub(crate) last_error: Option<String>,
    pub(crate) pending_seek_position: Option<f64>,
    next_poll: Instant,
    poll_rx: Option<mpsc::Receiver<PollResult>>,
}

impl App {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        Self {
            track: None,
            cover_texture: None,
            last_loaded_art_path: None,
            last_error: None,
            pending_seek_position: None,
            next_poll: Instant::now(),
            poll_rx: None,
        }
    }

    pub(crate) fn poll_if_needed(&mut self, ctx: &egui::Context) {
        if let Some(rx) = &self.poll_rx {
            match rx.try_recv() {
                Ok(result) => {
                    self.poll_rx = None;
                    self.apply_poll_result(result, ctx);
                    self.next_poll = Instant::now() + POLL_EVERY;
                }
                Err(mpsc::TryRecvError::Empty) => {
                    ctx.request_repaint_after(POLL_CHECK);
                    return;
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.poll_rx = None;
                    self.next_poll = Instant::now() + POLL_EVERY;
                }
            }
        }

        if self.poll_rx.is_none() && Instant::now() >= self.next_poll {
            let (tx, rx) = mpsc::channel();
            thread::spawn(move || {
                let _ = tx.send(music::query());
            });
            self.poll_rx = Some(rx);
            ctx.request_repaint_after(POLL_CHECK);
        } else if self.poll_rx.is_none() {
            ctx.request_repaint_after(
                self.next_poll.saturating_duration_since(Instant::now()),
            );
        }
    }

    fn apply_poll_result(&mut self, result: PollResult, ctx: &egui::Context) {
        self.pending_seek_position = None;

        match result {
            Ok(Some(track)) => {
                let art_changed = track.artwork_path != self.last_loaded_art_path;
                self.last_error = None;

                if art_changed {
                    self.cover_texture = None;
                    self.last_loaded_art_path = track.artwork_path.clone();

                    if let Some(path) = &self.last_loaded_art_path {
                        match texture::load_from_file(ctx, path) {
                            Ok(tex) => self.cover_texture = Some(tex),
                            Err(err) => {
                                self.last_error =
                                    Some(format!("Failed to load artwork: {err:#}"));
                            }
                        }
                    }
                }

                self.track = Some(track);
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
    }

    pub(crate) fn execute_command(&mut self, cmd: music::PlayerCommand) {
        thread::spawn(move || {
            let _ = music::send_command(cmd);
        });
        self.poll_rx = None;
        self.next_poll = Instant::now() + COMMAND_SETTLE;
    }

    pub(crate) fn seek(&mut self, position: f64) {
        self.pending_seek_position = Some(position);
        self.execute_command(music::PlayerCommand::Seek(position));
    }
}
