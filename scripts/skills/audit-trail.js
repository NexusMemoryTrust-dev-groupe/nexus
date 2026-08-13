// Skill: audit-trail [<memory_id>]
// Reconstructs the full decision chain for one memory — the compliance answer
// to "why did we decide this?". Reads memory_records, audit_events and
// automatic_commits directly from the Nexus database (read-only).
// Without an argument, lists the most recent memories (id + title) so the
// caller can pick one.
import { open, fail } from './db.js';

const id = process.argv[2];
const { db } = open();

if (!id) {
  const recent = db
    .prepare('SELECT id, title, status, created_at FROM memory_records ORDER BY created_at DESC LIMIT 20')
    .all();
  if (recent.length === 0) fail('No memories found in the database');
  const out = ['RECENT MEMORIES — pick one and run: node audit-trail.js <memory_id>', ''];
  for (const m of recent) {
    out.push(`  ${m.id}  ${m.title}  (${m.status} · ${m.created_at})`);
  }
  console.log(out.join('\n'));
  process.exit(0);
}

const mem = db
  .prepare('SELECT id, title, summary, status, layer, author, created_at, updated_at, reason FROM memory_records WHERE id = ?')
  .get(id);
if (!mem) fail(`Memory ${id} not found`);

const events = db
  .prepare("SELECT id, event_type, actor, detail, related_memory_id, created_at FROM audit_events WHERE memory_id = ? ORDER BY created_at")
  .all(id);

const commits = db
  .prepare("SELECT version_number, change_type, change_reason, created_at, created_by FROM automatic_commits WHERE entity_id = ? AND entity_type = 'Memory' ORDER BY version_number")
  .all(id);

const out = [];
out.push(`MEMORY  ${mem.title}`);
out.push(`  id:       ${mem.id}`);
out.push(`  state:    ${mem.status} · layer: ${mem.layer}`);
out.push(`  author:   ${mem.author} · created: ${mem.created_at}`);
out.push(`  updated:  ${mem.updated_at}`);
if (mem.reason) out.push(`  reason:   ${mem.reason}`);

out.push(`\nDECISION JOURNAL (${events.length} events)`);
if (events.length === 0) out.push('  (no audit events recorded yet)');
for (const e of events) {
  const line = `  [${e.created_at}] ${e.event_type} by ${e.actor || 'unknown'}`;
  out.push(e.related_memory_id ? `${line} → ${e.related_memory_id}` : line);
  if (e.detail) out.push(`      ${e.detail}`);
}

out.push(`\nVERSION HISTORY (${commits.length} versions)`);
if (commits.length === 0) out.push('  (no versions recorded)');
for (const c of commits) {
  out.push(`  v${c.version_number} ${c.change_type} by ${c.created_by || 'system'} [${c.created_at}]`);
  if (c.change_reason) out.push(`      ${c.change_reason}`);
}

console.log(out.join('\n'));
