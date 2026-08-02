#!/usr/bin/env node
'use strict';

/**
 * `nexus` launcher.
 *
 * Behaviour:
 *   1. If Nexus is already installed, start it.
 *   2. Otherwise download the official installer (if not cached) and run it
 *      interactively so the user can choose the install drive, then start the
 *      freshly installed app.
 *
 * The previous version assumed a bare `nexus.exe` sat next to this script,
 * which never existed because the release only ships an NSIS/MSI installer.
 */

const { spawn, spawnSync } = require('child_process');
const path = require('path');
const lib = require('../scripts/nexus-install-lib');

function startApp(exePath) {
  console.log(`  starting ${path.basename(exePath)}...`);
  // `detached` + `unref` so the GUI keeps running after the CLI returns —
  // otherwise closing the terminal would kill the desktop app.
  const child = spawn(exePath, process.argv.slice(2), {
    detached: true,
    stdio: 'ignore',
  });
  child.on('error', (err) => {
    console.error(`  failed to start Nexus: ${err.message}`);
    process.exit(1);
  });
  child.unref();
}

function runInstaller(installerPath) {
  console.log('');
  console.log('  Launching the Nexus installer.');
  console.log('  You can choose the installation drive and folder there.');
  console.log('');

  // Synchronous: we must wait for the install to finish before locating the
  // executable. `cmd /c start /wait` keeps UAC elevation working for NSIS.
  const res = spawnSync('cmd', ['/c', 'start', '/wait', '""', installerPath], {
    stdio: 'inherit',
    windowsVerbatimArguments: true,
  });
  if (res.error) throw res.error;
}

async function main() {
  const installed = lib.findInstalledBinary();
  if (installed) {
    startApp(installed);
    return;
  }

  console.log('');
  console.log('  Nexus is not installed yet.');
  const { installerPath, version } = await lib.ensureInstallerDownloaded();
  console.log(`  version: ${version}`);

  runInstaller(installerPath);

  const nowInstalled = lib.findInstalledBinary();
  if (nowInstalled) {
    startApp(nowInstalled);
  } else {
    console.log('');
    console.log('  Installation did not complete (or was cancelled).');
    console.log('  Re-run "nexus" to try again, or launch the installer manually:');
    console.log(`  ${installerPath}`);
    console.log('');
    process.exit(1);
  }
}

main().catch((err) => {
  console.error('');
  console.error(`  ${err.message}`);
  console.error('');
  console.error('  Download Nexus manually from:');
  console.error(`  https://github.com/${lib.GITHUB_REPO}/releases/latest`);
  console.error('');
  process.exit(1);
});
