#!/usr/bin/env node
'use strict';

/**
 * Postinstall for `nexus-memory-trust`.
 *
 * Downloads (and caches) the official Windows installer for the latest
 * release. It deliberately does not *run* the installer here: npm lifecycle
 * scripts must stay non-interactive, and launching a UAC prompt from
 * `npm install` is hostile. `nexus` (bin/nexus.js) performs the actual
 * install on first run, where the user can pick the target drive.
 *
 * A postinstall failure must never break `npm install` — the launcher retries
 * and prints actionable guidance, so this script always exits 0.
 */

const lib = require('./nexus-install-lib');

async function main() {
  console.log('');
  console.log('  Nexus Memory Trust');
  console.log('  ------------------');

  const existing = lib.findInstalledBinary();
  if (existing) {
    console.log(`  already installed: ${existing}`);
    console.log('  run "nexus" to start.');
    console.log('');
    return;
  }

  const { version, assetName } = await lib.ensureInstallerDownloaded();
  console.log('');
  console.log(`  installer ready (${version}): ${assetName}`);
  console.log('  run "nexus" to install and start Nexus.');
  console.log('');
}

main().catch((err) => {
  console.log('');
  console.log(`  Could not pre-download the installer: ${err.message}`);
  console.log('  This is not fatal — run "nexus" to retry, or download from:');
  console.log(`  https://github.com/${lib.GITHUB_REPO}/releases/latest`);
  console.log('');
  // Exit 0 on purpose: a transient network error must not fail `npm install`.
  process.exit(0);
});
