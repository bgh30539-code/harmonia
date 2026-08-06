#!/usr/bin/env bash
# Harmonia system-dependency preflight check (bash standalone variant).
#
# Drop-in equivalent of scripts/check-system.mjs for machines where Node is
# not available: same checks, same output, same exit codes.
#
# Usage:
#   scripts/check-system.sh           # exit 0 if ready, 1 if missing deps
#   scripts/check-system.sh --json    # machine-readable JSON on stdout
#
# Like the Node version, the pkg-config binary can be overridden so the
# failure path is testable:
#   HARMONIA_PKG_CONFIG=/nonexistent scripts/check-system.sh

set -u

# --- apt package mapping -----------------------------------------------------

declare -A APT_PACKAGES=(
  [pkgConfig]="pkg-config"
  [cc]="build-essential"
  [webkit]="libwebkit2gtk-4.1-dev"
  [gtk3]="libgtk-3-dev"
  [alsa]="libasound2-dev"
  [rsvg]="librsvg2-dev"
  [openssl]="libssl-dev"
  [appindicator]="libayatana-appindicator3-dev"
  [xdo]="libxdo-dev"
  [fakeroot]="fakeroot"
)

# --- probes ------------------------------------------------------------------

has_command() {
  command -v "$1" >/dev/null 2>&1
}

pkgconfig_exists() {
  local bin="${HARMONIA_PKG_CONFIG:-pkg-config}"
  has_command "$bin" || return 1
  "$bin" --exists "$1" >/dev/null 2>&1
}

header_exists() {
  [ -r "$1" ]
}

# One probe function per check id.
probe_pkgConfig() {
  has_command "${HARMONIA_PKG_CONFIG:-pkg-config}"
}
probe_cc() {
  has_command cc || has_command gcc
}
probe_webkit() {
  pkgconfig_exists webkit2gtk-4.1
}
probe_gtk3() {
  pkgconfig_exists gtk+-3.0
}
probe_alsa() {
  pkgconfig_exists alsa
}
probe_rsvg() {
  pkgconfig_exists librsvg-2.0
}
probe_openssl() {
  pkgconfig_exists openssl
}
probe_appindicator() {
  pkgconfig_exists ayatana-appindicator3-0.1
}
probe_xdo() {
  header_exists /usr/include/xdo.h
}
probe_fakeroot() {
  has_command fakeroot
}

# id|required|label|missingHint   (required: 1 fails the build, 0 advisory)
CHECKS=(
  "pkgConfig|1|pkg-config (locates native libraries)|Install the 'pkg-config' package."
  "cc|1|C compiler (build-essential / gcc)|Install 'build-essential' (Debian/Ubuntu) or the equivalent."
  "webkit|1|WebKitGTK 4.1 (Tauri webview)|Install 'libwebkit2gtk-4.1-dev'."
  "gtk3|1|GTK 3 development headers|Install 'libgtk-3-dev'."
  "alsa|1|ALSA headers (audio output)|Install 'libasound2-dev' (also needs libasound2 at runtime)."
  "rsvg|1|librsvg (icon rendering)|Install 'librsvg2-dev'."
  "openssl|1|OpenSSL headers|Install 'libssl-dev'."
  "appindicator|1|AppIndicator (system tray)|Install 'libayatana-appindicator3-dev'."
  "xdo|1|libxdo (global media-key shortcuts)|Install 'libxdo-dev' (header /usr/include/xdo.h)."
  "fakeroot|0|fakeroot (deb bundling)|Install 'fakeroot' if you need to build .deb packages."
)

# --- logic -------------------------------------------------------------------

# Populates MISSING_ALL with "id|required|label|hint" for every failed check.
MISSING_ALL=()
run_checks() {
  local entry id required label hint
  for entry in "${CHECKS[@]}"; do
    IFS='|' read -r id required label hint <<< "$entry"
    if probe_$id; then
      continue
    fi
    MISSING_ALL+=("$id|$required|$label|$hint")
  done
}

apt_command() {
  # args: check ids. Unmapped ids are skipped, mirroring the mjs version's
  # .filter(Boolean); the :- default also avoids a `set -u` "unbound variable"
  # abort if a future check is added without a package mapping.
  local pkgs=() id pkg
  for id in "$@"; do
    pkg="${APT_PACKAGES[$id]:-}"
    if [ -n "$pkg" ]; then
      pkgs+=("$pkg")
    fi
  done
  printf 'sudo apt-get install -y %s' "${pkgs[*]}"
}

# Minimal JSON string escaping. Current labels/hints contain only `( ) ' . ,`,
# but escape `\` and `"` so the output stays valid if they ever appear.
json_escape() {
  local s="$1"
  s="${s//\\/\\\\}"
  s="${s//\"/\\\"}"
  printf '%s' "$s"
}

render_human() {
  # args: ids of required-missing checks
  local entry id required label hint
  printf '\nHarmonia is missing required system packages.\n\n'
  printf 'On Debian/Ubuntu, install them with:\n\n  '
  apt_command "$@"
  printf '\n\nMissing:\n'
  for entry in "${MISSING_ALL[@]}"; do
    IFS='|' read -r id required label hint <<< "$entry"
    if [ "$required" = "1" ]; then
      printf '  - %s (%s)\n' "$label" "$hint"
    fi
  done
  printf '\nSee docs/INSTALL.md for Fedora/Arch package equivalents and the full guide.\n\n'
}

render_json() {
  local entry id required label hint all_ids=() ok req_bool
  for entry in "${MISSING_ALL[@]}"; do
    IFS='|' read -r id required label hint <<< "$entry"
    all_ids+=("$id")
  done

  ok=true
  for entry in "${MISSING_ALL[@]}"; do
    IFS='|' read -r id required label hint <<< "$entry"
    if [ "$required" = "1" ]; then
      ok=false
      break
    fi
  done

  printf '{\n  "ok": %s,\n  "missing": ' "$ok"
  if [ "${#MISSING_ALL[@]}" -eq 0 ]; then
    printf '[],\n'
  else
    printf '[\n'
    local n="${#MISSING_ALL[@]}" i=0
    for entry in "${MISSING_ALL[@]}"; do
      IFS='|' read -r id required label hint <<< "$entry"
      i=$((i + 1))
      # Emit JSON booleans (true/false) to match the mjs version, not 1/0.
      if [ "$required" = "1" ]; then
        req_bool=true
      else
        req_bool=false
      fi
      printf '    {\n      "id": "%s",\n      "label": "%s",\n      "hint": "%s",\n      "required": %s\n    }' \
        "$(json_escape "$id")" "$(json_escape "$label")" "$(json_escape "$hint")" "$req_bool"
      if [ "$i" -lt "$n" ]; then
        printf ','
      fi
      printf '\n'
    done
    printf '  ],\n'
  fi
  printf '  "installCommand": "'
  apt_command "${all_ids[@]}"
  printf '"\n}\n'
}

# --- entry point -------------------------------------------------------------

main() {
  local json=0 arg
  # Honor --json in any position, like the mjs version's argv scan.
  for arg in "$@"; do
    if [ "$arg" = "--json" ]; then
      json=1
    fi
  done

  run_checks

  local required_ids=() entry id required label hint
  for entry in "${MISSING_ALL[@]}"; do
    IFS='|' read -r id required label hint <<< "$entry"
    if [ "$required" = "1" ]; then
      required_ids+=("$id")
    fi
  done

  if [ "$json" = "1" ]; then
    render_json
  elif [ "${#required_ids[@]}" -gt 0 ]; then
    render_human "${required_ids[@]}"
  else
    if [ "${#MISSING_ALL[@]}" -gt 0 ]; then
      # Advisory-only failures: exit 0 but mention them.
      printf '✓ Required system dependencies present.\n  Advisory:'
      local first=1
      for entry in "${MISSING_ALL[@]}"; do
        IFS='|' read -r id required label hint <<< "$entry"
        if [ "$first" = "1" ]; then
          printf ' %s' "$label"
          first=0
        else
          printf '; %s' "$label"
        fi
      done
      printf '.\n'
    else
      printf '✓ All Harmonia system dependencies are present.\n'
    fi
  fi

  if [ "${#required_ids[@]}" -gt 0 ]; then
    exit 1
  fi
  exit 0
}

main "$@"
