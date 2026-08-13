import { useCallback, useEffect, useState } from 'react';
import type { CSSProperties } from 'react';
import { invoke } from '@tauri-apps/api/core';
import {
  AlertTriangle, CheckCircle2, FileCode2, Lightbulb, Play, RefreshCw,
  Search, Sparkles, XCircle, Zap,
} from 'lucide-react';
import type { Skill, SkillOutput, SkillProposal } from '../../types';
import {
  InfoTip, PageHero, StrataAlert, StrataSkeletons, StrataVoid,
} from '../ui/Instruments';

// ── Skill row ───────────────────────────────────────────────────────────────

function SkillRow({ skill }: { skill: Skill }) {
  const [output, setOutput] = useState<SkillOutput | null>(null);
  const [busy, setBusy] = useState(false);

  const run = useCallback(async () => {
    setBusy(true);
    try {
      const out = await invoke<SkillOutput>('skills_run', { name: skill.name, args: null });
      setOutput(out);
    } catch (err) {
      setOutput({ success: false, stdout: '', stderr: String(err), exit_code: 1, duration_ms: 0, timed_out: false });
    } finally {
      setBusy(false);
    }
  }, [skill.name]);

  return (
    <div className="st-skill" style={{ '--skill-color': skill.enabled ? 'var(--tangerine)' : 'var(--muted-2)' } as CSSProperties}>
      <span className="st-skill-icon"><FileCode2 size={15} /></span>
      <span style={{ minWidth: 0, flex: 1 }}>
        <span className="st-skill-head">
          <span className="st-skill-name">{skill.name}</span>
          <span className="st-skill-meta">
            <code>{skill.command}</code> · {skill.enabled ? 'enabled' : 'disabled'}
          </span>
        </span>
        <span className="st-skill-desc">{skill.description}</span>
        {output && (
          <span className={`st-skill-output${output.success ? ' is-ok' : ' is-err'}`}>
            {output.success
              ? (output.stdout || '(no output)')
              : (output.stderr || '(failed)')}
            <span className="st-skill-output-meta">
              {output.duration_ms}ms{output.timed_out ? ' · timed out' : ''} · exit {output.exit_code ?? '?'}
            </span>
          </span>
        )}
      </span>
      <button type="button" className="st-btn" disabled={busy || !skill.enabled} onClick={() => void run()}>
        {busy ? <RefreshCw size={13} className="spinning" /> : <Play size={13} />}
        Run
      </button>
    </div>
  );
}

// ── Proposal row ────────────────────────────────────────────────────────────

function ProposalRow({
  proposal,
  onDecide,
}: {
  proposal: SkillProposal;
  onDecide: (p: SkillProposal, approve: boolean) => void;
}) {
  const statusColor =
    proposal.status === 'approved' ? 'var(--mint)' :
    proposal.status === 'rejected' ? 'var(--rose)' : 'var(--gold)';
  return (
    <div className="st-skill" style={{ '--skill-color': statusColor } as CSSProperties}>
      <span className="st-skill-icon" style={{ color: statusColor, background: `${statusColor}15` }}>
        <Lightbulb size={15} />
      </span>
      <span style={{ minWidth: 0, flex: 1 }}>
        <span className="st-skill-head">
          <span className="st-skill-name">{proposal.name}</span>
          <span className="st-skill-meta">
            {proposal.category} · {proposal.action} · ×{proposal.occurrences} · {proposal.status}
          </span>
        </span>
        <span className="st-skill-desc">{proposal.description}</span>
      </span>
      {proposal.status === 'proposed' && (
        <button
          type="button"
          className="st-btn"
          onClick={() => onDecide(proposal, true)}
          title="Create the actual skill"
        >
          <CheckCircle2 size={13} style={{ color: 'var(--mint)' }} /> Approve
        </button>
      )}
      {proposal.status === 'proposed' && (
        <button
          type="button"
          className="st-btn st-btn--ghost"
          onClick={() => onDecide(proposal, false)}
          title="Reject this proposal"
        >
          <XCircle size={13} style={{ color: 'var(--rose)' }} /> Reject
        </button>
      )}
    </div>
  );
}

// ── Main view ───────────────────────────────────────────────────────────────

export function SkillsView() {
  const [skills, setSkills] = useState<Skill[]>([]);
  const [proposals, setProposals] = useState<SkillProposal[]>([]);
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(async () => {
    setBusy(true);
    try {
      const [s, p] = await Promise.all([
        invoke<Skill[]>('skills_list'),
        invoke<SkillProposal[]>('skill_genesis_candidates', { status: null }),
      ]);
      setSkills(s);
      setProposals(p);
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

  const scan = useCallback(async () => {
    setBusy(true);
    try {
      const result = await invoke<{ new_proposals: number }>('skill_genesis_scan', {
        limit: 2000,
        minOccurrences: 3,
      });
      await load();
      if (result.new_proposals > 0) {
        // The proposals list refreshed; nothing else to surface.
      }
    } catch (err) {
      setError(String(err));
    } finally {
      setBusy(false);
    }
  }, [load]);

  const decide = useCallback(async (proposal: SkillProposal, approve: boolean) => {
    try {
      await invoke(approve ? 'skill_genesis_approve' : 'skill_genesis_reject', { id: proposal.id });
      await load();
    } catch (err) {
      setError(String(err));
    }
  }, [load]);

  const hero = (
    <PageHero
      kicker="System 8 · Capability Factory"
      title="Skills & Skill Genesis"
      copy="Runnable capabilities live here as scripts. Skill Genesis watches the flight log for repeated operations and proposes turning them into permanent skills — approve to create the real thing."
      accent="var(--tangerine)"
      secondary="var(--gold)"
      stats={[
        { label: 'Skills', value: String(skills.length), color: 'var(--tangerine)' },
        { label: 'Proposals', value: String(proposals.filter((p) => p.status === 'proposed').length), color: 'var(--gold)' },
      ]}
    />
  );

  const actions = (
    <div className="st-radar-actions">
      <button type="button" className="st-action-btn" disabled={busy} onClick={scan}>
        {busy ? <RefreshCw size={13} className="spinning" /> : <Search size={13} />}
        Scan for new skills
      </button>
      <button type="button" className="st-action-btn" disabled={busy} onClick={() => void load()}>
        <RefreshCw size={13} className={busy ? 'spinning' : undefined} />
        Refresh
      </button>
    </div>
  );

  if (loading) return <div className="st-page">{hero}{actions}<StrataSkeletons /></div>;

  if (error) {
    return (
      <div className="st-page" style={{ '--st-accent': 'var(--tangerine)' } as CSSProperties}>
        {hero}{actions}
        <StrataAlert icon={AlertTriangle}>{error}</StrataAlert>
      </div>
    );
  }

  return (
    <div className="st-page" style={{ '--st-accent': 'var(--tangerine)' } as CSSProperties}>
      {hero}{actions}

      {/* Skill Genesis proposals */}
      <div className="st-section-head" style={{ margin: '4px 0 10px' }}>
        <h2 className="st-section-title" style={{ '--section-color': 'var(--tangerine)' } as CSSProperties}>
          <Sparkles size={14} /> Skill Genesis proposals
        </h2>
        <InfoTip text="Patterns detected in the flight log. Approving creates a real runnable skill; rejecting marks the signature as known so it is never proposed again." />
      </div>
      <p className="st-section-desc">Patterns detected in the flight log. Approving creates a real runnable skill; rejecting marks the signature as known so it is never proposed again.</p>
      {proposals.length === 0 ? (
        <StrataVoid icon={Sparkles} title="No proposals yet">
          Run <code>skill_genesis_scan</code> (the button above) to detect repeated operations worth turning into skills.
        </StrataVoid>
      ) : (
        <div className="st-panel">
          <div className="st-skill-list">
            {proposals.map((p) => <ProposalRow key={p.id} proposal={p} onDecide={decide} />)}
          </div>
        </div>
      )}

      {/* Skills */}
      <div className="st-section-head" style={{ margin: '26px 0 10px' }}>
        <h2 className="st-section-title" style={{ '--section-color': 'var(--tangerine)' } as CSSProperties}>
          <Zap size={14} /> Runnable skills
        </h2>
        <InfoTip text="Each skill runs a script with a 30s timeout. Run one here to verify it works end to end." />
      </div>
      <p className="st-section-desc">Runnable capabilities installed in the ecosystem. Each skill runs a script with a 30s timeout — run one here to verify it works end to end.</p>
      {skills.length === 0 ? (
        <StrataVoid icon={Zap} title="No skills installed">
          Skills can be registered via <code>skills_register</code>, approved from proposals above, or seeded during setup.
        </StrataVoid>
      ) : (
        <div className="st-panel">
          <div className="st-skill-list">
            {skills.map((s) => <SkillRow key={s.id} skill={s} />)}
          </div>
        </div>
      )}
    </div>
  );
}
