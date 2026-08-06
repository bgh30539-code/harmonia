//! SQLite persistence layer for the music library.
//!
//! The database stores tracks, aggregated artists/albums, playlists,
//! library folders and settings. All access goes through a `Mutex<Connection>`
//! so the connection can be shared safely between the Tauri command thread,
//! the scan pipeline and the audio engine thread.
//!
//! Performance notes:
//! - WAL journal mode for concurrent readers while a scan writes.
//! - Prepared statements are re-prepared per call; SQLite's statement cache
//!   makes this cheap, and it keeps the API simple and borrow-safe.
//! - Bulk inserts happen inside explicit transactions (batches).
//! - Search uses an FTS5 index (`tracks_fts`) maintained by triggers, so
//!   full-text queries never scan the `tracks` table.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Mutex;

use rusqlite::{params, params_from_iter, types::Value, Connection, OptionalExtension};

use crate::error::{CoreError, CoreResult};
use crate::models::{
    Album, Artist, Folder, LibraryStats, Playlist, RepeatMode, SearchFilters, SortField, Track,
};
use crate::playlists::smart_playlist_where;
use crate::settings::Settings;
use crate::util::now_secs;

const SCHEMA: &str = r#"
PRAGMA journal_mode = WAL;
PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS schema_version (
    version INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS folders (
    path TEXT PRIMARY KEY,
    mtime INTEGER NOT NULL DEFAULT 0,
    enabled INTEGER NOT NULL DEFAULT 1
);

CREATE TABLE IF NOT EXISTS tracks (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    path TEXT NOT NULL UNIQUE,
    title TEXT NOT NULL DEFAULT '',
    artist TEXT NOT NULL DEFAULT '',
    album TEXT NOT NULL DEFAULT '',
    album_artist TEXT NOT NULL DEFAULT '',
    genre TEXT NOT NULL DEFAULT '',
    composer TEXT NOT NULL DEFAULT '',
    year INTEGER,
    track_no INTEGER,
    disc_no INTEGER,
    duration_ms INTEGER NOT NULL DEFAULT 0,
    bitrate INTEGER,
    sample_rate INTEGER,
    channels INTEGER,
    format TEXT NOT NULL DEFAULT '',
    folder TEXT NOT NULL DEFAULT '',
    file_size INTEGER NOT NULL DEFAULT 0,
    mtime INTEGER NOT NULL DEFAULT 0,
    art_hash TEXT,
    date_added INTEGER NOT NULL DEFAULT 0,
    last_played INTEGER,
    play_count INTEGER NOT NULL DEFAULT 0,
    favorite INTEGER NOT NULL DEFAULT 0,
    replay_gain_db REAL,
    lyrics TEXT,
    lyrics_synced TEXT
);

CREATE INDEX IF NOT EXISTS idx_tracks_artist ON tracks(artist);
CREATE INDEX IF NOT EXISTS idx_tracks_album ON tracks(album);
CREATE INDEX IF NOT EXISTS idx_tracks_genre ON tracks(genre);
CREATE INDEX IF NOT EXISTS idx_tracks_year ON tracks(year);
CREATE INDEX IF NOT EXISTS idx_tracks_title ON tracks(title);
CREATE INDEX IF NOT EXISTS idx_tracks_favorite ON tracks(favorite);
CREATE INDEX IF NOT EXISTS idx_tracks_last_played ON tracks(last_played);
CREATE INDEX IF NOT EXISTS idx_tracks_play_count ON tracks(play_count);
CREATE INDEX IF NOT EXISTS idx_tracks_folder ON tracks(folder);

-- FTS5 index backing full-text search. Triggers keep it in sync with every
-- write to the indexed columns (insert, delete, and targeted updates); the
-- trigger on UPDATE only fires when one of the indexed columns actually
-- changes, so favorite/play-count tweaks never touch the index.
-- Databases created before this index existed are backfilled once in
-- init_schema (marker key `fts_indexed` in the settings table).
CREATE VIRTUAL TABLE IF NOT EXISTS tracks_fts USING fts5(
    title, artist, album, album_artist, genre, composer, format,
    tokenize = 'unicode61'
);

CREATE TRIGGER IF NOT EXISTS tracks_fts_ai AFTER INSERT ON tracks BEGIN
    INSERT INTO tracks_fts(rowid, title, artist, album, album_artist, genre, composer, format)
    VALUES (new.id, new.title, new.artist, new.album, new.album_artist, new.genre, new.composer, new.format);
END;

CREATE TRIGGER IF NOT EXISTS tracks_fts_ad AFTER DELETE ON tracks BEGIN
    DELETE FROM tracks_fts WHERE rowid = old.id;
END;

CREATE TRIGGER IF NOT EXISTS tracks_fts_au AFTER UPDATE OF
    title, artist, album, album_artist, genre, composer, format ON tracks BEGIN
    DELETE FROM tracks_fts WHERE rowid = old.id;
    INSERT INTO tracks_fts(rowid, title, artist, album, album_artist, genre, composer, format)
    VALUES (new.id, new.title, new.artist, new.album, new.album_artist, new.genre, new.composer, new.format);
END;

CREATE TABLE IF NOT EXISTS playlists (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    kind TEXT NOT NULL DEFAULT 'static',
    rules TEXT,
    pinned INTEGER NOT NULL DEFAULT 0,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS playlist_tracks (
    playlist_id INTEGER NOT NULL REFERENCES playlists(id) ON DELETE CASCADE,
    track_id INTEGER NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
    position INTEGER NOT NULL,
    added_at INTEGER NOT NULL,
    PRIMARY KEY (playlist_id, track_id)
);

CREATE TABLE IF NOT EXISTS settings (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
"#;

const TRACK_COLUMNS: &str = "id, path, title, artist, album, album_artist, genre, composer, \
     year, track_no, disc_no, duration_ms, bitrate, sample_rate, channels, format, folder, \
     art_hash, favorite, play_count, last_played, date_added, replay_gain_db, lyrics, lyrics_synced";

/// Payload used to insert or refresh a track row.
#[derive(Debug, Clone)]
pub struct TrackUpsert {
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
    pub format: String,
    pub folder: String,
    pub file_size: i64,
    pub mtime: i64,
    pub art_hash: Option<String>,
    pub replay_gain_db: Option<f32>,
    pub lyrics: Option<String>,
    pub lyrics_synced: Option<String>,
}

/// Neutralises FTS5 query syntax in a raw user term so it is treated as a
/// literal phrase. Double quotes are doubled (the FTS5 escape rule) and line
/// breaks are stripped (not allowed inside quoted strings).
fn fts_escape(term: &str) -> String {
    term.replace('"', "\"\"").replace(['\n', '\r'], " ")
}

fn track_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Track> {
    Ok(Track {
        id: row.get("id")?,
        path: row.get("path")?,
        title: row.get("title")?,
        artist: row.get("artist")?,
        album: row.get("album")?,
        album_artist: row.get("album_artist")?,
        genre: row.get("genre")?,
        composer: row.get("composer")?,
        year: row.get("year")?,
        track_no: row.get("track_no")?,
        disc_no: row.get("disc_no")?,
        duration_ms: row.get("duration_ms")?,
        bitrate: row.get("bitrate")?,
        sample_rate: row.get("sample_rate")?,
        channels: row.get("channels")?,
        format: row.get("format")?,
        folder: row.get("folder")?,
        art_hash: row.get("art_hash")?,
        favorite: row.get::<_, i64>("favorite")? != 0,
        play_count: row.get("play_count")?,
        last_played: row.get("last_played")?,
        date_added: row.get("date_added")?,
        replay_gain_db: row.get("replay_gain_db")?,
        lyrics: row.get("lyrics")?,
        lyrics_synced: row.get("lyrics_synced")?,
    })
}

/// Wrapper around a SQLite connection that is safe to share between threads.
pub struct Database {
    conn: Mutex<Connection>,
}

impl Database {
    /// Opens (creating if needed) the database at `path`.
    pub fn open(path: &Path) -> CoreResult<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(path)?;
        Self::from_conn(conn)
    }

    /// In-memory database, used by tests.
    pub fn open_in_memory() -> CoreResult<Self> {
        Self::from_conn(Connection::open_in_memory()?)
    }

    fn from_conn(conn: Connection) -> CoreResult<Self> {
        let db = Self {
            conn: Mutex::new(conn),
        };
        db.init_schema()?;
        Ok(db)
    }

    fn init_schema(&self) -> CoreResult<()> {
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        conn.execute_batch(SCHEMA)?;
        // Databases created before the FTS5 index existed need a one-time
        // backfill. New writes stay in sync via the triggers in SCHEMA, so
        // this only ever needs to run once per database.
        let indexed: i64 = conn.query_row(
            "SELECT COUNT(*) FROM settings WHERE key = 'fts_indexed'",
            [],
            |r| r.get(0),
        )?;
        if indexed == 0 {
            // Run the backfill and the marker write atomically: a crash in
            // between would leave the index populated without the marker, and
            // the next open would re-index every row (duplicate postings).
            conn.execute_batch(
                "BEGIN;\n\
                 INSERT INTO tracks_fts(rowid, title, artist, album, album_artist, genre, composer, format)\n\
                 SELECT id, title, artist, album, album_artist, genre, composer, format FROM tracks;\n\
                 INSERT INTO settings(key, value) VALUES ('fts_indexed', '1');\n\
                 COMMIT;",
            )?;
        }
        Ok(())
    }

    /// A poisoned mutex (a panicked thread died while holding the lock) must
    /// never take the whole application down with it — recover the lock and
    /// continue. The connection itself stays usable.
    fn conn(&self) -> std::sync::MutexGuard<'_, Connection> {
        self.conn.lock().unwrap_or_else(|e| e.into_inner())
    }

    // ------------------------------------------------------------------
    // Settings
    // ------------------------------------------------------------------

    pub fn get_setting(&self, key: &str) -> CoreResult<Option<String>> {
        let conn = self.conn();
        let value = conn
            .query_row(
                "SELECT value FROM settings WHERE key = ?1",
                params![key],
                |r| r.get::<_, String>(0),
            )
            .optional()?;
        Ok(value)
    }

    pub fn set_setting(&self, key: &str, value: &str) -> CoreResult<()> {
        let conn = self.conn();
        conn.execute(
            "INSERT INTO settings (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
        Ok(())
    }

    pub fn all_settings(&self) -> CoreResult<HashMap<String, String>> {
        let conn = self.conn();
        let mut stmt = conn.prepare("SELECT key, value FROM settings")?;
        let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
        let mut out = HashMap::new();
        for row in rows {
            let (k, v) = row?;
            out.insert(k, v);
        }
        Ok(out)
    }

    /// Loads the persisted [`Settings`] blob.
    ///
    /// Corruption protection: if the stored JSON is missing or unparseable
    /// (a crash during a previous write, a partial upgrade, manual edits), the
    /// settings fall back to defaults **and** `library_folders` is re-seeded
    /// from the `folders` table — the authoritative list of library roots.
    /// This guarantees a corrupted settings row never silently forgets the
    /// user's music folders.
    pub fn load_settings(&self) -> CoreResult<Settings> {
        let mut settings: Settings = self
            .get_setting("settings")?
            .as_deref()
            .and_then(|raw| serde_json::from_str(raw).ok())
            .unwrap_or_default();
        if settings.library_folders.is_empty() {
            settings.library_folders = self.list_folders()?.into_iter().map(|f| f.path).collect();
        }
        Ok(settings)
    }

    /// Serializes and stores the settings blob in a single atomic SQL write.
    pub fn save_settings(&self, settings: &Settings) -> CoreResult<()> {
        let json = serde_json::to_string(settings)?;
        self.set_setting("settings", &json)
    }

    // ------------------------------------------------------------------
    // Library folders
    // ------------------------------------------------------------------

    pub fn add_folder(&self, path: &str) -> CoreResult<()> {
        let conn = self.conn();
        conn.execute(
            "INSERT OR IGNORE INTO folders (path, mtime, enabled) VALUES (?1, 0, 1)",
            params![path],
        )?;
        Ok(())
    }

    pub fn remove_folder(&self, path: &str) -> CoreResult<()> {
        let conn = self.conn();
        conn.execute("DELETE FROM folders WHERE path = ?1", params![path])?;
        conn.execute(
            "DELETE FROM tracks WHERE folder = ?1 OR folder LIKE ?2",
            params![path, format!("{}/%", path.trim_end_matches('/'))],
        )?;
        Ok(())
    }

    pub fn list_folders(&self) -> CoreResult<Vec<Folder>> {
        let conn = self.conn();
        let mut stmt = conn.prepare("SELECT path, mtime, enabled FROM folders ORDER BY path")?;
        let rows = stmt.query_map([], |r| {
            Ok(Folder {
                path: r.get(0)?,
                mtime: r.get(1)?,
                enabled: r.get::<_, i64>(2)? != 0,
            })
        })?;
        let mut folders = Vec::new();
        for row in rows {
            folders.push(row?);
        }
        Ok(folders)
    }

    pub fn update_folder_mtime(&self, path: &str, mtime: i64) -> CoreResult<()> {
        let conn = self.conn();
        conn.execute(
            "UPDATE folders SET mtime = ?1 WHERE path = ?2",
            params![mtime, path],
        )?;
        Ok(())
    }

    // ------------------------------------------------------------------
    // Tracks
    // ------------------------------------------------------------------

    /// Inserts or refreshes a track row. Play count, favorites and add dates
    /// are preserved across rescans.
    pub fn upsert_track(&self, u: &TrackUpsert) -> CoreResult<()> {
        let conn = self.conn();
        conn.execute(
            "INSERT INTO tracks (path, title, artist, album, album_artist, genre, composer, \
                year, track_no, disc_no, duration_ms, bitrate, sample_rate, channels, format, \
                folder, file_size, mtime, art_hash, replay_gain_db, lyrics, lyrics_synced, date_added) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, \
                ?17, ?18, ?19, ?20, ?21, ?22, strftime('%s','now')) \
             ON CONFLICT(path) DO UPDATE SET \
                title = excluded.title, artist = excluded.artist, album = excluded.album, \
                album_artist = excluded.album_artist, genre = excluded.genre, \
                composer = excluded.composer, year = excluded.year, track_no = excluded.track_no, \
                disc_no = excluded.disc_no, duration_ms = excluded.duration_ms, \
                bitrate = excluded.bitrate, sample_rate = excluded.sample_rate, \
                channels = excluded.channels, format = excluded.format, folder = excluded.folder, \
                file_size = excluded.file_size, mtime = excluded.mtime, art_hash = excluded.art_hash, \
                replay_gain_db = excluded.replay_gain_db, lyrics = excluded.lyrics, \
                lyrics_synced = excluded.lyrics_synced",
            params![
                u.path, u.title, u.artist, u.album, u.album_artist, u.genre, u.composer, u.year,
                u.track_no, u.disc_no, u.duration_ms, u.bitrate, u.sample_rate, u.channels,
                u.format, u.folder, u.file_size, u.mtime, u.art_hash, u.replay_gain_db,
                u.lyrics, u.lyrics_synced
            ],
        )?;
        Ok(())
    }

    /// Bulk upsert inside a single transaction. Returns the number of rows touched.
    pub fn upsert_tracks(&self, tracks: &[TrackUpsert]) -> CoreResult<usize> {
        let mut conn = self.conn();
        let tx = conn.transaction()?;
        {
            let mut stmt = tx.prepare(
                "INSERT INTO tracks (path, title, artist, album, album_artist, genre, composer, \
                    year, track_no, disc_no, duration_ms, bitrate, sample_rate, channels, format, \
                    folder, file_size, mtime, art_hash, replay_gain_db, lyrics, lyrics_synced, date_added) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, \
                    ?17, ?18, ?19, ?20, ?21, ?22, strftime('%s','now')) \
                 ON CONFLICT(path) DO UPDATE SET \
                    title = excluded.title, artist = excluded.artist, album = excluded.album, \
                    album_artist = excluded.album_artist, genre = excluded.genre, \
                    composer = excluded.composer, year = excluded.year, track_no = excluded.track_no, \
                    disc_no = excluded.disc_no, duration_ms = excluded.duration_ms, \
                    bitrate = excluded.bitrate, sample_rate = excluded.sample_rate, \
                    channels = excluded.channels, format = excluded.format, folder = excluded.folder, \
                    file_size = excluded.file_size, mtime = excluded.mtime, art_hash = excluded.art_hash, \
                    replay_gain_db = excluded.replay_gain_db, lyrics = excluded.lyrics, \
                    lyrics_synced = excluded.lyrics_synced",
            )?;
            for u in tracks {
                stmt.execute(params![
                    u.path,
                    u.title,
                    u.artist,
                    u.album,
                    u.album_artist,
                    u.genre,
                    u.composer,
                    u.year,
                    u.track_no,
                    u.disc_no,
                    u.duration_ms,
                    u.bitrate,
                    u.sample_rate,
                    u.channels,
                    u.format,
                    u.folder,
                    u.file_size,
                    u.mtime,
                    u.art_hash,
                    u.replay_gain_db,
                    u.lyrics,
                    u.lyrics_synced
                ])?;
            }
        }
        let n = tx.changes();
        tx.commit()?;
        Ok(n as usize)
    }

    pub fn get_track(&self, id: i64) -> CoreResult<Option<Track>> {
        let conn = self.conn();
        let sql = format!("SELECT {TRACK_COLUMNS} FROM tracks WHERE id = ?1");
        let track = conn
            .query_row(&sql, params![id], track_from_row)
            .optional()?;
        Ok(track)
    }

    pub fn get_track_by_path(&self, path: &str) -> CoreResult<Option<Track>> {
        let conn = self.conn();
        let sql = format!("SELECT {TRACK_COLUMNS} FROM tracks WHERE path = ?1");
        let track = conn
            .query_row(&sql, params![path], track_from_row)
            .optional()?;
        Ok(track)
    }

    /// Map of path -> (mtime, file_size) for incremental scanning.
    pub fn all_paths_with_mtime(&self) -> CoreResult<HashMap<String, (i64, i64)>> {
        let conn = self.conn();
        let mut stmt = conn.prepare("SELECT path, mtime, file_size FROM tracks")?;
        let rows = stmt.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                (r.get::<_, i64>(1)?, r.get::<_, i64>(2)?),
            ))
        })?;
        let mut map = HashMap::new();
        for row in rows {
            let (p, v) = row?;
            map.insert(p, v);
        }
        Ok(map)
    }

    /// Deletes every track whose folder lives under one of `roots` but whose
    /// path is not in `keep`. Uses a temporary table so it scales to libraries
    /// with hundreds of thousands of files.
    pub fn delete_tracks_not_in(
        &self,
        keep: &std::collections::HashSet<String>,
        roots: &[String],
    ) -> CoreResult<usize> {
        let mut conn = self.conn();
        let tx = conn.transaction()?;
        tx.execute_batch(
            "CREATE TEMP TABLE IF NOT EXISTS _keep (path TEXT PRIMARY KEY); DELETE FROM _keep;",
        )?;
        {
            let mut stmt = tx.prepare("INSERT INTO _keep (path) VALUES (?1)")?;
            for path in keep {
                stmt.execute(params![path])?;
            }
        }
        let mut conds: Vec<String> = Vec::new();
        let mut params: Vec<Value> = Vec::new();
        for root in roots {
            let root = root.trim_end_matches('/');
            conds.push("folder = ?".to_string());
            params.push(Value::Text(root.to_string()));
            conds.push("folder LIKE ?".to_string());
            params.push(Value::Text(format!("{root}/%")));
        }
        let where_clause = if conds.is_empty() {
            "0".to_string()
        } else {
            format!("({})", conds.join(" OR "))
        };
        let sql = format!(
            "DELETE FROM tracks WHERE path NOT IN (SELECT path FROM _keep) AND {where_clause}"
        );
        let n = tx.execute(&sql, params_from_iter(params.iter()))?;
        tx.commit()?;
        Ok(n)
    }

    pub fn delete_track_by_path(&self, path: &str) -> CoreResult<()> {
        let conn = self.conn();
        conn.execute("DELETE FROM tracks WHERE path = ?1", params![path])?;
        Ok(())
    }

    pub fn count_tracks(&self) -> CoreResult<i64> {
        let conn = self.conn();
        let n = conn.query_row("SELECT COUNT(*) FROM tracks", [], |r| r.get::<_, i64>(0))?;
        Ok(n)
    }

    pub fn get_tracks(
        &self,
        offset: i64,
        limit: i64,
        sort: SortField,
        desc: bool,
        folder: Option<&str>,
    ) -> CoreResult<Vec<Track>> {
        let conn = self.conn();
        let dir = if desc { "DESC" } else { "ASC" };
        let order = match sort {
            SortField::Title | SortField::Artist | SortField::Album => {
                format!("{} COLLATE NOCASE {dir}", sort.column())
            }
            _ => format!("{} {dir}", sort.column()),
        };
        let sql = if folder.is_some() {
            format!(
                "SELECT {TRACK_COLUMNS} FROM tracks WHERE folder = ?1 ORDER BY {order} LIMIT ?2 OFFSET ?3"
            )
        } else {
            format!("SELECT {TRACK_COLUMNS} FROM tracks ORDER BY {order} LIMIT ?1 OFFSET ?2")
        };
        let mut stmt = conn.prepare(&sql)?;
        let rows = if let Some(folder) = folder {
            stmt.query_map(params![folder, limit, offset], track_from_row)?
        } else {
            stmt.query_map(params![limit, offset], track_from_row)?
        };
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    pub fn get_tracks_by_ids(&self, ids: &[i64]) -> CoreResult<Vec<Track>> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let conn = self.conn();
        let placeholders = ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let sql = format!("SELECT {TRACK_COLUMNS} FROM tracks WHERE id IN ({placeholders})");
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(params_from_iter(ids.iter()), track_from_row)?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    pub fn set_favorite(&self, id: i64, favorite: bool) -> CoreResult<()> {
        let conn = self.conn();
        conn.execute(
            "UPDATE tracks SET favorite = ?1 WHERE id = ?2",
            params![favorite as i64, id],
        )?;
        Ok(())
    }

    pub fn mark_played(&self, id: i64) -> CoreResult<()> {
        let conn = self.conn();
        conn.execute(
            "UPDATE tracks SET play_count = play_count + 1, last_played = ?1 WHERE id = ?2",
            params![now_secs(), id],
        )?;
        Ok(())
    }

    pub fn get_favorites(&self) -> CoreResult<Vec<Track>> {
        let conn = self.conn();
        let sql = format!(
            "SELECT {TRACK_COLUMNS} FROM tracks WHERE favorite = 1 ORDER BY title COLLATE NOCASE"
        );
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map([], track_from_row)?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    pub fn recently_played(&self, limit: i64) -> CoreResult<Vec<Track>> {
        let conn = self.conn();
        let sql = format!(
            "SELECT {TRACK_COLUMNS} FROM tracks WHERE last_played IS NOT NULL \
             ORDER BY last_played DESC LIMIT ?1"
        );
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(params![limit], track_from_row)?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    pub fn most_played(&self, limit: i64) -> CoreResult<Vec<Track>> {
        let conn = self.conn();
        let sql = format!(
            "SELECT {TRACK_COLUMNS} FROM tracks WHERE play_count > 0 ORDER BY play_count DESC LIMIT ?1"
        );
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(params![limit], track_from_row)?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    /// Full-text search backed by the FTS5 index, combined with structured
    /// filters. Multi-word queries require every word to match somewhere in
    /// the track text fields. Each word is matched case-insensitively as a
    /// word prefix, so "roc" finds "Rock Anthem"; FTS5 syntax characters in
    /// the user input are neutralised by quoting.
    pub fn search(
        &self,
        query: &str,
        filters: &SearchFilters,
        limit: i64,
    ) -> CoreResult<Vec<Track>> {
        let mut conditions: Vec<String> = Vec::new();
        let mut params: Vec<Value> = Vec::new();

        let terms: Vec<&str> = query.split_whitespace().filter(|t| !t.is_empty()).collect();
        if !terms.is_empty() {
            // Every term must match (FTS5 implicit AND). Quoting neutralises
            // FTS5 query syntax; the trailing `*` turns each term into a word
            // prefix query, mirroring the common `%term%` LIKE use case while
            // using the index instead of a full table scan.
            let match_expr = terms
                .iter()
                .map(|t| format!("\"{}\"*", fts_escape(t)))
                .collect::<Vec<_>>()
                .join(" ");
            conditions
                .push("id IN (SELECT rowid FROM tracks_fts WHERE tracks_fts MATCH ?)".to_string());
            params.push(Value::Text(match_expr));
        }

        if let Some(genre) = &filters.genre {
            conditions.push("genre = ?".to_string());
            params.push(Value::Text(genre.clone()));
        }
        if let Some(composer) = &filters.composer {
            conditions.push("composer LIKE ?".to_string());
            params.push(Value::Text(format!(
                "%{}%",
                crate::util::escape_like(composer)
            )));
        }
        if let Some(format) = &filters.format {
            conditions.push("format = ?".to_string());
            params.push(Value::Text(format.to_lowercase()));
        }
        if let Some(folder) = &filters.folder {
            conditions.push("folder LIKE ?".to_string());
            params.push(Value::Text(format!(
                "%{}%",
                crate::util::escape_like(folder)
            )));
        }
        if let (Some(min), Some(max)) = (filters.year_min, filters.year_max) {
            conditions.push("year BETWEEN ? AND ?".to_string());
            params.push(Value::Integer(min));
            params.push(Value::Integer(max));
        } else if let Some(min) = filters.year_min {
            conditions.push("year >= ?".to_string());
            params.push(Value::Integer(min));
        } else if let Some(max) = filters.year_max {
            conditions.push("year <= ?".to_string());
            params.push(Value::Integer(max));
        }
        if let Some(min) = filters.bitrate_min {
            conditions.push("bitrate >= ?".to_string());
            params.push(Value::Integer(min));
        }
        if let (Some(min), Some(max)) = (filters.duration_min_ms, filters.duration_max_ms) {
            conditions.push("duration_ms BETWEEN ? AND ?".to_string());
            params.push(Value::Integer(min));
            params.push(Value::Integer(max));
        } else if let Some(min) = filters.duration_min_ms {
            conditions.push("duration_ms >= ?".to_string());
            params.push(Value::Integer(min));
        } else if let Some(max) = filters.duration_max_ms {
            conditions.push("duration_ms <= ?".to_string());
            params.push(Value::Integer(max));
        }

        let where_clause = if conditions.is_empty() {
            String::new()
        } else {
            format!("WHERE {}", conditions.join(" AND "))
        };
        params.push(Value::Integer(limit));

        let conn = self.conn();
        let sql = format!(
            "SELECT {TRACK_COLUMNS} FROM tracks {where_clause} \
             ORDER BY favorite DESC, play_count DESC, title COLLATE NOCASE LIMIT ?"
        );
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(params_from_iter(params.iter()), track_from_row)?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    // ------------------------------------------------------------------
    // Albums & artists
    // ------------------------------------------------------------------

    pub fn get_albums(&self) -> CoreResult<Vec<Album>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT MIN(id) AS id, album AS title, artist, MAX(year) AS year, \
                MIN(art_hash) AS art_hash, COUNT(*) AS track_count, SUM(duration_ms) AS duration_ms \
             FROM tracks GROUP BY album, artist \
             ORDER BY artist COLLATE NOCASE, album COLLATE NOCASE",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok(Album {
                id: r.get("id")?,
                title: r.get("title")?,
                artist: r.get("artist")?,
                year: r.get("year")?,
                art_hash: r.get("art_hash")?,
                track_count: r.get("track_count")?,
                duration_ms: r.get("duration_ms")?,
            })
        })?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    pub fn get_album_tracks(&self, title: &str, artist: &str) -> CoreResult<Vec<Track>> {
        let conn = self.conn();
        let sql = format!(
            "SELECT {TRACK_COLUMNS} FROM tracks WHERE album = ?1 AND artist = ?2 \
             ORDER BY disc_no, track_no, title COLLATE NOCASE"
        );
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(params![title, artist], track_from_row)?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    pub fn get_artists(&self) -> CoreResult<Vec<Artist>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT MIN(id) AS id, artist AS name, MIN(art_hash) AS art_hash, \
                COUNT(*) AS track_count, COUNT(DISTINCT album) AS album_count \
             FROM tracks GROUP BY artist ORDER BY artist COLLATE NOCASE",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok(Artist {
                id: r.get("id")?,
                name: r.get("name")?,
                art_hash: r.get("art_hash")?,
                track_count: r.get("track_count")?,
                album_count: r.get("album_count")?,
            })
        })?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    pub fn tracks_by_artist(&self, artist: &str) -> CoreResult<Vec<Track>> {
        let conn = self.conn();
        let sql = format!(
            "SELECT {TRACK_COLUMNS} FROM tracks WHERE artist = ?1 \
             ORDER BY album COLLATE NOCASE, disc_no, track_no"
        );
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(params![artist], track_from_row)?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    // ------------------------------------------------------------------
    // Playlists
    // ------------------------------------------------------------------

    pub fn list_playlists(&self) -> CoreResult<Vec<Playlist>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT p.id, p.name, p.kind, p.rules, p.pinned, p.created_at, p.updated_at, \
                (SELECT COUNT(*) FROM playlist_tracks pt WHERE pt.playlist_id = p.id) AS track_count \
             FROM playlists p ORDER BY p.pinned DESC, p.updated_at DESC",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok(Playlist {
                id: r.get(0)?,
                name: r.get(1)?,
                kind: r.get(2)?,
                rules: r.get(3)?,
                pinned: r.get::<_, i64>(4)? != 0,
                created_at: r.get(5)?,
                updated_at: r.get(6)?,
                track_count: r.get(7)?,
            })
        })?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    pub fn create_playlist(&self, name: &str, kind: &str, rules: Option<&str>) -> CoreResult<i64> {
        let conn = self.conn();
        let now = now_secs();
        conn.execute(
            "INSERT INTO playlists (name, kind, rules, pinned, created_at, updated_at) \
             VALUES (?1, ?2, ?3, 0, ?4, ?4)",
            params![name, kind, rules, now],
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn rename_playlist(&self, id: i64, name: &str) -> CoreResult<()> {
        let conn = self.conn();
        conn.execute(
            "UPDATE playlists SET name = ?1, updated_at = ?2 WHERE id = ?3",
            params![name, now_secs(), id],
        )?;
        Ok(())
    }

    pub fn delete_playlist(&self, id: i64) -> CoreResult<()> {
        let conn = self.conn();
        conn.execute("DELETE FROM playlists WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn set_playlist_pinned(&self, id: i64, pinned: bool) -> CoreResult<()> {
        let conn = self.conn();
        conn.execute(
            "UPDATE playlists SET pinned = ?1 WHERE id = ?2",
            params![pinned as i64, id],
        )?;
        Ok(())
    }

    pub fn update_playlist_rules(&self, id: i64, rules: &str) -> CoreResult<()> {
        let conn = self.conn();
        conn.execute(
            "UPDATE playlists SET rules = ?1, kind = 'smart', updated_at = ?2 WHERE id = ?3",
            params![rules, now_secs(), id],
        )?;
        Ok(())
    }

    pub fn get_playlist_tracks(&self, id: i64) -> CoreResult<Vec<Track>> {
        let conn = self.conn();
        // Prefix the columns so the JOIN is unambiguous.
        let cols: Vec<String> = TRACK_COLUMNS
            .split(", ")
            .map(|c| format!("t.{c}"))
            .collect();
        let sql = format!(
            "SELECT {} FROM playlist_tracks pt JOIN tracks t ON t.id = pt.track_id \
             WHERE pt.playlist_id = ?1 ORDER BY pt.position",
            cols.join(", ")
        );
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(params![id], track_from_row)?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    pub fn add_track_to_playlist(&self, playlist_id: i64, track_id: i64) -> CoreResult<()> {
        let conn = self.conn();
        conn.execute(
            "INSERT OR IGNORE INTO playlist_tracks (playlist_id, track_id, position, added_at) \
             VALUES (?1, ?2, (SELECT COALESCE(MAX(position), 0) + 1 FROM playlist_tracks WHERE playlist_id = ?1), ?3)",
            params![playlist_id, track_id, now_secs()],
        )?;
        conn.execute(
            "UPDATE playlists SET updated_at = ?1 WHERE id = ?2",
            params![now_secs(), playlist_id],
        )?;
        Ok(())
    }

    pub fn remove_track_from_playlist(&self, playlist_id: i64, track_id: i64) -> CoreResult<()> {
        let conn = self.conn();
        conn.execute(
            "DELETE FROM playlist_tracks WHERE playlist_id = ?1 AND track_id = ?2",
            params![playlist_id, track_id],
        )?;
        conn.execute(
            "UPDATE playlists SET updated_at = ?1 WHERE id = ?2",
            params![now_secs(), playlist_id],
        )?;
        Ok(())
    }

    /// Reorders a playlist to match `ordered_track_ids`.
    pub fn reorder_playlist(&self, playlist_id: i64, ordered_track_ids: &[i64]) -> CoreResult<()> {
        let mut conn = self.conn();
        let tx = conn.transaction()?;
        {
            let mut stmt = tx.prepare(
                "UPDATE playlist_tracks SET position = ?1 WHERE playlist_id = ?2 AND track_id = ?3",
            )?;
            for (pos, track_id) in ordered_track_ids.iter().enumerate() {
                stmt.execute(params![pos as i64, playlist_id, track_id])?;
            }
        }
        tx.execute(
            "UPDATE playlists SET updated_at = ?1 WHERE id = ?2",
            params![now_secs(), playlist_id],
        )?;
        tx.commit()?;
        Ok(())
    }

    /// Evaluates a smart playlist against the current library.
    pub fn smart_playlist_tracks(&self, rules_json: &str) -> CoreResult<Vec<Track>> {
        let rules: crate::models::SmartRules = serde_json::from_str(rules_json)
            .map_err(|e| CoreError::Invalid(format!("invalid smart playlist rules: {e}")))?;
        let (where_sql, params) = smart_playlist_where(&rules)?;
        let conn = self.conn();
        let sql = format!(
            "SELECT {TRACK_COLUMNS} FROM tracks WHERE {where_sql} \
             ORDER BY artist COLLATE NOCASE, album COLLATE NOCASE, disc_no, track_no"
        );
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(params_from_iter(params.iter()), track_from_row)?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    // ------------------------------------------------------------------
    // Stats
    // ------------------------------------------------------------------

    pub fn library_stats(&self) -> CoreResult<LibraryStats> {
        let conn = self.conn();
        let (tracks, albums, artists, playlists, total) = conn.query_row(
            "SELECT
                (SELECT COUNT(*) FROM tracks),
                (SELECT COUNT(*) FROM (SELECT 1 FROM tracks GROUP BY album, artist)),
                (SELECT COUNT(*) FROM (SELECT 1 FROM tracks GROUP BY artist)),
                (SELECT COUNT(*) FROM playlists),
                (SELECT COALESCE(SUM(duration_ms), 0) FROM tracks)",
            [],
            |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, i64>(1)?,
                    r.get::<_, i64>(2)?,
                    r.get::<_, i64>(3)?,
                    r.get::<_, i64>(4)?,
                ))
            },
        )?;
        Ok(LibraryStats {
            tracks,
            albums,
            artists,
            playlists,
            total_duration_ms: total,
        })
    }

    /// Loads a [`RepeatMode`] stored by its serde name.
    pub fn get_repeat_mode(&self) -> CoreResult<RepeatMode> {
        Ok(self
            .get_setting("repeat")?
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default())
    }

    pub fn set_repeat_mode(&self, mode: RepeatMode) -> CoreResult<()> {
        self.set_setting("repeat", &serde_json::to_string(&mode)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn upsert(db: &Database, path: &str, title: &str, artist: &str, album: &str, genre: &str) {
        db.upsert_track(&TrackUpsert {
            path: path.to_string(),
            title: title.to_string(),
            artist: artist.to_string(),
            album: album.to_string(),
            album_artist: artist.to_string(),
            genre: genre.to_string(),
            composer: String::new(),
            year: Some(2020),
            track_no: Some(1),
            disc_no: Some(1),
            duration_ms: 180_000,
            bitrate: Some(320),
            sample_rate: Some(44_100),
            channels: Some(2),
            format: "mp3".to_string(),
            folder: "/music".to_string(),
            file_size: 1000,
            mtime: 1,
            art_hash: None,
            replay_gain_db: None,
            lyrics: None,
            lyrics_synced: None,
        })
        .unwrap();
    }

    #[test]
    fn upsert_is_idempotent_and_preserves_stats() {
        let db = Database::open_in_memory().unwrap();
        upsert(&db, "/music/a.mp3", "A", "Artist", "Album", "Rock");
        db.mark_played(db.get_track_by_path("/music/a.mp3").unwrap().unwrap().id)
            .unwrap();
        upsert(&db, "/music/a.mp3", "A", "Artist", "Album", "Rock");
        let t = db.get_track_by_path("/music/a.mp3").unwrap().unwrap();
        assert_eq!(t.play_count, 1);
        assert_eq!(db.count_tracks().unwrap(), 1);
    }

    #[test]
    fn search_matches_and_respects_filters() {
        let db = Database::open_in_memory().unwrap();
        upsert(
            &db,
            "/m/r1.mp3",
            "Rock Anthem",
            "The Band",
            "Greatest",
            "Rock",
        );
        upsert(
            &db,
            "/m/j1.mp3",
            "Jazz Mood",
            "Cool Cats",
            "Late Night",
            "Jazz",
        );
        let found = db.search("rock", &SearchFilters::default(), 50).unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].title, "Rock Anthem");

        let filters = SearchFilters {
            genre: Some("Jazz".into()),
            ..Default::default()
        };
        let found = db.search("", &filters, 50).unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].title, "Jazz Mood");
    }

    #[test]
    fn fts_index_stays_in_sync_across_writes() {
        let db = Database::open_in_memory().unwrap();
        upsert(
            &db,
            "/m/r1.mp3",
            "Rock Anthem",
            "The Band",
            "Greatest",
            "Pop",
        );
        upsert(
            &db,
            "/m/j1.mp3",
            "Jazz Mood",
            "Cool Cats",
            "Late Night",
            "Jazz",
        );

        // Title, artist, album and genre columns are all searchable.
        assert_eq!(
            db.search("rock", &SearchFilters::default(), 50)
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            db.search("anthem", &SearchFilters::default(), 50)
                .unwrap()
                .len(),
            1
        );
        // Multi-word AND across artist tokens.
        assert_eq!(
            db.search("the band", &SearchFilters::default(), 50)
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            db.search("late night", &SearchFilters::default(), 50)
                .unwrap()
                .len(),
            1
        );
        // Every word must match somewhere: one good, one bad word => no hit.
        assert_eq!(
            db.search("rock jazz", &SearchFilters::default(), 50)
                .unwrap()
                .len(),
            0
        );

        // Writes to non-indexed columns must NOT touch the index (the AU
        // trigger only fires on UPDATE OF the indexed columns).
        let jazz_id = db.get_track_by_path("/m/j1.mp3").unwrap().unwrap().id;
        db.set_favorite(jazz_id, true).unwrap();
        db.mark_played(jazz_id).unwrap();
        assert_eq!(
            db.search("jazz", &SearchFilters::default(), 50)
                .unwrap()
                .len(),
            1
        );

        // Renaming a track must move its index entry (UPDATE trigger). The
        // other columns (artist/album/genre) keep no trace of "rock".
        upsert(
            &db,
            "/m/r1.mp3",
            "Country Roads",
            "The Band",
            "Greatest",
            "Pop",
        );
        assert_eq!(
            db.search("rock", &SearchFilters::default(), 50)
                .unwrap()
                .len(),
            0
        );
        assert_eq!(
            db.search("country", &SearchFilters::default(), 50)
                .unwrap()
                .len(),
            1
        );

        // Deleting a track must remove its index entry (DELETE trigger).
        db.delete_track_by_path("/m/r1.mp3").unwrap();
        assert_eq!(
            db.search("country", &SearchFilters::default(), 50)
                .unwrap()
                .len(),
            0
        );
    }

    #[test]
    fn fts_prefix_and_special_character_matching() {
        let db = Database::open_in_memory().unwrap();
        upsert(
            &db,
            "/m/u.mp3",
            "Rock \"Unplugged\" (Live)",
            "A & B",
            "Session",
            "Rock",
        );
        // Word-prefix matching mirrors the old %term% LIKE for prefix inputs.
        assert_eq!(
            db.search("roc", &SearchFilters::default(), 50)
                .unwrap()
                .len(),
            1
        );
        // Quotes and parentheses in user input are treated literally.
        assert_eq!(
            db.search("unplugged", &SearchFilters::default(), 50)
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            db.search("rock live", &SearchFilters::default(), 50)
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            db.search("rock jazz", &SearchFilters::default(), 50)
                .unwrap()
                .len(),
            0
        );
    }

    #[test]
    fn fts_backfills_existing_databases_on_upgrade() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("old.db");
        {
            // A database created before the FTS5 index existed: tracks table
            // with data, no tracks_fts table, no backfill marker.
            let conn = Connection::open(&db_path).unwrap();
            conn.execute_batch(
                "CREATE TABLE tracks (\
                    id INTEGER PRIMARY KEY AUTOINCREMENT,\
                    path TEXT NOT NULL UNIQUE, title TEXT NOT NULL DEFAULT '',\
                    artist TEXT NOT NULL DEFAULT '', album TEXT NOT NULL DEFAULT '',\
                    album_artist TEXT NOT NULL DEFAULT '', genre TEXT NOT NULL DEFAULT '',\
                    composer TEXT NOT NULL DEFAULT '', year INTEGER, track_no INTEGER,\
                    disc_no INTEGER, duration_ms INTEGER NOT NULL DEFAULT 0, bitrate INTEGER,\
                    sample_rate INTEGER, channels INTEGER, format TEXT NOT NULL DEFAULT '',\
                    folder TEXT NOT NULL DEFAULT '', file_size INTEGER NOT NULL DEFAULT 0,\
                    mtime INTEGER NOT NULL DEFAULT 0, art_hash TEXT,\
                    date_added INTEGER NOT NULL DEFAULT 0, last_played INTEGER,\
                    play_count INTEGER NOT NULL DEFAULT 0, favorite INTEGER NOT NULL DEFAULT 0,\
                    replay_gain_db REAL, lyrics TEXT, lyrics_synced TEXT\
                );\
                CREATE TABLE settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);\
                INSERT INTO tracks (path, title, artist, album, genre, format, folder, file_size, mtime, date_added)\
                VALUES ('/m/old.mp3', 'Oldies Gold', 'Vintage', 'Retro', 'Pop', 'mp3', '/m', 1, 1, 1);",
            )
            .unwrap();
        }
        // Opening with the current code must create the index and backfill it.
        let db = Database::open(&db_path).unwrap();
        assert_eq!(
            db.search("oldies", &SearchFilters::default(), 50)
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            db.search("vintage", &SearchFilters::default(), 50)
                .unwrap()
                .len(),
            1
        );
    }

    #[test]
    fn albums_and_artists_aggregate() {
        let db = Database::open_in_memory().unwrap();
        upsert(&db, "/m/a1.mp3", "Song One", "Artist A", "Album X", "Rock");
        upsert(&db, "/m/a2.mp3", "Song Two", "Artist A", "Album X", "Rock");
        upsert(&db, "/m/b1.mp3", "Lonely", "Artist B", "Album Y", "Jazz");

        let albums = db.get_albums().unwrap();
        assert_eq!(albums.len(), 2);
        let x = albums.iter().find(|a| a.title == "Album X").unwrap();
        assert_eq!(x.track_count, 2);

        let artists = db.get_artists().unwrap();
        assert_eq!(artists.len(), 2);
        let a = artists.iter().find(|a| a.name == "Artist A").unwrap();
        assert_eq!(a.album_count, 1);
        assert_eq!(a.track_count, 2);
    }

    #[test]
    fn playlists_crud_and_order() {
        let db = Database::open_in_memory().unwrap();
        upsert(&db, "/m/a.mp3", "A", "X", "X1", "Rock");
        upsert(&db, "/m/b.mp3", "B", "X", "X1", "Rock");
        let t1 = db.get_track_by_path("/m/a.mp3").unwrap().unwrap().id;
        let t2 = db.get_track_by_path("/m/b.mp3").unwrap().unwrap().id;

        let pid = db.create_playlist("Roadtrip", "static", None).unwrap();
        db.add_track_to_playlist(pid, t1).unwrap();
        db.add_track_to_playlist(pid, t2).unwrap();
        db.add_track_to_playlist(pid, t1).unwrap(); // duplicate ignored

        let tracks = db.get_playlist_tracks(pid).unwrap();
        assert_eq!(tracks.len(), 2);

        db.reorder_playlist(pid, &[t2, t1]).unwrap();
        let tracks = db.get_playlist_tracks(pid).unwrap();
        assert_eq!(tracks[0].id, t2);
        assert_eq!(tracks[1].id, t1);

        db.remove_track_from_playlist(pid, t2).unwrap();
        assert_eq!(db.get_playlist_tracks(pid).unwrap().len(), 1);

        let playlists = db.list_playlists().unwrap();
        assert_eq!(playlists.len(), 1);
        assert_eq!(playlists[0].track_count, 1);
    }

    #[test]
    fn favorites_recent_most_played() {
        let db = Database::open_in_memory().unwrap();
        upsert(&db, "/m/a.mp3", "A", "X", "X1", "Rock");
        upsert(&db, "/m/b.mp3", "B", "X", "X1", "Rock");
        let a = db.get_track_by_path("/m/a.mp3").unwrap().unwrap().id;
        let b = db.get_track_by_path("/m/b.mp3").unwrap().unwrap().id;

        db.set_favorite(a, true).unwrap();
        assert_eq!(db.get_favorites().unwrap().len(), 1);

        db.mark_played(b).unwrap();
        db.mark_played(b).unwrap();
        let recent = db.recently_played(10).unwrap();
        assert_eq!(recent[0].id, b);
        let most = db.most_played(10).unwrap();
        assert_eq!(most[0].play_count, 2);
    }

    #[test]
    fn smart_playlist_filters() {
        let db = Database::open_in_memory().unwrap();
        upsert(&db, "/m/r.mp3", "R", "A", "Al", "Rock");
        upsert(&db, "/m/j.mp3", "J", "B", "Al", "Jazz");
        let rules = serde_json::json!({
            "matchAll": true,
            "rules": [{ "field": "genre", "op": "eq", "value": "Rock" }]
        })
        .to_string();
        let tracks = db.smart_playlist_tracks(&rules).unwrap();
        assert_eq!(tracks.len(), 1);
        assert_eq!(tracks[0].title, "R");
    }

    #[test]
    fn delete_tracks_not_in_removes_stale_rows() {
        let db = Database::open_in_memory().unwrap();
        upsert(&db, "/music/stale.mp3", "Stale", "A", "Al", "Rock");
        upsert(&db, "/other/keep.mp3", "Keep", "B", "Al2", "Jazz");
        let keep = std::collections::HashSet::from(["/other/keep.mp3".to_string()]);
        let removed = db
            .delete_tracks_not_in(&keep, &["/music".to_string()])
            .unwrap();
        assert_eq!(removed, 1);
        assert!(db.get_track_by_path("/music/stale.mp3").unwrap().is_none());
        assert!(db.get_track_by_path("/other/keep.mp3").unwrap().is_some());
    }

    #[test]
    fn settings_roundtrip() {
        let db = Database::open_in_memory().unwrap();
        db.set_setting("theme", "dark").unwrap();
        assert_eq!(db.get_setting("theme").unwrap().as_deref(), Some("dark"));
        db.set_repeat_mode(RepeatMode::All).unwrap();
        assert_eq!(db.get_repeat_mode().unwrap(), RepeatMode::All);
    }

    #[test]
    fn settings_save_load_roundtrip() {
        let db = Database::open_in_memory().unwrap();
        let settings = Settings {
            volume: 0.42,
            library_folders: vec!["/music/a".into(), "/music/b".into()],
            close_to_tray: false,
            ..Settings::default()
        };
        db.save_settings(&settings).unwrap();
        let loaded = db.load_settings().unwrap();
        assert_eq!(loaded.volume, 0.42);
        assert_eq!(loaded.library_folders, settings.library_folders);
        assert!(!loaded.close_to_tray);
    }

    #[test]
    fn corrupted_settings_fall_back_to_folders_table() {
        let db = Database::open_in_memory().unwrap();
        // The library roots live in the folders table — the settings blob must
        // never be the only copy.
        db.add_folder("/music/a").unwrap();
        db.add_folder("/music/b").unwrap();
        // Simulate a truncated/corrupted settings row from a crashed write.
        db.set_setting("settings", "{\"theme\": \"dark\", \"volu")
            .unwrap();
        let settings = db.load_settings().unwrap();
        assert_eq!(
            settings.library_folders,
            vec!["/music/a".to_string(), "/music/b".to_string()]
        );
        // And a fully missing row behaves the same way.
        let db2 = Database::open_in_memory().unwrap();
        db2.add_folder("/muzak").unwrap();
        let settings = db2.load_settings().unwrap();
        assert_eq!(settings.library_folders, vec!["/muzak".to_string()]);
    }

    #[test]
    fn unknown_settings_fields_are_tolerated() {
        let db = Database::open_in_memory().unwrap();
        // Older or newer versions may carry extra keys; serde must ignore them.
        db.set_setting(
            "settings",
            r#"{"volume":0.5,"libraryFolders":["/m"],"someFutureField":true}"#,
        )
        .unwrap();
        let settings = db.load_settings().unwrap();
        assert_eq!(settings.volume, 0.5);
        assert_eq!(settings.library_folders, vec!["/m".to_string()]);
        assert!(settings.close_to_tray); // default for non-Windows
    }
}
