import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  Album,
  Artist,
  Folder,
  ImportResult,
  LibraryStats,
  LyricsData,
  PlaybackSnapshot,
  Playlist,
  RepeatMode,
  ResumeInfo,
  ScanProgress,
  ScanStats,
  SearchFilters,
  Settings,
  SortField,
  Track,
} from "./types";

// ---------------------------------------------------------------------------
// Library
// ---------------------------------------------------------------------------

export const scanLibrary = (force: boolean) => invoke<void>("scan_library", { force });
export const addFolder = () => invoke<string | null>("add_folder");
export const removeFolder = (path: string) => invoke<void>("remove_folder", { path });
export const listFolders = () => invoke<Folder[]>("list_folders");
export const getStats = () => invoke<LibraryStats>("get_stats");
export const getTracks = (
  offset: number,
  limit: number,
  sort: SortField,
  desc: boolean,
  folder?: string | null,
) => invoke<Track[]>("get_tracks", { offset, limit, sort, desc, folder });
export const getAlbums = () => invoke<Album[]>("get_albums");
export const getAlbumTracks = (title: string, artist: string) =>
  invoke<Track[]>("get_album_tracks", { title, artist });
export const getArtists = () => invoke<Artist[]>("get_artists");
export const getArtistTracks = (artist: string) =>
  invoke<Track[]>("get_artist_tracks", { artist });
export const searchLibrary = (query: string, filters: SearchFilters) =>
  invoke<Track[]>("search_library", { query, filters });
export const setFavorite = (id: number, favorite: boolean) =>
  invoke<void>("set_favorite", { id, favorite });
export const getFavorites = () => invoke<Track[]>("get_favorites");
export const getRecentlyPlayed = (limit: number) =>
  invoke<Track[]>("get_recently_played", { limit });
export const getMostPlayed = (limit: number) => invoke<Track[]>("get_most_played", { limit });
export const getLyrics = (trackId: number) =>
  invoke<LyricsData | null>("get_lyrics", { trackId });

// ---------------------------------------------------------------------------
// Playback
// ---------------------------------------------------------------------------

export const playTrack = (id: number) => invoke<void>("play_track", { id });
export const playContext = (ids: number[], startIndex = 0) =>
  invoke<void>("play_context", { ids, startIndex });
export const playAlbum = (title: string, artist: string, shuffle = false) =>
  invoke<void>("play_album", { title, artist, shuffle });
export const playArtist = (artist: string, shuffle = false) =>
  invoke<void>("play_artist", { artist, shuffle });
export const playPlaylist = (id: number, shuffle = false) =>
  invoke<void>("play_playlist", { id, shuffle });
export const playFavorites = (shuffle = false) => invoke<void>("play_favorites", { shuffle });
export const playRecent = (shuffle = false) => invoke<void>("play_recent", { shuffle });
export const playNext = () => invoke<void>("play_next");
export const playPrevious = () => invoke<void>("play_previous");
export const togglePlayback = () => invoke<void>("toggle_playback");
export const pause = () => invoke<void>("pause");
export const resume = () => invoke<void>("resume");
export const stop = () => invoke<void>("stop");
export const seek = (positionMs: number) => invoke<void>("seek", { positionMs });
export const setVolume = (volume: number) => invoke<void>("set_volume", { volume });
export const setShuffle = (on: boolean) => invoke<void>("set_shuffle", { on });
export const setRepeat = (mode: RepeatMode) => invoke<void>("set_repeat", { mode });
export const setSpeed = (speed: number) => invoke<void>("set_speed", { speed });
export const setSleepTimer = (kind: string, minutes?: number | null) =>
  invoke<void>("set_sleep_timer", { kind, minutes });
export const getPlayback = () => invoke<PlaybackSnapshot>("get_playback");
export const addToQueue = (ids: number[]) => invoke<void>("add_to_queue", { ids });
export const clearQueue = () => invoke<void>("clear_queue");
export const removeQueueItem = (index: number) =>
  invoke<void>("remove_queue_item", { index });
export const moveQueueItem = (from: number, to: number) =>
  invoke<void>("move_queue_item", { from, to });
export const resumeInfo = () => invoke<ResumeInfo | null>("resume_info");
export const continueSession = () => invoke<void>("continue_session");

// ---------------------------------------------------------------------------
// Playlists
// ---------------------------------------------------------------------------

export const listPlaylists = () => invoke<Playlist[]>("list_playlists");
export const createPlaylist = (name: string, kind?: string, rules?: string | null) =>
  invoke<number>("create_playlist", { name, kind, rules });
export const renamePlaylist = (id: number, name: string) =>
  invoke<void>("rename_playlist", { id, name });
export const deletePlaylist = (id: number) => invoke<void>("delete_playlist", { id });
export const setPlaylistPinned = (id: number, pinned: boolean) =>
  invoke<void>("set_playlist_pinned", { id, pinned });
export const getPlaylistTracks = (id: number) => invoke<Track[]>("get_playlist_tracks", { id });
export const addTracksToPlaylist = (id: number, trackIds: number[]) =>
  invoke<void>("add_tracks_to_playlist", { id, trackIds });
export const removeTrackFromPlaylist = (playlistId: number, trackId: number) =>
  invoke<void>("remove_track_from_playlist", { playlistId, trackId });
export const reorderPlaylist = (id: number, orderedIds: number[]) =>
  invoke<void>("reorder_playlist", { id, orderedIds });
export const updateSmartRules = (id: number, rules: string) =>
  invoke<void>("update_smart_rules", { id, rules });
export const exportPlaylist = (id: number, format: string) =>
  invoke<string | null>("export_playlist", { id, format });
export const importPlaylist = () => invoke<number | null>("import_playlist");

// ---------------------------------------------------------------------------
// Artwork, settings, system
// ---------------------------------------------------------------------------

const artworkCache = new Map<string, string | null>();

export async function artwork(hash: string | null): Promise<string | null> {
  if (!hash) return null;
  const cached = artworkCache.get(hash);
  if (cached !== undefined) return cached;
  const url = await invoke<string | null>("get_artwork", { hash });
  artworkCache.set(hash, url);
  return url;
}

export const getSettings = () => invoke<Settings>("get_settings");
export const updateSettings = (settings: Settings) =>
  invoke<void>("update_settings", { settings });
export const getAudioDevices = () => invoke<string[]>("get_audio_devices");
export const setMiniPlayer = (enabled: boolean) =>
  invoke<void>("set_mini_player", { enabled });
export const importPaths = (paths: string[]) => invoke<ImportResult>("import_paths", { paths });
export const quitApp = () => invoke<void>("quit_app");
export const notify = (title: string, body: string) =>
  invoke<void>("notify_now", { title, body });

// ---------------------------------------------------------------------------
// Events
// ---------------------------------------------------------------------------

export interface EventMap {
  "player://state": PlaybackSnapshot;
  "player://position": { positionMs: number };
  "scan://progress": ScanProgress;
  "scan://done": ScanStats;
  "library://changed": void;
  "toast://show": { message: string; kind: string };
  "ui://mini": boolean;
  "settings://changed": Settings;
}

export async function onEvent<K extends keyof EventMap>(
  event: K,
  handler: (payload: EventMap[K]) => void,
): Promise<UnlistenFn> {
  return listen<EventMap[K]>(event, (e) => handler(e.payload));
}
