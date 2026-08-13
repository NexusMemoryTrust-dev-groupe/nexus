import { useCallback, useEffect, useState } from 'react';
import type { CSSProperties } from 'react';
import { invoke } from '@tauri-apps/api/core';
import {
  AlertTriangle, BadgeCheck, Bot, CheckCircle2, Lock, Plus, RefreshCw,
  Shield, ShieldCheck, Trash2, Wrench,
} from 'lucide-react';
import type { AgentPassport } from '../../types';
import {
  InfoTip, PageHero, StrataAlert, StrataModal, StrataSelect, StrataSkeletons, StrataVoid,
} from '../ui/Instruments';

// ── Passport card ───────────────────────────────────────────────────────────

function PassportCard({
  passport,
  onActivate,
  onDelete,
}: {
  passport: AgentPassport;
  onActivate: (p: AgentPassport) => void;
  onDelete: (p: AgentPassport) => void;
}) {
  const trust = passport.trustLevel;
  return (
    <div className={`st-passport${passport.isActive ? ' is-active' : ''}`} style={{ '--pass-color': passport.isActive ? 'var(--mint)' : 'var(--periwinkle)' } as CSSProperties}>
      <div className="st-passport-head">
        <span className="st-passport-avatar"><Bot size={18} /></span>
        <span style={{ minWidth: 0, flex: 1 }}>
          <span className="st-passport-name">
            {passport.displayName || passport.name}
            {passport.isActive && <BadgeCheck size={13} style={{ color: 'var(--mint)' }} />}
          </span>
          <span className="st-passport-meta">
            {passport.name} · {passport.role} · {passport.memoryScope} scope
          </span>
        </span>
        <span className="st-passport-trust" title={`Trust level ${trust}/10`}>
          <Shield size={12} /> {trust}/10
        </span>
      </div>
      {passport.description && <p className="st-passport-desc">{passport.description}</p>}

      <div className="st-passport-section">
        <span className="st-passport-section-label"><Wrench size={11} /> skills</span>
        <div className="st-passport-chips">
          {passport.skills.length === 0
            ? <span className="st-passport-chip st-passport-chip--empty">none</span>
            : passport.skills.map((s) => <span key={s} className="st-passport-chip">{s}</span>)}
        </div>
      </div>
      <div className="st-passport-section">
        <span className="st-passport-section-label"><Bot size={11} /> tools</span>
        <div className="st-passport-chips">
          {passport.tools.length === 0
            ? <span className="st-passport-chip st-passport-chip--empty">none</span>
            : passport.tools.map((t) => <span key={t} className="st-passport-chip">{t}</span>)}
        </div>
      </div>
      <div className="st-passport-section">
        <span className="st-passport-section-label"><Lock size={11} /> constraints</span>
        <div className="st-passport-chips">
          {passport.constraints.length === 0
            ? <span className="st-passport-chip st-passport-chip--empty">none</span>
            : passport.constraints.map((c) => <span key={c} className="st-passport-chip st-passport-chip--constraint">{c}</span>)}
        </div>
      </div>

      <div className="st-passport-actions">
        {!passport.isActive && (
          <button type="button" className="st-btn" onClick={() => onActivate(passport)}>
            <CheckCircle2 size={13} style={{ color: 'var(--mint)' }} /> Set active
          </button>
        )}
        <button type="button" className="st-btn st-btn--ghost" onClick={() => onDelete(passport)}>
          <Trash2 size={13} style={{ color: 'var(--rose)' }} /> Delete
        </button>
      </div>
    </div>
  );
}

// ── Main view ───────────────────────────────────────────────────────────────

export function PassportView() {
  const [passports, setPassports] = useState<AgentPassport[]>([]);
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // New passport form
  const [name, setName] = useState('');
  const [displayName, setDisplayName] = useState('');
  const [role, setRole] = useState('generalist');
  const [description, setDescription] = useState('');
  const [skills, setSkills] = useState('');
  const [tools, setTools] = useState('');
  const [constraints, setConstraints] = useState('');
  const [trustLevel, setTrustLevel] = useState(5);
  const [memoryScope, setMemoryScope] = useState('project');
  const [formOpen, setFormOpen] = useState(false);
  const [pendingDelete, setPendingDelete] = useState<AgentPassport | null>(null);

  const load = useCallback(async () => {
    setBusy(true);
    try {
      const list = await invoke<AgentPassport[]>('passport_list');
      setPassports(list);
      setError(null);
    } catch (err) {
      setError(String(err));
    } finally {
      setBusy(false);
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  const upsert = useCallback(async () => {
    if (!name.trim()) return;
    setBusy(true);
    try {
      await invoke('passport_upsert', {
        name: name.trim(),
        displayName: displayName || null,
        role,
        description: description || null,
        skills: skills.split(',').map((s) => s.trim()).filter(Boolean),
        tools: tools.split(',').map((s) => s.trim()).filter(Boolean),
        constraints: constraints.split(',').map((s) => s.trim()).filter(Boolean),
        trustLevel,
        memoryScope,
      });
      setName('');
      setFormOpen(false);
      await load();
    } catch (err) {
      setError(String(err));
    } finally {
      setBusy(false);
    }
  }, [name, displayName, role, description, skills, tools, constraints, trustLevel, memoryScope, load]);

  const activate = useCallback(async (p: AgentPassport) => {
    try {
      await invoke('passport_set_active', { name: p.name, active: true });
      await load();
    } catch (err) {
      setError(String(err));
    }
  }, [load]);

  const requestDelete = useCallback((p: AgentPassport) => setPendingDelete(p), []);

  const confirmDelete = useCallback(async () => {
    if (!pendingDelete) return;
    try {
      await invoke('passport_delete', { name: pendingDelete.name });
      setPendingDelete(null);
      await load();
    } catch (err) {
      setError(String(err));
      setPendingDelete(null);
    }
  }, [pendingDelete, load]);

  const activeCount = passports.filter((p) => p.isActive).length;

  const hero = (
    <PageHero
      kicker="System 7 · Agent Identity"
      title="Agent Passports"
      copy="Every agent carries an identity card: role, trust level, memory scope, the skills and tools it may use, and its constraints. The active passport defines how the ecosystem behaves right now."
      accent="var(--cyan)"
      secondary="var(--periwinkle)"
      stats={[
        { label: 'Passports', value: String(passports.length), color: 'var(--cyan)' },
        { label: 'Active', value: String(activeCount), color: 'var(--mint)' },
      ]}
    />
  );

  const actions = (
    <div className="st-radar-actions">
      <button type="button" className="st-action-btn" disabled={busy} onClick={() => void load()}>
        <RefreshCw size={13} className={busy ? 'spinning' : undefined} />
        Refresh
      </button>
    </div>
  );

  if (loading) return <div className="st-page">{hero}{actions}<StrataSkeletons /></div>;

  if (error) {
    return (
      <div className="st-page" style={{ '--st-accent': 'var(--cyan)' } as CSSProperties}>
        {hero}{actions}
        <StrataAlert icon={AlertTriangle}>{error}</StrataAlert>
      </div>
    );
  }

  return (
    <div className="st-page" style={{ '--st-accent': 'var(--cyan)' } as CSSProperties}>
      {hero}{actions}

      {/* New passport */}
      <div className="st-sys-toolbar">
        <button type="button" className="st-btn" onClick={() => setFormOpen(!formOpen)}>
          <Plus size={13} /> {formOpen ? 'Cancel' : 'New passport'}
        </button>
      </div>
      {formOpen && (
        <div className="st-sys-probe st-sys-probe--grid">
          <input className="st-sys-input" placeholder="Agent name *" value={name} onChange={(e) => setName(e.target.value)} />
          <input className="st-sys-input" placeholder="Display name" value={displayName} onChange={(e) => setDisplayName(e.target.value)} />
          <StrataSelect
            value={role}
            onChange={setRole}
            ariaLabel="Role"
            options={['generalist', 'researcher', 'writer', 'reviewer', 'automation', 'archivist'].map((r) => ({ value: r, label: r }))}
          />
          <StrataSelect
            value={memoryScope}
            onChange={setMemoryScope}
            ariaLabel="Memory scope"
            options={['project', 'team', 'personal', 'global'].map((s) => ({ value: s, label: s }))}
          />
          <input className="st-sys-input" type="number" min={1} max={10} value={trustLevel} onChange={(e) => setTrustLevel(Number(e.target.value))} title="Trust level 1-10" />
          <input className="st-sys-input" placeholder="Description" value={description} onChange={(e) => setDescription(e.target.value)} />
          <input className="st-sys-input" placeholder="Skills (comma separated)" value={skills} onChange={(e) => setSkills(e.target.value)} />
          <input className="st-sys-input" placeholder="Tools (comma separated)" value={tools} onChange={(e) => setTools(e.target.value)} />
          <input className="st-sys-input" placeholder="Constraints (comma separated)" value={constraints} onChange={(e) => setConstraints(e.target.value)} />
          <button type="button" className="st-btn" disabled={!name.trim()} onClick={upsert}>
            <ShieldCheck size={13} /> Save passport
          </button>
        </div>
      )}

      {/* Passports */}
      <div className="st-section-head" style={{ margin: '18px 0 10px' }}>
        <h2 className="st-section-title">Passports <InfoTip text="The active passport (green) defines the ecosystem's current identity. Passports are consulted by the firewall when agents request memory." /></h2>
      </div>
      <p className="st-section-desc">Agent identities — the active passport (green) defines the ecosystem's current identity and is consulted by the firewall when agents request memory.</p>
      {passports.length === 0 ? (
        <StrataVoid icon={Bot} title="No passports yet">
          Create the first passport above — or ask Copilot with <code>/passport create &lt;name&gt;</code>.
        </StrataVoid>
      ) : (
        <div className="st-passport-grid">
          {passports.map((p) => (
            <PassportCard key={p.name} passport={p} onActivate={activate} onDelete={requestDelete} />
          ))}
        </div>
      )}

      {pendingDelete && (
        <StrataModal
          title="Delete passport"
          icon={AlertTriangle}
          accent="var(--rose)"
          onClose={() => setPendingDelete(null)}
          footer={
            <>
              <button type="button" className="st-btn st-btn--ghost" onClick={() => setPendingDelete(null)}>
                Cancel
              </button>
              <button
                type="button"
                className="st-btn"
                style={{ '--st-accent': 'var(--rose)' } as CSSProperties}
                onClick={() => void confirmDelete()}
              >
                <Trash2 size={13} /> Delete
              </button>
            </>
          }
        >
          Delete the passport for <strong style={{ color: 'var(--bone)' }}>{pendingDelete.name}</strong>? The identity card is removed and the agent loses its role, trust level and memory scope mapping.
        </StrataModal>
      )}
    </div>
  );
}
