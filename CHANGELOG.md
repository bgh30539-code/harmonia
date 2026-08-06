# Changelog

All notable changes to Harmonia are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project
adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- Initial release candidate.

## [0.1.2] - 2026-08-06

### Added
- **Android support (first official build)** — the same Harmonia codebase
  now compiles and runs on Android: MP3, FLAC, WAV, OGG and M4A playback via
  the existing audio engine, plus library browsing (albums, artists,
  playlists, favorites, recently played), search, queue, settings and themes.
- **Responsive mobile UI** — Material-style bottom navigation, a slide-in
  sidebar on phones, full-screen player and queue drawers, simplified track
  tables, touch-first album cards and safe-area handling. The desktop app
  inherits the same responsive behaviour in narrow windows.
- **Android permissions** — runtime requests for media-library access
  (scoped storage, API 24–32 vs 33+) and track-change notifications; the app
  degrades gracefully when the user declines.
- **Android packaging pipeline** — CI builds a universal APK and an AAB and
  attaches both to every release, alongside the Linux and Windows bundles.

### Changed
- Desktop-only integrations (system tray, global media keys, single-instance
  guard) are now `cfg`-gated so the same crate builds for Android.
- The release workflow builds and uploads Android artifacts before the draft
  release is published, so every release ships the complete artifact set.

### Known limitations (Android)
- Background playback with notification/lock-screen/bluetooth controls and
  audio-focus handling needs a native media session; scheduled for v0.1.3.
- v0.1.2 auto-indexes the standard shared music folders (Music, Download)
  instead of full media-store discovery; folder picking via the system
  picker (SAF) is also scheduled for v0.1.3.

## [0.1.1] - 2026-08-06

### Fixed
- **Window close button** — closing the window always works now. Hiding to
the system tray is opt-in (Settings → Playback), only engages when the tray
was actually created, and a missing tray can never trap the window. The
close handler is panic-safe, panics are logged, and quitting fully
terminates the process with no leftover background threads.
- **Music library path persistence** — a corrupted or missing settings row
can no longer silently forget configured library folders. The `folders`
table is the source of truth and settings loading re-seeds library roots
from it (with regression tests). Missing/unavailable folders are skipped
gracefully instead of crashing.
- **Settings persistence** — settings are serialized atomically and saved
immediately when changed; a stale or empty folder list can no longer wipe
the configured library roots.
- **General stability** — poison-safe mutex handling across the database,
audio engine and window-event paths; panic hook logs instead of dying
silently (important on Windows, which has no console); unreadable or
corrupted audio files and tags are skipped with a warning.

### Added
- **Windows support** — official NSIS installer (`.exe`) with Start Menu
shortcut, optional desktop shortcut, uninstaller and file associations for
MP3/FLAC/OGG/WAV/M4A (double-clicking a music file opens Harmonia).
- **Portable executable** — the standalone `harmonia.exe` is attached to
releases (runs without installation when the WebView2 runtime is present,
as on Windows 10/11).
- **Desktop integration** — window size, position and maximized state are
remembered across launches; the last opened section is restored on startup;
clean shutdown flushes the playback position.
- **Notifications** — optional desktop notification when the playing track
changes (on by default, toggle in Settings).

### Changed
- CI builds and uploads Windows artifacts on every release, and a Windows
package job continuously validates NSIS builds on the main branch.
- `check-system` preflight is now platform-aware (no Linux-only checks on
Windows).
- Documentation updated for cross-platform installation.

## [0.1.0] - 2026-08-05

### Added
- **Library**: recursive multi-threaded scanning, incremental rescans,
  filesystem watching, drag & drop import, folder management.
- **Database**: SQLite index (WAL) for tracks, albums, artists, playlists;
  designed for 100,000+ tracks.
- **Playback**: gapless decoding, configurable crossfade, ReplayGain,
  shuffle/repeat, reorderable queue, seek, playback speed with pitch
  preservation, session resume, sleep timer.
- **DSP**: 10-band equalizer, bass boost, balance, mono downmix, volume
  normalization.
- **Metadata**: ID3v1/v2, Vorbis comments, FLAC tags, embedded artwork,
  plain + synchronized lyrics, ReplayGain tags.
- **Playlists**: static and smart playlists, favorites, recently played, most
  played, pinned, M3U/M3U8/PLS/XSPF import & export.
- **Search**: instant + fuzzy search with genre/codec/year/bitrate/duration
  filters.
- **Now playing**: spinning artwork, animated visualizer, technical details,
  lyrics panel.
- **Platform**: system tray, MPRIS media keys, in-app global shortcuts, mini
  player, notifications, resume banner.
- **UI**: Material 3 inspired design, system/light/dark themes, accent colors,
  virtualized scrolling, English & Spanish localization.
- **Packaging**: AppImage, deb, desktop file, icons, MIME associations.
- **Quality**: unit tests for core, clippy-clean, formatted, CI pipeline with
  automated bundle builds.

[Unreleased]: https://github.com/bgh30539-code/harmonia/compare/v0.1.2...HEAD
[0.1.2]: https://github.com/bgh30539-code/harmonia/compare/v0.1.1...v0.1.2
[0.1.1]: https://github.com/bgh30539-code/harmonia/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/bgh30539-code/harmonia/releases/tag/v0.1.0
