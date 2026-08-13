import { useCallback, useEffect, useState } from 'react';
import type { CSSProperties } from 'react';
import { invoke } from '@tauri-apps/api/core';
import {
  AlertTriangle, Beaker, FlaskConical, RefreshCw, Sparkles,
} from 'lucide-react';
import type { LabExperiment } from '../../types';
import {
  InfoTip, PageHero, StrataAlert, StrataSkeletons, StrataVoid,
} from '../ui/Instruments';
import { pct } from '../../lib/format';

// ── Strategy colours ─────────────────────────────────────────────────────────

const STRATEGY_COLORS: Record<string, string> = {
  compact: 'var(--cyan)',
  balanced: 'var(--periwinkle)',
  rich: 'var(--tangerine)',
};

function strategyColor(s: string) {
  return STRATEGY_COLORS[s] ?? 'var(--bone)';
}

// ── Experiment card ─────────────────────────────────────────────────────────

function ExperimentCard({ exp, index }: { exp: LabExperiment; index: number }) {
  const best = exp.bestStrategy;
  return (
    <div className="st-lab-exp" style={{ '--st-i': index } as CSSProperties}>
      <div className="st-lab-exp-head">
        <span className="st-lab-exp-query">{exp.query}</span>
        <span className="st-sys-meta">{exp.createdAt}</span>
      </div>
      <div className="st-lab-exp-badge" style={{ color: strategyColor(best) }}>
        <Sparkles size={12} /> best: {best}
      </div>
      <div className="st-lab-grid">
        {exp.results.map((r) => {
          const isBest = r.strategy === best;
          return (
            <div
              key={r.strategy}
              className={`st-lab-card${isBest ? ' is-best' : ''}`}
              style={{ '--lab-color': strategyColor(r.strategy) } as CSSProperties}
            >
              <div className="st-lab-card-head">
                <span className="st-lab-card-name">{r.strategy}</span>
                {isBest && <span className="st-lab-card-best">winner</span>}
              </div>
              <div className="st-lab-card-stats">
                <span>{r.memories} memories</span>
                <span>{r.entities} entities</span>
                <span>{r.tokens} tokens</span>
              </div>
              <div className="st-lab-card-bars">
                <span className="st-lab-bar"><i style={{ width: `${Math.round(r.avgRelevance * 100)}%`, background: 'var(--cyan)' }} /></span>
                <span className="st-lab-bar"><i style={{ width: `${Math.round(r.maturity * 100)}%`, background: 'var(--periwinkle)' }} /></span>
                <span className="st-lab-bar"><i style={{ width: `${Math.round(r.accuracy * 100)}%`, background: 'var(--tangerine)' }} /></span>
              </div>
              <div className="st-lab-card-meta">
                relevance {pct(r.avgRelevance)} · maturity {pct(r.maturity)} · accuracy {pct(r.accuracy)}
              </div>
              <div className="st-lab-card-meta">
                {r.efficiencyPerKToken.toFixed(1)} pts/k-token · {r.buildMs}ms
              </div>
            </div>
          );
        })}
      </div>
      {exp.summary && <p className="st-lab-summary">{exp.summary}</p>}
    </div>
  );
}

// ── Main view ───────────────────────────────────────────────────────────────

export function ContextLabView() {
  const [history, setHistory] = useState<LabExperiment[]>([]);
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [query, setQuery] = useState('');

  const load = useCallback(async () => {
    setBusy(true);
    try {
      const h = await invoke<LabExperiment[]>('context_lab_history', { limit: 10 });
      setHistory(h);
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

  const run = useCallback(async () => {
    if (!query.trim()) return;
    setBusy(true);
    try {
      await invoke('context_lab_run', { query: query.trim() });
      setQuery('');
      await load();
    } catch (err) {
      setError(String(err));
    } finally {
      setBusy(false);
    }
  }, [query, load]);

  const hero = (
    <PageHero
      kicker="System 6 · Experiment Bench"
      title="Context Lab"
      copy="One question, three assembly strategies — compact, balanced, rich. Each run measures how much memory fits, how mature the layers are, and the predicted answer accuracy, so Nexus learns which strategy wins per question."
      accent="var(--periwinkle)"
      secondary="var(--cyan)"
      stats={[
        { label: 'Experiments', value: String(history.length), color: 'var(--periwinkle)' },
        { label: 'Best so far', value: history.length ? history[0].bestStrategy : '—', color: 'var(--cyan)' },
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
      <div className="st-page" style={{ '--st-accent': 'var(--periwinkle)' } as CSSProperties}>
        {hero}{actions}
        <StrataAlert icon={AlertTriangle}>{error}</StrataAlert>
      </div>
    );
  }

  return (
    <div className="st-page" style={{ '--st-accent': 'var(--periwinkle)' } as CSSProperties}>
      {hero}{actions}

      {/* Run a new experiment */}
      <section className="st-section-head" style={{ margin: '4px 0 10px' }}>
        <h2 className="st-section-title" style={{ '--section-color': 'var(--periwinkle)' } as CSSProperties}>
          <FlaskConical size={14} /> New experiment
        </h2>
        <InfoTip text="Type a real question and Nexus will build the context with every strategy and compare the results." />
      </section>
      <p className="st-section-desc">Type a real question and Nexus builds the context with every strategy — compact, balanced, rich — and compares the results.</p>
      <div className="st-sys-probe">
        <input
          className="st-sys-input"
          placeholder="What are you asking the knowledge base?"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          onKeyDown={(e) => { if (e.key === 'Enter') void run(); }}
        />
        <button type="button" className="st-btn" disabled={busy || !query.trim()} onClick={run}>
          <FlaskConical size={13} /> Run
        </button>
      </div>

      {/* History */}
      <div className="st-section-head" style={{ margin: '26px 0 10px' }}>
        <h2 className="st-section-title" style={{ '--section-color': 'var(--periwinkle)' } as CSSProperties}>
          <Beaker size={14} /> Experiments
        </h2>
        <InfoTip text="The latest runs. The winner per question and the full metrics are stored so Nexus keeps learning which strategy to pick." />
      </div>
      <p className="st-section-desc">The latest runs. The winner per question and the full metrics are stored so Nexus keeps learning which strategy to pick.</p>
      {history.length === 0 ? (
        <StrataVoid icon={Beaker} title="No experiments yet">
          Run your first experiment above to compare compact, balanced and rich context assembly.
        </StrataVoid>
      ) : (
        <div className="st-section-frame">
          <div className="st-lab-list">
            {history.map((exp, i) => <ExperimentCard key={`${exp.createdAt}-${i}`} exp={exp} index={i} />)}
          </div>
        </div>
      )}
    </div>
  );
}
