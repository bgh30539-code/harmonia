# Roadmap

Priorities are guided by the principles in the [README](../README.md):
performance first, no mock features, maintainability for the next ten years.

## v0.1 — current

- [x] Library: recursive scanning, incremental rescans, folder watching
- [x] SQLite index (tracks, albums, artists, playlists)
- [x] Playback: gapless, crossfade, ReplayGain, queue, shuffle, repeat, speed
- [x] DSP: 10-band EQ, bass boost, balance, mono
- [x] Metadata: ID3v1/v2, Vorbis, FLAC, embedded artwork, lyrics
- [x] Playlists: static + smart, M3U/PLS/XSPF import & export
- [x] Search with filters, favorites, recent, most played
- [x] System tray, MPRIS, mini player, notifications
- [x] Themes (system/light/dark), accent colors, i18n (en/es)
- [x] Packaging: AppImage, deb, icons, desktop file, MIME
- [x] CI pipeline with tests, lint, and bundle artifacts

## v0.2 — audio & metadata depth

- [ ] Podcast support (RSS feeds, episode tracking)
- [ ] Internet radio (stream URLs, preset stations)
- [ ] Artist biographies & missing-artwork fetching (MusicBrainz / Cover Art Archive)
- [ ] Tag editor (edit tags + artwork in place)
- [ ] Duplicate finder (hash-based)
- [ ] Audio device hot-swap detection

## v0.3 — social & statistics

- [ ] Last.fm scrobbling
- [ ] Music statistics dashboard (per-artist/album listening time)
- [ ] Shareable smart-playlist presets
- [ ] Custom CSS themes (import/export)

## v0.4 — extensibility

- [ ] Plugin system (Rust + JS plugins with a stable API)
- [ ] Visualizer modes (spectrum driven by real backend PCM taps)
- [ ] Multi-window support (detachable queue/now-playing)
- [ ] Streaming/S3-backed libraries

## Long-term

- AI-generated playlists from listening history
- Lyrics editor and community lyrics sync
- Library consolidation / folder merge tools
- Non-Linux platforms (macOS, Windows) via Tauri

### Note on visuals

The current visualizer animates from real playback state (playing/paused,
position). A true spectrum analyser requires tapping decoded PCM from the audio
engine — that's scheduled for v0.4 so it ships as a real feature, not a mock.
