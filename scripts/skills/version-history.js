// Skill: version-history [<memory_id>]
// Full version history of one memory (automatic_commits rows): who changed
// what, when, and why (change_reason). The audit trail of the content itself.
// Without an argument, lists the most recent memories (id + title).
import { open, fail } from './db.js';

const id = process.argv[2];
const { db } = open();

if (!id) {
  const recent = db
    .prepare('SELECT id, title, status, created_at FROM memory_records ORDER BY created_at DESC LIMIT 20')
    .all();
  if (recent.length === 0) fail('No memories found in the database');
  const out = ['RECENT MEMORIES — pick one and run: node version-history.js <memory_id>', ''];
  for (const m of recent) {
    out.push(`  ${m.id}  ${m.title}  (${m.status} · ${m.created_at})`);
  }
  console.log(out.join('\n'));
  process.exit(0);
}

const mem = db
  .prepare('SELECT id, title FROM memory_records WHERE id = ?')
  .get(id);
if (!mem) fail(`Memory ${id} not found`);

const commits = db
  .prepare("SELECT version_number, change_type, change_reason, created_at, created_by, size_bytes FROM automatic_commits WHERE entity_id = ? AND entity_type = 'Memory' ORDER BY version_number")
  .all(id);

const out = [`VERSION HISTORY — ${mem.title} (${commits.length} version(s))`];
if (commits.length === 0) out.push('  (no versions recorded)');
for (const c of commits) {
  out.push(`  v${c.version_number}  ${c.change_type}  by ${c.created_by || 'system'}  [${c.created_at}]  (${c.size_bytes} B)`);
  if (c.change_reason) out.push(`      ${c.change_reason}`);
}
console.log(out.join('\n'));
