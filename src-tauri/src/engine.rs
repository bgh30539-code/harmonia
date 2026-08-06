//! Audio engine.
//!
//! All audio playback lives on a single dedicated thread. The thread owns the
//! rodio output device, the `Player` handles and the queue, so no Send/Sync
//! concerns leak into the command layer. Commands arrive over an mpsc channel;
//! playback state is mirrored into a shared snapshot for cheap reads; Tauri
//! events are emitted straight from this thread via `AppHandle`.
//!
//! ## Playback flow
//!
//! - A track is decoded with `rodio::Decoder` (symphonia backend, gapless),
//!   wrapped in `ChannelVolume` (mono/balance) and optionally the custom EQ
//!   source, then appended to the player.
//! - A 100 ms tick loop drives auto-advance, ReplayGain volume, crossfade
//!   ramps, the sleep timer, position reporting and resume persistence.

use std::path::Path;
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use harmonia_core::models::{RepeatMode, Track};
use harmonia_core::{Database, Settings};
use rand::Rng;
use rodio::cpal::traits::{DeviceTrait, HostTrait};
use rodio::source::ChannelVolume;
use rodio::{Decoder, DeviceSinkBuilder, MixerDeviceSink, Player, Source};
use serde::Serialize;
use tauri::{AppHandle, Emitter};

use crate::dsp_source::EqualizerSource;

// ---------------------------------------------------------------------------
// Command channel
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub enum SleepTimer {
    Off,
    Minutes(u64),
    EndOfTrack,
    EndOfAlbum,
}

#[derive(Debug)]
pub enum AudioCommand {
    PlayContext { ids: Vec<i64>, start_index: usize },
    PlayTrack(i64),
    PlayNext,
    PlayPrevious,
    Toggle,
    Pause,
    Resume,
    Stop,
    Seek { position_ms: u64 },
    SetVolume(f32),
    SetShuffle(bool),
    SetRepeat(RepeatMode),
    SetSpeed(f32),
    SetSleepTimer(SleepTimer),
    AddToQueue(Vec<i64>),
    ClearQueue,
    RemoveQueueItem(usize),
    MoveQueueItem { from: usize, to: usize },
    RestoreSession { track_id: i64, position_ms: u64 },
    ReloadSettings,
}

// ---------------------------------------------------------------------------
// Snapshot shared with the command layer / UI
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SleepTimerUi {
    pub kind: String,
    pub remaining_secs: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaybackSnapshot {
    pub playing: bool,
    pub current: Option<Track>,
    pub position_ms: u64,
    pub duration_ms: u64,
    pub volume: f32,
    pub shuffle: bool,
    pub repeat: RepeatMode,
    pub speed: f32,
    /// Tracks in playback order (capped for very large queues).
    pub queue: Vec<Track>,
    pub queue_index: i64,
    pub queue_total: i64,
    pub sleep_timer: SleepTimerUi,
}

impl Default for PlaybackSnapshot {
    fn default() -> Self {
        Self {
            playing: false,
            current: None,
            position_ms: 0,
            duration_ms: 0,
            volume: 1.0,
            shuffle: false,
            repeat: RepeatMode::Off,
            speed: 1.0,
            queue: Vec::new(),
            queue_index: -1,
            queue_total: 0,
            sleep_timer: SleepTimerUi {
                kind: "off".into(),
                remaining_secs: None,
            },
        }
    }
}

/// Handle the rest of the application uses to talk to the engine.
pub struct EngineHandle {
    pub tx: Sender<AudioCommand>,
    pub snapshot: Arc<Mutex<PlaybackSnapshot>>,
}

/// State captured from settings when the engine boots.
struct InitialState {
    volume: f32,
    shuffle: bool,
    repeat: RepeatMode,
    speed: f32,
    crossfade_secs: f32,
    replay_gain: bool,
    device: Option<String>,
}

// ---------------------------------------------------------------------------
// Engine internals
// ---------------------------------------------------------------------------

struct CrossState {
    player: Player,
    track: Track,
    started: Instant,
}

/// Maximum number of queue entries mirrored into the UI snapshot.
const MAX_VISIBLE_QUEUE: usize = 2000;
/// How many ticks (100 ms each) between resume-state writes.
const RESUME_SAVE_EVERY: u64 = 100;
/// How many ticks between position events.
const POSITION_EVENT_EVERY: u64 = 3;

struct Engine {
    app: AppHandle,
    sink: MixerDeviceSink,
    player: Player,
    current: Option<Track>,
    queue: Vec<Track>,
    /// Playback order: `order[i]` is an index into `queue`.
    order: Vec<usize>,
    cursor: i64,
    shuffle: bool,
    repeat: RepeatMode,
    volume: f32,
    base_vol: f32,
    speed: f32,
    crossfade_secs: f32,
    replay_gain: bool,
    playing: bool,
    position_ms: u64,
    cross: Option<CrossState>,
    sleep: SleepTimer,
    sleep_deadline: Option<Instant>,
    sleep_album: Option<String>,
    device: Option<String>,
    tick: u64,
    last_state_key: Option<String>,
}

fn gain_factor(track: &Track, settings: &Settings) -> f32 {
    if settings.replay_gain {
        if let Some(gain_db) = track.replay_gain_db {
            return 10f32.powf(gain_db / 20.0).clamp(0.25, 4.0);
        }
    }
    1.0
}

fn open_sink(device: Option<&str>) -> Result<MixerDeviceSink, String> {
    match device {
        Some(name) => {
            let host = rodio::cpal::default_host();
            let devices = host
                .output_devices()
                .map_err(|e| format!("cannot enumerate audio devices: {e}"))?;
            for device in devices {
                if device
                    .description()
                    .map(|d| d.name() == name)
                    .unwrap_or(false)
                {
                    return DeviceSinkBuilder::from_device(device)
                        .map_err(|e| e.to_string())?
                        .open_stream()
                        .map_err(|e| e.to_string());
                }
            }
            Err(format!("audio device not found: {name}"))
        }
        None => DeviceSinkBuilder::open_default_sink().map_err(|e| e.to_string()),
    }
}

fn load_source(path: &str, settings: &Settings) -> Result<Box<dyn Source + Send>, String> {
    let file = std::fs::File::open(path).map_err(|e| format!("cannot open {path}: {e}"))?;
    let byte_len = file.metadata().map(|m| m.len()).unwrap_or(0);
    let hint = Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_string();
    let decoder = Decoder::builder()
        .with_data(file)
        .with_byte_len(byte_len)
        .with_hint(&hint)
        .with_gapless(true)
        .build()
        .map_err(|e| format!("cannot decode {path}: {e}"))?;

    let (left, right) = if settings.mono {
        (1.0, 1.0)
    } else {
        (
            (1.0 - settings.balance).clamp(0.0, 1.0),
            (1.0 + settings.balance).clamp(0.0, 1.0),
        )
    };
    let stereo = ChannelVolume::new(decoder, vec![left, right]);

    if settings.eq_enabled || settings.bass_boost_db.abs() >= 0.05 {
        let eq = EqualizerSource::new(stereo, settings.eq_gains.clone(), settings.bass_boost_db);
        Ok(Box::new(eq))
    } else {
        Ok(Box::new(stereo))
    }
}

impl Engine {
    fn new(app: AppHandle, sink: MixerDeviceSink, initial: InitialState) -> Self {
        let player = Player::connect_new(sink.mixer());
        Self {
            app,
            sink,
            player,
            current: None,
            queue: Vec::new(),
            order: Vec::new(),
            cursor: -1,
            shuffle: initial.shuffle,
            repeat: initial.repeat,
            volume: initial.volume,
            base_vol: initial.volume,
            speed: initial.speed,
            crossfade_secs: initial.crossfade_secs,
            replay_gain: initial.replay_gain,
            playing: false,
            position_ms: 0,
            cross: None,
            sleep: SleepTimer::Off,
            sleep_deadline: None,
            sleep_album: None,
            device: initial.device,
            tick: 0,
            last_state_key: None,
        }
    }

    fn toast(&self, message: impl Into<String>) {
        let _ = self.app.emit(
            "toast://show",
            serde_json::json!({ "message": message.into(), "kind": "info" }),
        );
    }

    fn mark_dirty(&mut self) {
        self.last_state_key = None;
    }

    // -- queue helpers ------------------------------------------------------

    fn current_id(&self) -> Option<i64> {
        self.current.as_ref().map(|t| t.id)
    }

    fn track_at(&self, order_pos: usize) -> Option<&Track> {
        self.order
            .get(order_pos)
            .and_then(|&idx| self.queue.get(idx))
    }

    fn next_track(&self) -> Option<Track> {
        if self.queue.is_empty() {
            return None;
        }
        let cursor = self.cursor.max(0) as usize;
        if cursor + 1 < self.order.len() {
            return self.track_at(cursor + 1).cloned();
        }
        if self.repeat == RepeatMode::All {
            return self.track_at(0).cloned();
        }
        None
    }

    fn set_playback_order(&mut self) {
        self.order = if self.shuffle {
            let mut idx: Vec<usize> = (0..self.queue.len()).collect();
            let mut rng = rand::thread_rng();
            for i in (1..idx.len()).rev() {
                let j = rng.gen_range(0..=i);
                idx.swap(i, j);
            }
            idx
        } else {
            (0..self.queue.len()).collect()
        };
    }

    fn position_of_queue_index(&self, queue_index: usize) -> Option<usize> {
        self.order.iter().position(|&i| i == queue_index)
    }

    // -- track lifecycle ----------------------------------------------------

    /// Loads and starts `track`. `count_play` controls whether the play
    /// counter / last-played timestamp is bumped (disabled for session
    /// restore so resuming never inflates statistics).
    fn start_track(&mut self, track: &Track, settings: &Settings, db: &Database, count_play: bool) {
        self.cancel_cross();
        match load_source(&track.path, settings) {
            Ok(source) => {
                self.player.stop();
                self.player.append(source);
                self.player.set_speed(self.speed);
                self.player.set_volume(0.0);
                self.player.play();
                self.base_vol = self.volume * gain_factor(track, settings);
                self.player.set_volume(self.base_vol);
                self.current = Some(track.clone());
                self.playing = true;
                self.position_ms = 0;
                if count_play {
                    let _ = db.mark_played(track.id);
                }
                self.mark_dirty();
            }
            Err(e) => {
                log::warn!("{e}");
                self.toast(e);
            }
        }
    }

    fn cancel_cross(&mut self) {
        if let Some(cross) = self.cross.take() {
            cross.player.stop();
        }
        self.player.set_volume(self.base_vol);
    }

    fn advance_cursor(&mut self) {
        let cursor = self.cursor.max(0) as usize;
        if cursor + 1 < self.order.len() {
            self.cursor = cursor as i64 + 1;
        } else if self.repeat == RepeatMode::All {
            self.cursor = 0;
        }
    }

    fn seek_to(&mut self, position_ms: u64) {
        if self.current.is_none() {
            return;
        }
        let duration = self
            .current
            .as_ref()
            .map(|t| t.duration_ms.max(0) as u64)
            .unwrap_or(0);
        let pos = position_ms.min(duration);
        match self.player.try_seek(Duration::from_millis(pos)) {
            Ok(()) => self.position_ms = pos,
            Err(e) => log::warn!("seek failed: {e}"),
        }
        self.mark_dirty();
    }

    fn save_resume(&self, db: &Database) {
        if let Some(track) = &self.current {
            let _ = db.set_setting("resume.track_id", &track.id.to_string());
            let _ = db.set_setting("resume.position_ms", &self.position_ms.to_string());
        }
    }

    // -- sleep timer --------------------------------------------------------

    fn set_sleep_timer(&mut self, timer: SleepTimer) {
        self.sleep = match timer {
            SleepTimer::Minutes(0) => SleepTimer::Off,
            other => other,
        };
        self.sleep_deadline = match self.sleep {
            SleepTimer::Minutes(m) => Some(Instant::now() + Duration::from_secs(m * 60)),
            _ => None,
        };
        self.sleep_album = if matches!(self.sleep, SleepTimer::EndOfAlbum) {
            self.current.as_ref().map(|t| t.album.clone())
        } else {
            None
        };
        self.mark_dirty();
    }

    fn pause_for_sleep(&mut self, db: &Database) {
        self.player.pause();
        self.playing = false;
        self.sleep = SleepTimer::Off;
        self.sleep_deadline = None;
        self.sleep_album = None;
        self.save_resume(db);
        self.mark_dirty();
        self.toast("Sleep timer finished — playback paused");
    }

    // -- command handling ---------------------------------------------------

    fn handle(&mut self, cmd: AudioCommand, db: &Database, settings: &Settings) {
        match cmd {
            AudioCommand::PlayContext { ids, start_index } => {
                let tracks = match db.get_tracks_by_ids(&ids) {
                    Ok(t) => t,
                    Err(e) => {
                        self.toast(format!("Database error: {e}"));
                        return;
                    }
                };
                if tracks.is_empty() {
                    return;
                }
                self.queue = tracks;
                self.set_playback_order();
                let start = start_index.min(self.queue.len() - 1);
                self.cursor = self.position_of_queue_index(start).unwrap_or(0) as i64;
                if let Some(track) = self.track_at(self.cursor.max(0) as usize) {
                    let track = track.clone();
                    self.start_track(&track, settings, db, true);
                }
                self.mark_dirty();
            }
            AudioCommand::PlayTrack(id) => {
                let found = self
                    .queue
                    .iter()
                    .position(|t| t.id == id)
                    .and_then(|pos| self.position_of_queue_index(pos));
                match found {
                    Some(order_pos) => {
                        self.cursor = order_pos as i64;
                        if let Some(track) = self.track_at(self.cursor.max(0) as usize) {
                            let track = track.clone();
                            self.start_track(&track, settings, db, true);
                        }
                    }
                    None => {
                        self.queue = Vec::new();
                        self.order = Vec::new();
                        self.cursor = 0;
                        match db.get_track(id) {
                            Ok(Some(track)) => {
                                self.queue.push(track.clone());
                                self.order.push(0);
                                self.start_track(&track, settings, db, true);
                            }
                            Ok(None) => self.toast("Track not found in library"),
                            Err(e) => self.toast(format!("Database error: {e}")),
                        }
                    }
                }
                self.mark_dirty();
            }
            AudioCommand::PlayNext => {
                self.cancel_cross();
                self.advance_cursor();
                self.play_current(settings, db);
            }
            AudioCommand::PlayPrevious => {
                self.cancel_cross();
                if self.position_ms > 3_000 {
                    self.seek_to(0);
                    return;
                }
                let cursor = self.cursor.max(0) as usize;
                if cursor > 0 {
                    self.cursor = cursor as i64 - 1;
                } else if self.repeat == RepeatMode::All {
                    self.cursor = self.order.len() as i64 - 1;
                } else {
                    self.cursor = 0;
                }
                self.play_current(settings, db);
            }
            AudioCommand::Toggle => {
                if self.playing {
                    self.player.pause();
                    self.playing = false;
                    self.save_resume(db);
                } else if self.current.is_some() {
                    self.player.play();
                    self.playing = true;
                }
                self.mark_dirty();
            }
            AudioCommand::Pause => {
                if self.playing {
                    self.player.pause();
                    self.playing = false;
                    self.save_resume(db);
                    self.mark_dirty();
                }
            }
            AudioCommand::Resume => {
                if !self.playing && self.current.is_some() {
                    self.player.play();
                    self.playing = true;
                    self.mark_dirty();
                }
            }
            AudioCommand::Stop => {
                self.cancel_cross();
                self.player.stop();
                self.playing = false;
                self.position_ms = 0;
                self.current = None;
                self.save_resume(db);
                self.mark_dirty();
            }
            AudioCommand::Seek { position_ms } => self.seek_to(position_ms),
            AudioCommand::SetVolume(v) => {
                self.volume = v.clamp(0.0, 1.0);
                self.base_vol = self.volume
                    * self
                        .current
                        .as_ref()
                        .map_or(1.0, |t| gain_factor(t, settings));
                self.player.set_volume(self.base_vol);
                self.mark_dirty();
            }
            AudioCommand::SetShuffle(on) => {
                if self.shuffle != on {
                    self.shuffle = on;
                    self.set_playback_order();
                    if let Some(current) = self.current_id() {
                        if let Some(pos) = self.queue.iter().position(|t| t.id == current) {
                            self.cursor = self.position_of_queue_index(pos).unwrap_or(0) as i64;
                        }
                    }
                    self.mark_dirty();
                }
            }
            AudioCommand::SetRepeat(mode) => {
                self.repeat = mode;
                self.mark_dirty();
            }
            AudioCommand::SetSpeed(speed) => {
                self.speed = speed.clamp(0.5, 2.0);
                self.player.set_speed(self.speed);
                self.mark_dirty();
            }
            AudioCommand::SetSleepTimer(timer) => self.set_sleep_timer(timer),
            AudioCommand::AddToQueue(ids) => {
                if ids.is_empty() {
                    return;
                }
                match db.get_tracks_by_ids(&ids) {
                    Ok(tracks) => {
                        let start_len = self.queue.len();
                        self.queue.extend(tracks);
                        for i in start_len..self.queue.len() {
                            self.order.push(i);
                        }
                        if self.current.is_none() && !self.queue.is_empty() {
                            self.cursor = 0;
                            let track = self.queue[0].clone();
                            self.start_track(&track, settings, db, true);
                        }
                        self.mark_dirty();
                    }
                    Err(e) => self.toast(format!("Database error: {e}")),
                }
            }
            AudioCommand::ClearQueue => {
                self.cancel_cross();
                self.player.stop();
                self.queue.clear();
                self.order.clear();
                self.cursor = -1;
                self.current = None;
                self.playing = false;
                self.mark_dirty();
            }
            AudioCommand::RemoveQueueItem(order_pos) => {
                if order_pos >= self.order.len() {
                    return;
                }
                let cursor = self.cursor.max(0) as usize;
                if order_pos == cursor {
                    self.cancel_cross();
                    self.player.stop();
                    self.current = None;
                    self.playing = false;
                }
                self.order.remove(order_pos);
                if !self.order.is_empty() && self.cursor as usize >= self.order.len() {
                    self.cursor = self.order.len() as i64 - 1;
                } else if self.order.is_empty() {
                    self.cursor = -1;
                }
                self.mark_dirty();
            }
            AudioCommand::MoveQueueItem { from, to } => {
                if from < self.order.len() && to < self.order.len() && from != to {
                    let cursor = self.cursor.max(0) as usize;
                    let item = self.order.remove(from);
                    self.order.insert(to, item);
                    self.cursor = match cursor {
                        c if c == from => to as i64,
                        c if from < to && c > from && c <= to => (c - 1) as i64,
                        c if to < from && c >= to && c < from => (c + 1) as i64,
                        c => c as i64,
                    };
                    self.mark_dirty();
                }
            }
            AudioCommand::RestoreSession {
                track_id,
                position_ms,
            } => {
                if let Ok(Some(track)) = db.get_track(track_id) {
                    self.queue = vec![track.clone()];
                    self.order = vec![0];
                    self.cursor = 0;
                    self.start_track(&track, settings, db, false);
                    self.seek_to(position_ms.min(track.duration_ms.max(0) as u64));
                    self.player.pause();
                    self.playing = false;
                    self.save_resume(db);
                    self.mark_dirty();
                }
            }
            AudioCommand::ReloadSettings => self.apply_settings(settings, db),
        }
    }

    fn play_current(&mut self, settings: &Settings, db: &Database) {
        match self.track_at(self.cursor.max(0) as usize) {
            Some(track) => {
                let track = track.clone();
                self.start_track(&track, settings, db, true);
            }
            None => {
                self.player.stop();
                self.playing = false;
                self.mark_dirty();
            }
        }
    }

    fn apply_settings(&mut self, settings: &Settings, db: &Database) {
        self.volume = settings.volume.clamp(0.0, 1.0);
        self.base_vol = self.volume
            * self
                .current
                .as_ref()
                .map_or(1.0, |t| gain_factor(t, settings));
        self.player.set_volume(self.base_vol);

        if (self.speed - settings.playback_speed).abs() > f32::EPSILON {
            self.speed = settings.playback_speed.clamp(0.5, 2.0);
            self.player.set_speed(self.speed);
        }
        self.shuffle = settings.shuffle;
        self.repeat = settings.repeat;
        self.crossfade_secs = settings.crossfade_seconds.clamp(0.0, 15.0);
        self.replay_gain = settings.replay_gain;

        if settings.audio_device != self.device {
            self.switch_device(settings);
        }
        self.mark_dirty();
        let _ = db;
    }

    /// Reopens the output device, preserving position and playback state.
    fn switch_device(&mut self, settings: &Settings) {
        match open_sink(settings.audio_device.as_deref()) {
            Ok(new_sink) => {
                let was_playing = self.playing;
                let was_pos = self.player.get_pos().as_millis() as u64;
                let current = self.current.clone();
                let player = Player::connect_new(new_sink.mixer());
                self.sink = new_sink;
                self.player = player;
                self.device = settings.audio_device.clone();
                if let Some(track) = current {
                    match load_source(&track.path, settings) {
                        Ok(source) => {
                            self.player.append(source);
                            self.player.set_speed(self.speed);
                            self.player.set_volume(self.base_vol);
                            self.playing = was_playing;
                            if was_playing {
                                self.player.play();
                            }
                            let _ = self.player.try_seek(Duration::from_millis(was_pos));
                        }
                        Err(e) => {
                            self.playing = false;
                            log::warn!("device switch: {e}");
                        }
                    }
                }
                self.mark_dirty();
            }
            Err(e) => {
                log::error!("device switch failed: {e}");
                self.toast(format!("Audio device error: {e}"));
            }
        }
    }

    // -- tick loop ----------------------------------------------------------

    fn tick(&mut self, db: &Database, settings: &Settings) {
        self.tick += 1;

        if self.current.is_none() {
            return;
        }

        if self.playing {
            self.position_ms = self.player.get_pos().as_millis() as u64;
            if self.tick.is_multiple_of(RESUME_SAVE_EVERY) {
                self.save_resume(db);
            }
        }

        // Crossfade ramp and promotion.
        if self.cross.is_some() {
            let fade_ms = (self.crossfade_secs * 1000.0).max(1.0);
            let elapsed = self
                .cross
                .as_ref()
                .map(|c| c.started.elapsed().as_millis() as f32)
                .unwrap_or(0.0);
            let t = (elapsed / fade_ms).clamp(0.0, 1.0);
            self.player.set_volume(self.base_vol * (1.0 - t));
            if let Some(cross) = &mut self.cross {
                cross.player.set_volume(self.base_vol * t);
            }
            if self.player.empty() {
                self.promote_cross();
            }
        }

        // Natural end of track (no crossfade active).
        if self.cross.is_none() && self.playing && self.player.empty() {
            self.on_track_ended(db, settings);
        }

        // Sleep timer countdown.
        if let Some(deadline) = self.sleep_deadline {
            if Instant::now() >= deadline {
                self.pause_for_sleep(db);
            }
        }
    }

    fn on_track_ended(&mut self, db: &Database, settings: &Settings) {
        if self.repeat == RepeatMode::One {
            if let Some(track) = self.current.clone() {
                self.start_track(&track, settings, db, true);
            }
            return;
        }

        // Sleep timers that cut playback at the track boundary.
        match &self.sleep {
            SleepTimer::EndOfTrack => {
                self.player.stop();
                self.playing = false;
                self.sleep = SleepTimer::Off;
                self.save_resume(db);
                self.mark_dirty();
                return;
            }
            SleepTimer::EndOfAlbum => {
                let current_album = self.current.as_ref().map(|t| t.album.clone());
                if let Some(next) = self.next_track() {
                    if current_album.as_ref().is_some_and(|a| next.album != *a) {
                        self.player.stop();
                        self.playing = false;
                        self.sleep = SleepTimer::Off;
                        self.sleep_album = None;
                        self.save_resume(db);
                        self.mark_dirty();
                        return;
                    }
                }
            }
            _ => {}
        }

        match self.next_track() {
            Some(next) if self.crossfade_secs > 0.0 => {
                self.begin_crossfade(next, settings, db);
            }
            Some(next) => {
                self.advance_cursor();
                self.start_track(&next, settings, db, true);
            }
            None => {
                // End of queue with repeat off.
                self.player.stop();
                self.playing = false;
                self.save_resume(db);
                self.mark_dirty();
            }
        }
    }

    fn begin_crossfade(&mut self, track: Track, settings: &Settings, db: &Database) {
        match load_source(&track.path, settings) {
            Ok(source) => {
                let cross_player = Player::connect_new(self.sink.mixer());
                cross_player.append(source);
                cross_player.set_speed(self.speed);
                cross_player.set_volume(0.0);
                cross_player.play();
                self.cross = Some(CrossState {
                    player: cross_player,
                    track,
                    started: Instant::now(),
                });
            }
            Err(e) => {
                log::warn!("crossfade start failed: {e}");
                self.advance_cursor();
                self.start_track(&track, settings, db, true);
            }
        }
    }

    fn promote_cross(&mut self) {
        let cross = match self.cross.take() {
            Some(c) => c,
            None => return,
        };
        self.player = cross.player;
        self.current = Some(cross.track);
        self.advance_cursor();
        self.playing = true;
        self.position_ms = 0;
        self.player.set_volume(self.base_vol);
        self.mark_dirty();
    }

    // -- publishing ---------------------------------------------------------

    fn build_snapshot(&self) -> PlaybackSnapshot {
        let mut queue: Vec<Track> = Vec::new();
        let n = self.order.len();
        let start = if n > MAX_VISIBLE_QUEUE {
            (self.cursor.max(0) as usize).saturating_sub(MAX_VISIBLE_QUEUE / 2)
        } else {
            0
        };
        for i in start..n.min(start + MAX_VISIBLE_QUEUE) {
            if let Some(track) = self.queue.get(self.order[i]) {
                queue.push(track.clone());
            }
        }
        let remaining = self
            .sleep_deadline
            .map(|d| d.saturating_duration_since(Instant::now()).as_secs());
        PlaybackSnapshot {
            playing: self.playing && self.current.is_some(),
            current: self.current.clone(),
            position_ms: self.position_ms,
            duration_ms: self
                .current
                .as_ref()
                .map(|t| t.duration_ms.max(0) as u64)
                .unwrap_or(0),
            volume: self.volume,
            shuffle: self.shuffle,
            repeat: self.repeat,
            speed: self.speed,
            queue,
            queue_index: self.cursor,
            queue_total: self.queue.len() as i64,
            sleep_timer: SleepTimerUi {
                kind: match self.sleep {
                    SleepTimer::Off => "off",
                    SleepTimer::Minutes(_) => "minutes",
                    SleepTimer::EndOfTrack => "endOfTrack",
                    SleepTimer::EndOfAlbum => "endOfAlbum",
                }
                .to_string(),
                remaining_secs: remaining,
            },
        }
    }

    fn publish(&mut self, snapshot: &Arc<Mutex<PlaybackSnapshot>>) {
        let state = self.build_snapshot();
        {
            let mut guard = snapshot.lock().unwrap();
            *guard = state.clone();
        }
        let key = format!(
            "{}|{}|{}|{}|{}|{}|{}",
            state.playing,
            state.current.as_ref().map(|t| t.id).unwrap_or(-1),
            state.queue_index,
            state.queue_total,
            state.shuffle,
            serde_json::to_string(&state.repeat).unwrap_or_default(),
            state.volume
        );
        if self.last_state_key.as_deref() != Some(key.as_str()) {
            self.last_state_key = Some(key);
            let _ = self.app.emit("player://state", &state);
        }
        if self.playing && self.tick.is_multiple_of(POSITION_EVENT_EVERY) {
            let _ = self.app.emit(
                "player://position",
                serde_json::json!({ "positionMs": state.position_ms }),
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Spawn
// ---------------------------------------------------------------------------

/// Starts the audio thread and returns a handle for the rest of the app.
pub fn spawn_engine(
    app: AppHandle,
    db: Arc<Database>,
    settings: Arc<Mutex<Settings>>,
) -> EngineHandle {
    let (tx, rx) = std::sync::mpsc::channel::<AudioCommand>();
    let snapshot = Arc::new(Mutex::new(PlaybackSnapshot::default()));
    let handle = EngineHandle {
        tx: tx.clone(),
        snapshot: snapshot.clone(),
    };

    std::thread::Builder::new()
        .name("harmonia-audio".into())
        .spawn(move || {
            let initial = {
                let guard = settings.lock().unwrap();
                InitialState {
                    volume: guard.volume,
                    shuffle: guard.shuffle,
                    repeat: guard.repeat,
                    speed: guard.playback_speed,
                    crossfade_secs: guard.crossfade_seconds,
                    replay_gain: guard.replay_gain,
                    device: guard.audio_device.clone(),
                }
            };
            let sink = match open_sink(initial.device.as_deref()) {
                Ok(s) => s,
                Err(e) => {
                    log::error!("cannot open audio output: {e}");
                    return;
                }
            };
            let mut engine = Engine::new(app.clone(), sink, initial);
            engine.publish(&snapshot);
            loop {
                while let Ok(cmd) = rx.try_recv() {
                    let guard = settings.lock().unwrap();
                    engine.handle(cmd, &db, &guard);
                }
                {
                    let guard = settings.lock().unwrap();
                    engine.tick(&db, &guard);
                }
                engine.publish(&snapshot);
                std::thread::sleep(Duration::from_millis(100));
            }
        })
        .expect("failed to spawn audio engine thread");

    handle
}
