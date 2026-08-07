// Skill: team-roster [--active]
// Lists team members with roles and per-member activity (authored / confirmed
// / updated memory counts). The trusted decision layer of the team.
import { open } from './db.js';

const { db } = open();

const members = db
  .prepare('SELECT id, name, role, active, created_at, updated_at FROM team_members ORDER BY name')
  .all();

if (members.length === 0) {
  console.log('No team members yet. Add one via Team Memory.');
  process.exit(0);
}

const stats = db.prepare(`
  SELECT author AS name,
         COUNT(*) AS total,
         SUM(CASE WHEN status = 'Confirmed' THEN 1 ELSE 0 END) AS confirmed,
         SUM(CASE WHEN status = 'Superseded' THEN 1 ELSE 0 END) AS superseded
  FROM memory_records GROUP BY author
`).all();
const byName = new Map(stats.map((s) => [s.name, s]));

const out = ['TEAM ROSTER'];
for (const m of members) {
  if (!m.active) continue;
  const s = byName.get(m.name);
  out.push(`  ${m.name.padEnd(20)} role=${m.role}  authored=${s ? s.total : 0} confirmed=${s ? s.confirmed : 0} superseded=${s ? s.superseded : 0}`);
}
console.log(out.join('\n'));
