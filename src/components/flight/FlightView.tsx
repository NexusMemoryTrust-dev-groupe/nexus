import { useCallback, useEffect, useState } from 'react';
import type { CSSProperties } from 'react';
import { invoke } from '@tauri-apps/api/core';
import {
  Activity, CheckCircle2, Clock3, CircleX, Eye, History, Play, RefreshCw, Search,
  SkipForward, Box, Layers, Users, FileText, Flame, Zap, ShieldAlert, BrainCircuit,
  GitBranch,
} from 'lucide-react';
import type { ContextChain, FlightRecord, FlightSession, FlightStats } from '../../types';
import {
  InfoTip, PageHero, StrataAlert, StrataSelect, StrataSkeletons, StrataVoid,
} from '../ui/Instruments';

// ── Outcome metadata ────────────────────────────────────────────────────────

const OUTCOME_META: Record<string, { icon: typeof CheckCircle2; color: string; label: string }> = {
  success: { icon: CheckCircle2, color: 'var(--mint)', label: 'success' },
  error: { icon: CircleX, color: 'var(--rose)', label: 'error' },
  blocked: { icon: ShieldAlert, color: 'var(--gold)', label: 'blocked' },
  skipped: { icon: SkipForward, color: 'var(--periwinkle)', label: 'skipped' },
};

function outcomeMeta(outcome: string) {
  return OUTCOME_META[outcome] ?? OUTCOME_META.success;
}

// ── Category metadata ───────────────────────────────────────────────────────

const CATEGORY_META: Record<string, { icon: typeof BrainCircuit; color: string }> = {
  memory: { icon: BrainCircuit, color: 'var(--cyan)' },
  conflict: { icon: Flame, color: 'var(--rose)' },
  firewall: { icon: ShieldAlert, color: 'var(--gold)' },
  rehearsal: { icon: RefreshCw, color: 'var(--mint)' },
  radar: { icon: Activity, color: 'var(--periwinkle)' },
  skill: { icon: Zap, color: 'var(--tangerine)' },
  context: { icon: Box, color: 'var(--bone)' },
  team: { icon: Users, color: 'var(--cyan)' },
  versioning: { icon: History, color: 'var(--periwinkle)' },
  mcp: { icon: Play, color: 'var(--tangerine)' },
  system: { icon: FileText, color: 'var(--bone)' },
};

function categoryMeta(category: string) {
  return CATEGORY_META[category] ?? CATEGORY_META.system;
}

// ── Record row ──────────────────────────────────────────────────────────────

function RecordRow({ record, onReplay }: { record: FlightRecord; onReplay: (r: FlightRecord) => void }) {
  const cat = categoryMeta(record.category);
  const out = outcomeMeta(record.outcome);
  const OutIcon = out.icon;
  const CatIcon = cat.icon;
  return (
    <div className="st-flight-row">
      <span className="st-flight-row-cat" style={{ color: cat.color, background: `${cat.color}15` }}>
        <CatIcon size={13} />
      </span>
      <span style={{ minWidth: 0, flex: 1 }}>
        <span className="st-flight-row-head">
          <span className="st-flight-row-action" style={{ color: cat.color }}>{record.action}</span>
          <span className="st-flight-row-meta">
            {record.category} · {record.actor} · {record.recordedAt}
          </span>
        </span>
        <span className="st-flight-row-summary">{record.summary}</span>
        {record.entityId && (
          <span className="st-flight-row-entity">→ {record.entityType}:{record.entityId}</span>
        )}
      </span>
      <span className="st-flight-row-outcome" style={{ color: out.color, background: `${out.color}15` }}>
        <OutIcon size={12} /> {out.label}
      </span>
      {record.durationMs > 0 && (
        <span className="st-flight-row-duration">{record.durationMs}ms</span>
      )}
      {record.entityId && (
        <button
          className="st-flight-row-replay"
          title="Replay this entity's flight chain"
          onClick={() => onReplay(record)}
        >
          <Eye size={13} />
        </button>
      )}
    </div>
  );
}

// ── Session row ─────────────────────────────────────────────────────────────

function SessionRow({ session }: { session: FlightSession }) {
  return (
    <div className="st-flight-session">
      <span className="st-flight-session-icon" style={{ color: 'var(--mint)', background: 'var(--mint)15' }}>
        <Clock3 size={13} />
      </span>
      <span style={{ minWidth: 0, flex: 1 }}>
        <span className="st-flight-session-head">
          <span className="st-flight-session-title">{session.title}</span>
          <span className="st-flight-session-meta">
            {session.actor} · {session.source} · since {session.startedAt}
          </span>
        </span>
        {session.purpose && <span className="st-flight-session-purpose">{session.purpose}</span>}
      </span>
    </div>
  );
}

// ── Context chain card (System 5: "why did the AI say this") ───────────────

function ChainCard({ chain }: { chain: ContextChain }) {
  return (
    <div className="st-flight-session">
      <span className="st-flight-session-icon" style={{ color: 'var(--periwinkle)', background: 'var(--periwinkle)15' }}>
        <GitBranch size={13} />
      </span>
      <span style={{ minWidth: 0, flex: 1 }}>
        <span className="st-flight-session-head">
          <span className="st-flight-session-title">{chain.query}</span>
          <span className="st-flight-session-meta">
            {chain.actor} · {chain.intent} · {Math.round(chain.answerConfidence * 100)}% · {chain.totalTokens} tokens · {chain.createdAt}
          </span>
        </span>
        {chain.answer && <span className="st-flight-session-purpose">→ {chain.answer}</span>}
        {chain.why && (
          <details className="st-chain-details">
            <summary>Why did the AI say this?</summary>
            <pre>{chain.why}</pre>
            {chain.pipeline && <pre className="st-chain-pipeline">{chain.pipeline}</pre>}
          </details>
        )}
      </span>
    </div>
  );
}

// ── Main view ───────────────────────────────────────────────────────────────

export function FlightView() {
  const [stats, setStats] = useState<FlightStats | null>(null);
  const [records, setRecords] = useState<FlightRecord[]>([]);
  const [sessions, setSessions] = useState<FlightSession[]>([]);
  const [chains, setChains] = useState<ContextChain[]>([]);
  const [chain, setChain] = useState<FlightRecord[] | null>(null);
  const [chainTitle, setChainTitle] = useState('');
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [category, setCategory] = useState<string>('');

  const load = useCallback(async () => {
    setBusy(true);
    try {
      const [s, r, sessionsResult, chainsResult] = await Promise.all([
        invoke<FlightStats>('flight_stats'),
        invoke<FlightRecord[]>('flight_recent', { limit: 60, category: category || null }),
        invoke<FlightSession[]>('flight_active_sessions', { limit: 20 }),
        invoke<ContextChain[]>('context_chain_recent', { limit: 10 }),
      ]);
      setStats(s);
      setRecords(r);
      setSessions(sessionsResult);
      setChains(chainsResult);
      setError(null);
    } catch (err) {
      setError(String(err));
    } finally {
      setBusy(false);
      setLoading(false);
    }
  }, [category]);

  useEffect(() => {
    void load();
  }, [load]);

  const runReplay = useCallback(async (record: FlightRecord) => {
    setChain(null);
    setChainTitle(`${record.entityType} ${record.entityId}`);
    try {
      const chainResult = await invoke<FlightRecord[]>('flight_replay', {
        entityType: record.entityType,
        entityId: record.entityId,
      });
      setChain(chainResult);
    } catch (err) {
      setError(String(err));
    }
  }, []);

  const categories = [
    '', 'memory', 'conflict', 'firewall', 'rehearsal', 'radar',
    'skill', 'context', 'team', 'versioning', 'mcp', 'system',
  ];

  const heroStats = stats
    ? [
        { label: 'Records', value: String(stats.totalRecords), color: 'var(--cyan)' },
        { label: 'Sessions', value: String(stats.totalSessions), color: 'var(--periwinkle)' },
        { label: 'Why-Chains', value: String(stats.contextChains ?? 0), color: 'var(--tangerine)' },
        { label: 'Errors', value: String(stats.byOutcome.error ?? 0), color: 'var(--rose)' },
      ]
    : [];

  return (
    <div className="st-page" style={{ '--st-accent': 'var(--cyan)' } as CSSProperties}>
      <PageHero
        kicker="System 5 · Operation Black-Box"
        title="Flight Recorder"
        copy="Every significant step of the ecosystem — memory creation, conflicts, quarantine, rehearsal, skill and MCP calls — logged chronologically and replayable per entity. The black box that explains what the system did and why."
        stats={heroStats}
        accent="var(--cyan)"
        secondary="var(--periwinkle)"
      />

      <div>
        <div className="st-toolbar">
          <label className="st-flight-filter">
            <span>Category</span>
            <StrataSelect
              value={category}
              onChange={setCategory}
              ariaLabel="Filter records by category"
              options={categories.map((c) => ({ value: c, label: c === '' ? 'all' : c }))}
            />
          </label>
          <span className="st-toolbar-sep" aria-hidden="true" />
          <button type="button" className="st-action-btn" onClick={() => void load()} disabled={busy}>
            <RefreshCw size={13} className={busy ? 'spinning' : undefined} />
            {busy ? 'Refreshing…' : 'Refresh'}
          </button>
        </div>

        {error && (
          <StrataAlert icon={ShieldAlert}>
            Flight recorder error: {error}. Ensure the backend is running.
          </StrataAlert>
        )}

        {loading ? (
          <StrataSkeletons />
        ) : (
          <>
            {/* ── Active sessions ── */}
            <section className="st-section">
              <div className="st-section-head">
                <h2 className="st-section-title">Active Sessions <InfoTip text="What operation runs are in progress right now — agent passes, tool batches, tasks." /></h2>
              </div>
              <p className="st-section-desc">Live operations currently running in the ecosystem. A session groups every step of one run so you can see what is executing right now.</p>
              {sessions.length === 0 ? (
                <StrataVoid icon={Clock3} title="No active sessions">
                  Start one with <code>/flight-session start &lt;title&gt; : purpose</code> in Copilot.
                </StrataVoid>
              ) : (
                <div className="st-flight-sessions st-section-frame">
                  {sessions.map((s) => <SessionRow key={s.id} session={s} />)}
                </div>
              )}
            </section>

            {/* ── Context chains (System 5: why did the AI say this) ── */}
            <section className="st-section">
              <div className="st-section-head">
                <h2 className="st-section-title">Why-Chains <InfoTip text="Recorded context chains: the exact memories, entities and pipeline stages behind an answer. Expand one to see why the AI said what it said." /></h2>
              </div>
              <p className="st-section-desc">Every answer's reasoning trail — the memories, entities and pipeline stages that produced it. Expand a chain to see exactly why the AI said what it said.</p>
              {chains.length === 0 ? (
                <StrataVoid icon={GitBranch} title="No why-chains recorded">
                  Record one with <code>/why &lt;query&gt; : &lt;answer&gt;</code> in Copilot or the MCP tool{' '}
                  <code>nexus_why</code> — from then on every answer is explainable.
                </StrataVoid>
              ) : (
                <div className="st-flight-sessions st-section-frame">
                  {chains.map((c) => <ChainCard key={c.id} chain={c} />)}
                </div>
              )}
            </section>

            {/* ── Recent records ── */}
            <section className="st-section">
              <div className="st-section-head">
                <h2 className="st-section-title">Recent Records <InfoTip text="The latest operations from the black box. Click the eye to replay an entity's full chain." /></h2>
              </div>
              <p className="st-section-desc">The most recent operations logged by the black box. Click the eye on a record to replay that entity's full flight chain.</p>
              {records.length === 0 ? (
                <StrataVoid icon={Layers} title="The black box is empty">
                  Log operations with <code>/flight log &lt;category&gt; &lt;action&gt; : &lt;summary&gt;</code>,
                  the MCP tool <code>nexus_flight_log</code>, or just keep using the system — domain events are recorded automatically.
                </StrataVoid>
              ) : (
                <div className="st-flight-records st-section-frame">
                  {records.map((r) => <RecordRow key={r.id} record={r} onReplay={runReplay} />)}
                </div>
              )}
            </section>

            {/* ── Replay chain ── */}
            {chain && (
              <section className="st-section">
                <div className="st-section-head">
                  <h2 className="st-section-title">Replay · {chainTitle}</h2>
                  <button
                    className="st-btn st-btn--ghost"
                    style={{ marginLeft: 'auto' }}
                    onClick={() => setChain(null)}
                  >
                    Close
                  </button>
                </div>
                {chain.length === 0 ? (
                  <StrataVoid icon={Search} title="No records for this entity">
                    Nothing touched {chainTitle} yet.
                  </StrataVoid>
                ) : (
                  <div className="st-flight-records st-section-frame">
                    {chain.map((r) => <RecordRow key={r.id} record={r} onReplay={runReplay} />)}
                  </div>
                )}
              </section>
            )}

            {/* ── Stats breakdown ── */}
            {stats && (Object.keys(stats.byCategory).length > 0 || Object.keys(stats.byOutcome).length > 0) && (
              <section className="st-section">
                <div className="st-section-head">
                  <h2 className="st-section-title">Breakdown <InfoTip text="Counts by category (where operations come from) and by outcome (how they ended)." /></h2>
                </div>
                <p className="st-section-desc">Where operations come from and how they ended — split by category and by outcome.</p>
                <div className="st-flight-breakdown">
                  <div className="st-flight-breakdown-col">
                    <div className="st-flight-breakdown-label">By category</div>
                    {Object.entries(stats.byCategory).sort((a, b) => b[1] - a[1]).map(([cat, count]) => {
                      const meta = categoryMeta(cat);
                      const Icon = meta.icon;
                      return (
                        <div key={cat} className="st-flight-breakdown-item">
                          <Icon size={12} style={{ color: meta.color }} />
                          <span>{cat}</span>
                          <span className="st-flight-breakdown-count">{count}</span>
                        </div>
                      );
                    })}
                  </div>
                  <div className="st-flight-breakdown-col">
                    <div className="st-flight-breakdown-label">By outcome</div>
                    {Object.entries(stats.byOutcome).sort((a, b) => b[1] - a[1]).map(([out, count]) => {
                      const meta = outcomeMeta(out);
                      const Icon = meta.icon;
                      return (
                        <div key={out} className="st-flight-breakdown-item">
                          <Icon size={12} style={{ color: meta.color }} />
                          <span>{out}</span>
                          <span className="st-flight-breakdown-count">{count}</span>
                        </div>
                      );
                    })}
                  </div>
                </div>
              </section>
            )}
          </>
        )}
      </div>
    </div>
  );
}
