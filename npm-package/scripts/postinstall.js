#!/usr/bin/env node

/**
 * Postinstall script for nexus-memory-trust
 * 
 * Downloads the pre-built Nexus binary for the current platform
 * from GitHub Releases.
 */

const https = require('https');
const fs = require('fs');
const path = require('path');
const { execSync } = require('child_process');
const os = require('os');

const GITHUB_REPO = 'NexusMemoryTrust-dev-groupe/nexus';
const BINARY_NAME = 'nexus';

// Platform detection
function getPlatform() {
  const platform = os.platform();
  switch (platform) {
    case 'win32': return 'windows';
    case 'darwin': return 'macos';
    case 'linux': return 'linux';
    default: throw new Error(`Unsupported platform: ${platform}`);
  }
}

// Architecture detection
function getArch() {
  const arch = os.arch();
  switch (arch) {
    case 'x64': return 'x64';
    case 'arm64': return 'arm64';
    default: throw new Error(`Unsupported architecture: ${arch}`);
  }
}

// Get binary extension
function getBinaryExt() {
  return getPlatform() === 'windows' ? '.exe' : '';
}

// Get release asset name
function getAssetName() {
  const platform = getPlatform();
  const ext = getBinaryExt();
  
  switch (platform) {
    case 'windows':
      return `nexus-windows-x64${ext}`;
    case 'macos':
      return `nexus-macos-arm64${ext}`;
    case 'linux':
      return `nexus-linux-x64${ext}`;
    default:
      throw new Error(`No binary available for ${platform}`);
  }
}

// Download file from URL
function download(url) {
  return new Promise((resolve, reject) => {
    const request = https.get(url, { headers: { 'User-Agent': 'nexus-memory-trust-installer' } }, (response) => {
      // Handle redirects
      if (response.statusCode === 302 || response.statusCode === 301) {
        return download(response.headers.location).then(resolve).catch(reject);
      }
      
      if (response.statusCode !== 200) {
        reject(new Error(`Download failed: HTTP ${response.statusCode}`));
        return;
      }
      
      const chunks = [];
      response.on('data', (chunk) => chunks.push(chunk));
      response.on('end', () => resolve(Buffer.concat(chunks)));
      response.on('error', reject);
    });
    
    request.on('error', reject);
    request.setTimeout(30000, () => {
      request.destroy();
      reject(new Error('Download timeout'));
    });
  });
}

// Get latest release version from GitHub
async function getLatestVersion() {
  const url = `https://api.github.com/repos/${GITHUB_REPO}/releases/latest`;
  
  return new Promise((resolve, reject) => {
    https.get(url, { headers: { 'User-Agent': 'nexus-memory-trust-installer' } }, (response) => {
      let data = '';
      response.on('data', (chunk) => data += chunk);
      response.on('end', () => {
        try {
          const release = JSON.parse(data);
          resolve(release.tag_name);
        } catch (e) {
          reject(new Error('Failed to parse GitHub response'));
        }
      });
      response.on('error', reject);
    }).on('error', reject);
  });
}

// Main installation
async function install() {
  console.log('🔧 Nexus Memory Trust Installer');
  console.log('━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━');
  
  try {
    const platform = getPlatform();
    const arch = getArch();
    console.log(`📋 Platform: ${platform} (${arch})`);
    
    // Get latest version
    console.log('🔍 Checking latest version...');
    const version = await getLatestVersion();
    console.log(`📦 Latest version: ${version}`);
    
    // Get asset name
    const assetName = getAssetName();
    console.log(`📥 Downloading: ${assetName}`);
    
    // Download binary
    const downloadUrl = `https://github.com/${GITHUB_REPO}/releases/download/${version}/${assetName}`;
    const binary = await download(downloadUrl);
    
    // Create bin directory
    const binDir = path.join(__dirname, '..', 'bin');
    if (!fs.existsSync(binDir)) {
      fs.mkdirSync(binDir, { recursive: true });
    }
    
    // Save binary
    const binaryPath = path.join(binDir, `${BINARY_NAME}${getBinaryExt()}`);
    fs.writeFileSync(binaryPath, binary);
    
    // Make executable on Unix
    if (platform !== 'windows') {
      fs.chmodSync(binaryPath, '755');
    }
    
    console.log('━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━');
    console.log('✅ Installation complete!');
    console.log('');
    console.log('🚀 Run Nexus:');
    console.log('   nexus');
    console.log('');
    console.log('📚 Documentation:');
    console.log('   https://github.com/NexusMemoryTrust-dev-groupe/nexus#readme');
    
  } catch (error) {
    console.error('');
    console.error('❌ Installation failed:', error.message);
    console.error('');
    console.error('💡 Try downloading manually from:');
    console.error(`   https://github.com/${GITHUB_REPO}/releases`);
    process.exit(1);
  }
}

// Run if called directly
if (require.main === module) {
  install();
}

module.exports = { install };
