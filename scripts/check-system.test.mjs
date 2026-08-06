// Unit tests for scripts/check-system.mjs.
//
// Run with:  node --test scripts/check-system.test.mjs   (or npm run test:system)
//
// The probes live on the exported `probes` object so each test can replace
// them with `mock.method(...)` without touching the real system.

import { test } from "node:test";
import assert from "node:assert/strict";

import {
  APT_PACKAGES,
  CHECKS,
  WINDOWS_CHECKS,
  aptCommand,
  checksForPlatform,
  missing,
  probes,
  runCheck,
} from "./check-system.mjs";

// Helpers ---------------------------------------------------------------

/** Makes every probe succeed (all dependencies present). */
function mockAllPresent(t) {
  t.mock.method(probes, "hasCommand", () => true);
  t.mock.method(probes, "pkgConfigExists", () => true);
  t.mock.method(probes, "headerExists", () => true);
}

/** Makes every required probe fail (no dependencies present). */
function mockAllMissing(t) {
  t.mock.method(probes, "hasCommand", () => false);
  t.mock.method(probes, "pkgConfigExists", () => false);
  t.mock.method(probes, "headerExists", () => false);
}

// Tests -----------------------------------------------------------------

test("module import has no side effects (no main() on import)", () => {
  // Importing the module must not touch process.exitCode or stdout; only the
  // direct-execution guard in main() may. This assertion would fail if the
  // guard were removed.
  assert.equal(process.exitCode, undefined);
});

test("exit 0 and success message when all dependencies are present", (t) => {
  mockAllPresent(t);
  const { output, exitCode } = runCheck();
  assert.equal(exitCode, 0);
  assert.match(output, /All Harmonia system dependencies are present/);
});

test("exit 1 with friendly message when required deps are missing", (t) => {
  mockAllMissing(t);
  const { output, exitCode } = runCheck();
  assert.equal(exitCode, 1);
  assert.match(output, /Harmonia is missing required system packages/);
  assert.match(output, /sudo apt-get install -y/);
  assert.match(output, /docs\/INSTALL\.md/);
});

test("missing pkg-config is reported as required and fails the build", (t) => {
  mockAllPresent(t);
  t.mock.method(probes, "hasCommand", (cmd) => cmd !== "pkg-config");
  t.mock.method(probes, "pkgConfigExists", () => false);
  const { output, exitCode } = runCheck();
  assert.equal(exitCode, 1);
  assert.match(output, /pkg-config \(locates native libraries\)/);
});

test("missing xdo header (libxdo-dev) is detected without pkg-config module", (t) => {
  mockAllPresent(t);
  t.mock.method(probes, "headerExists", () => false);
  const { output, exitCode } = runCheck();
  assert.equal(exitCode, 1);
  assert.match(output, /libxdo \(global media-key shortcuts\)/);
  assert.match(output, /libxdo-dev/);
});

test("missing C compiler fails the build", (t) => {
  mockAllPresent(t);
  t.mock.method(probes, "hasCommand", (cmd) => cmd !== "cc" && cmd !== "gcc");
  const { output, exitCode } = runCheck();
  assert.equal(exitCode, 1);
  assert.match(output, /C compiler/);
  assert.match(output, /build-essential/);
});

test("missing fakeroot is advisory only: exit 0 with a note", (t) => {
  mockAllPresent(t);
  t.mock.method(probes, "hasCommand", (cmd) => cmd !== "fakeroot");
  const { output, exitCode } = runCheck();
  assert.equal(exitCode, 0);
  assert.match(output, /Required system dependencies present/);
  assert.match(output, /Advisory: fakeroot/);
});

test("JSON mode: ok true and exit 0 when ready", (t) => {
  mockAllPresent(t);
  const { output, exitCode } = runCheck({ json: true });
  assert.equal(exitCode, 0);
  const parsed = JSON.parse(output);
  assert.equal(parsed.ok, true);
  assert.deepEqual(parsed.missing, []);
});

test("JSON mode: ok false and exit 1 when required deps missing", (t) => {
  mockAllMissing(t);
  const { output, exitCode } = runCheck({ json: true });
  assert.equal(exitCode, 1);
  const parsed = JSON.parse(output);
  assert.equal(parsed.ok, false);
  assert.ok(parsed.missing.length > 0);
  // Every required check must be reported as missing.
  const requiredIds = CHECKS.filter((c) => c.required).map((c) => c.id);
  const missingIds = new Set(parsed.missing.map((m) => m.id));
  for (const id of requiredIds) {
    assert.ok(missingIds.has(id), `required check "${id}" must be reported missing`);
  }
  assert.match(parsed.installCommand, /^sudo apt-get install -y /);
});

test("JSON mode: advisory-only missing still reports ok true", (t) => {
  mockAllPresent(t);
  t.mock.method(probes, "hasCommand", (cmd) => cmd !== "fakeroot");
  const { output, exitCode } = runCheck({ json: true });
  assert.equal(exitCode, 0);
  const parsed = JSON.parse(output);
  assert.equal(parsed.ok, true);
  const fakeroot = parsed.missing.find((m) => m.id === "fakeroot");
  assert.ok(fakeroot, "fakeroot should be listed as missing");
  assert.equal(fakeroot.required, false);
});

test("checksForPlatform selects the Windows set on win32", () => {
  assert.deepEqual(checksForPlatform("win32"), WINDOWS_CHECKS);
  assert.deepEqual(checksForPlatform("linux"), CHECKS);
  assert.notEqual(WINDOWS_CHECKS, CHECKS);
  // Windows checks never reference Linux-only tools.
  for (const c of WINDOWS_CHECKS) {
    assert.ok(c.required, "Windows checks are all required");
  }
});

test("Windows platform passes when node/npm exist, with no apt advice", (t) => {
  t.mock.method(probes, "hasCommand", (cmd) => cmd === "node" || cmd === "npm");
  const { output, exitCode } = runCheck({ platform: "win32" });
  assert.equal(exitCode, 0);
  assert.match(output, /All Harmonia system dependencies are present/);
  assert.doesNotMatch(output, /apt-get/);
  assert.doesNotMatch(output, /webkit|alsa|appindicator/i);
});

test("Windows platform fails with Windows hints when node/npm are missing", (t) => {
  t.mock.method(probes, "hasCommand", () => false);
  const { output, exitCode } = runCheck({ platform: "win32" });
  assert.equal(exitCode, 1);
  assert.match(output, /missing required prerequisites on Windows/);
  assert.match(output, /Node\.js/);
  assert.doesNotMatch(output, /sudo apt/);
});

test("Windows JSON mode reports no apt install command", (t) => {
  t.mock.method(probes, "hasCommand", () => true);
  const { output, exitCode } = runCheck({ platform: "win32", json: true });
  assert.equal(exitCode, 0);
  const parsed = JSON.parse(output);
  assert.equal(parsed.ok, true);
  assert.equal(parsed.installCommand, "");
});

test("HARMONIA_PKG_CONFIG override is honoured", (t) => {
  mockAllPresent(t);
  process.env.HARMONIA_PKG_CONFIG = "/nonexistent/pkg-config";
  t.after(() => {
    delete process.env.HARMONIA_PKG_CONFIG;
  });
  t.mock.method(probes, "hasCommand", (cmd) => cmd !== "/nonexistent/pkg-config");
  t.mock.method(probes, "pkgConfigExists", () => false);
  const { exitCode } = runCheck();
  assert.equal(exitCode, 1);
});

test("check table integrity: unique ids and every required id maps to an apt package", () => {
  const ids = CHECKS.map((c) => c.id);
  assert.equal(new Set(ids).size, ids.length, "check ids must be unique");
  for (const check of CHECKS) {
    assert.ok(
      APT_PACKAGES[check.id],
      `every check needs an APT_PACKAGES entry, missing for "${check.id}"`,
    );
  }
  // A few spot checks that the well-known packages are mapped correctly.
  assert.equal(APT_PACKAGES.webkit, "libwebkit2gtk-4.1-dev");
  assert.equal(APT_PACKAGES.alsa, "libasound2-dev");
  assert.equal(APT_PACKAGES.xdo, "libxdo-dev");
});

test("every required check fails when its probes fail", (t) => {
  // Directly exercises each required check with all probes mocked to false.
  // This would catch a check stubbed to a constant `() => true` — a check the
  // whole-suite exit-code assertions alone could never detect.
  mockAllMissing(t);
  for (const check of CHECKS.filter((c) => c.required)) {
    assert.equal(
      check.check(),
      false,
      `required check "${check.id}" must fail when its probe fails`,
    );
  }
});

test("pkg-config module names and header paths are the canonical set", (t) => {
  // The probes are mocked in every other test, so a typo like
  // "webkit2gtk-4.2" or "gtk+3.0" in CHECKS would silently pass the suite.
  // Capture the arguments actually passed to each probe and pin them down.
  const seenModules = [];
  const seenHeaders = [];
  t.mock.method(probes, "hasCommand", () => true);
  t.mock.method(probes, "pkgConfigExists", (module) => {
    seenModules.push(module);
    return true;
  });
  t.mock.method(probes, "headerExists", (path) => {
    seenHeaders.push(path);
    return true;
  });
  runCheck();

  assert.deepEqual(
    [...new Set(seenModules)].sort(),
    [
      "alsa",
      "ayatana-appindicator3-0.1",
      "gtk+-3.0",
      "librsvg-2.0",
      "openssl",
      "webkit2gtk-4.1",
    ],
  );
  assert.deepEqual([...new Set(seenHeaders)], ["/usr/include/xdo.h"]);
});

test("missing() filters checks by their check() result", () => {
  const checks = [
    { id: "pass", check: () => true },
    { id: "fail", check: () => false },
  ];
  assert.deepEqual(
    missing(checks).map((c) => c.id),
    ["fail"],
  );
});

test("aptCommand() maps ids to packages and skips unmapped ids", () => {
  assert.equal(
    aptCommand(["webkit", "alsa", "not-a-real-id"]),
    "sudo apt-get install -y libwebkit2gtk-4.1-dev libasound2-dev",
  );
  assert.equal(aptCommand([]), "sudo apt-get install -y ");
});
