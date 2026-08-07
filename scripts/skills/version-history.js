// Skill: version-history <memory_id>
// Full version history of one memory (automatic_commits rows): who changed
// what, when, and why (change_reason). The audit trail of the content itself.
import { open, fail } from './db.js';

const id = process.argv[2];
if (!id) fail('usage: version-history <memory_id>');

const { db } = open();

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
