# Development guide

## Prerequisites

- Rust stable (1.77+)
- Node.js 20+
- Tauri v2 system dependencies — see [INSTALL.md](INSTALL.md)

## First setup

```bash
npm install          # frontend deps
cargo fetch          # (optional) pre-download crate sources
```

## Daily workflow

```bash
npm run tauri dev    # launch the app with hot reload
```

This runs the Vite dev server on port 1420 and starts the Tauri shell against
it. Rust changes trigger an automatic rebuild.

## Useful commands

```bash
# Core crate (no display needed)
cargo test -p harmonia-core
cargo clippy -p harmonia-core -- -D warnings
cargo fmt -p harmonia-core --check

# Frontend
npx tsc --noEmit
npm run build

# Whole workspace
cargo test
npm run tauri build
```

## Project conventions

- **Rust**: `cargo fmt` + `cargo clippy -- -D warnings` must be clean. Follow
  existing module structure; no new `unwrap()` on user-controlled data.
- **TypeScript**: strict mode is on. Prefer the existing discriminated-union
  navigation and context store over adding global state.
- **Styling**: colors and spacing come from CSS variables in
  `src/styles/global.css`; never hardcode colors in components.
- **i18n**: every user-facing string goes through `t("key")` in
  `src/i18n/locales/{en,es}.json`. Add both locales when adding a string.
- **No placeholders**: no `TODO`, `FIXME`, `unimplemented!()`, or mock
  implementations are allowed in merged code.

## Adding a backend command

1. Implement the logic in `harmonia-core` (pure, testable).
2. Add a `#[tauri::command]` in `src-tauri/src/commands.rs`.
3. Register it in `lib.rs` (`invoke_handler`).
4. Wrap it in `src/api.ts`.
5. Call it from the UI and handle the rejection with a toast.

## Adding an event

1. Emit from the shell (e.g. `app.emit("player://state", …)`).
2. Add the payload type to `EventMap` in `src/api.ts`.
3. Subscribe in `store.tsx` and route into context state.

## Testing

- **Unit tests** live next to their modules in `harmonia-core` (`#[cfg(test)]`).
  They cover the DB, metadata parsing, playlists, DSP filters, and the
  scanner's deduplication logic. The core test suite uses temporary SQLite
  files, never the user's real database.
- **Frontend** is typechecked (`tsc`) and built (`vite build`) in CI.
- **End-to-end** packaging is validated by the CI `package` job, which produces
  real AppImage and `.deb` artifacts on every push to `main`.

## Android development

Prerequisites: Android SDK + NDK (`ANDROID_HOME` set, NDK r26b+), JDK 17+.
See [tauri.app/start/prerequisites/#android](https://tauri.app/start/prerequisites/#android)
and the Android section of [INSTALL.md](INSTALL.md).

```bash
rustup target add aarch64-linux-android armv7-linux-androideabi i686-linux-android x86_64-linux-android
npm run tauri -- android dev       # run on a connected device/emulator
HARMONIA_BUILD_ANDROID=1 npm run tauri -- android build --apk --aab
```

The Android project lives in `src-tauri/gen/android/` (regenerate with
`npx tauri android init`). Desktop-only integrations (tray, global media
keys, single-instance) are `#[cfg(not(target_os = "android"))]`-gated in
`src-tauri/src/lib.rs` — keep them that way when adding platform features.
The frontend is responsive: phones get a bottom navigation bar, the sidebar
slides in as a drawer, and queue/now-playing go full-screen
(`src/styles/global.css`, the `@media (max-width: 719px)` block).

## Release process

1. Bump `version` in `src-tauri/tauri.conf.json`, `package.json` **and** the
   workspace `Cargo.toml`.
2. Update `CHANGELOG.md` — add a `## [x.y.z]` section for the new version.
3. Merge to `main` and push a matching tag:

   ```bash
   git tag v0.1.0
   git push origin v0.1.0
   ```

4. The `Release` workflow (`.github/workflows/release.yml`) verifies the
   project, builds AppImage + `.deb`, Android APK + AAB and Windows NSIS
   bundles, and attaches everything to a **draft** GitHub release. The tag
   must match the version in `tauri.conf.json` or the workflow fails early.
5. The Windows job publishes the draft once every artifact is uploaded — the
   release only ever becomes visible with the complete artifact set.

To test packaging without a release, push to `main` and download the bundles
from the CI `package` job's artifacts.

## Troubleshooting

- `ERROR: failed to run custom build command for webkit2gtk-sys` → install the
  `-dev` packages listed in [INSTALL.md](INSTALL.md).
- Vite says port 1420 is in use → kill the stale process or change the port in
  `vite.config.ts` **and** `tauri.conf.json` together.
- Audio output issues → check `Settings → Audio → Output device`.
