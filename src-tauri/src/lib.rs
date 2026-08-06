//! Harmonia — a modern, fast, beautiful music player for Linux and Windows.
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
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use harmonia_core::{watcher::LibraryWatcher, Database, Settings};
use tauri::{AppHandle, Emitter, Manager, RunEvent};
use tauri_plugin_global_shortcut::{
    Code, GlobalShortcutExt, Shortcut, ShortcutEvent, ShortcutState,
};

use crate::engine::{spawn_engine, AudioCommand};
use crate::state::AppState;

/// Locks the shared settings mutex, surviving a poisoned lock (a thread that
/// panicked while holding it). The alternative — panicking from inside the
/// window close handler — is exactly the kind of "close button stops
/// responding" bug we want to rule out.
fn lock_settings(settings: &Mutex<Settings>) -> std::sync::MutexGuard<'_, Settings> {
    settings.lock().unwrap_or_else(|e| e.into_inner())
}

/// (Re)builds the filesystem watcher from the configured library folders.
/// Folders that no longer exist are skipped so a removed/mounted-elsewhere
/// path can never take the watcher down.
pub(crate) fn refresh_watcher(app: &AppHandle) {
    let state = app.state::<AppState>();
    let folders: Vec<PathBuf> = lock_settings(&state.settings)
        .library_folders
        .iter()
        .map(PathBuf::from)
        .filter(|p| p.is_dir())
        .collect();
    let mut guard = state.watcher.lock().unwrap_or_else(|e| e.into_inner());
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

// ---------------------------------------------------------------------------
// Window geometry persistence
// ---------------------------------------------------------------------------

/// Sanity range for restored window dimensions (physical pixels). Guards
/// against corrupted settings pushing the window off-screen or to size 0.
const MIN_WIN_W: f64 = 640.0;
const MIN_WIN_H: f64 = 480.0;
const MAX_WIN: f64 = 16_384.0;

/// Records the current outer size/position into the in-memory settings.
/// Skipped while the mini player is active so its tiny geometry never
/// overwrites the remembered full window. When `persist` is true the blob is
/// written to the database immediately (called on close/destroy).
fn update_window_geometry(window: &tauri::Window, state: &AppState, persist: bool) {
    if lock_settings(&state.settings).mini_player {
        return;
    }
    let size = window.outer_size().ok();
    let position = window.outer_position().ok();
    let maximized = window.is_maximized().unwrap_or(false);
    {
        let mut s = lock_settings(&state.settings);
        if let Some(sz) = size {
            if sz.width >= 200 && sz.height >= 200 {
                s.window_width = sz.width as f64;
                s.window_height = sz.height as f64;
            }
        }
        if let Some(p) = position {
            s.window_x = Some(p.x as f64);
            s.window_y = Some(p.y as f64);
        }
        s.window_maximized = maximized;
    }
    if persist {
        let s = lock_settings(&state.settings).clone();
        let _ = state.db.save_settings(&s);
    }
}

/// Debounced persistence for window moves/resizes: while the user drags or
/// resizes, only the in-memory settings are updated (cheap); one background
/// write happens ~600 ms after the last event.
fn debounce_window_persist(state: &AppState) {
    if state.window_save_pending.swap(true, Ordering::Relaxed) {
        return;
    }
    let settings = state.settings.clone();
    let db = state.db.clone();
    let pending = state.window_save_pending.clone();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(600));
        let s = lock_settings(&settings).clone();
        let _ = db.save_settings(&s);
        pending.store(false, Ordering::Relaxed);
    });
}

/// Restores the saved window geometry on startup. Position values are only
/// applied when they look sane (a disconnected monitor can leave stale
/// coordinates behind, in which case the OS centers the window instead).
fn apply_window_geometry(app: &tauri::App, state: &AppState) {
    let Some(window) = app.get_webview_window("main") else {
        return;
    };
    let s = lock_settings(&state.settings);
    let w = s.window_width.clamp(MIN_WIN_W, MAX_WIN) as u32;
    let h = s.window_height.clamp(MIN_WIN_H, MAX_WIN) as u32;
    let (x, y, maximized) = (s.window_x, s.window_y, s.window_maximized);
    drop(s);
    let _ = window.set_size(tauri::Size::Physical(tauri::PhysicalSize::new(w, h)));
    if let (Some(x), Some(y)) = (x, y) {
        if (-10_000.0..=50_000.0).contains(&x) && (-10_000.0..=50_000.0).contains(&y) {
            let _ = window.set_position(tauri::Position::Physical(tauri::PhysicalPosition::new(
                x as i32, y as i32,
            )));
        }
    }
    if maximized {
        let _ = window.maximize();
    }
}

/// Final flush on application exit: persist the resume position (so "last
/// played song" survives an actual quit, not just a hide-to-tray) plus the
/// current settings blob. Best-effort — shutdown must never be blocked by a
/// failing write.
fn flush_on_exit(app: &AppHandle) {
    let Some(state) = app.try_state::<AppState>() else {
        return;
    };
    {
        let snap = state.snapshot.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(track) = &snap.current {
            let _ = state
                .db
                .set_setting("resume.track_id", &track.id.to_string());
            let _ = state
                .db
                .set_setting("resume.position_ms", &snap.position_ms.to_string());
        }
    }
    let s = lock_settings(&state.settings).clone();
    let _ = state.db.save_settings(&s);
    log::info!("Harmonia shutting down cleanly");
}

pub fn run() {
    env_logger::init();

    // The Windows build has no console (windows_subsystem = "windows"), so a
    // panic would otherwise vanish. Route panics into the log file instead of
    // dying silently — and make sure a panicked thread never takes the whole
    // process down unexpectedly.
    std::panic::set_hook(Box::new(|info| {
        log::error!("panic: {info}");
        eprintln!("Harmonia panicked: {info}");
    }));

    let app = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, argv, _cwd| {
            on_second_instance(app, argv);
        }))
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_notification::init())
        .setup(|app| {
            let paths = crate::paths::AppPaths::resolve(app)?;

            let db = Arc::new(Database::open(&paths.db_path)?);
            // `load_settings` re-seeds library folders from the folders table
            // if the stored blob is missing or corrupted.
            let mut settings = db.load_settings()?;

            // Keep the folders table in sync with the settings blob.
            for folder in &settings.library_folders {
                let _ = db.add_folder(folder);
            }
            if settings.library_folders.is_empty() {
                if let Some(music) = default_music_folder(app) {
                    let music = music.to_string_lossy().into_owned();
                    let _ = db.add_folder(&music);
                    settings.library_folders.push(music);
                }
            }

            // Mini-player is a transient window mode, not a persistent one:
            // always start in the normal window.
            settings.mini_player = false;

            let settings = Arc::new(Mutex::new(settings));
            let engine = spawn_engine(app.handle().clone(), db.clone(), settings.clone());
            let state = AppState {
                db,
                settings: settings.clone(),
                paths,
                engine_tx: engine.tx,
                snapshot: engine.snapshot,
                watcher: Mutex::new(None),
                tray_active: Arc::new(AtomicBool::new(false)),
                window_save_pending: Arc::new(AtomicBool::new(false)),
            };
            app.manage(state);

            let st = app.state::<AppState>();
            apply_window_geometry(app, &st);

            refresh_watcher(app.handle());
            register_global_shortcuts(app.handle());
            match crate::tray::build_tray(app) {
                Ok(()) => st.tray_active.store(true, Ordering::Relaxed),
                Err(e) => log::warn!("system tray unavailable, closing the window will quit: {e}"),
            }

            // First run: scan the library in the background.
            let empty = st.db.count_tracks().unwrap_or(0) == 0;
            if empty && !lock_settings(&st.settings).library_folders.is_empty() {
                let _ = commands::trigger_scan(app.handle().clone(), false);
            }
            // Resume the previous session (last played song + position).
            let resume = lock_settings(&st.settings).resume_last_session;
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
            commands::quit_app,
            commands::notify_now,
        ])
        .on_window_event(|window, event| match event {
            tauri::WindowEvent::CloseRequested { api, .. } => {
                // The close button must ALWAYS work: if the user opted into
                // close-to-tray (and the tray actually exists) the window
                // hides and playback continues; otherwise the window closes
                // and, being the only window, the app exits.
                let Some(state) = window.try_state::<AppState>() else {
                    return;
                };
                let settings = lock_settings(&state.settings);
                let hide_to_tray = settings.close_to_tray
                    && !settings.mini_player
                    && state.tray_active.load(Ordering::Relaxed);
                drop(settings);
                if hide_to_tray {
                    api.prevent_close();
                    let _ = window.hide();
                    // Remember the geometry before hiding.
                    update_window_geometry(window, &state, true);
                } else {
                    update_window_geometry(window, &state, true);
                }
            }
            tauri::WindowEvent::Resized(_) | tauri::WindowEvent::Moved(_) => {
                if let Some(state) = window.try_state::<AppState>() {
                    update_window_geometry(window, &state, false);
                    debounce_window_persist(&state);
                }
            }
            tauri::WindowEvent::Destroyed => {
                if let Some(state) = window.try_state::<AppState>() {
                    update_window_geometry(window, &state, true);
                }
            }
            _ => {}
        })
        .build(tauri::generate_context!())
        .expect("error while building Harmonia");

    app.run(|app_handle, event| {
        if let RunEvent::ExitRequested { .. } = event {
            flush_on_exit(app_handle);
        }
    });
}
