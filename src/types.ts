// Mirrors the serde models in harmonia-core and the Tauri engine.

export type RepeatMode = "off" | "all" | "one";
export type SortField =
  | "title"
  | "artist"
  | "album"
  | "year"
  | "duration"
  | "dateAdded"
  | "playCount";

export interface Track {
  id: number;
  path: string;
  title: string;
  artist: string;
  album: string;
  albumArtist: string;
  genre: string;
  composer: string;
  year: number | null;
  trackNo: number | null;
  discNo: number | null;
  durationMs: number;
  bitrate: number | null;
  sampleRate: number | null;
  channels: number | null;
  format: string;
  folder: string;
  artHash: string | null;
  favorite: boolean;
  playCount: number;
  lastPlayed: number | null;
  dateAdded: number;
  replayGainDb: number | null;
  lyrics: string | null;
  lyricsSynced: string | null;
}

export interface Album {
  id: number;
  title: string;
  artist: string;
  year: number | null;
  artHash: string | null;
  trackCount: number;
  durationMs: number;
}

export interface Artist {
  id: number;
  name: string;
  artHash: string | null;
  trackCount: number;
  albumCount: number;
}

export interface Playlist {
  id: number;
  name: string;
  kind: "static" | "smart";
  rules: string | null;
  pinned: boolean;
  trackCount: number;
  createdAt: number;
  updatedAt: number;
}

export interface SmartRule {
  field: string;
  op: string;
  value: string;
}

export interface SmartRules {
  matchAll: boolean;
  rules: SmartRule[];
}

export interface SearchFilters {
  genre?: string | null;
  composer?: string | null;
  format?: string | null;
  folder?: string | null;
  yearMin?: number | null;
  yearMax?: number | null;
  bitrateMin?: number | null;
  durationMinMs?: number | null;
  durationMaxMs?: number | null;
}

export interface LibraryStats {
  tracks: number;
  albums: number;
  artists: number;
  playlists: number;
  totalDurationMs: number;
}

export interface Folder {
  path: string;
  mtime: number;
  enabled: boolean;
}

export interface ScanProgress {
  phase: string;
  current: number;
  total: number;
}

export interface ScanStats {
  found: number;
  added: number;
  updated: number;
  removed: number;
  failed: number;
  elapsedMs: number;
}

export interface Settings {
  theme: "system" | "light" | "dark";
  accent: string;
  volume: number;
  shuffle: boolean;
  repeat: RepeatMode;
  crossfadeSeconds: number;
  replayGain: boolean;
  playbackSpeed: number;
  audioDevice: string | null;
  language: string;
  libraryFolders: string[];
  resumeLastSession: boolean;
  eqEnabled: boolean;
  eqGains: number[];
  bassBoostDb: number;
  balance: number;
  mono: boolean;
  cacheSizeMb: number;
  miniPlayer: boolean;
}

export interface SleepTimerUi {
  kind: "off" | "minutes" | "endOfTrack" | "endOfAlbum";
  remainingSecs: number | null;
}

export interface PlaybackSnapshot {
  playing: boolean;
  current: Track | null;
  positionMs: number;
  durationMs: number;
  volume: number;
  shuffle: boolean;
  repeat: RepeatMode;
  speed: number;
  queue: Track[];
  queueIndex: number;
  queueTotal: number;
  sleepTimer: SleepTimerUi;
}

export interface LyricsData {
  plain: string | null;
  synced: string | null;
}

export interface ImportResult {
  added: number;
  failed: number;
}

export interface Toast {
  id: number;
  message: string;
  kind: "info" | "success" | "error";
}

export interface ResumeInfo {
  trackId: number;
  positionMs: number;
}

export const EQ_BANDS = [
  60, 170, 310, 600, 1000, 3000, 6000, 12000, 14000, 16000,
];
