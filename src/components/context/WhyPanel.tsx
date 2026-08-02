import { useMemo, useState } from 'react';
import {
  Brain, ChevronDown, ChevronRight, Filter, Hash, Network,
  Scissors, Sparkles,
} from 'lucide-react';
import { useLocale } from '../../stores/localeStore';
import type {
  ContextDropCause, ContextReason, ContextScorePart, ContextTrace,
} from '../../types';

/**
 * "Why is this in my context?"
 *
 * Every other tool in this space hands the model a pile of notes and hopes for
 * the best. The reason this panel exists is that our value claim - a *ranked,
 * compressed* context - is unverifiable unless the user can see the reasoning.
 * So each item shows what put it there, what it scored and why, what it costs in
 * tokens, and - for the items that did not make it - what pushed them out.
 */

// ── Reason rendering ────────────────────────────────────────────────────────

/**
 * Render one inclusion reason as human copy.
 *
 * The switch is exhaustive over the tagged union, so adding a reason in Rust
 * surfaces here as a TypeScript error rather than as a blank badge.
 */
function reasonText(r: ContextReason, t: (k: string) => string): string {
  switch (r.kind) {
    case 'queryMatch':
      return `${t('why.reason.queryMatch')}: "${r.query}"`;
    case 'keywordMatch':
      return `${t('why.reason.keywordMatch')}: "${r.keyword}"`;
    case 'graphExpansion':
      return `${t('why.reason.graphExpansion')} "${r.fromTitle}" (${r.hops} ${
        r.hops === 1 ? t('why.hop') : t('why.hops')
      })`;
    case 'memorySearch':
      return `${t('why.reason.memorySearch')}: "${r.query}"`;
    case 'recentActivity':
      return r.ageDays <= 0
        ? t('why.reason.today')
        : `${t('why.reason.recentActivity')}: ${r.ageDays} ${t('why.days')}`;
    case 'highImportance':
      return `${t('why.reason.highImportance')}: ${r.importance.toFixed(2)}`;
  }
}

function reasonColor(kind: ContextReason['kind']): string {
  switch (kind) {
    case 'queryMatch':      return 'var(--gold)';
    case 'keywordMatch':    return 'var(--cyan)';
    case 'graphExpansion':  return 'var(--periwinkle)';
    case 'memorySearch':    return 'var(--tangerine)';
    case 'recentActivity':  return 'var(--mint)';
    case 'highImportance':  return 'var(--steel)';
  }
}

function dropText(d: ContextDropCause, t: (k: string) => string): string {
  switch (d.kind) {
    case 'belowRelevance':
      // Showing both numbers turns "pruned" into something actionable: the user
      // can see a near miss and raise or lower the floor deliberately.
      return `${t('why.drop.belowRelevance')} ${d.score.toFixed(2)} < ${d.floor.toFixed(2)}`;
    case 'tokenBudget':
      return `${t('why.drop.tokenBudget')} (${d.limit})`;
    case 'entityCap':
      return `${t('why.drop.entityCap')} (${d.cap})`;
  }
}

// ── Score breakdown ─────────────────────────────────────────────────────────

/** Localised label for a score component, falling back to the raw id. */
function partLabel(component: string, t: (k: string) => string): string {
  const key = `why.part.${component}`;
  const localised = t(key);
  return localised === key ? component : localised;
}

function ScoreBreakdown({ parts }: { parts: ContextScorePart[] }) {
  const { t } = useLocale();
  if (parts.length === 0) return null;

  const max = Math.max(...parts.map((p) => Math.abs(p.points)), 0.0001);

  return (
    <div style={{ marginTop: '8px', display: 'flex', flexDirection: 'column', gap: '4px' }}>
      {parts.map((p, i) => (
        <div key={`${p.component}-${i}`} style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
          <span style={{
            fontSize: '10px', color: 'var(--muted-2)', minWidth: '92px',
            textTransform: 'capitalize',
          }}>
            {partLabel(p.component, t)}
          </span>
          <div style={{
            flex: 1, height: '4px', background: 'var(--carbon-soft)',
            borderRadius: '2px', overflow: 'hidden',
          }}>
            <div style={{
              height: '100%',
              width: `${(Math.abs(p.points) / max) * 100}%`,
              background: 'var(--gold)',
              borderRadius: '2px',
            }} />
          </div>
          <span style={{
            fontSize: '10px', color: 'var(--muted)', fontFamily: 'var(--mono)',
            minWidth: '38px', textAlign: 'right',
          }}>
            +{p.points.toFixed(2)}
          </span>
        </div>
      ))}
    </div>
  );
}

// ── One traced item ─────────────────────────────────────────────────────────

function TraceRow({ trace }: { trace: ContextTrace }) {
  const { t } = useLocale();
  const [open, setOpen] = useState(false);
  const hasDetail = trace.scoreParts.length > 0;

  return (
    <div style={{
      background: 'var(--carbon)',
      borderRadius: 'var(--radius-xs)',
      padding: '10px 12px',
      opacity: trace.included ? 1 : 0.62,
      borderLeft: `2px solid ${trace.included ? 'var(--mint)' : 'var(--rose)'}`,
    }}>
      <div
        onClick={() => hasDetail && setOpen(!open)}
        style={{
          display: 'flex', alignItems: 'center', gap: '8px',
          cursor: hasDetail ? 'pointer' : 'default',
        }}
      >
        {trace.kind === 'entity'
          ? <Network size={12} style={{ color: 'var(--cyan)', flexShrink: 0 }} />
          : <Brain size={12} style={{ color: 'var(--tangerine)', flexShrink: 0 }} />
        }
        <span style={{
          fontSize: '12px', fontWeight: 600, color: 'var(--bone)',
          flex: 1, minWidth: 0, overflow: 'hidden',
          textOverflow: 'ellipsis', whiteSpace: 'nowrap',
        }}>
          {trace.title || trace.id.slice(0, 8)}
        </span>

        {trace.tokens > 0 && (
          <span style={{
            display: 'inline-flex', alignItems: 'center', gap: '3px',
            fontSize: '10px', color: 'var(--muted-2)', fontFamily: 'var(--mono)',
          }}>
            <Hash size={9} />
            {trace.tokens}
          </span>
        )}

        {trace.score !== null && (
          <span style={{
            padding: '1px 6px', borderRadius: '999px',
            fontSize: '10px', fontWeight: 700, fontFamily: 'var(--mono)',
            background: 'var(--gold-soft)', color: 'var(--gold)',
          }}>
            {trace.score.toFixed(2)}
          </span>
        )}

        {hasDetail && (open
          ? <ChevronDown size={12} style={{ color: 'var(--muted-3)' }} />
          : <ChevronRight size={12} style={{ color: 'var(--muted-3)' }} />
        )}
      </div>

      {/* Reasons: the actual answer to "why is this here" */}
      <div style={{ display: 'flex', flexWrap: 'wrap', gap: '4px', marginTop: '6px' }}>
        {trace.reasons.map((r, i) => (
          <span
            key={`${r.kind}-${i}`}
            style={{
              padding: '1px 7px', borderRadius: '999px', fontSize: '10px',
              background: `${reasonColor(r.kind)}18`,
              color: reasonColor(r.kind),
              border: `1px solid ${reasonColor(r.kind)}30`,
            }}
          >
            {reasonText(r, t)}
          </span>
        ))}
        {trace.dropped && (
          <span style={{
            padding: '1px 7px', borderRadius: '999px', fontSize: '10px',
            background: 'rgba(255,112,133,0.12)', color: 'var(--rose)',
            border: '1px solid rgba(255,112,133,0.28)',
            display: 'inline-flex', alignItems: 'center', gap: '3px',
          }}>
            <Scissors size={9} />
            {dropText(trace.dropped, t)}
          </span>
        )}
      </div>

      {open && <ScoreBreakdown parts={trace.scoreParts} />}
    </div>
  );
}

// ── Panel ───────────────────────────────────────────────────────────────────

type Tab = 'included' | 'dropped';

export function WhyPanel({ traces }: { traces: ContextTrace[] }) {
  const { t } = useLocale();
  const [tab, setTab] = useState<Tab>('included');
  const [open, setOpen] = useState(true);

  const { included, dropped } = useMemo(() => ({
    included: traces.filter((x) => x.included),
    dropped: traces.filter((x) => !x.included),
  }), [traces]);

  // An older backend sends no provenance; hiding beats rendering an empty shell.
  if (traces.length === 0) return null;

  const rows = tab === 'included' ? included : dropped;

  return (
    <div style={{
      background: 'var(--surface)', border: '1px solid var(--line)',
      borderRadius: 'var(--radius)', marginBottom: '12px', overflow: 'hidden',
    }}>
      <button
        onClick={() => setOpen(!open)}
        style={{
          display: 'flex', alignItems: 'center', gap: '8px', width: '100%',
          padding: '12px 16px', background: 'none', border: 'none',
          cursor: 'pointer', textAlign: 'left',
        }}
      >
        <Sparkles size={14} style={{ color: 'var(--gold)', flexShrink: 0 }} />
        <span style={{ fontSize: '13px', fontWeight: 600, color: 'var(--bone)', flex: 1 }}>
          {t('why.title')}
        </span>
        <span style={{
          padding: '1px 6px', borderRadius: '999px', fontSize: '10px',
          fontWeight: 600, background: 'var(--gold-soft)', color: 'var(--gold)',
        }}>
          {traces.length}
        </span>
        {open
          ? <ChevronDown size={14} style={{ color: 'var(--muted-3)' }} />
          : <ChevronRight size={14} style={{ color: 'var(--muted-3)' }} />
        }
      </button>

      {open && (
        <div style={{ padding: '0 16px 14px' }}>
          <p style={{
            fontSize: '11px', color: 'var(--muted)', lineHeight: 1.5,
            margin: '0 0 10px',
          }}>
            {t('why.subtitle')}
          </p>

          <div style={{ display: 'flex', gap: '6px', marginBottom: '10px' }}>
            {(['included', 'dropped'] as Tab[]).map((id) => {
              const active = tab === id;
              const count = id === 'included' ? included.length : dropped.length;
              return (
                <button
                  key={id}
                  onClick={() => setTab(id)}
                  style={{
                    display: 'inline-flex', alignItems: 'center', gap: '5px',
                    padding: '5px 11px', fontSize: '11px', fontWeight: 600,
                    borderRadius: '999px', cursor: 'pointer',
                    background: active ? 'var(--raised)' : 'transparent',
                    border: `1px solid ${active ? 'var(--line)' : 'transparent'}`,
                    color: active ? 'var(--bone)' : 'var(--muted-2)',
                  }}
                >
                  {id === 'included'
                    ? <Sparkles size={10} />
                    : <Filter size={10} />}
                  {t(id === 'included' ? 'why.tab.included' : 'why.tab.dropped')}
                  <span style={{ fontFamily: 'var(--mono)', opacity: 0.7 }}>{count}</span>
                </button>
              );
            })}
          </div>

          {rows.length === 0 ? (
            <div style={{
              padding: '18px', textAlign: 'center',
              fontSize: '11px', color: 'var(--muted-2)',
            }}>
              {t(tab === 'included' ? 'why.empty.included' : 'why.empty.dropped')}
            </div>
          ) : (
            <div style={{ display: 'flex', flexDirection: 'column', gap: '5px' }}>
              {rows.map((tr) => <TraceRow key={`${tr.kind}-${tr.id}`} trace={tr} />)}
            </div>
          )}
        </div>
      )}
    </div>
  );
}
