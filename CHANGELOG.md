# Changelog

All notable changes to Harmonia are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project
adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- Initial release candidate.

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

[Unreleased]: https://github.com/example/harmonia/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/example/harmonia/releases/tag/v0.1.0
