import { useCallback, useEffect, useRef, useState } from 'react';
import type { CSSProperties } from 'react';
import { invoke } from '@tauri-apps/api/core';
import {
  AlertTriangle, CheckCircle2, Clock3, Eye, Radar, RefreshCw, ShieldQuestion,
  Sparkles, TriangleAlert,
} from 'lucide-react';
import type { Memory, RadarSnapshot } from '../../types';
import { useLocale } from '../../stores/localeStore';
import { useMemoryStore } from '../../stores/memoryStore';
import { useUiStore } from '../../stores/uiStore';
import { pct } from '../../lib/format';
import {
  ImpactBlocks, InfoTip, PageHero, StrataAlert, StrataSkeletons, StrataVoid,
} from '../ui/Instruments';

// ── Action metadata ─────────────────────────────────────────────────────────
// One source of truth for how a radar item's action renders. `resolve` is the
// loudest (a human decision is required), the rest are progressively calmer.

const ACTION_META: Record<string, { icon: typeof Radar; color: string; soft: string; labelKey: string }> = {
  resolve: { icon: TriangleAlert, color: 'var(--rose)', soft: 'var(--rose-soft)', labelKey: 'radar.action.resolve' },
  recheck: { icon: Clock3, color: 'var(--gold)', soft: 'var(--gold-soft)', labelKey: 'radar.action.recheck' },
  confirm: { icon: ShieldQuestion, color: 'var(--periwinkle)', soft: 'var(--periwinkle-soft)', labelKey: 'radar.action.confirm' },
  review: { icon: Eye, color: 'var(--cyan)', soft: 'var(--cyan-soft)', labelKey: 'radar.action.review' },
};

function actionMeta(action: string) {
  return ACTION_META[action] ?? ACTION_META.review;
}

// ── Count tile ──────────────────────────────────────────────────────────────

function CountTile({
  icon: Icon,
  label,
  value,
  color,
  hint,
}: {
  icon: typeof Radar;
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

// ── Main view ───────────────────────────────────────────────────────────────

export function RadarView() {
  const { t } = useLocale();
  const { selectMemory } = useMemoryStore();
  const { setActiveView } = useUiStore();
  const [snapshot, setSnapshot] = useState<RadarSnapshot | null>(null);
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  // Which action just finished, so its button can flash a success state.
  const [done, setDone] = useState<'refresh' | 'seen' | null>(null);
  const doneTimer = useRef<ReturnType<typeof setTimeout> | null>(null);

  const load = useCallback(async (markSeen: boolean) => {
    setBusy(true);
    try {
      const result = markSeen
        ? await invoke<RadarSnapshot>('radar_scan_and_seen')
        : await invoke<RadarSnapshot>('get_radar_snapshot');
      setSnapshot(result);
      setError(null);
      const kind = markSeen ? 'seen' : 'refresh';
      setDone(kind);
      if (doneTimer.current) clearTimeout(doneTimer.current);
      doneTimer.current = setTimeout(() => setDone(null), 1_800);
    } catch (err) {
      setError(String(err));
    } finally {
      setBusy(false);
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    load(false);
  }, [load]);

  // Open the underlying memory in its detail sheet — the reason an item sits
  // on the radar (what changed, when it expires) is best read on the record.
  const openMemory = useCallback(async (memoryId: string) => {
    try {
      const memory = await invoke<Memory | null>('get_memory', { id: memoryId });
      if (!memory) return;
      selectMemory(memory);
      setActiveView('memory');
    } catch {
      // The record may have been deleted since the snapshot was built; stay put.
    }
  }, [selectMemory, setActiveView]);

  const hero = (
    <PageHero
      kicker={t('radar.hero.kicker')}
      title={t('radar.title')}
      copy={t('radar.hero.sub')}
      accent="var(--periwinkle)"
      secondary="var(--tangerine)"
      stats={[
        { label: t('radar.stats.attention'), value: snapshot ? `${snapshot.attentionScore}%` : '—', color: 'var(--periwinkle)' },
        { label: t('radar.stats.total'), value: snapshot ? String(snapshot.counts.total) : '—' },
      ]}
    />
  );

  const actions = (
    <div className="st-radar-actions">
      <button
        type="button"
        className="st-action-btn"
        disabled={busy}
        onClick={() => load(false)}
      >
        {done === 'refresh' ? <CheckCircle2 size={13} style={{ color: 'var(--mint)' }} /> : <RefreshCw size={13} className={busy ? 'spinning' : undefined} />}
        {done === 'refresh' ? t('radar.refresh.done') : t('radar.refresh')}
      </button>
      <button
        type="button"
        className="st-action-btn"
        disabled={busy}
        onClick={() => load(true)}
      >
        {done === 'seen' ? <CheckCircle2 size={13} style={{ color: 'var(--mint)' }} /> : <CheckCircle2 size={13} />}
        {done === 'seen' ? t('radar.markSeen.done') : t('radar.markSeen')}
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

  if (!snapshot) return null;

  const counts = snapshot.counts;

  return (
    <div className="st-page" style={{ '--st-accent': 'var(--periwinkle)' } as CSSProperties}>
      {hero}{actions}

      {/* What the radar is telling you at a glance */}
      <section className="st-radar-tiles">
        <CountTile
          icon={TriangleAlert}
          label={t('radar.counts.conflicted')}
          value={counts.conflicted}
          color="var(--rose)"
          hint={t('radar.counts.conflicted.hint')}
        />
        <CountTile
          icon={Clock3}
          label={t('radar.counts.expiring')}
          value={counts.expiring}
          color="var(--gold)"
          hint={t('radar.counts.expiring.hint')}
        />
        <CountTile
          icon={ShieldQuestion}
          label={t('radar.counts.inferred')}
          value={counts.inferred}
          color="var(--periwinkle)"
          hint={t('radar.counts.inferred.hint')}
        />
        <CountTile
          icon={Sparkles}
          label={t('radar.counts.new')}
          value={counts.newSinceLastScan}
          color="var(--cyan)"
          hint={t('radar.counts.new.hint')}
        />
      </section>

      {/* Since marker */}
      <p className="st-section-hint" style={{ margin: '0 3px 14px' }}>
        {snapshot.since
          ? t('radar.since.since') + ' ' + snapshot.since
          : t('radar.since.first')}
      </p>

      {/* Actionable items */}
      {snapshot.items.length === 0 ? (
        <StrataVoid icon={CheckCircle2} title={t('radar.empty.title')} accent="var(--mint)">
          {t('radar.empty.desc')}
        </StrataVoid>
      ) : (
        <div className="st-panel">
          <div className="st-radar-list">
            {snapshot.items.map((item, index) => {
              const meta = actionMeta(item.action);
              const Icon = meta.icon;
              return (
                <button
                  key={item.id}
                  type="button"
                  className="st-radar-row st-rise"
                  style={{ '--st-i': index, '--row-color': meta.color } as CSSProperties}
                  onClick={() => openMemory(item.id)}
                >
                  <span className="st-radar-row-icon" style={{ color: meta.color, background: `${meta.color}15` }}>
                    <Icon size={15} />
                  </span>
                  <span style={{ minWidth: 0, flex: 1 }}>
                    <span className="st-radar-row-title">{item.title}</span>
                    <span className="st-radar-row-reason">{item.reason}</span>
                    <span className="st-radar-row-meta">
                      {t(meta.labelKey)} · {item.memoryState}
                      {item.expiresAt ? ` · ${t('radar.expires')} ${item.expiresAt}` : ''}
                    </span>
                  </span>
                  <span className="st-radar-row-impact">
                    <ImpactBlocks value={item.importance} label={t('inst.impact')} />
                    <span className="st-radar-row-import">{pct(item.importance)}%</span>
                  </span>
                </button>
              );
            })}
          </div>
        </div>
      )}
    </div>
  );
}
