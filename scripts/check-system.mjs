#!/usr/bin/env node
// Harmonia system-dependency preflight check.
//
// Tauri's `beforeDevCommand` / `beforeBuildCommand` run this script before the
// frontend or the Rust crate ever compiles. Without it, a missing system
// package (e.g. no libasound2-dev) surfaces as a cryptic failure deep inside
// a `-sys` crate compile. This script fails fast with a readable list of the
// exact packages to install instead.
//
// Usage:
//   node scripts/check-system.mjs          # exit 0 if ready, 1 if missing deps
//   node scripts/check-system.mjs --json   # machine-readable JSON on stdout
//   npm run test:system                    # unit tests (node:test, mocked probes)
//
// The module is deliberately structured for testability:
//   - `probes` is an exported object holding the probe functions, so tests can
//     replace them with `mock.method(probes, "hasCommand", ...)`.
//   - `runCheck()` is the pure decision function: it takes options and returns
//     `{ output, exitCode }` without touching `process`.
//   - `main()` only runs when the file is executed directly, never on import.

import { execFileSync } from "node:child_process";
import { accessSync, constants, realpathSync } from "node:fs";
import { fileURLToPath } from "node:url";

export const APT_PACKAGES = {
  pkgConfig: "pkg-config",
  cc: "build-essential",
  webkit: "libwebkit2gtk-4.1-dev",
  gtk3: "libgtk-3-dev",
  alsa: "libasound2-dev",
  rsvg: "librsvg2-dev",
  openssl: "libssl-dev",
  appindicator: "libayatana-appindicator3-dev",
  xdo: "libxdo-dev",
  fakeroot: "fakeroot",
};

// --- probe helpers (exported so tests can mock them) ------------------------

export const probes = {
  hasCommand(cmd) {
    try {
      execFileSync("sh", ["-c", `command -v ${cmd}`], { stdio: "ignore" });
      return true;
    } catch {
      return false;
    }
  },

  pkgConfigExists(module) {
    const bin = process.env.HARMONIA_PKG_CONFIG || "pkg-config";
    if (!this.hasCommand(bin)) return false;
    try {
      execFileSync(bin, ["--exists", module], { stdio: "ignore" });
      return true;
    } catch {
      return false;
    }
  },

  headerExists(path) {
    try {
      accessSync(path, constants.R_OK);
      return true;
    } catch {
      return false;
    }
  },
};

// --- check definitions ------------------------------------------------------
// Each entry: { id, label, required, check, missingHint }
// `required: true` fails the build; `required: false` is advisory only.

export const CHECKS = [
  {
    id: "pkgConfig",
    label: "pkg-config (locates native libraries)",
    required: true,
    check: () => probes.hasCommand(process.env.HARMONIA_PKG_CONFIG || "pkg-config"),
    missingHint: "Install the 'pkg-config' package.",
  },
  {
    id: "cc",
    label: "C compiler (build-essential / gcc)",
    required: true,
    check: () => probes.hasCommand("cc") || probes.hasCommand("gcc"),
    missingHint: "Install 'build-essential' (Debian/Ubuntu) or the equivalent.",
  },
  {
    id: "webkit",
    label: "WebKitGTK 4.1 (Tauri webview)",
    required: true,
    check: () => probes.pkgConfigExists("webkit2gtk-4.1"),
    missingHint: "Install 'libwebkit2gtk-4.1-dev'.",
  },
  {
    id: "gtk3",
    label: "GTK 3 development headers",
    required: true,
    check: () => probes.pkgConfigExists("gtk+-3.0"),
    missingHint: "Install 'libgtk-3-dev'.",
  },
  {
    id: "alsa",
    label: "ALSA headers (audio output)",
    required: true,
    check: () => probes.pkgConfigExists("alsa"),
    missingHint: "Install 'libasound2-dev' (also needs libasound2 at runtime).",
  },
  {
    id: "rsvg",
    label: "librsvg (icon rendering)",
    required: true,
    check: () => probes.pkgConfigExists("librsvg-2.0"),
    missingHint: "Install 'librsvg2-dev'.",
  },
  {
    id: "openssl",
    label: "OpenSSL headers",
    required: true,
    check: () => probes.pkgConfigExists("openssl"),
    missingHint: "Install 'libssl-dev'.",
  },
  {
    id: "appindicator",
    label: "AppIndicator (system tray)",
    required: true,
    check: () => probes.pkgConfigExists("ayatana-appindicator3-0.1"),
    missingHint: "Install 'libayatana-appindicator3-dev'.",
  },
  {
    id: "xdo",
    label: "libxdo (global media-key shortcuts)",
    required: true,
    check: () => probes.headerExists("/usr/include/xdo.h"),
    missingHint: "Install 'libxdo-dev' (header /usr/include/xdo.h).",
  },
  {
    id: "fakeroot",
    label: "fakeroot (deb bundling)",
    required: false,
    check: () => probes.hasCommand("fakeroot"),
    missingHint: "Install 'fakeroot' if you need to build .deb packages.",
  },
];

// --- output -----------------------------------------------------------------

export function missing(checks) {
  return checks.filter((c) => !c.check());
}

export function aptCommand(missingIds) {
  const pkgs = missingIds
    .map((id) => APT_PACKAGES[id])
    .filter(Boolean)
    .join(" ");
  return `sudo apt-get install -y ${pkgs}`;
}

export function renderHuman(missingChecks) {
  const lines = [];
  lines.push("");
  lines.push("Harmonia is missing required system packages.");
  lines.push("");
  lines.push("On Debian/Ubuntu, install them with:");
  lines.push("");
  lines.push(`  ${aptCommand(missingChecks.map((c) => c.id))}`);
  lines.push("");
  lines.push("Missing:");
  for (const c of missingChecks) {
    lines.push(`  - ${c.label} (${c.missingHint})`);
  }
  lines.push("");
  lines.push(
    "See docs/INSTALL.md for Fedora/Arch package equivalents and the full guide.",
  );
  lines.push("");
  return lines.join("\n");
}

export function renderJson(missingChecks, ok) {
  return JSON.stringify(
    {
      ok,
      missing: missingChecks.map((c) => ({
        id: c.id,
        label: c.label,
        hint: c.missingHint,
        required: c.required,
      })),
      installCommand: aptCommand(missingChecks.map((c) => c.id)),
    },
    null,
    2,
  );
}

/**
 * Runs the checks and returns `{ output, exitCode }` without touching
 * `process`, so it is directly unit-testable.
 *
 * @param {{ json?: boolean }} [options]
 */
export function runCheck(options = {}) {
  const json = Boolean(options.json);
  const missingChecks = missing(CHECKS);
  const requiredMissing = missingChecks.filter((c) => c.required);

  // A non-zero exit is what stops the `&&` chain in tauri.conf.json, so it
  // must be set in every mode, not just the human one.
  const exitCode = requiredMissing.length > 0 ? 1 : 0;

  let output;
  if (json) {
    output = renderJson(missingChecks, requiredMissing.length === 0);
  } else if (requiredMissing.length > 0) {
    output = renderHuman(requiredMissing);
  } else {
    const advisories = missingChecks.filter((c) => !c.required);
    if (advisories.length > 0) {
      output =
        "✓ Required system dependencies present.\n" +
        `  Advisory: ${advisories.map((c) => c.label).join("; ")}.`;
    } else {
      output = "✓ All Harmonia system dependencies are present.";
    }
  }

  return { output, exitCode };
}

function main() {
  const { output, exitCode } = runCheck({ json: process.argv.includes("--json") });
  process.stdout.write(`${output}\n`);
  process.exitCode = exitCode;
}

// Run only when executed directly (not when imported by tests). Both sides are
// realpath'd so relative invocations and symlinks resolve to the same file.
const isMain =
  process.argv[1] &&
  realpathSync(process.argv[1]) === fileURLToPath(import.meta.url);
if (isMain) {
  main();
}
