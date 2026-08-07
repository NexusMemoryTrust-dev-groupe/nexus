// Skill: code-search <query> [limit]
// Searches the indexed code graph (code_symbols + code_files): functions,
// structs, types and modules matching the query. The answer to "where is X
// implemented?" without opening the editor. Default limit 15.
import { open, fail } from './db.js';

const q = (process.argv[2] || '').trim();
if (!q) fail('usage: code-search <query> [limit]');

const limit = Math.min(50, Math.max(1, parseInt(process.argv[3] || '15', 10) || 15));

const { db } = open();

let rows;
try {
  rows = db.prepare(`
    SELECT s.name, s.kind, s.line, f.path
    FROM code_symbols s
    JOIN code_files f ON f.id = s.file_id
    WHERE s.name LIKE ? OR s.name LIKE ?
    ORDER BY s.name
    LIMIT ?
  `).all(`%${q}%`, `%${q}%`, limit);
} catch {
  rows = [];
}

if (rows.length === 0) {
  console.log(`No code symbols match "${q}". (Is the code graph indexed? Use code_import / workspace sync.)`);
  process.exit(0);
}

const out = [`CODE SEARCH "${q}" — ${rows.length} symbol(s)`];
for (const r of rows) {
  out.push(`  ${r.kind.padEnd(8)} ${r.name}  → ${r.path}:${r.line}`);
}
console.log(out.join('\n'));
