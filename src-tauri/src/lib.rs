//! Harmonia — a modern, fast, beautiful music player for Linux.
//!
//! The application is split into a pure-Rust core crate (`harmonia-core`:
//! database, metadata, scanning, playlists, DSP) and this Tauri crate, which
//! owns the audio engine thread, the command layer, the tray and the global
//! shortcuts.

mod commands;
mod dsp_source;
mod engine;
mod paths;
mod state;
mod tray;

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use harmonia_core::{watcher::LibraryWatcher, Database, Settings};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_global_shortcut::{
    Code, GlobalShortcutExt, Shortcut, ShortcutEvent, ShortcutState,
};

use crate::engine::{spawn_engine, AudioCommand};
use crate::state::AppState;

fn load_settings(db: &Database) -> Settings {
    db.get_setting("settings")
        .ok()
        .flatten()
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or_default()
}

fn persist_settings(db: &Database, settings: &Settings) {
    if let Ok(json) = serde_json::to_string(settings) {
        let _ = db.set_setting("settings", &json);
    }
}

/// (Re)builds the filesystem watcher from the configured library folders.
pub(crate) fn refresh_watcher(app: &AppHandle) {
    let state = app.state::<AppState>();
    let folders: Vec<PathBuf> = state
        .settings
        .lock()
        .unwrap()
        .library_folders
        .iter()
        .map(PathBuf::from)
        .collect();
    let mut guard = state.watcher.lock().unwrap();
    *guard = None;
    if folders.is_empty() {
        return;
    }
    let handler_app = app.clone();
    let handler_db = state.db.clone();
    let art_dir = state.paths.art_dir.clone();
    match LibraryWatcher::spawn(&folders, move |paths| {
        if let Err(e) = harmonia_core::watcher::sync_paths(&handler_db, &paths, &art_dir, |_| {}) {
            log::warn!("watcher sync failed: {e}");
        }
        let _ = handler_app.emit("library://changed", ());
    }) {
        Ok(watcher) => *guard = Some(watcher),
        Err(e) => log::warn!("filesystem watcher failed to start: {e}"),
    }
}

/// Shared handler for the global media-key shortcuts.
fn on_media_shortcut(app: &AppHandle, shortcut: &Shortcut, event: ShortcutEvent) {
    if event.state != ShortcutState::Pressed {
        return;
    }
    let cmd = match shortcut.key {
        Code::MediaPlayPause => AudioCommand::Toggle,
        Code::MediaTrackNext => AudioCommand::PlayNext,
        Code::MediaTrackPrevious => AudioCommand::PlayPrevious,
        _ => return,
    };
    if let Some(state) = app.try_state::<AppState>() {
        let _ = state.engine_tx.send(cmd);
    }
}

/// Registers the global media keys. Failures are logged rather than aborting
/// startup — media keys can be unavailable on headless/remote setups.
fn register_global_shortcuts(app: &AppHandle) {
    for key in ["MediaPlayPause", "MediaTrackNext", "MediaTrackPrevious"] {
        if let Err(e) = app.global_shortcut().on_shortcut(key, on_media_shortcut) {
            log::warn!("failed to register global shortcut {key}: {e}");
        }
    }
}

/// Second instance launched (e.g. files opened with Harmonia): focus the
/// existing window and import any audio files passed on the command line.
fn on_second_instance(app: &AppHandle, argv: Vec<String>) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.set_focus();
    }
    commands::handle_open_files(app, &argv);
}

fn default_music_folder(app: &tauri::App) -> Option<PathBuf> {
    let home = app.path().home_dir().ok()?;
    let candidate = home.join("Music");
    candidate.is_dir().then_some(candidate)
}

pub fn run() {
    env_logger::init();

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, argv, _cwd| {
            on_second_instance(app, argv);
        }))
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .setup(|app| {
            let paths = crate::paths::AppPaths::resolve(app)?;

            let db = Arc::new(Database::open(&paths.db_path)?);
            let mut settings = load_settings(&db);

            // Seed the folders table and add a sensible default root.
            for folder in &settings.library_folders {
                let _ = db.add_folder(folder);
            }
            if settings.library_folders.is_empty() {
                if let Some(music) = default_music_folder(app) {
                    let music = music.to_string_lossy().into_owned();
                    let _ = db.add_folder(&music);
                    settings.library_folders.push(music);
                    persist_settings(&db, &settings);
                }
            }

            let settings = Arc::new(Mutex::new(settings));
            let engine = spawn_engine(app.handle().clone(), db.clone(), settings.clone());
            let state = AppState {
                db,
                settings: settings.clone(),
                paths,
                engine_tx: engine.tx,
                snapshot: engine.snapshot,
                watcher: Mutex::new(None),
            };
            app.manage(state);

            refresh_watcher(app.handle());
            register_global_shortcuts(app.handle());
            crate::tray::build_tray(app)?;

            let st = app.state::<AppState>();
            // First run: scan the library in the background.
            let empty = st.db.count_tracks().unwrap_or(0) == 0;
            if empty && !st.settings.lock().unwrap().library_folders.is_empty() {
                let _ = commands::trigger_scan(app.handle().clone(), false);
            }
            // Resume the previous session.
            let resume = st.settings.lock().unwrap().resume_last_session;
            if resume {
                let info = st
                    .db
                    .get_setting("resume.track_id")
                    .ok()
                    .flatten()
                    .and_then(|s| s.parse::<i64>().ok());
                if let Some(track_id) = info {
                    let position_ms = st
                        .db
                        .get_setting("resume.position_ms")
                        .ok()
                        .flatten()
                        .and_then(|s| s.parse::<u64>().ok())
                        .unwrap_or(0);
                    let _ = st.engine_tx.send(AudioCommand::RestoreSession {
                        track_id,
                        position_ms,
                    });
                }
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // Library
            commands::scan_library,
            commands::add_folder,
            commands::remove_folder,
            commands::list_folders,
            commands::get_stats,
            commands::get_tracks,
            commands::get_albums,
            commands::get_album_tracks,
            commands::get_artists,
            commands::get_artist_tracks,
            commands::search_library,
            commands::set_favorite,
            commands::get_favorites,
            commands::get_recently_played,
            commands::get_most_played,
            commands::get_lyrics,
            // Playback
            commands::play_track,
            commands::play_context,
            commands::play_album,
            commands::play_artist,
            commands::play_playlist,
            commands::play_favorites,
            commands::play_recent,
            commands::play_next,
            commands::play_previous,
            commands::toggle_playback,
            commands::pause,
            commands::resume,
            commands::stop,
            commands::seek,
            commands::set_volume,
            commands::set_shuffle,
            commands::set_repeat,
            commands::set_speed,
            commands::set_sleep_timer,
            commands::get_playback,
            commands::add_to_queue,
            commands::clear_queue,
            commands::remove_queue_item,
            commands::move_queue_item,
            commands::resume_info,
            commands::continue_session,
            // Playlists
            commands::list_playlists,
            commands::create_playlist,
            commands::rename_playlist,
            commands::delete_playlist,
            commands::set_playlist_pinned,
            commands::get_playlist_tracks,
            commands::add_tracks_to_playlist,
            commands::remove_track_from_playlist,
            commands::reorder_playlist,
            commands::update_smart_rules,
            commands::export_playlist,
            commands::import_playlist,
            // Artwork, settings, system
            commands::get_artwork,
            commands::get_settings,
            commands::update_settings,
            commands::get_audio_devices,
            commands::set_mini_player,
            commands::import_paths,
        ])
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                let state = window.state::<AppState>();
                let hide_to_tray = !state.settings.lock().unwrap().mini_player;
                if hide_to_tray {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running Harmonia");
}
