/**
 * Nexus MCP Client Library for Node.js
 * 
 * Solves JSON parsing issues when calling Nexus MCP from PowerShell/Windows.
 * Spawns `nexus --mcp` and communicates via JSON-RPC over stdio.
 * 
 * Usage:
 *   const { start, call, stop } = require('./nexus-mcp-lib');
 *   await start();
 *   const result = await call('nexus_search_memories', { query: 'architecture' });
 *   console.log(result.result.content[0].text);
 *   stop();
 */

const { spawn } = require('child_process');

// Auto-detect nexus path based on platform
function getNexusPath() {
  const os = require('os');
  if (os.platform() === 'win32') {
    // Common install locations
    const paths = [
      `${os.homedir()}\\AppData\\Local\\Nexus\\nexus.exe`,
      `C:\\Program Files\\Nexus\\nexus.exe`,
    ];
    const fs = require('fs');
    for (const p of paths) {
      if (fs.existsSync(p)) return p;
    }
    // Fallback — assume nexus is in PATH
    return 'nexus';
  }
  return 'nexus';
}

let id = 0;
let proc = null;
let pending = {};

function start() {
  return new Promise((resolve) => {
    const nexusPath = getNexusPath();
    proc = spawn(nexusPath, ['--mcp'], { stdio: ['pipe', 'pipe', 'pipe'] });
    
    proc.stdout.on('data', (data) => {
      const lines = data.toString().split('\n').filter(l => l.trim());
      for (const line of lines) {
        try {
          const msg = JSON.parse(line);
          if (msg.id && pending[msg.id]) {
            pending[msg.id](msg);
            delete pending[msg.id];
          }
        } catch (e) {}
      }
    });
    
    proc.stderr.on('data', () => {});
    setTimeout(resolve, 500);
  });
}

function call(tool, args = {}) {
  return new Promise((resolve, reject) => {
    const myId = ++id;
    const request = JSON.stringify({
      jsonrpc: '2.0',
      id: myId,
      method: 'tools/call',
      params: { name: tool, arguments: args }
    });
    pending[myId] = resolve;
    proc.stdin.write(request + '\n');
    setTimeout(() => {
      if (pending[myId]) {
        delete pending[myId];
        reject(new Error('Timeout'));
      }
    }, 10000);
  });
}

function stop() {
  if (proc) proc.kill();
}

module.exports = { start, call, stop };
