import { useState, useEffect, useCallback, useRef } from 'react';
import { invoke } from '@tauri-apps/api/core';
import {
  TrendingDown, Coins, Zap, Clock, Calendar, BarChart3,
  ArrowUpRight, Sparkles, RefreshCw,
} from 'lucide-react';
import { useLocale } from '../../stores/localeStore';

interface SavingsStats {
  total_interactions: number;
  total_tokens_saved: number;
  total_cost_saved_usd: number;
  avg_tokens_per_interaction: number;
  tokens_saved_today: number;
  cost_saved_today: number;
  tokens_saved_week: number;
  cost_saved_week: number;
  tokens_saved_month: number;
  cost_saved_month: number;
  tokens_saved_year: number;
  cost_saved_year: number;
  obsidian_equivalent_tokens: number;
  obsidian_equivalent_cost_usd: number;
  recent_interactions: InteractionRecord[];

  // Provenance of every figure above.
  //
  // The baseline used to be `interactions * 2000` — a constant nobody could
  // reproduce. It is now the summed, measured cost of reading each candidate
  // source in full, so the comparison is auditable. These fields let the page
  // state *how* a number was produced instead of asking the reader to trust it.
  baseline_tokens: number;
  baseline_cost_usd: number;
  /** Rows recorded with a measured baseline (excludes pre-V11 estimates). */
  measured_interactions: number;
  /** Subset of the above counted with the real BPE vocabulary. */
  exact_interactions: number;
  /** 'exact' when the tokenizer vocabulary is loaded, otherwise 'estimated'. */
  token_method: string;
}

interface InteractionRecord {
  tokens_saved: number;
  cost_saved_usd: number;
  entities_count: number;
  memories_count: number;
  query_preview: string;
  created_at: string;
}

// ── Animated counter hook ──
function useAnimatedNumber(target: number, duration = 800): number {
  const [display, setDisplay] = useState(0);
  const prevRef = useRef(0);
  const frameRef = useRef<number>(0);

  useEffect(() => {
    const from = prevRef.current;
    const to = target;
    const start = performance.now();

    function tick(now: number) {
      const elapsed = now - start;
      const progress = Math.min(elapsed / duration, 1);
      // ease-out cubic
      const eased = 1 - Math.pow(1 - progress, 3);
      setDisplay(Math.round(from + (to - from) * eased));
      if (progress < 1) {
        frameRef.current = requestAnimationFrame(tick);
      } else {
        prevRef.current = to;
      }
    }

    frameRef.current = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(frameRef.current);
  }, [target, duration]);

  return display;
}

// ── Format helpers ──
function formatTokens(n: number): string {
  if (n >= 1_000_000) return (n / 1_000_000).toFixed(1) + 'M';
  if (n >= 1_000) return (n / 1_000).toFixed(1) + 'K';
  return n.toLocaleString();
}

function formatUsd(n: number): string {
  if (n >= 1) return '$' + n.toFixed(2);
  if (n >= 0.01) return '$' + n.toFixed(3);
  return '$' + n.toFixed(4);
}

// ── Animated stat card ──
function StatCard({
  icon: Icon,
  label,
  value,
  subValue,
  color,
  delay = 0,
}: {
  icon: typeof TrendingDown;
  label: string;
  value: number;
  subValue?: string;
  color: string;
  delay?: number;
}) {
  const [visible, setVisible] = useState(false);
  useEffect(() => {
    const t = setTimeout(() => setVisible(true), delay);
    return () => clearTimeout(t);
  }, [delay]);

  const animatedValue = useAnimatedNumber(value);

  return (
    <div
      style={{
        background: 'var(--surface)',
        border: '1px solid var(--line)',
        borderRadius: 'var(--radius-sm)',
        padding: '20px',
        opacity: visible ? 1 : 0,
        transform: visible ? 'translateY(0)' : 'translateY(12px)',
        transition: 'opacity 0.5s ease, transform 0.5s ease',
      }}
    >
      <div style={{ display: 'flex', alignItems: 'center', gap: '8px', marginBottom: '10px' }}>
        <div style={{
          width: '32px', height: '32px', borderRadius: 'var(--radius-xs)',
          background: `${color}15`, display: 'flex', alignItems: 'center', justifyContent: 'center',
        }}>
          <Icon size={16} style={{ color }} />
        </div>
        <span style={{ fontSize: '12px', color: 'var(--muted-2)', textTransform: 'uppercase', letterSpacing: '0.06em' }}>
          {label}
        </span>
      </div>
      <div style={{
        fontSize: '28px', fontWeight: 700, color: 'var(--bone)',
        fontFamily: 'var(--brand)', letterSpacing: '-0.02em',
      }}>
        {formatTokens(animatedValue)}
      </div>
      {subValue && (
        <div style={{
          fontSize: '12px', color: 'var(--muted)', marginTop: '4px',
          fontFamily: 'var(--mono)',
        }}>
          {subValue}
        </div>
      )}
    </div>
  );
}

// ── Savings comparison bar ──
function ComparisonBar({ withNexus, withoutNexus }: { withNexus: number; withoutNexus: number }) {
  const [anim, setAnim] = useState(false);
  useEffect(() => { setTimeout(() => setAnim(true), 300); }, []);

  const maxVal = Math.max(withNexus, withoutNexus, 1);
  const nexusPct = (withNexus / maxVal) * 100;
  const manualPct = (withoutNexus / maxVal) * 100;

  return (
    <div style={{ marginTop: '24px' }}>
      <div style={{
        display: 'flex', justifyContent: 'space-between', alignItems: 'center',
        marginBottom: '12px',
      }}>
        <span style={{ fontSize: '13px', fontWeight: 600, color: 'var(--bone)' }}>
          Token Usage Comparison
        </span>
        <span style={{
          fontSize: '11px', color: 'var(--mint)',
          background: 'var(--mint-soft)', padding: '3px 10px',
          borderRadius: '999px', fontWeight: 600,
        }}>
          {withoutNexus > 0 ? Math.round(((withoutNexus - withNexus) / withoutNexus) * 100) : 0}% savings
        </span>
      </div>

      {/* Without Nexus */}
      <div style={{ marginBottom: '10px' }}>
        <div style={{ display: 'flex', justifyContent: 'space-between', marginBottom: '4px' }}>
          <span style={{ fontSize: '11px', color: 'var(--muted-2)' }}>Without Nexus (Obsidian/Notion)</span>
          <span style={{ fontSize: '11px', color: 'var(--rose)', fontFamily: 'var(--mono)' }}>
            {formatTokens(withoutNexus)} tokens
          </span>
        </div>
        <div style={{ height: '8px', background: 'var(--carbon-soft)', borderRadius: '4px', overflow: 'hidden' }}>
          <div style={{
            height: '100%', borderRadius: '4px',
            background: 'linear-gradient(90deg, var(--rose), rgba(255, 112, 133, 0.6))',
            width: anim ? `${manualPct}%` : '0%',
            transition: 'width 1.2s cubic-bezier(0.22, 1, 0.36, 1)',
          }} />
        </div>
      </div>

      {/* With Nexus */}
      <div>
        <div style={{ display: 'flex', justifyContent: 'space-between', marginBottom: '4px' }}>
          <span style={{ fontSize: '11px', color: 'var(--muted-2)' }}>With Nexus</span>
          <span style={{ fontSize: '11px', color: 'var(--mint)', fontFamily: 'var(--mono)' }}>
            {formatTokens(withNexus)} tokens
          </span>
        </div>
        <div style={{ height: '8px', background: 'var(--carbon-soft)', borderRadius: '4px', overflow: 'hidden' }}>
          <div style={{
            height: '100%', borderRadius: '4px',
            background: 'linear-gradient(90deg, var(--mint), rgba(117, 212, 161, 0.6))',
            width: anim ? `${nexusPct}%` : '0%',
            transition: 'width 1.2s cubic-bezier(0.22, 1, 0.36, 1)',
          }} />
        </div>
      </div>
    </div>
  );
}

// ── Live interaction feed ──
function LiveFeed({ interactions }: { interactions: InteractionRecord[] }) {
  const [visible, setVisible] = useState(false);
  useEffect(() => { setTimeout(() => setVisible(true), 600); }, []);

  if (interactions.length === 0) {
    return (
      <div style={{
        textAlign: 'center', padding: '40px 20px',
        color: 'var(--muted-2)', fontSize: '13px',
      }}>
        <Sparkles size={24} style={{ opacity: 0.3, marginBottom: '8px' }} />
        <div>No interactions yet. Start using Nexus to see savings.</div>
      </div>
    );
  }

  return (
    <div style={{ maxHeight: '300px', overflowY: 'auto' }}>
      {interactions.map((inter, i) => (
        <div
          key={i}
          style={{
            display: 'flex', alignItems: 'center', justifyContent: 'space-between',
            padding: '10px 0',
            borderBottom: i < interactions.length - 1 ? '1px solid var(--line)' : 'none',
            opacity: visible ? 1 : 0,
            transform: visible ? 'translateX(0)' : 'translateX(-10px)',
            transition: `opacity 0.3s ease ${i * 50}ms, transform 0.3s ease ${i * 50}ms`,
          }}
        >
          <div style={{ flex: 1, minWidth: 0 }}>
            <div style={{
              fontSize: '13px', color: 'var(--bone)',
              overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap',
            }}>
              {inter.query_preview || 'Context build'}
            </div>
            <div style={{
              fontSize: '11px', color: 'var(--muted-2)', marginTop: '2px',
              fontFamily: 'var(--mono)',
            }}>
              {inter.entities_count} entities · {inter.memories_count} memories
            </div>
          </div>
          <div style={{ textAlign: 'right', marginLeft: '12px', flexShrink: 0 }}>
            <div style={{ fontSize: '13px', fontWeight: 600, color: 'var(--mint)', fontFamily: 'var(--mono)' }}>
              +{formatTokens(inter.tokens_saved)}
            </div>
            <div style={{ fontSize: '10px', color: 'var(--muted-2)', fontFamily: 'var(--mono)' }}>
              {formatUsd(inter.cost_saved_usd)}
            </div>
          </div>
        </div>
      ))}
    </div>
  );
}

// ── Price table (full models + Nexus savings + subscriptions) ──
interface ModelPricing {
  company: string;
  name: string;
  inputPer1M: number;   // USD per 1M input tokens
  outputPer1M: number;  // USD per 1M output tokens
  context: string;       // context window
  purpose: string;       // short description
}

const ALL_MODELS: ModelPricing[] = [
  // OpenAI
  { company: 'OpenAI', name: 'GPT-5.6 Sol',       inputPer1M: 5.00,  outputPer1M: 30.00, context: '1M',  purpose: 'Flagship' },
  { company: 'OpenAI', name: 'GPT-5.6 Terra',     inputPer1M: 2.00,  outputPer1M: 12.00, context: '1M',  purpose: 'Main' },
  { company: 'OpenAI', name: 'GPT-5.6 Luna',      inputPer1M: 0.20,  outputPer1M: 1.20,  context: '1M',  purpose: 'Budget' },
  // Anthropic
  { company: 'Anthropic', name: 'Claude Fable 5',    inputPer1M: 10.00, outputPer1M: 50.00, context: '500k+', purpose: 'Max quality' },
  { company: 'Anthropic', name: 'Claude Opus 5',     inputPer1M: 5.00,  outputPer1M: 25.00, context: '500k+', purpose: 'Top reasoning' },
  { company: 'Anthropic', name: 'Claude Sonnet 5',   inputPer1M: 2.00,  outputPer1M: 10.00, context: '500k+', purpose: 'Best value' },
  { company: 'Anthropic', name: 'Claude Sonnet 4.6', inputPer1M: 3.00,  outputPer1M: 15.00, context: '500k',  purpose: 'Prev gen' },
  { company: 'Anthropic', name: 'Claude Haiku 4.5',  inputPer1M: 1.00,  outputPer1M: 5.00,  context: '500k',  purpose: 'Fast' },
  // Google
  { company: 'Google', name: 'Gemini 3.1 Pro',   inputPer1M: 2.00,  outputPer1M: 12.00, context: '1M',  purpose: 'Main Gemini' },
  { company: 'Google', name: 'Gemini 2.5 Pro',   inputPer1M: 1.25,  outputPer1M: 10.00, context: '1M',  purpose: 'Prev gen' },
  { company: 'Google', name: 'Gemini Flash',     inputPer1M: 0.35,  outputPer1M: 1.50,  context: '1M',  purpose: 'Budget' },
  // xAI
  { company: 'xAI', name: 'Grok 4.x',   inputPer1M: 2.50,  outputPer1M: 12.50, context: '2M',  purpose: 'Reasoning' },
  { company: 'xAI', name: 'Grok Fast',   inputPer1M: 0.80,  outputPer1M: 5.00,  context: '2M',  purpose: 'Fast' },
  // DeepSeek
  { company: 'DeepSeek', name: 'DeepSeek V4',       inputPer1M: 0.30,  outputPer1M: 1.20,  context: '256k', purpose: 'Universal' },
  { company: 'DeepSeek', name: 'DeepSeek V4 Flash',  inputPer1M: 0.14,  outputPer1M: 0.90,  context: '256k', purpose: 'Cheapest' },
  // Moonshot
  { company: 'Moonshot', name: 'Kimi K3',    inputPer1M: 0.50,  outputPer1M: 2.00,  context: '1M',  purpose: 'Code' },
  // Alibaba
  { company: 'Alibaba', name: 'Qwen 3',     inputPer1M: 0.40,  outputPer1M: 2.00,  context: '1M',  purpose: 'Universal' },
  // Mistral
  { company: 'Mistral', name: 'Magistral Medium', inputPer1M: 2.00, outputPer1M: 8.00,  context: '256k', purpose: 'Reasoning' },
  { company: 'Mistral', name: 'Mistral Small',     inputPer1M: 0.80, outputPer1M: 2.50,  context: '128k', purpose: 'Fast' },
  // Cohere
  { company: 'Cohere', name: 'Command R+',  inputPer1M: 3.00,  outputPer1M: 15.00, context: '128k', purpose: 'RAG' },
  { company: 'Cohere', name: 'Command R',   inputPer1M: 1.00,  outputPer1M: 5.00,  context: '128k', purpose: 'Enterprise' },
];

interface SubPlan {
  service: string;
  free: string;
  paid: string;
  max: string;
}

const SUBSCRIPTIONS: SubPlan[] = [
  { service: 'ChatGPT',      free: 'Free',      paid: 'Plus $20/mo',        max: 'Pro $200/mo' },
  { service: 'Claude',       free: 'Free',      paid: 'Pro $20/mo',         max: 'Max $100–200/mo' },
  { service: 'Gemini',       free: 'Free',      paid: 'Advanced $20/mo',    max: 'Ultra / Enterprise' },
  { service: 'Grok',         free: 'Limited',   paid: 'SuperGrok ~$30/mo',  max: 'Enterprise' },
  { service: 'Perplexity',   free: 'Free',      paid: 'Pro $20/mo',         max: 'Enterprise' },
];

function formatUsd4(n: number): string {
  if (n >= 1) return '$' + n.toFixed(2);
  if (n >= 0.01) return '$' + n.toFixed(3);
  return '$' + n.toFixed(4);
}

function PriceTable({ totalTokensSaved }: { totalTokensSaved: number }) {
  const [section, setSection] = useState<'models' | 'subscriptions'>('models');

  // Group models by company for visual separation
  const grouped: { company: string; models: ModelPricing[] }[] = [];
  for (const m of ALL_MODELS) {
    const last = grouped[grouped.length - 1];
    if (last && last.company === m.company) {
      last.models.push(m);
    } else {
      grouped.push({ company: m.company, models: [m] });
    }
  }

  return (
    <div>
      {/* Section tabs */}
      <div style={{
        display: 'flex', gap: '2px', marginBottom: '12px',
        background: 'var(--carbon-soft)', borderRadius: 'var(--radius-xs)',
        padding: '3px',
      }}>
        {(['models', 'subscriptions'] as const).map(s => (
          <button
            key={s}
            onClick={() => setSection(s)}
            style={{
              flex: 1, padding: '8px 12px',
              background: section === s ? 'var(--surface)' : 'transparent',
              border: 'none', borderRadius: 'var(--radius-xs)',
              fontSize: '12px', fontWeight: 600,
              color: section === s ? 'var(--bone)' : 'var(--muted-2)',
              cursor: 'pointer', transition: 'all 0.2s',
              textTransform: 'uppercase', letterSpacing: '0.04em',
            }}
          >
            {s === 'models' ? 'Model Pricing' : 'Subscriptions'}
          </button>
        ))}
      </div>

      {section === 'models' && (
        <div style={{
          background: 'var(--surface)', border: '1px solid var(--line)',
          borderRadius: 'var(--radius-sm)', overflow: 'hidden',
        }}>
          {/* Header */}
          <div style={{
            padding: '12px 16px', borderBottom: '1px solid var(--line)',
            fontSize: '12px', fontWeight: 600, color: 'var(--muted-2)',
            textTransform: 'uppercase', letterSpacing: '0.06em',
            display: 'flex', alignItems: 'center', gap: '6px',
          }}>
            <BarChart3 size={14} />
            LLM Pricing — per 1M tokens (August 2026)
          </div>

          {/* Column headers */}
          <div style={{
            display: 'grid',
            gridTemplateColumns: '120px 90px 90px 80px 1fr 100px',
            padding: '8px 16px',
            borderBottom: '1px solid var(--line)',
            background: 'var(--carbon-soft)',
            fontSize: '10px', fontWeight: 700, color: 'var(--muted-2)',
            textTransform: 'uppercase', letterSpacing: '0.06em',
          }}>
            <span>Model</span>
            <span style={{ textAlign: 'right' }}>Input</span>
            <span style={{ textAlign: 'right' }}>Output</span>
            <span style={{ textAlign: 'right' }}>Context</span>
            <span style={{ textAlign: 'right' }}>Purpose</span>
            <span style={{ textAlign: 'right', color: 'var(--mint)' }}>Nexus Saved</span>
          </div>

          {/* Model rows grouped by company */}
          {grouped.map((group) => (
            <div key={group.company}>
              {/* Company header */}
              <div style={{
                padding: '6px 16px',
                background: 'rgba(117,212,161,0.03)',
                borderBottom: '1px solid var(--line)',
                fontSize: '11px', fontWeight: 700, color: 'var(--periwinkle)',
                letterSpacing: '0.03em',
              }}>
                {group.company}
              </div>

              {group.models.map((m) => {
                // Nexus savings = totalTokensSaved * inputPrice / 1_000_000
                const nexusSavedUsd = totalTokensSaved * (m.inputPer1M / 1_000_000);
                return (
                  <div
                    key={m.name}
                    style={{
                      display: 'grid',
                      gridTemplateColumns: '120px 90px 90px 80px 1fr 100px',
                      padding: '8px 16px',
                      borderBottom: '1px solid var(--line)',
                      fontSize: '12px',
                    }}
                  >
                    <span style={{ color: 'var(--bone)', fontWeight: 500 }}>{m.name}</span>
                    <span style={{ color: 'var(--muted)', fontFamily: 'var(--mono)', textAlign: 'right' }}>
                      ${m.inputPer1M.toFixed(2)}
                    </span>
                    <span style={{ color: 'var(--muted)', fontFamily: 'var(--mono)', textAlign: 'right' }}>
                      ${m.outputPer1M.toFixed(2)}
                    </span>
                    <span style={{
                      color: 'var(--muted-2)', fontSize: '11px',
                      display: 'flex', alignItems: 'center', justifyContent: 'flex-end',
                    }}>
                      {m.context}
                    </span>
                    <span style={{
                      color: 'var(--muted-2)', fontSize: '11px',
                      display: 'flex', alignItems: 'center', justifyContent: 'flex-end',
                    }}>
                      {m.purpose}
                    </span>
                    <span style={{
                      color: nexusSavedUsd > 0 ? 'var(--mint)' : 'var(--muted-2)',
                      fontFamily: 'var(--mono)', textAlign: 'right', fontWeight: 600,
                    }}>
                      {nexusSavedUsd > 0 ? formatUsd4(nexusSavedUsd) : '—'}
                    </span>
                  </div>
                );
              })}
            </div>
          ))}

          {/* Summary row */}
          <div style={{
            padding: '10px 16px',
            background: 'rgba(117,212,161,0.05)',
            fontSize: '11px', color: 'var(--muted)',
            display: 'flex', justifyContent: 'space-between',
          }}>
            <span>Based on <span style={{ fontFamily: 'var(--mono)', color: 'var(--bone)' }}>{formatTokens(totalTokensSaved)}</span> input tokens saved</span>
            <span style={{ color: 'var(--muted-2)' }}>Nexus column = input tokens × model price</span>
          </div>
        </div>
      )}

      {section === 'subscriptions' && (
        <div style={{
          background: 'var(--surface)', border: '1px solid var(--line)',
          borderRadius: 'var(--radius-sm)', overflow: 'hidden',
        }}>
          {/* Header */}
          <div style={{
            padding: '12px 16px', borderBottom: '1px solid var(--line)',
            fontSize: '12px', fontWeight: 600, color: 'var(--muted-2)',
            textTransform: 'uppercase', letterSpacing: '0.06em',
            display: 'flex', alignItems: 'center', gap: '6px',
          }}>
            <BarChart3 size={14} />
            Subscription Costs
          </div>

          {/* Column headers */}
          <div style={{
            display: 'grid',
            gridTemplateColumns: '110px 1fr 1fr 1fr',
            padding: '8px 16px',
            borderBottom: '1px solid var(--line)',
            background: 'var(--carbon-soft)',
            fontSize: '10px', fontWeight: 700, color: 'var(--muted-2)',
            textTransform: 'uppercase', letterSpacing: '0.06em',
          }}>
            <span>Service</span>
            <span>Free Tier</span>
            <span>Paid</span>
            <span>Max</span>
          </div>

          {SUBSCRIPTIONS.map((s, i) => (
            <div
              key={s.service}
              style={{
                display: 'grid',
                gridTemplateColumns: '110px 1fr 1fr 1fr',
                padding: '10px 16px',
                borderBottom: i < SUBSCRIPTIONS.length - 1 ? '1px solid var(--line)' : 'none',
                fontSize: '12px',
              }}
            >
              <span style={{ color: 'var(--bone)', fontWeight: 500 }}>{s.service}</span>
              <span style={{ color: 'var(--mint)', fontFamily: 'var(--mono)' }}>{s.free}</span>
              <span style={{ color: 'var(--muted)', fontFamily: 'var(--mono)' }}>{s.paid}</span>
              <span style={{ color: 'var(--muted)', fontFamily: 'var(--mono)' }}>{s.max}</span>
            </div>
          ))}

          {/* Note */}
          <div style={{
            padding: '10px 16px',
            background: 'rgba(169,156,248,0.04)',
            fontSize: '11px', color: 'var(--muted)',
          }}>
            Nexus replaces these subscriptions for context management — you keep your existing API keys, we just save tokens.
          </div>
        </div>
      )}
    </div>
  );
}

// ── Main SavingsView ──
export function SavingsView() {
  const { t } = useLocale();
  const [stats, setStats] = useState<SavingsStats | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const loadStats = useCallback(async () => {
    try {
      const result = await invoke<SavingsStats>('get_savings_stats');
      setStats(result);
      setError(null);
    } catch (err) {
      setError(String(err));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    loadStats();
    // Auto-refresh every 30 seconds for live updates
    const interval = setInterval(loadStats, 30000);
    return () => clearInterval(interval);
  }, [loadStats]);

  return (
    <div style={{ maxWidth: '1000px', margin: '0 auto', padding: '32px 24px' }}>
      {/* Header */}
      <div style={{
        display: 'flex', alignItems: 'flex-start', justifyContent: 'space-between',
        marginBottom: '32px',
      }}>
        <div>
          <h1 style={{
            fontFamily: 'var(--brand)', fontSize: '28px', fontWeight: 700,
            color: 'var(--bone)', letterSpacing: '-0.02em', margin: 0,
            display: 'flex', alignItems: 'center', gap: '10px',
          }}>
            <TrendingDown size={24} style={{ color: 'var(--mint)' }} />
            Savings
          </h1>
          <p style={{
            fontSize: 'var(--text)', color: 'var(--muted)', marginTop: '6px',
          }}>
            Token savings from using Nexus context engine
          </p>
        </div>
        <button
          onClick={loadStats}
          className="settings-action-btn"
          style={loading ? { opacity: 0.5 } : undefined}
        >
          <RefreshCw size={13} className={loading ? 'spinning' : 'settings-action-icon'} /> Reload
        </button>
      </div>

      {error && (
        <div style={{
          padding: '12px 16px', marginBottom: '20px',
          background: 'var(--rose-soft)', border: '1px solid rgba(255,112,133,0.2)',
          borderRadius: 'var(--radius-sm)', fontSize: '13px', color: 'var(--rose)',
        }}>
          {error}
        </div>
      )}

      {/* Hero stat — total tokens saved */}
      <div style={{
        background: 'linear-gradient(135deg, rgba(117,212,161,0.08), rgba(99,216,210,0.05))',
        border: '1px solid rgba(117,212,161,0.15)',
        borderRadius: 'var(--radius)', padding: '28px 32px',
        marginBottom: '28px', textAlign: 'center',
      }}>
        <div style={{
          fontSize: '12px', color: 'var(--muted-2)', textTransform: 'uppercase',
          letterSpacing: '0.1em', marginBottom: '8px',
        }}>
          Total Tokens Saved
        </div>
        <div style={{
          fontSize: '48px', fontWeight: 800, color: 'var(--mint)',
          fontFamily: 'var(--brand)', letterSpacing: '-0.03em',
          lineHeight: 1,
        }}>
          {formatTokens(useAnimatedNumber(stats?.total_tokens_saved || 0, 1200))}
        </div>
        <div style={{
          fontSize: '14px', color: 'var(--muted)', marginTop: '8px',
          fontFamily: 'var(--mono)',
        }}>
          {formatUsd(stats?.total_cost_saved_usd || 0)} saved
        </div>
        <div style={{
          fontSize: '12px', color: 'var(--muted-2)', marginTop: '12px',
        }}>
          across {stats?.total_interactions || 0} context interactions
        </div>
      </div>

      {/* Period breakdown */}
      <div style={{ display: 'grid', gridTemplateColumns: 'repeat(4, 1fr)', gap: '12px', marginBottom: '28px' }}>
        <StatCard
          icon={Zap}
          label="Today"
          value={stats?.tokens_saved_today || 0}
          subValue={formatUsd(stats?.cost_saved_today || 0)}
          color="var(--tangerine)"
          delay={0}
        />
        <StatCard
          icon={Clock}
          label="This Week"
          value={stats?.tokens_saved_week || 0}
          subValue={formatUsd(stats?.cost_saved_week || 0)}
          color="var(--periwinkle)"
          delay={100}
        />
        <StatCard
          icon={Calendar}
          label="This Month"
          value={stats?.tokens_saved_month || 0}
          subValue={formatUsd(stats?.cost_saved_month || 0)}
          color="var(--cyan)"
          delay={200}
        />
        <StatCard
          icon={TrendingDown}
          label="This Year"
          value={stats?.tokens_saved_year || 0}
          subValue={formatUsd(stats?.cost_saved_year || 0)}
          color="var(--mint)"
          delay={300}
        />
      </div>

      {/* Average per interaction */}
      <div style={{
        background: 'var(--surface)', border: '1px solid var(--line)',
        borderRadius: 'var(--radius-sm)', padding: '16px 20px',
        marginBottom: '28px', display: 'flex', alignItems: 'center', gap: '16px',
      }}>
        <div style={{
          width: '40px', height: '40px', borderRadius: '50%',
          background: 'var(--gold-soft)', display: 'flex',
          alignItems: 'center', justifyContent: 'center',
        }}>
          <Coins size={18} style={{ color: 'var(--gold)' }} />
        </div>
        <div>
          <div style={{ fontSize: '13px', color: 'var(--muted-2)' }}>
            Average per interaction
          </div>
          <div style={{
            fontSize: '20px', fontWeight: 700, color: 'var(--bone)',
            fontFamily: 'var(--brand)',
          }}>
            {formatTokens(stats?.avg_tokens_per_interaction || 0)} tokens
          </div>
        </div>
      </div>

      {/* Comparison bar */}
      <div style={{
        background: 'var(--surface)', border: '1px solid var(--line)',
        borderRadius: 'var(--radius-sm)', padding: '20px',
        marginBottom: '28px',
      }}>
        {/*
          `withNexus` must be the tokens actually SENT, not the tokens saved.
          It used to receive `total_tokens_saved`, which inverted the chart:
          a perfect 100% saving would have drawn the longest bar. The tokens
          sent are the measured baseline minus the measured saving.
        */}
        <ComparisonBar
          withNexus={Math.max((stats?.baseline_tokens || 0) - (stats?.total_tokens_saved || 0), 0)}
          withoutNexus={stats?.baseline_tokens || 0}
        />
      </div>

      {/* Marketing callout */}
      <div style={{
        background: 'linear-gradient(135deg, rgba(169,156,248,0.08), rgba(120,169,255,0.05))',
        border: '1px solid rgba(169,156,248,0.15)',
        borderRadius: 'var(--radius)', padding: '24px',
        marginBottom: '28px',
      }}>
        <div style={{
          display: 'flex', alignItems: 'flex-start', gap: '14px',
        }}>
          <Sparkles size={20} style={{ color: 'var(--periwinkle)', marginTop: '2px', flexShrink: 0 }} />
          <div>
            <div style={{
              fontSize: '15px', fontWeight: 600, color: 'var(--bone)',
              marginBottom: '6px',
            }}>
              {t('savings.callout.title')}
            </div>
            <div style={{
              fontSize: '13px', color: 'var(--muted)', lineHeight: 1.6,
            }}>
              {t('savings.callout.body1')}{' '}
              <span style={{ color: 'var(--rose)', fontWeight: 600 }}>
                {formatTokens(stats?.baseline_tokens || 0)} {t('savings.tokens')}
              </span>{' '}
              ({formatUsd(stats?.baseline_cost_usd || 0)}).{' '}
              {t('savings.callout.body2')}{' '}
              <span style={{ color: 'var(--mint)', fontWeight: 600 }}>
                {formatTokens(stats?.total_tokens_saved || 0)} {t('savings.tokens')}
              </span>{' '}
              ({formatUsd(stats?.total_cost_saved_usd || 0)}).{' '}
              {t('savings.callout.body3')}
            </div>
          </div>
        </div>
      </div>

      {/* Recent interactions */}
      <div style={{
        background: 'var(--surface)', border: '1px solid var(--line)',
        borderRadius: 'var(--radius-sm)', overflow: 'hidden',
        marginBottom: '28px',
      }}>
        <div style={{
          padding: '12px 16px', borderBottom: '1px solid var(--line)',
          fontSize: '12px', fontWeight: 600, color: 'var(--muted-2)',
          textTransform: 'uppercase', letterSpacing: '0.06em',
          display: 'flex', alignItems: 'center', gap: '6px',
        }}>
          <ArrowUpRight size={14} />
          Recent Interactions (live feed)
        </div>
        <div style={{ padding: '0 16px' }}>
          <LiveFeed interactions={stats?.recent_interactions || []} />
        </div>
      </div>

      {/* Price table reference */}
      <PriceTable totalTokensSaved={stats?.total_tokens_saved || 0} />
    </div>
  );
}
