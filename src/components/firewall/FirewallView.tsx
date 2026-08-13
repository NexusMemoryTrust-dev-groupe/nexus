import { useCallback, useEffect, useState } from 'react';
import type { CSSProperties } from 'react';
import { invoke } from '@tauri-apps/api/core';
import {
  AlertTriangle, Ban, CheckCircle2, Eye, EyeOff, FileWarning, Plus, RefreshCw,
  ShieldAlert, ShieldCheck, ShieldQuestion, Trash2, UserCog, UserX,
} from 'lucide-react';
import type {
  AgentPolicy, FirewallAssessment, FirewallRule, QuarantineEntry,
} from '../../types';
import {
  InfoTip, PageHero, StrataAlert, StrataModal, StrataSelect, StrataSkeletons, StrataVoid,
} from '../ui/Instruments';
import { pct } from '../../lib/format';

// ── Verdict colours ─────────────────────────────────────────────────────────

const VERDICT_META: Record<string, { icon: typeof ShieldCheck; color: string; label: string }> = {
  allow: { icon: ShieldCheck, color: 'var(--mint)', label: 'allow' },
  quarantine: { icon: ShieldQuestion, color: 'var(--gold)', label: 'quarantine' },
  block: { icon: Ban, color: 'var(--rose)', label: 'block' },
  deny: { icon: UserX, color: 'var(--rose)', label: 'deny' },
};

function verdictMeta(v: string) {
  return VERDICT_META[v] ?? VERDICT_META.allow;
}

// ── Score bar ───────────────────────────────────────────────────────────────

function ScoreBar({ label, value, color }: { label: string; value: number; color: string }) {
  return (
    <div className="st-sys-score">
      <span className="st-sys-score-label">{label}</span>
      <span className="st-sys-score-track">
        <span className="st-sys-score-fill" style={{ width: `${Math.round(value * 100)}%`, background: color }} />
      </span>
      <span className="st-sys-score-value">{pct(value)}</span>
    </div>
  );
}

// ── Main view ───────────────────────────────────────────────────────────────

export function FirewallView() {
  const [rules, setRules] = useState<FirewallRule[]>([]);
  const [quarantine, setQuarantine] = useState<QuarantineEntry[]>([]);
  const [policies, setPolicies] = useState<AgentPolicy[]>([]);
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Check tool state
  const [checkTitle, setCheckTitle] = useState('');
  const [checkContent, setCheckContent] = useState('');
  const [assessment, setAssessment] = useState<FirewallAssessment | null>(null);

  // Policy form state
  const [policyAgent, setPolicyAgent] = useState('');
  const [policyVis, setPolicyVis] = useState('');
  const [policyLayers, setPolicyLayers] = useState('');
  const [policyDeny, setPolicyDeny] = useState('');

  // Add-rule modal state
  const [ruleModalOpen, setRuleModalOpen] = useState(false);
  const [rulePattern, setRulePattern] = useState('');
  const [ruleAction, setRuleAction] = useState('block');
  const [ruleReason, setRuleReason] = useState('');
  const [ruleSaving, setRuleSaving] = useState(false);
  const [ruleError, setRuleError] = useState<string | null>(null);

  const load = useCallback(async () => {
    setBusy(true);
    try {
      const [r, q, p] = await Promise.all([
        invoke<FirewallRule[]>('firewall_rules'),
        invoke<QuarantineEntry[]>('quarantine_list', { status: null }),
        invoke<AgentPolicy[]>('agent_policy_list'),
      ]);
      setRules(r);
      setQuarantine(q);
      setPolicies(p);
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

  const addRule = useCallback(() => {
    setRulePattern('');
    setRuleAction('block');
    setRuleReason('');
    setRuleError(null);
    setRuleModalOpen(true);
  }, []);

  const createRule = useCallback(async () => {
    if (!rulePattern.trim()) return;
    setRuleSaving(true);
    setRuleError(null);
    try {
      await invoke('firewall_rule_add', {
        pattern: rulePattern.trim(),
        action: ruleAction,
        reason: ruleReason.trim() || 'added from UI',
      });
      setRuleModalOpen(false);
      setRulePattern('');
      setRuleReason('');
      await load();
    } catch (err) {
      setRuleError(String(err));
    } finally {
      setRuleSaving(false);
    }
  }, [rulePattern, ruleAction, ruleReason, load]);

  const deleteRule = useCallback(async (id: string) => {
    try {
      await invoke('firewall_rule_delete', { id });
      await load();
    } catch (err) {
      setError(String(err));
    }
  }, [load]);

  const toggleRule = useCallback(async (rule: FirewallRule) => {
    try {
      await invoke('firewall_rule_set_enabled', { id: rule.id, enabled: !rule.enabled });
      await load();
    } catch (err) {
      setError(String(err));
    }
  }, [load]);

  const approveQuarantine = useCallback(async (id: string) => {
    try {
      await invoke('quarantine_approve', { id });
      await load();
    } catch (err) {
      setError(String(err));
    }
  }, [load]);

  const rejectQuarantine = useCallback(async (id: string) => {
    try {
      await invoke('quarantine_reject', { id });
      await load();
    } catch (err) {
      setError(String(err));
    }
  }, [load]);

  const runCheck = useCallback(async () => {
    if (!checkTitle.trim() && !checkContent.trim()) return;
    setBusy(true);
    try {
      const a = await invoke<FirewallAssessment>('firewall_check', {
        title: checkTitle,
        content: checkContent,
      });
      setAssessment(a);
    } catch (err) {
      setError(String(err));
    } finally {
      setBusy(false);
    }
  }, [checkTitle, checkContent]);

  const addPolicy = useCallback(async () => {
    if (!policyAgent.trim()) return;
    try {
      await invoke('agent_policy_add', {
        agent: policyAgent.trim(),
        role: 'assistant',
        allowedVisibility: policyVis,
        allowedLayers: policyLayers,
        denyPatterns: policyDeny,
      });
      setPolicyAgent('');
      await load();
    } catch (err) {
      setError(String(err));
    }
  }, [policyAgent, policyVis, policyLayers, policyDeny, load]);

  const deletePolicy = useCallback(async (id: string) => {
    try {
      await invoke('agent_policy_delete', { id });
      await load();
    } catch (err) {
      setError(String(err));
    }
  }, [load]);

  const hero = (
    <PageHero
      kicker="System 4 · Gatekeeper"
      title="Memory Firewall"
      copy="Everything that enters the memory pool passes through: content is scored for toxicity, spam, injection and PII, screened by your rules, and quarantined when uncertain. Agent policies then decide which agent may read what."
      accent="var(--gold)"
      secondary="var(--rose)"
      stats={[
        { label: 'Rules', value: String(rules.length), color: 'var(--bone)' },
        { label: 'Quarantine', value: String(quarantine.length), color: 'var(--gold)' },
        { label: 'Policies', value: String(policies.length), color: 'var(--cyan)' },
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
      <div className="st-page" style={{ '--st-accent': 'var(--gold)' } as CSSProperties}>
        {hero}{actions}
        <StrataAlert icon={AlertTriangle}>{error}</StrataAlert>
      </div>
    );
  }

  return (
    <div className="st-page" style={{ '--st-accent': 'var(--gold)' } as CSSProperties}>
      {hero}{actions}

      {/* Check tool */}
      <div className="st-section-head" style={{ margin: '4px 0 10px' }}>
        <h2 className="st-section-title" style={{ '--section-color': 'var(--gold)' } as CSSProperties}>
          <ShieldAlert size={14} /> Screening probe
        </h2>
        <InfoTip text="Score a piece of content against the firewall without saving it — useful to test what a memory would be judged as before capture." />
      </div>
      <p className="st-section-desc">Score content against the firewall before it is captured — a safe way to see what a memory would be judged as.</p>
      <div className="st-sys-probe">
        <input
          className="st-sys-input"
          placeholder="Title"
          value={checkTitle}
          onChange={(e) => setCheckTitle(e.target.value)}
        />
        <textarea
          className="st-sys-input"
          placeholder="Content to screen…"
          rows={3}
          value={checkContent}
          onChange={(e) => setCheckContent(e.target.value)}
        />
        <button type="button" className="st-btn" disabled={busy || (!checkTitle.trim() && !checkContent.trim())} onClick={runCheck}>
          <ShieldAlert size={13} /> Screen
        </button>
      </div>
      {assessment && (
        <div className="st-sys-assessment" style={{ borderColor: `${verdictMeta(assessment.verdict).color}55` }}>
          <div className="st-sys-assessment-head">
            <span className="st-sys-verdict" style={{ color: verdictMeta(assessment.verdict).color }}>
              <span className="st-sys-verdict-icon">
                {(() => {
                  const V = verdictMeta(assessment.verdict).icon;
                  return <V size={14} />;
                })()}
              </span>
              {verdictMeta(assessment.verdict).label}
            </span>
            <button className="st-btn st-btn--ghost" onClick={() => setAssessment(null)} style={{ padding: '3px 8px' }}>
              Close
            </button>
          </div>
          <div className="st-sys-scores">
            <ScoreBar label="toxicity" value={assessment.toxicity} color="var(--rose)" />
            <ScoreBar label="spam" value={assessment.spam} color="var(--gold)" />
            <ScoreBar label="injection" value={assessment.injection} color="var(--periwinkle)" />
            <ScoreBar label="pii" value={assessment.pii} color="var(--cyan)" />
          </div>
          {assessment.reasons.length > 0 && (
            <ul className="st-sys-reasons">
              {assessment.reasons.map((r, i) => <li key={i}>{r}</li>)}
            </ul>
          )}
          {assessment.matchedRuleIds.length > 0 && (
            <p className="st-sys-meta">Matched rules: {assessment.matchedRuleIds.join(', ')}</p>
          )}
        </div>
      )}

      {/* Rules */}
      <div className="st-section-head" style={{ margin: '26px 0 10px' }}>
        <h2 className="st-section-title" style={{ '--section-color': 'var(--gold)' } as CSSProperties}>
          <ShieldCheck size={14} /> Rules
        </h2>
        <InfoTip text="User-defined content rules: when the pattern appears in title or content, the entry is blocked outright or sent to quarantine." />
      </div>
      <p className="st-section-desc">User-defined content rules: when a pattern appears in a title or content, the entry is blocked outright or sent to quarantine.</p>
      <div className="st-sys-toolbar">
        <button type="button" className="st-btn" onClick={addRule}>
          <Plus size={13} /> Add rule
        </button>
      </div>
      {rules.length === 0 ? (
        <StrataVoid icon={ShieldCheck} title="No rules yet" accent="var(--mint)">
          Rules are optional — the built-in scoring already blocks toxicity, spam, injection and flags PII.
        </StrataVoid>
      ) : (
        <div className="st-panel">
          <div className="st-radar-list">
            {rules.map((rule, index) => (
              <div key={rule.id} className="st-radar-row" style={{ '--st-i': index, '--row-color': rule.action === 'block' ? 'var(--rose)' : 'var(--gold)' } as CSSProperties}>
                <span
                  className="st-radar-row-icon"
                  style={{
                    color: rule.action === 'block' ? 'var(--rose)' : 'var(--gold)',
                    background: `${rule.action === 'block' ? 'var(--rose)' : 'var(--gold)'}15`,
                  }}
                >
                  {rule.action === 'block' ? <Ban size={15} /> : <ShieldQuestion size={15} />}
                </span>
                <span style={{ minWidth: 0, flex: 1 }}>
                  <span className="st-radar-row-title"><code>{rule.pattern}</code></span>
                  <span className="st-radar-row-reason">
                    {rule.action}{rule.reason ? ` · ${rule.reason}` : ''}
                  </span>
                  <span className="st-radar-row-meta">
                    {rule.enabled ? 'enabled' : 'disabled'} · {rule.createdAt}
                  </span>
                </span>
                <button type="button" className="st-btn st-btn--ghost" onClick={() => void toggleRule(rule)} title={rule.enabled ? 'Disable' : 'Enable'}>
                  {rule.enabled ? <EyeOff size={13} /> : <Eye size={13} />}
                </button>
                <button type="button" className="st-btn st-btn--ghost" onClick={() => void deleteRule(rule.id)} title="Delete rule">
                  <Trash2 size={13} style={{ color: 'var(--rose)' }} />
                </button>
              </div>
            ))}
          </div>
        </div>
      )}

      {/* Quarantine */}
      <div className="st-section-head" style={{ margin: '26px 0 10px' }}>
        <h2 className="st-section-title" style={{ '--section-color': 'var(--gold)' } as CSSProperties}>
          <FileWarning size={14} /> Quarantine
        </h2>
        <InfoTip text="Entries flagged by scoring sit here until a human decides: approve turns them into real memories, reject discards them." />
      </div>
      <p className="st-section-desc">Entries flagged by the built-in scoring sit here until a human decides: approve turns them into real memories, reject discards them.</p>
      {quarantine.length === 0 ? (
        <StrataVoid icon={ShieldCheck} title="Quarantine is empty" accent="var(--mint)">
          Flagged content will appear here for a human decision.
        </StrataVoid>
      ) : (
        <div className="st-panel">
          <div className="st-radar-list">
            {quarantine.map((entry, index) => (
              <div key={entry.id} className="st-radar-row" style={{ '--st-i': index, '--row-color': 'var(--gold)' } as CSSProperties}>
                <span className="st-radar-row-icon" style={{ color: 'var(--gold)', background: 'var(--gold)15' }}>
                  <FileWarning size={15} />
                </span>
                <span style={{ minWidth: 0, flex: 1 }}>
                  <span className="st-radar-row-title">{entry.title}</span>
                  <span className="st-radar-row-reason">{entry.content}</span>
                  <span className="st-radar-row-meta">
                    {entry.source} · {entry.author} · pii {pct(entry.scores.pii)} · injection {pct(entry.scores.injection)}
                  </span>
                </span>
                <button type="button" className="st-btn" onClick={() => void approveQuarantine(entry.id)}>
                  <CheckCircle2 size={13} style={{ color: 'var(--mint)' }} /> Approve
                </button>
                <button type="button" className="st-btn st-btn--ghost" onClick={() => void rejectQuarantine(entry.id)}>
                  <Trash2 size={13} style={{ color: 'var(--rose)' }} /> Reject
                </button>
              </div>
            ))}
          </div>
        </div>
      )}

      {/* Agent policies */}
      <div className="st-section-head" style={{ margin: '26px 0 10px' }}>
        <h2 className="st-section-title" style={{ '--section-color': 'var(--gold)' } as CSSProperties}>
          <UserCog size={14} /> Agent policies
        </h2>
        <InfoTip text="Per-agent access control: allowed visibilities and layers, plus deny patterns. An agent with no policy is denied by default." />
      </div>
      <p className="st-section-desc">Per-agent access control: allowed visibilities and layers, plus deny patterns. An agent with no policy is denied by default.</p>
      <div className="st-sys-toolbar" style={{ flexWrap: 'wrap' }}>
        <input className="st-sys-input" placeholder="Agent name (e.g. claude-code)" value={policyAgent} onChange={(e) => setPolicyAgent(e.target.value)} />
        <input className="st-sys-input" placeholder="Allowed visibility (public,private…)" value={policyVis} onChange={(e) => setPolicyVis(e.target.value)} />
        <input className="st-sys-input" placeholder="Allowed layers (semantic,procedural…)" value={policyLayers} onChange={(e) => setPolicyLayers(e.target.value)} />
        <input className="st-sys-input" placeholder="Deny patterns (comma separated)" value={policyDeny} onChange={(e) => setPolicyDeny(e.target.value)} />
        <button type="button" className="st-btn" disabled={!policyAgent.trim()} onClick={addPolicy}>
          <Plus size={13} /> Add policy
        </button>
      </div>
      {policies.length === 0 ? (
        <StrataVoid icon={ShieldCheck} title="No policies" accent="var(--mint)">
          Without a policy an agent is denied access to every memory (deny by default).
        </StrataVoid>
      ) : (
        <div className="st-panel">
          <div className="st-radar-list">
            {policies.map((policy, index) => (
              <div key={policy.id} className="st-radar-row" style={{ '--st-i': index, '--row-color': 'var(--cyan)' } as CSSProperties}>
                <span className="st-radar-row-icon" style={{ color: 'var(--cyan)', background: 'var(--cyan)15' }}>
                  <ShieldCheck size={15} />
                </span>
                <span style={{ minWidth: 0, flex: 1 }}>
                  <span className="st-radar-row-title">
                    {policy.agent} <span className="st-sys-meta">({policy.role})</span>
                  </span>
                  <span className="st-radar-row-reason">
                    visibility: {policy.allowedVisibility.length ? policy.allowedVisibility.join(', ') : 'all'}
                    {' · '}layers: {policy.allowedLayers.length ? policy.allowedLayers.join(', ') : 'all'}
                    {policy.denyPatterns.length ? ` · deny: ${policy.denyPatterns.join(', ')}` : ''}
                  </span>
                  <span className="st-radar-row-meta">
                    {policy.enabled ? 'enabled' : 'disabled'} · {policy.createdAt}
                  </span>
                </span>
                <button type="button" className="st-btn st-btn--ghost" onClick={() => void deletePolicy(policy.id)} title="Delete policy">
                  <Trash2 size={13} style={{ color: 'var(--rose)' }} />
                </button>
              </div>
            ))}
          </div>
        </div>
      )}

      {ruleModalOpen && (
        <StrataModal
          title="Add firewall rule"
          icon={ShieldCheck}
          accent="var(--gold)"
          onClose={() => setRuleModalOpen(false)}
          footer={
            <>
              <button type="button" className="st-btn st-btn--ghost" disabled={ruleSaving} onClick={() => setRuleModalOpen(false)}>
                Cancel
              </button>
              <button
                type="button"
                className="st-btn"
                disabled={ruleSaving || !rulePattern.trim()}
                onClick={() => void createRule()}
              >
                {ruleSaving ? <RefreshCw size={13} className="spinning" /> : <Plus size={13} />}
                {ruleSaving ? 'Adding…' : 'Create rule'}
              </button>
            </>
          }
        >
          <label className="st-modal-label">Pattern <span className="st-sys-meta">(substring, case-insensitive)</span></label>
          <input
            className="st-sys-input"
            placeholder="e.g. v1.0.0 migration"
            value={rulePattern}
            onChange={(event) => setRulePattern(event.target.value)}
            onKeyDown={(event) => {
              if (event.key === 'Enter' && rulePattern.trim() && !ruleSaving) void createRule();
            }}
            autoFocus
          />
          <label className="st-modal-label">Action</label>
          <StrataSelect
            value={ruleAction}
            onChange={setRuleAction}
            ariaLabel="Rule action"
            options={[
              { value: 'block', label: 'block — reject outright' },
              { value: 'quarantine', label: 'quarantine — hold for review' },
            ]}
          />
          <label className="st-modal-label">Reason <span className="st-sys-meta">(optional)</span></label>
          <input
            className="st-sys-input"
            placeholder="Why this rule exists"
            value={ruleReason}
            onChange={(event) => setRuleReason(event.target.value)}
          />
          {ruleError && <StrataAlert icon={AlertTriangle}>{ruleError}</StrataAlert>}
        </StrataModal>
      )}
    </div>
  );
}
