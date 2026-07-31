#!/usr/bin/env node

/**
 * Nexus Memory Trust Launcher
 * 
 * This script launches the Nexus desktop application.
 * The binary is downloaded during npm install via postinstall script.
 */

const { execSync, spawn } = require('child_process');
const path = require('path');
const fs = require('fs');
const os = require('os');

// Get binary path
function getBinaryPath() {
  const binDir = path.join(__dirname);
  const ext = os.platform() === 'win32' ? '.exe' : '';
  const binaryPath = path.join(binDir, `nexus${ext}`);
  
  if (!fs.existsSync(binaryPath)) {
    throw new Error(
      'Nexus binary not found. Please reinstall:\n' +
      '  npm install -g nexus-memory-trust'
    );
  }
  
  return binaryPath;
}

// Main launcher
function launch() {
  try {
    const binaryPath = getBinaryPath();
    
    console.log('🚀 Starting Nexus Memory Trust...');
    
    // Spawn the process
    const child = spawn(binaryPath, [], {
      stdio: 'inherit',
      detached: false,
    });
    
    // Handle exit
    child.on('close', (code) => {
      if (code !== 0) {
        console.error(`Nexus exited with code ${code}`);
        process.exit(code);
      }
    });
    
    // Handle errors
    child.on('error', (err) => {
      if (err.code === 'ENOENT') {
        console.error('❌ Nexus binary not found. Please reinstall:');
        console.error('   npm install -g nexus-memory-trust');
      } else {
        console.error('❌ Failed to start Nexus:', err.message);
      }
      process.exit(1);
    });
    
    // Handle SIGINT (Ctrl+C)
    process.on('SIGINT', () => {
      child.kill('SIGTERM');
      process.exit(0);
    });
    
  } catch (error) {
    console.error('❌', error.message);
    process.exit(1);
  }
}

// Run
launch();
