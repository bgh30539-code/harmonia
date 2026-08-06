use serde::{Deserialize, Serialize};

use crate::models::RepeatMode;

/// User configurable application settings.
///
/// The struct derives `Default` and uses `#[serde(default)]` so that a
/// partial JSON patch from the frontend deserializes cleanly and missing
/// fields keep their previous values.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    /// "system" | "light" | "dark"
    pub theme: String,
    /// Accent color as a hex string, e.g. "#7c5cff".
    pub accent: String,
    /// Master volume, 0.0 - 1.0.
    pub volume: f32,
    pub shuffle: bool,
    pub repeat: RepeatMode,
    /// Seconds of overlap when crossfading between tracks. 0 disables it.
    pub crossfade_seconds: f32,
    /// Apply ReplayGain tags (track gain, falling back to album gain).
    pub replay_gain: bool,
    /// Playback speed multiplier (0.5 - 2.0).
    pub playback_speed: f32,
    /// Preferred output device name; None = system default.
    pub audio_device: Option<String>,
    /// ISO 639-1 language code for the UI ("en", "es").
    pub language: String,
    /// Root folders of the music library.
    pub library_folders: Vec<String>,
    /// Resume playback where it was left on the previous session.
    pub resume_last_session: bool,
    pub eq_enabled: bool,
    /// Gains (dB) for the ten fixed EQ bands, see [`crate::dsp::EQ_BANDS_HZ`].
    pub eq_gains: Vec<f32>,
    /// Bass boost gain in dB applied via a low shelf at 110 Hz.
    pub bass_boost_db: f32,
    /// Stereo balance, -1.0 (full left) .. 1.0 (full right).
    pub balance: f32,
    /// Downmix to mono.
    pub mono: bool,
    /// Upper bound for the embedded artwork cache, in MiB.
    pub cache_size_mb: u64,
    /// Whether the UI is currently in mini-player mode (single small window).
    pub mini_player: bool,
    /// Hide to the system tray when the window close button is pressed instead
    /// of quitting. Disabled on Windows by default, where the native
    /// convention is for the close button to terminate the application.
    pub close_to_tray: bool,
    /// Remembered main-window geometry, restored on the next launch
    /// (desktop integration). Values are physical pixels.
    pub window_width: f64,
    pub window_height: f64,
    pub window_x: Option<f64>,
    pub window_y: Option<f64>,
    pub window_maximized: bool,
    /// Show a desktop notification when the playing track changes.
    pub notify_on_track_change: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme: "system".to_string(),
            accent: "#7c5cff".to_string(),
            volume: 0.9,
            shuffle: false,
            repeat: RepeatMode::Off,
            crossfade_seconds: 0.0,
            replay_gain: true,
            playback_speed: 1.0,
            audio_device: None,
            language: "en".to_string(),
            library_folders: Vec::new(),
            resume_last_session: true,
            eq_enabled: false,
            eq_gains: vec![0.0; 10],
            bass_boost_db: 0.0,
            balance: 0.0,
            mono: false,
            cache_size_mb: 512,
            mini_player: false,
            close_to_tray: cfg!(not(windows)),
            window_width: 1280.0,
            window_height: 820.0,
            window_x: None,
            window_y: None,
            window_maximized: false,
            notify_on_track_change: true,
        }
    }
}

impl Settings {
    pub fn accent_ok(accent: &str) -> bool {
        accent.len() == 7 && accent.starts_with('#')
    }

    pub fn language_ok(lang: &str) -> bool {
        matches!(lang, "en" | "es")
    }
}
