// Skill: radar-scan [limit]
// Latest memory activity — the "radar" view: newest, most important and
// recently confirmed memories. Default limit 12, pass a number to override.
import { open } from './db.js';

const { db } = open();
const limit = Math.min(50, Math.max(1, parseInt(process.argv[2] || '12', 10) || 12));

const rows = db.prepare(`
  SELECT id, title, status, layer, author, importance_score, created_at, updated_at
  FROM memory_records
  ORDER BY updated_at DESC
  LIMIT ?
`).all(limit);

if (rows.length === 0) {
  console.log('No memories recorded yet.');
  process.exit(0);
}

const out = [`RADAR — ${rows.length} most recently updated memories`];
for (const r of rows) {
  out.push(`  [${r.status.padEnd(10)}] ${r.title}  (${r.layer}, imp ${r.importance_score.toFixed(2)}) by ${r.author || '?'} — updated ${r.updated_at}`);
  out.push(`      id: ${r.id}`);
}
console.log(out.join('\n'));
