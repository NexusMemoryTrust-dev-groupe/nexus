'use strict';

/**
 * Tests for the npm installer logic.
 *
 * These exist because the previous installer hardcoded an asset name
 * (`nexus-windows-x64.exe`) that the release pipeline has never produced, so
 * every `npm install -g nexus-memory-trust` failed with a 404 and nobody
 * noticed. The asset matcher is now the single point where a naming change can
 * break installation, so it is pinned down here and run in CI.
 *
 * Deliberately dependency-free (plain assertions, no test framework) so CI can
 * run it with a bare `node scripts/nexus-install-lib.test.js`.
 */

const assert = require('assert');
const path = require('path');
const os = require('os');

const lib = require('./nexus-install-lib');

let passed = 0;
let failed = 0;

function test(name, fn) {
  try {
    fn();
    passed += 1;
    console.log(`  PASS  ${name}`);
  } catch (err) {
    failed += 1;
    console.error(`  FAIL  ${name}`);
    console.error(`        ${err.message}`);
  }
}

// Asset lists shaped like a real Tauri release. The names come from
// tauri-bundler's own format strings, verified against the crate source:
//   NSIS: "{productName}_{version}_{arch}-setup.exe"
//   MSI:  "{productName}_{version}_{arch}_{language}.msi"
const RELEASE_ASSETS = [
  { name: 'Nexus_1.0.0_x64-setup.exe', size: 1, browser_download_url: 'u1' },
  { name: 'Nexus_1.0.0_arm64-setup.exe', size: 2, browser_download_url: 'u2' },
  { name: 'Nexus_1.0.0_x64_en-US.msi', size: 3, browser_download_url: 'u3' },
  { name: 'SHA256SUMS.txt', size: 4, browser_download_url: 'u4' },
];

console.log('nexus-install-lib');

// ── Asset selection ────────────────────────────────────────────────────────

test('picks the x64 NSIS installer', () => {
  const a = lib.selectInstallerAsset(RELEASE_ASSETS, 'x64');
  assert.strictEqual(a.name, 'Nexus_1.0.0_x64-setup.exe');
});

test('picks the arm64 NSIS installer', () => {
  const a = lib.selectInstallerAsset(RELEASE_ASSETS, 'arm64');
  assert.strictEqual(a.name, 'Nexus_1.0.0_arm64-setup.exe');
});

test('does not confuse arm64 with x64', () => {
  // Substring matching is the risk here: "x64" must not match "arm64" assets,
  // which would install the wrong binary rather than failing loudly.
  const a = lib.selectInstallerAsset(RELEASE_ASSETS, 'arm64');
  assert.ok(!a.name.includes('x64'), `matched wrong arch: ${a.name}`);
});

test('falls back to the MSI when no NSIS installer is published', () => {
  const msiOnly = RELEASE_ASSETS.filter((a) => a.name.endsWith('.msi'));
  const a = lib.selectInstallerAsset(msiOnly, 'x64');
  assert.ok(a.name.endsWith('.msi'), `expected an MSI, got ${a.name}`);
});

test('survives a version bump without code changes', () => {
  // The whole point of pattern matching: 9.9.9 must resolve just as 1.0.0 does.
  const bumped = [
    { name: 'Nexus_9.9.9_x64-setup.exe', size: 1, browser_download_url: 'u' },
  ];
  const a = lib.selectInstallerAsset(bumped, 'x64');
  assert.strictEqual(a.name, 'Nexus_9.9.9_x64-setup.exe');
});

test('throws a listing-aware error when the arch is absent', () => {
  const noArm = RELEASE_ASSETS.filter((a) => !a.name.includes('arm64'));
  assert.throws(
    () => lib.selectInstallerAsset(noArm, 'arm64'),
    (err) => {
      // The message must name the available assets, otherwise a 404-style
      // failure gives the user nothing to act on.
      assert.ok(/arm64/i.test(err.message), 'error should mention the arch');
      assert.ok(/Nexus_1\.0\.0_x64-setup\.exe/.test(err.message),
        'error should list what was available');
      return true;
    }
  );
});

test('throws on a completely empty release', () => {
  assert.throws(() => lib.selectInstallerAsset([], 'x64'), /none/i);
});

// ── Architecture detection ─────────────────────────────────────────────────

test('detectArch reports a 64-bit architecture we publish for', () => {
  // 32-bit is intentionally unsupported: the ONNX runtime behind semantic
  // search ships no i686 binaries (verified: ort-sys has no prebuilt for
  // i686-pc-windows-msvc), and Windows 11 has no 32-bit edition.
  const arch = lib.detectArch();
  assert.ok(['x64', 'arm64'].includes(arch), `unexpected arch: ${arch}`);
});

// ── Cache location ─────────────────────────────────────────────────────────

test('installer cache lives under LOCALAPPDATA, not the package directory', () => {
  // Caching inside node_modules would be wiped on every reinstall and would
  // need write access to a global npm prefix.
  const dir = lib.cacheDir();
  const base = process.env.LOCALAPPDATA || path.join(os.homedir(), 'AppData', 'Local');
  assert.ok(dir.startsWith(base), `cache outside LOCALAPPDATA: ${dir}`);
  assert.ok(dir.includes('Nexus'), `cache not namespaced: ${dir}`);
});

test('cachedInstallerPath keeps the asset filename', () => {
  const p = lib.cachedInstallerPath('Nexus_1.0.0_x64-setup.exe');
  assert.strictEqual(path.basename(p), 'Nexus_1.0.0_x64-setup.exe');
});

// ── Checksum verification ──────────────────────────────────────────────────

test('verifyChecksum reports absence rather than pretending to verify', async () => {
  // A missing SHA256SUMS file must not silently count as "verified".
  const result = await lib.verifyChecksum([], 'Nexus_1.0.0_x64-setup.exe', Buffer.from('x'));
  assert.strictEqual(result.verified, false);
  assert.ok(result.reason, 'must explain why verification did not happen');
});

// ── Exported surface ───────────────────────────────────────────────────────

test('exports the functions postinstall and the launcher rely on', () => {
  for (const fn of [
    'assertSupportedPlatform',
    'detectArch',
    'selectInstallerAsset',
    'fetchLatestRelease',
    'verifyChecksum',
    'cacheDir',
    'cachedInstallerPath',
    'findInstalledBinary',
    'ensureInstallerDownloaded',
  ]) {
    assert.strictEqual(typeof lib[fn], 'function', `missing export: ${fn}`);
  }
});

// ── Summary ────────────────────────────────────────────────────────────────

// verifyChecksum is async; give its assertion a tick to land before summarising.
setImmediate(() => {
  console.log('');
  console.log(`  ${passed} passed, ${failed} failed`);
  if (failed > 0) process.exit(1);
});
