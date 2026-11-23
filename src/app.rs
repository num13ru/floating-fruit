use std::{
    collections::{HashMap, VecDeque},
    hash::{DefaultHasher, Hasher},
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

use anyhow::Result;
use eframe::egui::{self, TextureHandle};

use crate::{music, progressbar::FillResources, texture};
use std::sync::{Arc, Mutex};

const POLL_EVERY: Duration = Duration::from_secs(1);
const POLL_CHECK: Duration = Duration::from_millis(50);
const COMMAND_SETTLE: Duration = Duration::from_millis(200);
const INTERPOLATION_STEP: Duration = Duration::from_millis(200);
const SEEK_CONFIRM_TOLERANCE: f64 = 2.0;
const ART_CACHE_CAP: usize = 10;
const GALLEY_CACHE_CAP: usize = 128;

type PollResult = Result<Option<music::TrackInfo>>;

enum WorkItem {
    Query(u64),
    ExtractArtwork(music::ArtworkRequest),
}

enum WorkResult {
    Query {
        request_id: u64,
        result: PollResult,
    },
    Artwork {
        request: music::ArtworkRequest,
        result: Result<music::ArtworkExtraction>,
    },
}

pub struct App {
    pub(crate) track: Option<music::TrackInfo>,
    pub(crate) cover_texture: Option<TextureHandle>,
    pub(crate) last_error: Option<String>,
    pub(crate) pending_seek_position: Option<f64>,
    poll_instant: Instant,
    last_track_id: Option<i64>,
    last_album_key: Option<String>,
    art_cache: VecDeque<(String, TextureHandle)>,
    pub(crate) galley_cache: GalleyCache,
    pub(crate) fill_resource: Arc<Mutex<Option<FillResources>>>,
    pub(crate) fill_color: [f32; 4],
    pub(crate) fill_intensity: f32,
    pub(crate) fill_speed: f32,
    poll_pending: bool,
    next_query_id: u64,
    latest_query_id: Option<u64>,
    next_poll: Instant,
    work_tx: mpsc::Sender<WorkItem>,
    result_rx: mpsc::Receiver<WorkResult>,
    last_pixels_per_point: f32,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let (work_tx, work_rx) = mpsc::channel::<WorkItem>();
        let (result_tx, result_rx) = mpsc::channel::<WorkResult>();

        thread::spawn(move || {
            while let Ok(item) = work_rx.recv() {
                let result = match item {
                    WorkItem::Query(request_id) => WorkResult::Query {
                        request_id,
                        result: music::query(),
                    },
                    WorkItem::ExtractArtwork(request) => {
                        let result = music::extract_artwork(&request);
                        WorkResult::Artwork { request, result }
                    }
                };
                if result_tx.send(result).is_err() {
                    break;
                }
            }
        });

        sweep_artwork_temp_files();

        Self {
            track: None,
            cover_texture: None,
            last_error: None,
            pending_seek_position: None,
            poll_instant: Instant::now(),
            last_track_id: None,
            last_album_key: None,
            art_cache: VecDeque::new(),
            galley_cache: HashMap::new(),
            fill_resource: Arc::new(Mutex::new(None)),
            fill_color: [200.0 / 255.0, 205.0 / 255.0, 215.0 / 255.0, 1.0],
            fill_intensity: 1.0,
            fill_speed: 1.0,
            poll_pending: false,
            next_query_id: 1,
            latest_query_id: None,
            next_poll: Instant::now(),
            work_tx,
            result_rx,
            last_pixels_per_point: cc.egui_ctx.pixels_per_point(),
        }
    }

    pub(crate) fn poll_if_needed(&mut self, ctx: &egui::Context) {
        let ppp = ctx.pixels_per_point();
        if (ppp - self.last_pixels_per_point).abs() > 0.01 {
            self.last_pixels_per_point = ppp;
            ctx.set_fonts(crate::fonts::font_definitions());
        }

        let mut latest_query: Option<(u64, PollResult)> = None;
        while let Ok(result) = self.result_rx.try_recv() {
            match result {
                WorkResult::Query { request_id, result } => {
                    if self.is_current_query_result(request_id) {
                        latest_query = Some((request_id, result));
                    }
                }
                WorkResult::Artwork { request, result } => {
                    self.apply_artwork_result(request, result, ctx);
                }
            }
        }

        if let Some((request_id, result)) = latest_query {
            self.poll_pending = false;
            self.latest_query_id = self.latest_query_id.filter(|id| *id != request_id);
            self.apply_poll_result(result, ctx);
            self.next_poll = Instant::now() + POLL_EVERY;
        }

        if !self.poll_pending && Instant::now() >= self.next_poll {
            self.request_query();
            ctx.request_repaint_after(POLL_CHECK);
        } else if self.poll_pending {
            ctx.request_repaint_after(POLL_CHECK);
        } else {
            ctx.request_repaint_after(self.next_poll.saturating_duration_since(Instant::now()));
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

                    let album_key = track.album_key();
                    let album_changed = self.last_album_key.as_deref() != Some(&album_key);
                    if album_changed {
                        self.last_album_key = Some(album_key.clone());
                        if let Some(cached) = self.art_cache_get(&album_key) {
                            self.cover_texture = Some(cached);
                        } else {
                            self.cover_texture = None;
                            let request = music::ArtworkRequest {
                                track_id: track.id,
                                album_key,
                            };
                            let _ = self.work_tx.send(WorkItem::ExtractArtwork(request));
                        }
                    }
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
                self.last_album_key = None;
                self.last_error = None;
                self.pending_seek_position = None;
            }
            Err(err) => {
                self.track = None;
                self.cover_texture = None;
                self.last_track_id = None;
                self.last_album_key = None;
                self.last_error = Some(format!("{err:#}"));
                self.pending_seek_position = None;
            }
        }
    }

    fn request_query(&mut self) {
        let request_id = self.next_query_id;
        self.next_query_id = self.next_query_id.wrapping_add(1).max(1);
        self.latest_query_id = Some(request_id);
        self.poll_pending = true;
        let _ = self.work_tx.send(WorkItem::Query(request_id));
    }

    fn is_current_query_result(&self, request_id: u64) -> bool {
        self.poll_pending && self.latest_query_id == Some(request_id)
    }

    fn apply_artwork_result(
        &mut self,
        request: music::ArtworkRequest,
        result: Result<music::ArtworkExtraction>,
        ctx: &egui::Context,
    ) {
        if self.last_track_id != Some(request.track_id)
            || self.last_album_key.as_deref() != Some(&request.album_key)
        {
            return;
        }

        match result {
            Ok(music::ArtworkExtraction::Path(path)) => {
                let name = texture_name_for(&request.album_key);
                let result = texture::load_from_file(ctx, &path, &name);
                let _ = std::fs::remove_file(&path);
                match result {
                    Ok(tex) => {
                        self.cover_texture = Some(tex.clone());
                        self.art_cache_insert(request.album_key, tex);
                    }
                    Err(err) => {
                        self.last_error = Some(format!("Failed to load artwork: {err:#}"));
                    }
                }
            }
            Ok(music::ArtworkExtraction::Missing) => {
                self.cover_texture = None;
            }
            Ok(music::ArtworkExtraction::Stale) => {}
            Err(err) => {
                self.last_error = Some(format!("Artwork extraction failed: {err:#}"));
            }
        }
    }

    fn art_cache_get(&mut self, key: &str) -> Option<TextureHandle> {
        let idx = self.art_cache.iter().position(|(k, _)| k == key)?;
        let entry = self.art_cache.remove(idx).unwrap();
        let tex = entry.1.clone();
        self.art_cache.push_back(entry);
        Some(tex)
    }

    fn art_cache_insert(&mut self, key: String, tex: TextureHandle) {
        self.art_cache.retain(|(k, _)| k != &key);
        self.art_cache.push_back((key, tex));
        while self.art_cache.len() > ART_CACHE_CAP {
            self.art_cache.pop_front();
        }
    }

    pub(crate) fn cached_galley(
        &mut self,
        painter: &egui::Painter,
        text: &str,
        font: egui::FontId,
        color: egui::Color32,
    ) -> std::sync::Arc<egui::Galley> {
        cached_galley(&mut self.galley_cache, painter, text, font, color)
    }

    pub(crate) fn effective_position(&self) -> f64 {
        if let Some(pending) = self.pending_seek_position {
            return pending;
        }
        let Some(track) = &self.track else {
            return 0.0;
        };
        match track.state {
            music::PlayerState::Playing => {
                (track.position + self.poll_instant.elapsed().as_secs_f64()).min(track.duration)
            }
            _ => track.position,
        }
    }

    pub(crate) fn reveal_in_app(&self) {
        thread::spawn(|| {
            let _ = music::reveal_current_track();
        });
    }

    pub(crate) fn execute_command(&mut self, cmd: music::PlayerCommand) {
        thread::spawn(move || {
            let _ = music::send_command(cmd);
        });
        self.invalidate_pending_query();
        self.next_poll = Instant::now() + COMMAND_SETTLE;
    }

    fn invalidate_pending_query(&mut self) {
        self.poll_pending = false;
        self.latest_query_id = None;
    }

    pub(crate) fn seek(&mut self, position: f64) {
        self.pending_seek_position = Some(position);
        self.execute_command(music::PlayerCommand::Seek(position));
    }
}

pub(crate) type GalleyCache = HashMap<
    (String, egui::FontFamily, u32, egui::Color32),
    std::sync::Arc<egui::Galley>,
>;

pub(crate) fn cached_galley(
    cache: &mut GalleyCache,
    painter: &egui::Painter,
    text: &str,
    font: egui::FontId,
    color: egui::Color32,
) -> std::sync::Arc<egui::Galley> {
    let key = (text.to_string(), font.family.clone(), font.size.to_bits(), color);
    if let Some(galley) = cache.get(&key) {
        return galley.clone();
    }
    let galley = painter.layout_no_wrap(text.to_string(), font, color);
    if cache.len() >= GALLEY_CACHE_CAP {
        cache.clear();
    }
    cache.insert(key, galley.clone());
    galley
}

fn texture_name_for(album_key: &str) -> String {
    let mut hasher = DefaultHasher::new();
    hasher.write(album_key.as_bytes());
    format!("album_art_{:x}", hasher.finish())
}

fn sweep_artwork_temp_files() {
    let Ok(rd) = std::fs::read_dir(std::env::temp_dir()) else { return };
    for entry in rd.flatten() {
        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue };
        if name.starts_with("am_now_playing_artwork_") {
            let _ = std::fs::remove_file(entry.path());
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::{HashMap, VecDeque},
        sync::{mpsc, Arc, Mutex},
        time::{Duration, Instant},
    };

    use anyhow::anyhow;
    use eframe::egui;

    use super::App;
    use crate::music::{self, ArtworkExtraction, PlayerState, TrackInfo};

    fn test_app() -> App {
        let (work_tx, _work_rx) = mpsc::channel();
        let (_result_tx, result_rx) = mpsc::channel();

        App {
            track: None,
            cover_texture: None,
            last_error: None,
            pending_seek_position: None,
            poll_instant: Instant::now(),
            last_track_id: None,
            last_album_key: None,
            art_cache: VecDeque::new(),
            galley_cache: HashMap::new(),
            fill_resource: Arc::new(Mutex::new(None)),
            fill_color: [200.0 / 255.0, 205.0 / 255.0, 215.0 / 255.0, 1.0],
            fill_intensity: 1.0,
            fill_speed: 1.0,
            poll_pending: false,
            next_query_id: 1,
            latest_query_id: None,
            next_poll: Instant::now() + Duration::from_secs(60),
            work_tx,
            result_rx,
            last_pixels_per_point: 1.0,
        }
    }

    fn track(id: i64, album: &str) -> TrackInfo {
        TrackInfo {
            state: PlayerState::Playing,
            title: format!("Track {id}"),
            artist: "Artist".into(),
            album: album.into(),
            album_artist: "Album Artist".into(),
            id,
            position: 0.0,
            duration: 100.0,
        }
    }

    #[test]
    fn stale_query_result_is_not_current_after_newer_request() {
        let mut app = test_app();
        app.poll_pending = true;
        app.latest_query_id = Some(2);

        assert!(!app.is_current_query_result(1));
        assert!(app.is_current_query_result(2));
    }

    #[test]
    fn command_invalidates_in_flight_query_result() {
        let mut app = test_app();
        app.poll_pending = true;
        app.latest_query_id = Some(1);

        app.invalidate_pending_query();

        assert!(!app.poll_pending);
        assert_eq!(app.latest_query_id, None);
        assert!(!app.is_current_query_result(1));
    }

    #[test]
    fn stale_artwork_result_does_not_mutate_error_state() {
        let mut app = test_app();
        let current = track(2, "Current Album");
        app.last_track_id = Some(current.id);
        app.last_album_key = Some(current.album_key());

        let stale_request = music::ArtworkRequest {
            track_id: 1,
            album_key: track(1, "Old Album").album_key(),
        };

        app.apply_artwork_result(
            stale_request,
            Err(anyhow!("stale load should be ignored")),
            &egui::Context::default(),
        );

        assert_eq!(app.last_error, None);
        assert_eq!(app.last_track_id, Some(2));
        assert_eq!(app.last_album_key, Some(current.album_key()));
    }

    #[test]
    fn matching_stale_artwork_marker_is_ignored() {
        let mut app = test_app();
        let current = track(1, "Current Album");
        let request = music::ArtworkRequest {
            track_id: current.id,
            album_key: current.album_key(),
        };
        app.last_track_id = Some(current.id);
        app.last_album_key = Some(current.album_key());

        app.apply_artwork_result(
            request,
            Ok(ArtworkExtraction::Stale),
            &egui::Context::default(),
        );

        assert_eq!(app.last_error, None);
        assert!(app.cover_texture.is_none());
    }
}
