import { useCallback, useEffect, useState } from 'react';
import type { CSSProperties } from 'react';
import { invoke } from '@tauri-apps/api/core';
import {
  AlertTriangle, BookOpenCheck, CheckCircle2, Copy, Database, Layers3,
  RefreshCw, Sparkles, Target,
} from 'lucide-react';
import type {
  CanonicalMemory, ConsolidationReport, RehearsalCycleReport, RehearsalPlan,
} from '../../types';
import {
  ImpactBlocks, InfoTip, PageHero, StrataAlert, StrataSkeletons, StrataVoid,
} from '../ui/Instruments';
import { pct } from '../../lib/format';

// ── Count tile ──────────────────────────────────────────────────────────────

function CountTile({
  icon: Icon,
  label,
  value,
  color,
  hint,
}: {
  icon: typeof Target;
  label: string;
  value: number;
  color: string;
  hint: string;
}) {
  return (
    <div className="st-radar-tile" style={{ '--tile-color': color } as CSSProperties}>
      <div className="st-radar-tile-head">
        <span className="st-radar-tile-icon"><Icon size={13} /></span>
        <span className="st-radar-tile-label">{label}</span>
        <InfoTip text={hint} />
      </div>
      <div className="st-radar-tile-value">{value}</div>
    </div>
  );
}

// ── Cycle report banner ─────────────────────────────────────────────────────

function CycleReport({
  report,
  kind,
  onClose,
}: {
  report: RehearsalCycleReport | ConsolidationReport;
  kind: 'rehearsal' | 'canonical';
  onClose: () => void;
}) {
  const isRehearsal = kind === 'rehearsal';
  const rows = isRehearsal
    ? [
        { label: 'Rehearsed', value: (report as RehearsalCycleReport).rehearsed, color: 'var(--mint)' },
        { label: 'Scheduled first', value: (report as RehearsalCycleReport).scheduledFirst, color: 'var(--cyan)' },
        { label: 'Decayed', value: (report as RehearsalCycleReport).decayed, color: 'var(--gold)' },
        { label: 'Skipped', value: (report as RehearsalCycleReport).skipped, color: 'var(--muted-2)' },
      ]
    : [
        { label: 'Clusters found', value: (report as ConsolidationReport).clustersFound, color: 'var(--periwinkle)' },
        { label: 'Canonical created', value: (report as ConsolidationReport).canonicalCreated, color: 'var(--mint)' },
        { label: 'Merged members', value: (report as ConsolidationReport).mergedMembers, color: 'var(--cyan)' },
        { label: 'Skipped existing', value: (report as ConsolidationReport).skippedExisting, color: 'var(--muted-2)' },
      ];
  return (
    <div className="st-alert" style={{ borderColor: 'var(--mint)' } as CSSProperties}>
      <CheckCircle2 size={14} style={{ color: 'var(--mint)' }} />
      <div style={{ flex: 1 }}>
        {isRehearsal ? 'Rehearsal cycle' : 'Canonical consolidation'} finished at{' '}
        <code>{report.ranAt}</code>:{' '}
        {rows.map((r) => `${r.label} ${r.value}`).join(' · ')}
      </div>
      <button className="st-btn st-btn--ghost" onClick={onClose} style={{ padding: '3px 8px' }}>
        Dismiss
      </button>
    </div>
  );
}

// ── Main view ───────────────────────────────────────────────────────────────

export function RehearsalView() {
  const [plan, setPlan] = useState<RehearsalPlan | null>(null);
  const [canonicals, setCanonicals] = useState<CanonicalMemory[]>([]);
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [report, setReport] = useState<RehearsalCycleReport | ConsolidationReport | null>(null);

  const load = useCallback(async () => {
    setBusy(true);
    try {
      const [p, c] = await Promise.all([
        invoke<RehearsalPlan>('get_rehearsal_plan'),
        invoke<CanonicalMemory[]>('list_canonical_memories', { limit: 100 }),
      ]);
      setPlan(p);
      setCanonicals(c);
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

  const runRehearsal = useCallback(async () => {
    setBusy(true);
    try {
      const r = await invoke<RehearsalCycleReport>('run_rehearsal_cycle');
      setReport(r);
      await load();
    } catch (err) {
      setError(String(err));
    } finally {
      setBusy(false);
    }
  }, [load]);

  const runCanonical = useCallback(async () => {
    setBusy(true);
    try {
      const r = await invoke<ConsolidationReport>('run_canonical_consolidation');
      setReport(r);
      await load();
    } catch (err) {
      setError(String(err));
    } finally {
      setBusy(false);
    }
  }, [load]);

  const rehearseNow = useCallback(async (id: string) => {
    try {
      await invoke('rehearse_memory', { id });
      await load();
    } catch (err) {
      setError(String(err));
    }
  }, [load]);

  const hero = (
    <PageHero
      kicker="System 2-3 · Sleep Cycle"
      title="Rehearsal & Consolidation"
      copy="The memory hygiene core: rehearsal strengthens what is due for review and forgets what was never seen; canonical consolidation collapses repeated records into one fact with full provenance."
      accent="var(--mint)"
      secondary="var(--cyan)"
      stats={plan ? [
        { label: 'Pool', value: String(plan.counts.total), color: 'var(--bone)' },
        { label: 'Due now', value: String(plan.counts.dueNow), color: 'var(--mint)' },
        { label: 'Never rehearsed', value: String(plan.counts.neverRehearsed), color: 'var(--gold)' },
        { label: 'Canonical', value: String(canonicals.length), color: 'var(--cyan)' },
      ] : []}
    />
  );

  const actions = (
    <div className="st-radar-actions">
      <button type="button" className="st-action-btn" disabled={busy} onClick={runRehearsal}>
        {busy ? <RefreshCw size={13} className="spinning" /> : <BookOpenCheck size={13} />}
        Run rehearsal cycle
      </button>
      <button type="button" className="st-action-btn" disabled={busy} onClick={runCanonical}>
        {busy ? <RefreshCw size={13} className="spinning" /> : <Database size={13} />}
        Run canonical consolidation
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
      <div className="st-page" style={{ '--st-accent': 'var(--mint)' } as CSSProperties}>
        {hero}{actions}
        <StrataAlert icon={AlertTriangle}>{error}</StrataAlert>
      </div>
    );
  }

  if (!plan) return null;

  return (
    <div className="st-page" style={{ '--st-accent': 'var(--mint)' } as CSSProperties}>
      {hero}{actions}

      {report && (
        <CycleReport
          report={report}
          kind={'rehearsed' in report ? 'rehearsal' : 'canonical'}
          onClose={() => setReport(null)}
        />
      )}

      {/* Counts */}
      <section className="st-radar-tiles">
        <CountTile
          icon={Target}
          label="Due now"
          value={plan.counts.dueNow}
          color="var(--mint)"
          hint="Memories whose next review date has passed — strengthen them with a rehearsal cycle."
        />
        <CountTile
          icon={BookOpenCheck}
          label="Rehearsed ≥1"
          value={plan.counts.rehearsedAtLeastOnce}
          color="var(--cyan)"
          hint="Records that have been reviewed at least once and follow an expanding interval."
        />
        <CountTile
          icon={Sparkles}
          label="Never rehearsed"
          value={plan.counts.neverRehearsed}
          color="var(--gold)"
          hint="Fresh records still waiting for their first review. Old unread ones decay in importance."
        />
        <CountTile
          icon={Layers3}
          label="Canonical facts"
          value={canonicals.length}
          color="var(--periwinkle)"
          hint="Consolidated facts collapsed from repeated records, keeping full provenance."
        />
      </section>

      {/* Due for review */}
      <div className="st-section-head" style={{ margin: '26px 0 10px' }}>
        <h2 className="st-section-title" style={{ '--section-color': 'var(--mint)' } as CSSProperties}>
          <BookOpenCheck size={14} /> Due for review
        </h2>
        <InfoTip text="These memories are due according to their rehearsal schedule. Run a cycle to rehearse all of them, or rehearse one manually." />
      </div>
      <p className="st-section-desc">Memories whose next review date has passed. Run a cycle to rehearse all of them at once, or rehearse one manually.</p>
      {plan.items.length === 0 ? (
        <StrataVoid icon={CheckCircle2} title="Nothing is due" accent="var(--mint)">
          All scheduled memories are up to date. New records will be scheduled after the first cycle.
        </StrataVoid>
      ) : (
        <div className="st-panel">
          <div className="st-radar-list">
            {plan.items.map((item, index) => (
              <div
                key={item.id}
                className="st-radar-row"
                style={{ '--st-i': index, '--row-color': item.overdueDays > 3 ? 'var(--gold)' : 'var(--mint)' } as CSSProperties}
              >
                <span
                  className="st-radar-row-icon"
                  style={{ color: item.overdueDays > 3 ? 'var(--gold)' : 'var(--mint)', background: `${item.overdueDays > 3 ? 'var(--gold)' : 'var(--mint)'}15` }}
                >
                  <BookOpenCheck size={15} />
                </span>
                <span style={{ minWidth: 0, flex: 1 }}>
                  <span className="st-radar-row-title">{item.title}</span>
                  <span className="st-radar-row-reason">{item.summary}</span>
                  <span className="st-radar-row-meta">
                    due {item.dueAt}
                    {item.overdueDays > 0 ? ` · ${item.overdueDays}d overdue` : ''}
                    {item.lastRehearsedAt ? ` · last ${item.lastRehearsedAt}` : ' · never rehearsed'}
                  </span>
                </span>
                <span className="st-radar-row-impact">
                  <ImpactBlocks value={item.importance} label="Importance" />
                  <span className="st-radar-row-import">{pct(item.confidence)}%</span>
                </span>
                <button
                  type="button"
                  className="st-btn"
                  onClick={() => void rehearseNow(item.id)}
                  title="Mark as rehearsed now"
                >
                  Rehearse
                </button>
              </div>
            ))}
          </div>
        </div>
      )}

      {/* Canonical memories */}
      <div className="st-section-head" style={{ margin: '26px 0 10px' }}>
        <h2 className="st-section-title" style={{ '--section-color': 'var(--mint)' } as CSSProperties}>
          <Layers3 size={14} /> Canonical facts
        </h2>
        <InfoTip text="One fact per cluster of repeated records. Cohesion shows how tightly the members agree; members keep their history and point here via superseded_by_id." />
      </div>
      <p className="st-section-desc">One fact per cluster of repeated records. Members keep their history and point here via superseded_by_id, so nothing is ever lost.</p>
      {canonicals.length === 0 ? (
        <StrataVoid icon={Database} title="No canonical memories yet">
          Run <code>run_canonical_consolidation</code> to collapse repeated records into single facts.
        </StrataVoid>
      ) : (
        <div className="st-panel">
          <div className="st-radar-list">
            {canonicals.map((cm, index) => (
              <div key={cm.id} className="st-radar-row" style={{ '--st-i': index, '--row-color': 'var(--cyan)' } as CSSProperties}>
                <span className="st-radar-row-icon" style={{ color: 'var(--cyan)', background: 'var(--cyan)15' }}>
                  <Copy size={15} />
                </span>
                <span style={{ minWidth: 0, flex: 1 }}>
                  <span className="st-radar-row-title">{cm.title}</span>
                  <span className="st-radar-row-reason">{cm.summary}</span>
                  <span className="st-radar-row-meta">
                    {cm.layer} · cohesion {pct(cm.cohesion)} · importance {pct(cm.importanceScore)} · confidence {pct(cm.confidenceScore)}
                  </span>
                </span>
                <span className="st-radar-row-meta" style={{ flex: 'none' }}>
                  {cm.memberCount} member{cm.memberCount === 1 ? '' : 's'}
                </span>
              </div>
            ))}
          </div>
        </div>
      )}
    </div>
  );
}
