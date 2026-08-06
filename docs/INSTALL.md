# Installation guide

## Binary releases

### Windows

Download `Harmonia-*-setup.exe` from the [releases page](../../releases) and
run it. The NSIS installer:

- adds a **Start Menu shortcut** under *Harmonia* and a **desktop shortcut**
  automatically;
- registers an **uninstaller** (via *Settings → Apps* or the Start Menu);
- registers **file associations** for `mp3`, `flac`, `ogg`, `opus`, `wav`,
  `m4a`, `aac` — double-clicking a music file opens it in Harmonia (files are
  imported into your library).

Prefer a **portable** copy? Download the standalone `harmonia.exe` from the
same release and run it directly — no installation required. It uses the
WebView2 runtime, which ships with Windows 10/11.

### AppImage

```bash
chmod +x Harmonia-*.AppImage
./Harmonia-*.AppImage
```

**Ubuntu 22.04+ / Debian 12+**: AppImages are built against FUSE 2. If you get
`fuse: device not found` or `dlopen(): error loading libfuse.so.2`, install the
FUSE 2 compatibility layer:

```bash
sudo apt install libfuse2          # Debian/Ubuntu
# or, without installing anything:
APPIMAGE_EXTRACT_AND_RUN=1 ./Harmonia-*.AppImage
```

### Debian package

```bash
sudo apt install ./harmonia_*.deb
```

This installs the binary, desktop entry, icons, and MIME associations
(`audio/*`). Launch from your application menu or run `harmonia`.

### Android

Download the `*-universal-release.apk` from the [releases page](../../releases)
and side-load it — enable *Install unknown apps* for the app you download
with (browser / file manager). Harmonia requires **Android 8.0+ (API 26)**
(the audio engine's Android backend needs AAudio, which ships with API 26).

On first launch the app requests access to your music library (scoped
storage) and notifications. It then indexes the standard shared music
folders (Music, Download) and re-scans when you tap *Add folder*. Declining
the permission just leaves the library empty — nothing breaks.

Folder picking via the system picker (SAF) and automatic discovery of the
whole media store are planned for v0.1.3.

## Building from source

### Requirements

| Tool | Version |
| --- | --- |
| Rust | 1.77+ (stable) |
| Node.js | 20+ |
| npm | 9+ |

**Windows** additionally needs the [Microsoft C++ Build
Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/) (MSVC
toolchain, included on GitHub's `windows-latest` runners).

### Building on Windows

```powershell
# 1. Install the MSVC toolchain (Visual Studio Build Tools) and Node.js 20+.
#    WebView2 is bundled by Tauri; Windows 10/11 already ship the runtime.

# 2. Install JavaScript deps and build the NSIS installer + portable exe.
npm install
npm run tauri build -- --bundles nsis

# 3. Outputs:
#    target\release\bundle\nsis\Harmonia-*-setup.exe   (installer)
#    target\release\harmonia.exe                        (portable binary)
```

### System packages

These are the **complete** prerequisites for Harmonia on Linux. Tauri v2 needs
the WebKitGTK 4.1 development headers, and the audio engine (rodio/cpal)
needs the ALSA development headers — both are easy to forget and each fails
the build with a cryptic error.

**Debian / Ubuntu (20.04+, tested on 22.04 and 24.04)**

```bash
sudo apt update
sudo apt install -y \
  libwebkit2gtk-4.1-dev \
  build-essential \
  curl wget file \
  libxdo-dev \
  libssl-dev \
  libayatana-appindicator3-dev \
  librsvg2-dev \
  libgtk-3-dev \
  libasound2-dev \
  pkg-config \
  libfuse2
```

Package-by-package:

| Package | Why it's needed |
| --- | --- |
| `libwebkit2gtk-4.1-dev` | WebView runtime for the Tauri UI (required, always) |
| `build-essential` | gcc/g++ toolchain for native crates |
| `curl wget file` | Tauri bundler needs these to fetch tools and inspect binaries |
| `libxdo-dev` | X11 automation (global shortcuts, `xdotool`) |
| `libssl-dev` | OpenSSL headers (download/HTTPS in some build steps) |
| `libayatana-appindicator3-dev` | System tray icon support |
| `librsvg2-dev` | SVG/icon rendering |
| `libgtk-3-dev` | GTK3 headers required alongside WebKitGTK |
| `libasound2-dev` | **ALSA headers — rodio/cpal audio output** |
| `pkg-config` | Locates the above libraries for native crates |
| `libfuse2` | Run AppImage artifacts (FUSE 2; Ubuntu 22.04+ ships FUSE 3 only) |

**Fedora**

```bash
sudo dnf install webkit2gtk4.1-devel openssl-devel libxdo-devel \
  libayatana-appindicator-devel librsvg2-devel gcc-c++ \
  gtk3-devel alsa-lib-devel pkg-config fuse
```

**Arch**

```bash
sudo pacman -S webkit2gtk-4.1 base-devel openssl libxdo \
  libayatana-appindicator librsvg gtk3 alsa-lib pkgconf fuse2
```

> **Tip — verify before building:** `pkg-config --exists webkit2gtk-4.1 && echo ok`
> should print `ok`; `ls /usr/include/alsa/asoundlib.h` should exist. If not,
> reinstall the dev packages above (or reboot if you just installed them).

### Build (Linux)

`npm run tauri build` produces:

- `bundle/appimage/*.AppImage`
- `bundle/deb/*.deb`
- `release/harmonia` (raw binary)

See [DEVELOPMENT.md](DEVELOPMENT.md) for the release process (tag → CI builds
the bundles for both Linux and Windows).

## First build, step by step

This walks through a clean-machine first build of the release bundles. Expect
the first Rust compile to take several minutes (hundreds of crates); later
builds are incremental and fast.

```bash
# 1. System packages (Debian/Ubuntu — see the table above).
#    Requires sudo; only needed once per machine.
sudo apt update && sudo apt install -y \
  libwebkit2gtk-4.1-dev build-essential curl wget file \
  libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev \
  libgtk-3-dev libasound2-dev pkg-config libfuse2

# 2. Sanity-check the two headers the build commonly fails on.
pkg-config --exists webkit2gtk-4.1 && echo "webkit: ok"
test -f /usr/include/alsa/asoundlib.h && echo "alsa: ok"

# 3. Install the JavaScript toolchain deps (Vite, React, @tauri-apps/cli).
npm install

# 4. Optional: quick core verification before the slow full build.
cargo test -p harmonia-core

# 5. Release build (runs `tsc && vite build` first, then the Rust release
#    build, then bundles the .deb and AppImage).
npm run tauri build

# 6. Inspect the output.
ls -lh src-tauri/target/release/bundle/appimage/*.AppImage
ls -lh src-tauri/target/release/bundle/deb/*.deb

# 7. Install the .deb and launch from your app menu, or run the AppImage:
sudo apt install ./src-tauri/target/release/bundle/deb/*.deb
./src-tauri/target/release/bundle/appimage/*.AppImage
```

**If you prefer the cargo-flavoured command** (`cargo tauri build`), install the
Tauri CLI first — the npm one is used by default because it is pinned in
`package.json` and always matches the frontend:

```bash
cargo install tauri-cli --version "^2"   # once, ~2 min compile
cargo tauri build                         # equivalent to npm run tauri build
```

**Common first-build failures:**

- `Failed to run custom build command for webkit2gtk-sys` → `libwebkit2gtk-4.1-dev`
  is missing or `pkg-config` cannot find it.
- `... alsa-sys ...` / `cannot find -lasound` → `libasound2-dev` is missing.
- `... librsvg ...` → `librsvg2-dev` is missing.
- `... libappindicator ...` → `libayatana-appindicator3-dev` is missing.
- `... xdo ...` → `libxdo-dev` is missing.
- Bundle step fails with `fakeroot` errors → install `fakeroot` (Debian/Ubuntu).
- `AppImage ... fuse: device not found` → install `libfuse2` or set
  `APPIMAGE_EXTRACT_AND_RUN=1`.

### Building the Android app

You need the [Android SDK + NDK](https://tauri.app/start/prerequisites/#android)
(`ANDROID_HOME` set, NDK r26b or newer) and a JDK 17+:

```bash
# One-time: install the Rust Android targets
rustup target add aarch64-linux-android armv7-linux-androideabi \
  i686-linux-android x86_64-linux-android

# Development build on a connected device/emulator
npm run tauri -- android dev

# Release APK (+ AAB for a future Play Store release)
HARMONIA_BUILD_ANDROID=1 npm run tauri -- android build --apk --aab
```

Outputs (universal, all ABIs):

- `src-tauri/gen/android/app/build/outputs/apk/universal/release/` — APK
- `src-tauri/gen/android/app/build/outputs/bundle/release/` — AAB

The release APK is signed with the Android debug key, which is fine for
side-loading. For the Play Store, configure a release keystore (see the
Tauri mobile documentation). The CI `package-android` / release jobs build
the APK on every push and release.

## Building without a display / headless CI

`harmonia-core` is UI-free and always buildable:

```bash
cargo test -p harmonia-core
cargo clippy -p harmonia-core -- -D warnings
```

## Troubleshooting

| Problem | Fix |
| --- | --- |
| `GLib-GIO: ... no such schema` | Install the app's desktop entry (`src-tauri/target/release/bundle/deb/*.deb`) |
| No audio device | Ensure PipeWire/PulseAudio is running; select the device in Settings → Audio |
| AppImage won't start | Run `APPIMAGE_EXTRACT_AND_RUN=1 ./Harmonia-*.AppImage` |
| Media keys don't work | Enable MPRIS (default); ensure no other player is grabbing the keys |
