use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};

use harmonia_core::watcher::LibraryWatcher;
use harmonia_core::{Database, Settings};

use crate::engine::AudioCommand;
use crate::paths::AppPaths;

/// Everything the command layer and background threads share.
pub struct AppState {
    pub db: Arc<Database>,
    pub settings: Arc<Mutex<Settings>>,
    pub paths: AppPaths,
    /// Command channel into the dedicated audio engine thread.
    pub engine_tx: Sender<AudioCommand>,
    /// Latest playback state, mirrored for cheap reads.
    pub snapshot: Arc<Mutex<crate::engine::PlaybackSnapshot>>,
    /// Keeps the filesystem watcher alive; replaced when folders change.
    pub watcher: Mutex<Option<LibraryWatcher>>,
}
