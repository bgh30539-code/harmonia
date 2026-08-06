use serde::{Deserialize, Serialize};

/// A single track as stored in the library.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Track {
    pub id: i64,
    pub path: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub album_artist: String,
    pub genre: String,
    pub composer: String,
    pub year: Option<i64>,
    pub track_no: Option<i64>,
    pub disc_no: Option<i64>,
    pub duration_ms: i64,
    pub bitrate: Option<i64>,
    pub sample_rate: Option<i64>,
    pub channels: Option<i64>,
    /// Container/codec label derived from the file extension.
    pub format: String,
    /// Parent directory of the file.
    pub folder: String,
    /// Hash key of the cached embedded artwork.
    pub art_hash: Option<String>,
    pub favorite: bool,
    pub play_count: i64,
    pub last_played: Option<i64>,
    pub date_added: i64,
    /// Effective ReplayGain (track gain, falling back to album gain), in dB.
    pub replay_gain_db: Option<f32>,
    pub lyrics: Option<String>,
    /// Synchronised lyrics as an LRC string, when present.
    pub lyrics_synced: Option<String>,
}

/// Aggregated album view.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Album {
    pub id: i64,
    pub title: String,
    pub artist: String,
    pub year: Option<i64>,
    pub art_hash: Option<String>,
    pub track_count: i64,
    pub duration_ms: i64,
}

/// Aggregated artist view.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Artist {
    pub id: i64,
    pub name: String,
    pub art_hash: Option<String>,
    pub track_count: i64,
    pub album_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Playlist {
    pub id: i64,
    pub name: String,
    /// "static" | "smart"
    pub kind: String,
    /// JSON-encoded smart playlist rules for smart playlists.
    pub rules: Option<String>,
    pub pinned: bool,
    pub track_count: i64,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum RepeatMode {
    #[default]
    Off,
    All,
    One,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SmartRule {
    /// Whitelisted field name: genre, artist, album, year, playCount,
    /// favorite, durationMs, bitrate, composer.
    pub field: String,
    /// Whitelisted operator: eq, ne, contains, gt, gte, lt, lte.
    pub op: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct SmartRules {
    /// true = all rules must match (AND), false = any rule (OR).
    pub match_all: bool,
    pub rules: Vec<SmartRule>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SearchFilters {
    pub genre: Option<String>,
    pub composer: Option<String>,
    pub format: Option<String>,
    pub folder: Option<String>,
    pub year_min: Option<i64>,
    pub year_max: Option<i64>,
    pub bitrate_min: Option<i64>,
    pub duration_min_ms: Option<i64>,
    pub duration_max_ms: Option<i64>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub enum SortField {
    #[default]
    Title,
    Artist,
    Album,
    Year,
    Duration,
    DateAdded,
    PlayCount,
}

impl SortField {
    /// Column name whitelist — never interpolate user input into SQL directly.
    pub fn column(&self) -> &'static str {
        match self {
            SortField::Title => "title",
            SortField::Artist => "artist",
            SortField::Album => "album",
            SortField::Year => "year",
            SortField::Duration => "duration_ms",
            SortField::DateAdded => "date_added",
            SortField::PlayCount => "play_count",
        }
    }
}

/// Emitted to the UI while a library scan is in progress.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanProgress {
    pub phase: String,
    pub current: u64,
    pub total: u64,
}

/// Aggregated numbers shown on the library dashboard.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LibraryStats {
    pub tracks: i64,
    pub albums: i64,
    pub artists: i64,
    pub playlists: i64,
    pub total_duration_ms: i64,
}

/// A folder registered as a library root.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Folder {
    pub path: String,
    pub mtime: i64,
    pub enabled: bool,
}

impl Folder {
    pub fn new(path: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            mtime: 0,
            enabled: true,
        }
    }
}
