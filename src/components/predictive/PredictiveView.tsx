import { useCallback, useEffect, useState } from 'react';
import type { CSSProperties } from 'react';
import { invoke } from '@tauri-apps/api/core';
import {
  AlertTriangle, ArrowRight, History, RefreshCw, Target, Zap,
} from 'lucide-react';
import type { PredictiveResponse } from '../../types';
import {
  InfoTip, PageHero, StrataAlert, StrataSkeletons, StrataVoid,
} from '../ui/Instruments';
import { pct } from '../../lib/format';

// ── Prediction card ─────────────────────────────────────────────────────────

function PredictionCard({ prediction, index }: { prediction: PredictiveResponse['predictions'][number]; index: number }) {
  const width = Math.round(prediction.confidence * 100);
  const color = width >= 60 ? 'var(--mint)' : width >= 35 ? 'var(--gold)' : 'var(--rose)';
  return (
    <div className="st-pred-card" style={{ '--st-i': index } as CSSProperties}>
      <div className="st-pred-card-head">
        <span className="st-pred-rank">#{index + 1}</span>
        <span className="st-pred-confidence" style={{ color }}>
          {pct(prediction.confidence)}
        </span>
      </div>
      <div className="st-pred-query">
        <ArrowRight size={13} style={{ color: 'var(--periwinkle)' }} />
        {prediction.suggestedQuery}
      </div>
      <div className="st-pred-track">
        <span className="st-pred-fill" style={{ width: `${width}%`, background: color }} />
      </div>
      <div className="st-pred-meta">
        {prediction.intentType} · {prediction.matches} history matches
      </div>
      {prediction.entities.length > 0 && (
        <div className="st-pred-chips">
          {prediction.entities.map((e) => <span key={e} className="st-pred-chip">{e}</span>)}
        </div>
      )}
    </div>
  );
}

// ── Main view ───────────────────────────────────────────────────────────────

export function PredictiveView() {
  const [response, setResponse] = useState<PredictiveResponse | null>(null);
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [query, setQuery] = useState('');

  const load = useCallback(async () => {
    setBusy(true);
    try {
      const result = await invoke<PredictiveResponse>('predictive_predict', {
        query: query || '',
        topK: 3,
      });
      setResponse(result);
      setError(null);
    } catch (err) {
      setError(String(err));
    } finally {
      setBusy(false);
      setLoading(false);
    }
  }, [query]);

  useEffect(() => {
    void load();
  }, [load]);

  const predict = useCallback(async () => {
    setLoading(true);
    await load();
    setLoading(false);
  }, [load]);

  const hero = (
    <PageHero
      kicker="System 9 · Second Guess"
      title="Predictive Context"
      copy="Every context build feeds the query history. Nexus builds Markov transitions from that history and answers: 'with this probability the next question will be X — and these entities are worth prewarming in advance'."
      accent="var(--periwinkle)"
      secondary="var(--mint)"
      stats={[
        { label: 'History', value: response ? String(response.history_size) : '—', color: 'var(--periwinkle)' },
        { label: 'Prewarm', value: response ? String(response.prewarm_entities.length) : '—', color: 'var(--mint)' },
      ]}
    />
  );

  const actions = (
    <div className="st-radar-actions">
      <button type="button" className="st-action-btn" disabled={busy} onClick={() => void predict()}>
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

      {/* Predict next */}
      <section className="st-section-head" style={{ margin: '4px 0 10px' }}>
        <h2 className="st-section-title" style={{ '--section-color': 'var(--periwinkle)' } as CSSProperties}>
          <Zap size={14} /> Predict the next query
        </h2>
        <InfoTip text="Type a query (or leave empty for the global pattern) and Nexus will predict the most likely follow-ups from Markov transitions." />
      </section>
      <p className="st-section-desc">Type a query (or leave it empty for the global pattern) and Nexus predicts the most likely follow-ups from Markov transitions.</p>
      <div className="st-sys-probe">
        <input
          className="st-sys-input"
          placeholder="e.g. what did we decide about the merge?"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          onKeyDown={(e) => { if (e.key === 'Enter') void predict(); }}
        />
        <button type="button" className="st-btn" disabled={busy} onClick={() => void predict()}>
          <Zap size={13} /> Predict
        </button>
      </div>

      {/* Predictions */}
      <div className="st-section-head" style={{ margin: '26px 0 10px' }}>
        <h2 className="st-section-title" style={{ '--section-color': 'var(--periwinkle)' } as CSSProperties}>
          <Target size={14} /> Predictions
        </h2>
        <InfoTip text="Ranked follow-up queries with confidence. Prewarm entities are the graph nodes worth loading ahead of the next request." />
      </div>
      <p className="st-section-desc">Ranked follow-up queries with confidence. Prewarm entities are the graph nodes worth loading ahead of the next request.</p>
      {!response || response.predictions.length === 0 ? (
        <StrataVoid icon={History} title="Not enough history yet">
          Every <code>build_context</code> call logs the query. Ask a few questions and predictions will appear here.
        </StrataVoid>
      ) : (
        <>
          <div className="st-section-frame">
            <div className="st-pred-grid">
              {response.predictions.map((p, i) => <PredictionCard key={`${p.suggestedQuery}-${i}`} prediction={p} index={i} />)}
            </div>
          </div>

          {response.prewarm_entities.length > 0 && (
            <div className="st-pred-prewarm">
              <span className="st-pred-prewarm-label"><Target size={12} /> prewarm</span>
              <div className="st-pred-chips">
                {response.prewarm_entities.map((e) => <span key={e} className="st-pred-chip st-pred-chip--prewarm">{e}</span>)}
              </div>
            </div>
          )}
        </>
      )}
    </div>
  );
}
