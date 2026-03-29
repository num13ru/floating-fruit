use std::{
    path::PathBuf,
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

use anyhow::Result;
use eframe::egui::{self, TextureHandle};

use crate::{music, texture};

const POLL_EVERY: Duration = Duration::from_secs(1);
const POLL_CHECK: Duration = Duration::from_millis(50);
const COMMAND_SETTLE: Duration = Duration::from_millis(200);
const INTERPOLATION_STEP: Duration = Duration::from_millis(200);
const SEEK_CONFIRM_TOLERANCE: f64 = 2.0;

type PollResult = Result<Option<music::TrackInfo>>;

enum WorkItem {
    Query,
    ExtractArtwork,
}

enum WorkResult {
    Query(PollResult),
    Artwork(Result<Option<PathBuf>>),
}

pub struct App {
    pub(crate) track: Option<music::TrackInfo>,
    pub(crate) cover_texture: Option<TextureHandle>,
    pub(crate) last_error: Option<String>,
    pub(crate) pending_seek_position: Option<f64>,
    poll_instant: Instant,
    last_track_id: Option<i64>,
    poll_pending: bool,
    next_poll: Instant,
    work_tx: mpsc::Sender<WorkItem>,
    result_rx: mpsc::Receiver<WorkResult>,
}

impl App {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let (work_tx, work_rx) = mpsc::channel::<WorkItem>();
        let (result_tx, result_rx) = mpsc::channel::<WorkResult>();

        thread::spawn(move || {
            while let Ok(item) = work_rx.recv() {
                let result = match item {
                    WorkItem::Query => WorkResult::Query(music::query()),
                    WorkItem::ExtractArtwork => WorkResult::Artwork(music::extract_artwork()),
                };
                if result_tx.send(result).is_err() {
                    break;
                }
            }
        });

        Self {
            track: None,
            cover_texture: None,
            last_error: None,
            pending_seek_position: None,
            poll_instant: Instant::now(),
            last_track_id: None,
            poll_pending: false,
            next_poll: Instant::now(),
            work_tx,
            result_rx,
        }
    }

    pub(crate) fn poll_if_needed(&mut self, ctx: &egui::Context) {
        let mut latest_query: Option<PollResult> = None;
        while let Ok(result) = self.result_rx.try_recv() {
            match result {
                WorkResult::Query(r) => {
                    if self.poll_pending {
                        latest_query = Some(r);
                    }
                }
                WorkResult::Artwork(r) => {
                    self.apply_artwork_result(r, ctx);
                }
            }
        }

        if let Some(result) = latest_query {
            self.poll_pending = false;
            self.apply_poll_result(result, ctx);
            self.next_poll = Instant::now() + POLL_EVERY;
        }

        if !self.poll_pending && Instant::now() >= self.next_poll {
            let _ = self.work_tx.send(WorkItem::Query);
            self.poll_pending = true;
            ctx.request_repaint_after(POLL_CHECK);
        } else if self.poll_pending {
            ctx.request_repaint_after(POLL_CHECK);
        } else {
            ctx.request_repaint_after(
                self.next_poll.saturating_duration_since(Instant::now()),
            );
        }

        if let Some(track) = &self.track
            && track.state == music::PlayerState::Playing
            && self.pending_seek_position.is_none()
        {
            ctx.request_repaint_after(INTERPOLATION_STEP);
        }
    }

    fn apply_poll_result(&mut self, result: PollResult, _ctx: &egui::Context) {
        match result {
            Ok(Some(track)) => {
                self.last_error = None;
                self.poll_instant = Instant::now();

                let track_changed = self.last_track_id != Some(track.id);
                if track_changed {
                    self.pending_seek_position = None;
                    self.last_track_id = Some(track.id);
                    self.cover_texture = None;
                    let _ = self.work_tx.send(WorkItem::ExtractArtwork);
                } else if let Some(pending) = self.pending_seek_position
                    && (track.position - pending).abs() < SEEK_CONFIRM_TOLERANCE
                {
                    self.pending_seek_position = None;
                }

                self.track = Some(track);
            }
            Ok(None) => {
                self.track = None;
                self.cover_texture = None;
                self.last_track_id = None;
                self.last_error = None;
                self.pending_seek_position = None;
            }
            Err(err) => {
                self.track = None;
                self.cover_texture = None;
                self.last_track_id = None;
                self.last_error = Some(format!("{err:#}"));
                self.pending_seek_position = None;
            }
        }
    }

    fn apply_artwork_result(&mut self, result: Result<Option<PathBuf>>, ctx: &egui::Context) {
        match result {
            Ok(Some(path)) => match texture::load_from_file(ctx, &path) {
                Ok(tex) => self.cover_texture = Some(tex),
                Err(err) => {
                    self.last_error = Some(format!("Failed to load artwork: {err:#}"));
                }
            },
            Ok(None) => {
                self.cover_texture = None;
            }
            Err(err) => {
                self.last_error = Some(format!("Artwork extraction failed: {err:#}"));
            }
        }
    }

    pub(crate) fn effective_position(&self) -> f64 {
        if let Some(pending) = self.pending_seek_position {
            return pending;
        }
        let Some(track) = &self.track else {
            return 0.0;
        };
        match track.state {
            music::PlayerState::Playing => (track.position
                + self.poll_instant.elapsed().as_secs_f64())
            .min(track.duration),
            _ => track.position,
        }
    }

    pub(crate) fn execute_command(&mut self, cmd: music::PlayerCommand) {
        thread::spawn(move || {
            let _ = music::send_command(cmd);
        });
        self.poll_pending = false;
        self.next_poll = Instant::now() + COMMAND_SETTLE;
    }

    pub(crate) fn seek(&mut self, position: f64) {
        self.pending_seek_position = Some(position);
        self.execute_command(music::PlayerCommand::Seek(position));
    }
}
