// Shared DB access for Nexus skill scripts.
// Resolves the database path the same way the Rust backend does:
//   %LOCALAPPDATA%/Nexus/nexus.db on Windows, ~/.nexus/nexus.db otherwise.
// Override with NEXUS_DB_PATH if the app uses a custom location.
import { DatabaseSync } from 'node:sqlite';
import path from 'node:path';
import os from 'node:os';

export function dbPath() {
  if (process.env.NEXUS_DB_PATH) return process.env.NEXUS_DB_PATH;
  if (process.platform === 'win32') {
    const base = process.env.LOCALAPPDATA || path.join(os.homedir(), 'AppData', 'Local');
    return path.join(base, 'Nexus', 'nexus.db');
  }
  return path.join(os.homedir(), '.nexus', 'nexus.db');
}

export function open() {
  const p = dbPath();
  const db = new DatabaseSync(p, { readOnly: true });
  return { db, path: p };
}

export function fail(msg) {
  process.stderr.write(`ERROR: ${msg}\n`);
  process.exit(1);
}
