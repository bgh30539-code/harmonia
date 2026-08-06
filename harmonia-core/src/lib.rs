//! # Harmonia core
//!
//! The backend-agnostic heart of the Harmonia music player. Everything in this
//! crate is pure Rust with no GUI or audio-backend dependencies, which keeps
//! it fully unit-testable and lets the application layer swap audio engines
//! without touching the data layer.
//!
//! ## Modules
//!
//! - [`db`] — SQLite persistence (tracks, albums, artists, playlists, settings)
//! - [`metadata`] — tag/property extraction and artwork caching via `lofty`
//! - [`library`] — recursive, incremental, parallel library scanning
//! - [`watcher`] — debounced live filesystem watching
//! - [`playlists`] — M3U/M3U8/PLS/XSPF interchange and smart-playlist rules
//! - [`dsp`] — biquad filters for the EQ and bass boost
//! - [`settings`] — user settings model
//! - [`models`] — serde data models shared with the UI
//! - [`util`] — small shared helpers

pub mod db;
pub mod dsp;
pub mod error;
pub mod library;
pub mod metadata;
pub mod models;
pub mod playlists;
pub mod settings;
pub mod util;
pub mod watcher;

pub use db::{Database, TrackUpsert};
pub use error::{CoreError, CoreResult};
pub use models::*;
pub use settings::Settings;
