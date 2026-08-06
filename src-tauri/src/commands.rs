//! Tauri command layer.
//!
//! Thin, synchronous bridges between the UI and the core library / audio
//! engine. Playback commands forward to the engine thread via the command
//! channel; library commands hit the database directly. Heavy operations
//! (scans) are spawned onto the async runtime and report progress via events.

use std::path::{Path, PathBuf};

use harmonia_core::models::{
    Album, Artist, Folder, LibraryStats, Playlist, RepeatMode, ScanProgress, SearchFilters,
    SortField, Track,
};
use harmonia_core::watcher::sync_paths;
use harmonia_core::Settings;
use rodio::cpal::traits::{DeviceTrait, HostTrait};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::engine::{AudioCommand, PlaybackSnapshot, SleepTimer};
use crate::state::AppState;

type CmdResult<T> = Result<T, String>;

pub fn persist_settings(state: &AppState) {
    let settings = state
        .settings
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone();
    let _ = state.db.save_settings(&settings);
}

pub fn refresh_watcher(app: &AppHandle) {
    crate::refresh_watcher(app);
}

pub fn toast(app: &AppHandle, message: impl Into<String>, kind: &str) {
    let _ = app.emit(
        "toast://show",
        serde_json::json!({ "message": message.into(), "kind": kind }),
    );
}

// ---------------------------------------------------------------------------
// Library
// ---------------------------------------------------------------------------

/// Starts a background library scan. Progress is reported via
/// `scan://progress`, completion via `scan://done`.
pub fn trigger_scan(app: AppHandle, force: bool) -> CmdResult<()> {
    let state = app.state::<AppState>();
    let db = state.db.clone();
    let folders = state.settings.lock().unwrap().library_folders.clone();
    let art_dir = state.paths.art_dir.clone();
    if folders.is_empty() {
        return Ok(());
    }
    tauri::async_runtime::spawn(async move {
        let on_progress = |p: ScanProgress| {
            let _ = app.emit("scan://progress", &p);
        };
        match harmonia_core::library::scan_library(&db, &folders, &art_dir, force, &on_progress) {
            Ok(stats) => {
                let _ = app.emit("scan://done", &stats);
                let _ = app.emit("library://changed", ());
            }
            Err(e) => toast(&app, format!("Scan failed: {e}"), "error"),
        }
    });
    Ok(())
}

#[tauri::command]
pub fn scan_library(app: AppHandle, force: bool) -> CmdResult<()> {
    trigger_scan(app, force)
}

#[tauri::command]
pub fn add_folder(app: AppHandle, state: State<'_, AppState>) -> CmdResult<Option<String>> {
    let picked = rfd::FileDialog::new()
        .set_title("Add music folder")
        .pick_folder();
    let Some(path) = picked else {
        return Ok(None);
    };
    let path_str = path.to_string_lossy().into_owned();
    state.db.add_folder(&path_str).map_err(|e| e.to_string())?;
    let mut settings = state.settings.lock().unwrap();
    if !settings.library_folders.contains(&path_str) {
        settings.library_folders.push(path_str.clone());
    }
    drop(settings);
    persist_settings(&state);
    refresh_watcher(&app);
    trigger_scan(app, false)?;
    Ok(Some(path_str))
}

#[tauri::command]
pub fn remove_folder(app: AppHandle, state: State<'_, AppState>, path: String) -> CmdResult<()> {
    state.db.remove_folder(&path).map_err(|e| e.to_string())?;
    {
        let mut settings = state.settings.lock().unwrap();
        settings.library_folders.retain(|f| f != &path);
    }
    persist_settings(&state);
    refresh_watcher(&app);
    let _ = app.emit("library://changed", ());
    Ok(())
}

#[tauri::command]
pub fn list_folders(state: State<'_, AppState>) -> CmdResult<Vec<Folder>> {
    state.db.list_folders().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_stats(state: State<'_, AppState>) -> CmdResult<LibraryStats> {
    state.db.library_stats().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_tracks(
    state: State<'_, AppState>,
    offset: i64,
    limit: i64,
    sort: SortField,
    desc: bool,
    folder: Option<String>,
) -> CmdResult<Vec<Track>> {
    state
        .db
        .get_tracks(offset, limit, sort, desc, folder.as_deref())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_albums(state: State<'_, AppState>) -> CmdResult<Vec<Album>> {
    state.db.get_albums().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_album_tracks(
    state: State<'_, AppState>,
    title: String,
    artist: String,
) -> CmdResult<Vec<Track>> {
    state
        .db
        .get_album_tracks(&title, &artist)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_artists(state: State<'_, AppState>) -> CmdResult<Vec<Artist>> {
    state.db.get_artists().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_artist_tracks(state: State<'_, AppState>, artist: String) -> CmdResult<Vec<Track>> {
    state
        .db
        .tracks_by_artist(&artist)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn search_library(
    state: State<'_, AppState>,
    query: String,
    filters: SearchFilters,
) -> CmdResult<Vec<Track>> {
    state
        .db
        .search(&query, &filters, 500)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn set_favorite(state: State<'_, AppState>, id: i64, favorite: bool) -> CmdResult<()> {
    state
        .db
        .set_favorite(id, favorite)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_favorites(state: State<'_, AppState>) -> CmdResult<Vec<Track>> {
    state.db.get_favorites().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_recently_played(state: State<'_, AppState>, limit: i64) -> CmdResult<Vec<Track>> {
    state.db.recently_played(limit).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_most_played(state: State<'_, AppState>, limit: i64) -> CmdResult<Vec<Track>> {
    state.db.most_played(limit).map_err(|e| e.to_string())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LyricsData {
    pub plain: Option<String>,
    pub synced: Option<String>,
}

#[tauri::command]
pub fn get_lyrics(state: State<'_, AppState>, track_id: i64) -> CmdResult<Option<LyricsData>> {
    let track = state.db.get_track(track_id).map_err(|e| e.to_string())?;
    Ok(track.map(|t| LyricsData {
        plain: t.lyrics,
        synced: t.lyrics_synced,
    }))
}

// ---------------------------------------------------------------------------
// Playback
// ---------------------------------------------------------------------------

pub fn send_engine(state: &AppState, cmd: AudioCommand) {
    let _ = state.engine_tx.send(cmd);
}

#[tauri::command]
pub fn play_track(state: State<'_, AppState>, id: i64) -> CmdResult<()> {
    send_engine(&state, AudioCommand::PlayTrack(id));
    Ok(())
}

#[tauri::command]
pub fn play_context(
    state: State<'_, AppState>,
    ids: Vec<i64>,
    start_index: usize,
) -> CmdResult<()> {
    send_engine(&state, AudioCommand::PlayContext { ids, start_index });
    Ok(())
}

#[tauri::command]
pub fn play_album(
    state: State<'_, AppState>,
    title: String,
    artist: String,
    shuffle: bool,
) -> CmdResult<()> {
    let tracks = state
        .db
        .get_album_tracks(&title, &artist)
        .map_err(|e| e.to_string())?;
    let ids: Vec<i64> = tracks.iter().map(|t| t.id).collect();
    send_engine(&state, AudioCommand::SetShuffle(shuffle));
    send_engine(
        &state,
        AudioCommand::PlayContext {
            ids,
            start_index: 0,
        },
    );
    Ok(())
}

#[tauri::command]
pub fn play_artist(state: State<'_, AppState>, artist: String, shuffle: bool) -> CmdResult<()> {
    let tracks = state
        .db
        .tracks_by_artist(&artist)
        .map_err(|e| e.to_string())?;
    let ids: Vec<i64> = tracks.iter().map(|t| t.id).collect();
    send_engine(&state, AudioCommand::SetShuffle(shuffle));
    send_engine(
        &state,
        AudioCommand::PlayContext {
            ids,
            start_index: 0,
        },
    );
    Ok(())
}

#[tauri::command]
pub fn play_playlist(state: State<'_, AppState>, id: i64, shuffle: bool) -> CmdResult<()> {
    let tracks = resolve_playlist(&state, id)?;
    let ids: Vec<i64> = tracks.iter().map(|t| t.id).collect();
    send_engine(&state, AudioCommand::SetShuffle(shuffle));
    send_engine(
        &state,
        AudioCommand::PlayContext {
            ids,
            start_index: 0,
        },
    );
    Ok(())
}

#[tauri::command]
pub fn play_favorites(state: State<'_, AppState>, shuffle: bool) -> CmdResult<()> {
    let tracks = state.db.get_favorites().map_err(|e| e.to_string())?;
    let ids: Vec<i64> = tracks.iter().map(|t| t.id).collect();
    send_engine(&state, AudioCommand::SetShuffle(shuffle));
    send_engine(
        &state,
        AudioCommand::PlayContext {
            ids,
            start_index: 0,
        },
    );
    Ok(())
}

#[tauri::command]
pub fn play_recent(state: State<'_, AppState>, shuffle: bool) -> CmdResult<()> {
    let tracks = state.db.recently_played(500).map_err(|e| e.to_string())?;
    let ids: Vec<i64> = tracks.iter().map(|t| t.id).collect();
    send_engine(&state, AudioCommand::SetShuffle(shuffle));
    send_engine(
        &state,
        AudioCommand::PlayContext {
            ids,
            start_index: 0,
        },
    );
    Ok(())
}

#[tauri::command]
pub fn play_next(state: State<'_, AppState>) -> CmdResult<()> {
    send_engine(&state, AudioCommand::PlayNext);
    Ok(())
}

#[tauri::command]
pub fn play_previous(state: State<'_, AppState>) -> CmdResult<()> {
    send_engine(&state, AudioCommand::PlayPrevious);
    Ok(())
}

#[tauri::command]
pub fn toggle_playback(state: State<'_, AppState>) -> CmdResult<()> {
    send_engine(&state, AudioCommand::Toggle);
    Ok(())
}

#[tauri::command]
pub fn pause(state: State<'_, AppState>) -> CmdResult<()> {
    send_engine(&state, AudioCommand::Pause);
    Ok(())
}

#[tauri::command]
pub fn resume(state: State<'_, AppState>) -> CmdResult<()> {
    send_engine(&state, AudioCommand::Resume);
    Ok(())
}

#[tauri::command]
pub fn stop(state: State<'_, AppState>) -> CmdResult<()> {
    send_engine(&state, AudioCommand::Stop);
    Ok(())
}

#[tauri::command]
pub fn seek(state: State<'_, AppState>, position_ms: u64) -> CmdResult<()> {
    send_engine(&state, AudioCommand::Seek { position_ms });
    Ok(())
}

#[tauri::command]
pub fn set_volume(state: State<'_, AppState>, volume: f32) -> CmdResult<()> {
    state.settings.lock().unwrap().volume = volume.clamp(0.0, 1.0);
    persist_settings(&state);
    send_engine(&state, AudioCommand::SetVolume(volume));
    Ok(())
}

#[tauri::command]
pub fn set_shuffle(state: State<'_, AppState>, on: bool) -> CmdResult<()> {
    state.settings.lock().unwrap().shuffle = on;
    persist_settings(&state);
    send_engine(&state, AudioCommand::SetShuffle(on));
    Ok(())
}

#[tauri::command]
pub fn set_repeat(state: State<'_, AppState>, mode: RepeatMode) -> CmdResult<()> {
    state.settings.lock().unwrap().repeat = mode;
    persist_settings(&state);
    send_engine(&state, AudioCommand::SetRepeat(mode));
    Ok(())
}

#[tauri::command]
pub fn set_speed(state: State<'_, AppState>, speed: f32) -> CmdResult<()> {
    state.settings.lock().unwrap().playback_speed = speed.clamp(0.5, 2.0);
    persist_settings(&state);
    send_engine(&state, AudioCommand::SetSpeed(speed));
    Ok(())
}

#[tauri::command]
pub fn set_sleep_timer(
    state: State<'_, AppState>,
    kind: String,
    minutes: Option<u64>,
) -> CmdResult<()> {
    let timer = match kind.as_str() {
        "off" => SleepTimer::Off,
        "minutes" => SleepTimer::Minutes(minutes.unwrap_or(30)),
        "endOfTrack" => SleepTimer::EndOfTrack,
        "endOfAlbum" => SleepTimer::EndOfAlbum,
        other => return Err(format!("unknown sleep timer kind: {other}")),
    };
    send_engine(&state, AudioCommand::SetSleepTimer(timer));
    Ok(())
}

#[tauri::command]
pub fn get_playback(state: State<'_, AppState>) -> CmdResult<PlaybackSnapshot> {
    let snapshot = state.snapshot.lock().unwrap().clone();
    Ok(snapshot)
}

#[tauri::command]
pub fn add_to_queue(state: State<'_, AppState>, ids: Vec<i64>) -> CmdResult<()> {
    send_engine(&state, AudioCommand::AddToQueue(ids));
    Ok(())
}

#[tauri::command]
pub fn clear_queue(state: State<'_, AppState>) -> CmdResult<()> {
    send_engine(&state, AudioCommand::ClearQueue);
    Ok(())
}

#[tauri::command]
pub fn remove_queue_item(state: State<'_, AppState>, index: usize) -> CmdResult<()> {
    send_engine(&state, AudioCommand::RemoveQueueItem(index));
    Ok(())
}

#[tauri::command]
pub fn move_queue_item(state: State<'_, AppState>, from: usize, to: usize) -> CmdResult<()> {
    send_engine(&state, AudioCommand::MoveQueueItem { from, to });
    Ok(())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResumeInfo {
    pub track_id: i64,
    pub position_ms: u64,
}

#[tauri::command]
pub fn resume_info(state: State<'_, AppState>) -> CmdResult<Option<ResumeInfo>> {
    let track_id = state
        .db
        .get_setting("resume.track_id")
        .map_err(|e| e.to_string())?
        .and_then(|s| s.parse::<i64>().ok());
    let position_ms = state
        .db
        .get_setting("resume.position_ms")
        .map_err(|e| e.to_string())?
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(0);
    Ok(track_id.map(|id| ResumeInfo {
        track_id: id,
        position_ms,
    }))
}

#[tauri::command]
pub fn continue_session(state: State<'_, AppState>) -> CmdResult<()> {
    let info = resume_info_impl(&state)?;
    if let Some(info) = info {
        send_engine(
            &state,
            AudioCommand::RestoreSession {
                track_id: info.track_id,
                position_ms: info.position_ms,
            },
        );
    }
    Ok(())
}

pub fn resume_info_impl(state: &AppState) -> CmdResult<Option<ResumeInfo>> {
    let track_id = state
        .db
        .get_setting("resume.track_id")
        .map_err(|e| e.to_string())?
        .and_then(|s| s.parse::<i64>().ok());
    let position_ms = state
        .db
        .get_setting("resume.position_ms")
        .map_err(|e| e.to_string())?
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(0);
    Ok(track_id.map(|id| ResumeInfo {
        track_id: id,
        position_ms,
    }))
}

// ---------------------------------------------------------------------------
// Playlists
// ---------------------------------------------------------------------------

pub fn resolve_playlist(state: &AppState, id: i64) -> CmdResult<Vec<Track>> {
    let playlists = state.db.list_playlists().map_err(|e| e.to_string())?;
    let playlist = playlists
        .into_iter()
        .find(|p| p.id == id)
        .ok_or_else(|| "playlist not found".to_string())?;
    if playlist.kind == "smart" {
        let rules = playlist
            .rules
            .ok_or_else(|| "smart playlist has no rules".to_string())?;
        state
            .db
            .smart_playlist_tracks(&rules)
            .map_err(|e| e.to_string())
    } else {
        state.db.get_playlist_tracks(id).map_err(|e| e.to_string())
    }
}

#[tauri::command]
pub fn list_playlists(state: State<'_, AppState>) -> CmdResult<Vec<Playlist>> {
    state.db.list_playlists().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn create_playlist(
    state: State<'_, AppState>,
    name: String,
    kind: Option<String>,
    rules: Option<String>,
) -> CmdResult<i64> {
    let name = name.trim();
    if name.is_empty() {
        return Err("playlist name cannot be empty".into());
    }
    let kind = kind.unwrap_or_else(|| "static".into());
    if kind == "smart" {
        let rules = rules.ok_or_else(|| "smart playlist requires rules".to_string())?;
        let parsed: harmonia_core::models::SmartRules =
            serde_json::from_str(&rules).map_err(|e| format!("invalid rules: {e}"))?;
        harmonia_core::playlists::validate_rules(&parsed).map_err(|e| e.to_string())?;
        let id = state
            .db
            .create_playlist(name, "smart", Some(&rules))
            .map_err(|e| e.to_string())?;
        Ok(id)
    } else {
        let id = state
            .db
            .create_playlist(name, "static", None)
            .map_err(|e| e.to_string())?;
        Ok(id)
    }
}

#[tauri::command]
pub fn rename_playlist(state: State<'_, AppState>, id: i64, name: String) -> CmdResult<()> {
    let name = name.trim();
    if name.is_empty() {
        return Err("playlist name cannot be empty".into());
    }
    state
        .db
        .rename_playlist(id, name)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_playlist(state: State<'_, AppState>, id: i64) -> CmdResult<()> {
    state.db.delete_playlist(id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn set_playlist_pinned(state: State<'_, AppState>, id: i64, pinned: bool) -> CmdResult<()> {
    state
        .db
        .set_playlist_pinned(id, pinned)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_playlist_tracks(state: State<'_, AppState>, id: i64) -> CmdResult<Vec<Track>> {
    state.db.get_playlist_tracks(id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn add_tracks_to_playlist(
    state: State<'_, AppState>,
    id: i64,
    track_ids: Vec<i64>,
) -> CmdResult<()> {
    for track_id in track_ids {
        state
            .db
            .add_track_to_playlist(id, track_id)
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub fn remove_track_from_playlist(
    state: State<'_, AppState>,
    playlist_id: i64,
    track_id: i64,
) -> CmdResult<()> {
    state
        .db
        .remove_track_from_playlist(playlist_id, track_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn reorder_playlist(
    state: State<'_, AppState>,
    id: i64,
    ordered_ids: Vec<i64>,
) -> CmdResult<()> {
    state
        .db
        .reorder_playlist(id, &ordered_ids)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn update_smart_rules(state: State<'_, AppState>, id: i64, rules: String) -> CmdResult<()> {
    let parsed: harmonia_core::models::SmartRules =
        serde_json::from_str(&rules).map_err(|e| format!("invalid rules: {e}"))?;
    harmonia_core::playlists::validate_rules(&parsed).map_err(|e| e.to_string())?;
    state
        .db
        .update_playlist_rules(id, &rules)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn export_playlist(
    state: State<'_, AppState>,
    id: i64,
    format: String,
) -> CmdResult<Option<String>> {
    let playlists = state.db.list_playlists().map_err(|e| e.to_string())?;
    let playlist = playlists
        .into_iter()
        .find(|p| p.id == id)
        .ok_or_else(|| "playlist not found".to_string())?;
    let tracks = resolve_playlist(&state, id)?;
    let ext = match format.as_str() {
        "m3u" | "m3u8" => "m3u",
        "pls" => "pls",
        "xspf" => "xspf",
        other => return Err(format!("unsupported format: {other}")),
    };
    let dialog = rfd::FileDialog::new()
        .set_title("Export playlist")
        .set_file_name(format!("{}.{}", playlist.name, ext))
        .add_filter("Playlist", &[ext]);
    let Some(target) = dialog.save_file() else {
        return Ok(None);
    };
    harmonia_core::playlists::export_playlist(&target, &format, &playlist.name, &tracks)
        .map_err(|e| e.to_string())?;
    Ok(Some(target.to_string_lossy().into_owned()))
}

#[tauri::command]
pub fn import_playlist(state: State<'_, AppState>, app: AppHandle) -> CmdResult<Option<i64>> {
    let Some(source) = rfd::FileDialog::new()
        .set_title("Import playlist")
        .add_filter("Playlists", &["m3u", "m3u8"])
        .pick_file()
    else {
        return Ok(None);
    };
    let paths = harmonia_core::playlists::import_m3u(&source).map_err(|e| e.to_string())?;
    let matched = harmonia_core::playlists::match_imported_paths(&state.db, &paths)
        .map_err(|e| e.to_string())?;
    if matched.is_empty() {
        toast(
            &app,
            "No tracks from the playlist are in your library",
            "info",
        );
        return Ok(None);
    }
    let name = source
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Imported")
        .to_string();
    let id = state
        .db
        .create_playlist(&name, "static", None)
        .map_err(|e| e.to_string())?;
    for (_, track) in &matched {
        state
            .db
            .add_track_to_playlist(id, track.id)
            .map_err(|e| e.to_string())?;
    }
    toast(
        &app,
        format!("Imported {} tracks into \"{name}\"", matched.len()),
        "success",
    );
    Ok(Some(id))
}

// ---------------------------------------------------------------------------
// Artwork, settings, system
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn get_artwork(state: State<'_, AppState>, hash: String) -> CmdResult<Option<String>> {
    if hash.is_empty() {
        return Ok(None);
    }
    for ext in ["jpg", "png", "gif", "webp"] {
        let file = state.paths.art_dir.join(format!("{hash}.{ext}"));
        if file.exists() {
            let bytes = std::fs::read(&file).map_err(|e| e.to_string())?;
            // Skip absurdly large covers to keep the IPC payloads sane.
            if bytes.len() > 512_000 {
                return Ok(None);
            }
            use base64::Engine as _;
            let data = base64::engine::general_purpose::STANDARD.encode(bytes);
            return Ok(Some(format!("data:image/{ext};base64,{data}")));
        }
    }
    Ok(None)
}

#[tauri::command]
pub fn get_settings(state: State<'_, AppState>) -> CmdResult<Settings> {
    Ok(state.settings.lock().unwrap().clone())
}

#[tauri::command]
pub fn update_settings(
    app: AppHandle,
    state: State<'_, AppState>,
    settings: Settings,
) -> CmdResult<()> {
    // Validate and normalize user input.
    let mut settings = settings;
    settings.volume = settings.volume.clamp(0.0, 1.0);
    settings.playback_speed = settings.playback_speed.clamp(0.5, 2.0);
    settings.crossfade_seconds = settings.crossfade_seconds.clamp(0.0, 15.0);
    settings.bass_boost_db = settings.bass_boost_db.clamp(-12.0, 12.0);
    settings.balance = settings.balance.clamp(-1.0, 1.0);
    settings.window_width = settings.window_width.clamp(200.0, 16_384.0);
    settings.window_height = settings.window_height.clamp(200.0, 16_384.0);
    if !Settings::accent_ok(&settings.accent) {
        settings.accent = "#7c5cff".to_string();
    }
    if !Settings::language_ok(&settings.language) {
        settings.language = "en".to_string();
    }
    if settings.eq_gains.len() != 10 {
        settings.eq_gains = settings.eq_gains.iter().take(10).copied().collect();
        while settings.eq_gains.len() < 10 {
            settings.eq_gains.push(0.0);
        }
    }

    // Never let a stale or empty folder list silently wipe the configured
    // library roots (the folders table is the source of truth).
    if settings.library_folders.is_empty() {
        if let Ok(folders) = state.db.list_folders() {
            settings.library_folders = folders.into_iter().map(|f| f.path).collect();
        }
    }

    let old_folders = state
        .settings
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .library_folders
        .clone();
    {
        let mut guard = state.settings.lock().unwrap_or_else(|e| e.into_inner());
        *guard = settings.clone();
    }
    persist_settings(&state);

    // Apply folder changes.
    if old_folders != settings.library_folders {
        for folder in &old_folders {
            if !settings.library_folders.contains(folder) {
                let _ = state.db.remove_folder(folder);
            }
        }
        for folder in &settings.library_folders {
            if !old_folders.contains(folder) {
                let _ = state.db.add_folder(folder);
            }
        }
        refresh_watcher(&app);
        let _ = trigger_scan(app.clone(), false);
    }

    // Push live settings to the engine (volume, device, EQ, ...).
    send_engine(&state, AudioCommand::ReloadSettings);

    // Notify the UI so theme/accent/language changes apply immediately.
    let _ = app.emit("settings://changed", &settings);
    Ok(())
}

#[tauri::command]
pub fn get_audio_devices() -> CmdResult<Vec<String>> {
    let host = rodio::cpal::default_host();
    let mut out = Vec::new();
    if let Ok(devices) = host.output_devices() {
        for device in devices {
            if let Ok(desc) = device.description() {
                out.push(desc.name().to_string());
            }
        }
    }
    out.sort();
    Ok(out)
}

#[tauri::command]
pub fn set_mini_player(app: AppHandle, state: State<'_, AppState>, enabled: bool) -> CmdResult<()> {
    // Flip the mode flag BEFORE resizing: window resize events are dispatched
    // asynchronously, and the geometry-save handler skips while the mini flag
    // is set — so the 420x150 mini size can never overwrite the remembered
    // full-window size.
    state
        .settings
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .mini_player = enabled;
    persist_settings(&state);
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| "main window not found".to_string())?;
    if enabled {
        window
            .set_size(tauri::Size::Logical(tauri::LogicalSize::new(420.0, 150.0)))
            .map_err(|e| e.to_string())?;
        window.set_always_on_top(true).map_err(|e| e.to_string())?;
    } else {
        // Restore the remembered full-window geometry rather than a hardcoded
        // size, so leaving mini-player returns the user to their layout.
        let s = state.settings.lock().unwrap_or_else(|e| e.into_inner());
        let (w, h) = (
            s.window_width.clamp(640.0, 16_384.0),
            s.window_height.clamp(480.0, 16_384.0),
        );
        drop(s);
        window
            .set_size(tauri::Size::Logical(tauri::LogicalSize::new(w, h)))
            .map_err(|e| e.to_string())?;
        window.set_always_on_top(false).map_err(|e| e.to_string())?;
    }
    let _ = app.emit("ui://mini", enabled);
    Ok(())
}

/// Fully quits the application, bypassing close-to-tray. Called from the
/// frontend (e.g. "Exit" actions); the tray Quit item triggers the same path
/// via the native exit request.
#[tauri::command]
pub fn quit_app(app: AppHandle) {
    app.exit(0);
}

/// Best-effort desktop notification. Failures (unsupported platform,
/// permission denied) are logged, never surfaced as errors to the caller.
#[tauri::command]
pub fn notify_now(app: AppHandle, title: String, body: String) -> CmdResult<()> {
    use tauri_plugin_notification::NotificationExt;
    app.notification()
        .builder()
        .title(title)
        .body(body)
        .show()
        .map_err(|e| e.to_string())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportResult {
    pub added: usize,
    pub failed: usize,
}

/// Imports audio files (from drag & drop or "open with") into the library.
pub fn import_paths_inner(
    state: &AppState,
    app: &AppHandle,
    paths: &[String],
) -> CmdResult<ImportResult> {
    let paths: Vec<PathBuf> = paths.iter().map(PathBuf::from).collect();
    let touched =
        sync_paths(&state.db, &paths, &state.paths.art_dir, |_| {}).map_err(|e| e.to_string())?;
    let _ = app.emit("library://changed", ());
    if touched > 0 {
        toast(
            app,
            format!("Added {touched} file(s) to your library"),
            "success",
        );
    }
    Ok(ImportResult {
        added: touched,
        failed: 0,
    })
}

#[tauri::command]
pub fn import_paths(
    app: AppHandle,
    state: State<'_, AppState>,
    paths: Vec<String>,
) -> CmdResult<ImportResult> {
    let state = &*state;
    import_paths_inner(state, &app, &paths)
}

/// Handles files passed to a second instance of the app ("open with").
pub fn handle_open_files(app: &AppHandle, argv: &[String]) {
    let files: Vec<String> = argv
        .iter()
        .filter(|a| {
            let p = Path::new(a);
            p.is_file() && harmonia_core::util::is_audio_file(p)
        })
        .cloned()
        .collect();
    if files.is_empty() {
        return;
    }
    if let Some(state) = app.try_state::<AppState>() {
        if let Ok(result) = import_paths_inner(&state, app, &files) {
            if result.added > 0 {
                let _ = app.emit("library://changed", ());
            }
        }
    }
}
