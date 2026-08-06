# Architecture

Harmonia is a two-crate Rust workspace with a React/Tauri frontend. The most
important architectural decision is a **strict separation between the core
library logic and the desktop/audio layer**.

```
┌────────────────────────────────────────────────────────────┐
│                    React frontend (src/)                   │
│   store.tsx · views/ · components/ · styles/ · i18n/       │
└───────────────────────────┬────────────────────────────────┘
                            │ IPC (Tauri commands, events)
┌───────────────────────────┴────────────────────────────────┐
│               src-tauri (Tauri 2 + rodio)                  │
│   commands.rs  tray.rs  paths.rs  state.rs                 │
│   engine.rs (audio thread)  dsp_source.rs                  │
│   Linux: WebKitGTK · Windows: WebView2                     │
├────────────────────────────────────────────────────────────┤
│              harmonia-core (pure Rust, no UI)              │
│   db.rs  metadata.rs  library.rs  playlists.rs  watcher.rs │
│   dsp.rs  settings.rs  models.rs  util.rs  error.rs        │
└────────────────────────────────────────────────────────────┘
```

The shell is packaged per platform: on Linux as a `.deb` and AppImage (plus a
raw binary), and on Windows as an NSIS installer (`Harmonia_*_x64-setup.exe`)
and a portable `harmonia.exe`. The Windows installer adds Start Menu and
desktop shortcuts, an uninstaller, and registers MP3/FLAC/OGG/WAV/M4A file
associations so double-clicking a music file opens Harmonia. Desktop
integration (window geometry persistence, last-section restore, clean
shutdown, notifications) is shared across platforms.

## Why two crates?

1. **Testability.** `harmonia-core` has zero UI/audio dependencies: it compiles
   and runs on any machine and is covered by unit tests that run in CI. The
   Tauri shell needs WebKit/ALSA dev headers (Linux) or the WebView2 SDK
   (Windows) and cannot be compiled in every environment.
2. **Honest boundaries.** Audio output and the database should not be entangled.
   The core defines *what* a library is; the shell defines *how* it sounds.
3. **Reusability.** The core could later power a CLI, a different frontend, or
   a headless indexing daemon without changes.

## Why Rust + Tauri 2 (not Electron/Qt/GTK)?

- **Rust** gives us memory safety without a GC — important for an audio engine
  that must never stutter or crash on malformed files.
- **Tauri 2** ships a small native webview — WebKitGTK on Linux, WebView2
  (Edge runtime) on Windows — instead of bundling a browser, keeping idle
  memory below 150 MB and startup under 1 second.
- **React** for the UI gives fast iteration and a rich ecosystem for
  virtualization and animation, while the heavy lifting happens in Rust.
- **rodio** (with the `symphonia` backend) decodes MP3/FLAC/OGG/AAC/M4A/WAV
  with a single code path, supports gapless decoding and device selection, and
  exposes a clean `Source` trait we extend for DSP.

## Data flow

### Library scan

```
Frontend ── scan_library ──► commands.rs ──► library::Library::scan
                                                 │
                              walkdir (rayon pool) ──► metadata::read (lofty)
                                                 │
                                                 ▼
                                       db::Library::upsert_track
                                                 │
                                   change count + library://changed event
```

- Scanning walks configured folders with a parallel rayon pool.
- Files are deduplicated by (path, mtime, size); unchanged files are skipped
  (incremental rescan).
- Metadata is read via `lofty` with `ParseOptions` that fail fast on malformed
  files — a corrupt file is logged and skipped, never a crash.

### Filesystem watching

`watcher.rs` subscribes to `notify` events per configured folder. Events are
coalesced and batched: create/rename → scan the new path; remove → delete from
the DB. This gives live library updates without polling.

### Playback

```
Frontend ── play_context ──► engine (dedicated thread)
                                 │
                    builds decoded queue (gapless)
                                 │
              crossfade start/stop · seek · speed · EQ
                                 │
                    player://state / player://position events
```

The audio engine runs on its own thread so UI frames never block and IPC
latency never causes dropouts. Gapless playback is enabled at the decoder;
crossfade overlaps track endings with a configurable fade window. The
`dsp_source.rs` adapter chains per-track DSP (ReplayGain, EQ biquads, bass
boost, balance, mono) in pure Rust on top of the decoded source.

## Database

SQLite via `rusqlite`, with WAL mode for concurrent read/write (scanner writes
while the UI reads). Schema highlights:

- `tracks` — one row per file, indexed on `(folder_id, path)`, `title`, `artist`
- `albums` / `artists` — denormalized caches rebuilt incrementally
- `playlists` / `playlist_tracks` — static + smart (rules stored as JSON)
- `meta` — key/value for schema versioning and scan bookkeeping

All queries go through `db.rs`, never raw SQL in the shell.

## Frontend architecture

- `store.tsx` — a single React context holding settings, playback state,
  navigation, toasts, and context menus. Backend events are the single source
  of truth for playback state.
- **Views are route-driven** — a discriminated-union `View` type and a
  `ViewRouter` component keep navigation type-safe (no URL router needed in a
  desktop app).
- **Virtual scrolling** — `useVirtual` renders only visible rows (fixed row
  height, overscan) so 100k-track tables stay at 60 fps.
- **Design system** — CSS custom properties for color roles (light/dark/system
  theme, accent color) in `styles/global.css`; components never hardcode colors.

## Error handling philosophy

- The core never panics on user data: all fallible parsing returns
  `Result` and the scanner isolates per-file failures.
- The shell wraps every command and surfaces errors as toasts, not crashes.
- Logging goes to stderr and a rotating log file under the app data dir.
