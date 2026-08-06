<p align="center">
  <img src="src-tauri/icons/icon-128x128.png" width="96" height="96" alt="Harmonia logo" />
</p>

<h1 align="center">Harmonia</h1>

<p align="center">
  A fast, beautiful, modern music player for Linux and Windows.<br />
  <i>Rust · Tauri 2 · React · rodio — Spotify meets Foobar2000, at 50 MB of RAM.</i>
</p>

<p align="center">
  <a href="#features">Features</a> ·
  <a href="#installation">Installation</a> ·
  <a href="#building-from-source">Building from source</a> ·
  <a href="docs/ARCHITECTURE.md">Architecture</a> ·
  <a href="docs/DEVELOPMENT.md">Development</a> ·
  <a href="docs/ROADMAP.md">Roadmap</a>
</p>

---

## Highlights

- **Native & lightweight** — Rust core + Tauri 2 shell. Cold start well under 1 second, idle memory below 150 MB.
- **Built for big libraries** — SQLite-backed index designed for 100,000+ tracks, with incremental rescans and filesystem watching.
- **Professional audio** — gapless playback, crossfade, ReplayGain, a 10-band equalizer, bass boost, balance, and pitch-preserving speed control (powered by [rodio](https://github.com/rustaudio/rodio)).
- **Beautiful UI** — Material 3–inspired design, dark/light/system themes, accent colors, smooth animations, virtualized scrolling for buttery lists.
- **Deep metadata** — ID3v2/v1, Vorbis comments, FLAC tags, embedded artwork, lyrics (plain + synchronized).

## Features

### Library
- Recursive folder scanning with fast multi-threaded indexing
- Incremental rescans (only changed files are re-read)
- Filesystem watching (`notify`) — new/changed/removed files are picked up live
- SQLite database with album, artist, and playlist caches
- 100,000+ track scale, virtualized track lists, paged loading
- Drag & drop files/folders onto the window to import

### Playback
- Gapless playback, configurable crossfade
- ReplayGain (track gain applied, album gain fallback)
- Shuffle, repeat (off / all / one), and an explicit queue you can reorder
- Seek, fast seeking, playback-speed control (pitch preserving)
- 10-band equalizer, bass boost, balance, mono downmix
- Sleep timer (minutes, end of track, end of album)
- Session resume, position memory per track

### Metadata & artwork
- MP3 (ID3v1/v2), FLAC, OGG, AAC/M4A, WAV
- Embedded artwork extraction with a size-bounded on-disk cache
- Lyrics: plain and synchronized (LRC-style)
- ReplayGain tags

### Playlists
- Static and smart playlists (rule-based, `match-all` / `match-any`)
- Recently played, most played, favorites
- Pinned playlists, drag-to-reorder, context-menu actions
- Import/export: M3U, M3U8, PLS, XSPF

### Now playing
- Spinning album art, animated visualizer
- Progress + remaining time
- Codec, bitrate, sample rate, channels, file path
- Lyrics panel

### Platform & polish
- System tray with playback controls
- MPRIS media key integration (desktop-wide media keys)
- Global shortcuts (in-app: space, arrows, Ctrl+F, M…)
- Mini player mode
- Desktop notifications on track change
- Window size/position remembered across launches; last opened section restored
- Linux: desktop file, MIME associations, AppImage + `.deb` bundles
- Windows: NSIS installer (`.exe`) with Start Menu/desktop shortcuts,
  uninstaller and MP3/FLAC/OGG/WAV/M4A file associations, plus a portable exe
- English & Spanish localization

## Installation

### Windows

Download `Harmonia-*-setup.exe` from the [releases page](../../releases) and
run it — the installer adds a Start Menu shortcut (and optionally a desktop
shortcut) plus an uninstaller, and registers MP3/FLAC/OGG/WAV/M4A file
associations so double-clicking a music file opens it in Harmonia.

Prefer a portable copy? Grab the standalone `harmonia.exe` from the same
release — it runs without installing (Windows 10/11 already ship the WebView2
runtime it uses).

### Linux

1. Grab the latest `.AppImage` or `.deb` from the [releases page](../../releases).

```bash
# AppImage
chmod +x Harmonia-*.AppImage
./Harmonia-*.AppImage

# Debian/Ubuntu
sudo apt install ./harmonia_*.deb
```

> Running AppImages on Ubuntu 22.04+ requires `libfuse2`:
> `sudo apt install libfuse2` — or set `APPIMAGE_EXTRACT_AND_RUN=1`.

The `.deb` installs the binary, desktop entry, icons and MIME associations
(`audio/*`); launch from your application menu or run `harmonia`.

### From source

See [docs/INSTALL.md](docs/INSTALL.md) for a full guide, including the system packages needed to build on Linux and the Windows build steps.

```bash
# 1. System dependencies (Debian/Ubuntu)
sudo apt install libwebkit2gtk-4.1-dev build-essential \
  libssl-dev libxdo-dev libayatana-appindicator3-dev librsvg2-dev

# 2. Build & run
npm install
npm run tauri dev          # development
npm run tauri build        # release (AppImage + deb)
```

## Project layout

```
harmonia/
├── harmonia-core/        # Pure-Rust core: DB, metadata, scanner, playlists, DSP
│   └── src/
│       ├── db.rs         # SQLite schema + queries
│       ├── metadata.rs   # lofty-based tag/artwork reading
│       ├── library.rs    # multi-threaded scanner
│       ├── playlists.rs  # static + smart playlists, import/export
│       ├── dsp.rs        # biquad EQ filters (pure, testable)
│       └── watcher.rs    # notify-based filesystem watching
├── src-tauri/            # App shell: Tauri + rodio audio engine
│   └── src/
│       ├── engine.rs     # Audio engine thread (gapless, crossfade, queue)
│       ├── dsp_source.rs # Source adapter applying EQ/replaygain/balance
│       ├── commands.rs   # All Tauri IPC commands
│       └── tray.rs       # System tray
└── src/                  # React frontend
    ├── store.tsx         # App state, event bus, shortcuts
    ├── components/       # Player bar, drawers, tables, modals…
    ├── views/            # Library, albums, artists, playlists, settings…
    └── styles/           # Design system (global.css)
```

The core crate is deliberately free of UI/audio dependencies so it compiles,
tests, and runs anywhere — including CI. See [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md).

## Documentation

| Document | Contents |
| --- | --- |
| [INSTALL.md](docs/INSTALL.md) | System requirements and install instructions per distro |
| [ARCHITECTURE.md](docs/ARCHITECTURE.md) | Design decisions and module responsibilities |
| [DEVELOPMENT.md](docs/DEVELOPMENT.md) | Setting up a dev environment, testing, tooling |
| [CONTRIBUTING.md](docs/CONTRIBUTING.md) | How to contribute, code style, PR workflow |
| [ROADMAP.md](docs/ROADMAP.md) | Planned features and milestones |

## License

MIT — see [LICENSE](LICENSE).
