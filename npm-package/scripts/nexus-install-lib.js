'use strict';

/**
 * Shared install/launch logic for the `nexus-memory-trust` npm package.
 *
 * Design notes
 * ------------
 * The previous implementation downloaded a hardcoded asset called
 * `nexus-windows-x64.exe`, which the release pipeline has never produced —
 * every `npm install` therefore failed with a 404. Asset names are now
 * *discovered* from the GitHub Releases API and matched by pattern, so a
 * version bump or a naming tweak in `tauri.conf.json` cannot silently break
 * installation again.
 *
 * We deliberately do NOT install silently. The user asked to be able to pick
 * the target drive, and a silent `/S` install would take that choice away, so
 * the downloaded NSIS installer is launched interactively on first run.
 */

const https = require('https');
const fs = require('fs');
const path = require('path');
const os = require('os');
const crypto = require('crypto');

const GITHUB_REPO = process.env.NEXUS_REPO || 'NexusMemoryTrust-dev-groupe/nexus';
const USER_AGENT = 'nexus-memory-trust-installer';
const DOWNLOAD_TIMEOUT_MS = 120_000;

// ── Platform ────────────────────────────────────────────────────────────────

/**
 * Nexus ships for Windows only. 32-bit (i686) is intentionally unsupported:
 * the ONNX runtime behind semantic search has no 32-bit binaries, and Windows
 * 11 has no 32-bit edition at all.
 */
function assertSupportedPlatform() {
  if (os.platform() !== 'win32') {
    throw new Error(
      `Nexus currently supports Windows only (detected: ${os.platform()}).\n` +
      'Follow https://github.com/' + GITHUB_REPO + ' for other platforms.'
    );
  }
}

/** Returns 'x64' or 'arm64' — the two architectures we publish. */
function detectArch() {
  // On ARM64 Windows, a 32-bit Node reports 'ia32' while the OS is ARM64.
  const envArch = (process.env.PROCESSOR_ARCHITECTURE || '').toLowerCase();
  const wow64 = (process.env.PROCESSOR_ARCHITEW6432 || '').toLowerCase();
  if (envArch.includes('arm') || wow64.includes('arm') || os.arch() === 'arm64') {
    return 'arm64';
  }
  if (os.arch() === 'x64' || envArch === 'amd64' || wow64 === 'amd64') {
    return 'x64';
  }
  throw new Error(
    `Unsupported architecture: ${os.arch()}. Nexus requires 64-bit Windows (x64 or ARM64).`
  );
}

// ── HTTP ────────────────────────────────────────────────────────────────────

function httpGet(url, { json = false, redirects = 5 } = {}) {
  return new Promise((resolve, reject) => {
    const req = https.get(url, { headers: { 'User-Agent': USER_AGENT } }, (res) => {
      const { statusCode, headers } = res;

      if (statusCode >= 300 && statusCode < 400 && headers.location) {
        res.resume();
        if (redirects <= 0) return reject(new Error('Too many redirects'));
        const next = new URL(headers.location, url).toString();
        return httpGet(next, { json, redirects: redirects - 1 }).then(resolve, reject);
      }

      if (statusCode !== 200) {
        res.resume();
        const hint = statusCode === 404
          ? ' (release asset not found — the release may still be a draft)'
          : statusCode === 403
            ? ' (GitHub API rate limit — retry in a few minutes)'
            : '';
        return reject(new Error(`HTTP ${statusCode}${hint}: ${url}`));
      }

      const chunks = [];
      res.on('data', (c) => chunks.push(c));
      res.on('end', () => {
        const buf = Buffer.concat(chunks);
        if (!json) return resolve(buf);
        try {
          resolve(JSON.parse(buf.toString('utf-8')));
        } catch (e) {
          reject(new Error(`Malformed JSON from ${url}: ${e.message}`));
        }
      });
      res.on('error', reject);
    });

    req.on('error', reject);
    req.setTimeout(DOWNLOAD_TIMEOUT_MS, () => {
      req.destroy();
      reject(new Error(`Timed out after ${DOWNLOAD_TIMEOUT_MS / 1000}s: ${url}`));
    });
  });
}

// ── Release resolution ──────────────────────────────────────────────────────

/**
 * Pick the NSIS installer asset for the given architecture.
 *
 * Matches on substrings instead of an exact filename so that
 * `Nexus_1.0.0_x64-setup.exe` and any future rename keep working.
 */
function selectInstallerAsset(assets, arch) {
  const candidates = assets.filter((a) => {
    const n = a.name.toLowerCase();
    return n.endsWith('.exe') && n.includes('setup') && n.includes(arch);
  });
  if (candidates.length > 0) return candidates[0];

  // Fall back to the MSI, which some corporate environments prefer anyway.
  const msi = assets.filter((a) => {
    const n = a.name.toLowerCase();
    return n.endsWith('.msi') && n.includes(arch);
  });
  if (msi.length > 0) return msi[0];

  const available = assets.map((a) => a.name).join(', ') || '(none)';
  throw new Error(
    `No ${arch} Windows installer in the latest release.\nAvailable assets: ${available}`
  );
}

async function fetchLatestRelease() {
  const url = `https://api.github.com/repos/${GITHUB_REPO}/releases/latest`;
  const release = await httpGet(url, { json: true });
  if (!release || !release.tag_name) {
    throw new Error('GitHub returned no published release yet.');
  }
  return release;
}

/**
 * Verify the download against the release checksum file when one is published.
 * A missing checksum file is not fatal — it just means we cannot verify.
 */
async function verifyChecksum(assets, assetName, buffer) {
  const sumsAsset = assets.find((a) => /sha256|checksums/i.test(a.name));
  if (!sumsAsset) return { verified: false, reason: 'no checksum file in release' };

  const text = (await httpGet(sumsAsset.browser_download_url)).toString('utf-8');
  const actual = crypto.createHash('sha256').update(buffer).digest('hex');

  for (const line of text.split(/\r?\n/)) {
    if (!line.trim()) continue;
    const [hash, ...rest] = line.trim().split(/[\s*]+/);
    const file = rest.join(' ').trim();
    if (file && assetName.toLowerCase().endsWith(path.basename(file).toLowerCase())) {
      if (hash.toLowerCase() !== actual) {
        throw new Error(
          `Checksum mismatch for ${assetName}.\n  expected: ${hash}\n  actual:   ${actual}\n` +
          'Refusing to run a tampered installer.'
        );
      }
      return { verified: true };
    }
  }
  return { verified: false, reason: `${assetName} absent from checksum file` };
}

// ── Cache ───────────────────────────────────────────────────────────────────

/** Where the downloaded installer is cached (per-user, no admin needed). */
function cacheDir() {
  const base = process.env.LOCALAPPDATA || path.join(os.homedir(), 'AppData', 'Local');
  return path.join(base, 'Nexus', 'installer');
}

function cachedInstallerPath(assetName) {
  return path.join(cacheDir(), assetName);
}

// ── Installed-app discovery ─────────────────────────────────────────────────

/**
 * Locate an installed Nexus executable.
 *
 * Checks the NSIS uninstall registry keys first (they hold the real install
 * directory, so a user who installed to D: is found correctly), then falls
 * back to the default per-user and per-machine locations.
 */
function findInstalledBinary() {
  const { execFileSync } = require('child_process');
  const roots = [
    'HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall',
    'HKLM\\Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall',
  ];

  for (const root of roots) {
    for (const key of [`${root}\\com.nexus.memorytrust`, `${root}\\Nexus`]) {
      try {
        const out = execFileSync('reg', ['query', key, '/v', 'InstallLocation'], {
          encoding: 'utf-8',
          stdio: ['ignore', 'pipe', 'ignore'],
        });
        const m = out.match(/InstallLocation\s+REG_[A-Z_]+\s+(.+)/);
        if (m) {
          const exe = path.join(m[1].trim(), 'Nexus.exe');
          if (fs.existsSync(exe)) return exe;
        }
      } catch {
        // Key absent — expected for whichever scope was not used.
      }
    }
  }

  const guesses = [
    path.join(process.env.LOCALAPPDATA || '', 'Programs', 'Nexus', 'Nexus.exe'),
    path.join(process.env.PROGRAMFILES || '', 'Nexus', 'Nexus.exe'),
    path.join(process.env['PROGRAMFILES(X86)'] || '', 'Nexus', 'Nexus.exe'),
  ];
  return guesses.find((p) => p && fs.existsSync(p)) || null;
}

// ── Download ────────────────────────────────────────────────────────────────

/**
 * Ensure the installer for the latest release is present in the cache.
 * Returns `{ installerPath, version, assetName, verified }`.
 */
async function ensureInstallerDownloaded({ log = console.log } = {}) {
  assertSupportedPlatform();
  const arch = detectArch();

  log(`  platform: Windows ${arch}`);
  log('  resolving latest release...');
  const release = await fetchLatestRelease();
  const assets = release.assets || [];
  const asset = selectInstallerAsset(assets, arch);

  const target = cachedInstallerPath(asset.name);
  if (fs.existsSync(target) && fs.statSync(target).size === asset.size) {
    log(`  already downloaded: ${asset.name}`);
    return { installerPath: target, version: release.tag_name, assetName: asset.name, verified: null };
  }

  log(`  downloading ${asset.name} (${(asset.size / 1048576).toFixed(1)} MB)...`);
  const buf = await httpGet(asset.browser_download_url);

  const check = await verifyChecksum(assets, asset.name, buf);
  if (check.verified) log('  checksum verified (SHA-256)');
  else log(`  checksum skipped: ${check.reason}`);

  fs.mkdirSync(cacheDir(), { recursive: true });
  // Write to a temp file then rename, so an interrupted download never leaves
  // a truncated installer that looks complete.
  const tmp = `${target}.partial`;
  fs.writeFileSync(tmp, buf);
  fs.renameSync(tmp, target);

  return {
    installerPath: target,
    version: release.tag_name,
    assetName: asset.name,
    verified: check.verified,
  };
}

module.exports = {
  GITHUB_REPO,
  assertSupportedPlatform,
  detectArch,
  httpGet,
  selectInstallerAsset,
  fetchLatestRelease,
  verifyChecksum,
  cacheDir,
  cachedInstallerPath,
  findInstalledBinary,
  ensureInstallerDownloaded,
};
