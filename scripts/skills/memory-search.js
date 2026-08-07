// Skill: memory-search <query> [limit]
// Full-text search over memories (title/summary/content) using the FTS5
// index. Prints ranked hits with state, layer and id. Default limit 10.
import { open, fail } from './db.js';

const q = (process.argv[2] || '').trim();
if (!q) fail('usage: memory-search <query> [limit]');

const limit = Math.min(30, Math.max(1, parseInt(process.argv[3] || '10', 10) || 10));

const { db } = open();

// FTS5 MATCH needs the query quoted to survive special characters; fall back
// to a plain LIKE scan when the tokenizer rejects the input.
let rows;
try {
  rows = db.prepare(`
    SELECT m.id, m.title, m.summary, m.status, m.layer, m.author, m.updated_at,
           bm25(memory_fts) AS rank
    FROM memory_fts
    JOIN memory_records m ON m.id = memory_fts.id
    WHERE memory_fts MATCH ?
    ORDER BY rank
    LIMIT ?
  `).all(`"${q.replace(/"/g, '""')}"`, limit);
} catch {
  rows = db.prepare(`
    SELECT id, title, summary, status, layer, author, updated_at, 0 AS rank
    FROM memory_records
    WHERE title LIKE ? OR summary LIKE ? OR content LIKE ?
    ORDER BY updated_at DESC
    LIMIT ?
  `).all(`%${q}%`, `%${q}%`, `%${q}%`, limit);
}

if (rows.length === 0) {
  console.log(`No memories match "${q}".`);
  process.exit(0);
}

const out = [`SEARCH "${q}" — ${rows.length} hit(s)`];
for (const r of rows) {
  out.push(`  [${r.status.padEnd(10)}] ${r.title}`);
  if (r.summary) out.push(`      ${r.summary.slice(0, 140)}`);
  out.push(`      ${r.layer} · by ${r.author || '?'} · ${r.updated_at}`);
  out.push(`      id: ${r.id}`);
}
console.log(out.join('\n'));
