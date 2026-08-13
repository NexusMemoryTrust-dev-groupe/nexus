import { useCallback, useEffect, useRef, useState } from 'react';
import type { CSSProperties } from 'react';
import { invoke } from '@tauri-apps/api/core';
import {
  AlertTriangle, CheckCircle2, Crown, RefreshCw, Scale, ShieldCheck, Zap,
} from 'lucide-react';
import type { ConflictGroup, Memory, TruthVerdict } from '../../types';
import { useLocale } from '../../stores/localeStore';
import { useMemoryStore } from '../../stores/memoryStore';
import { useConflictStore } from '../../stores/conflictStore';
import { useUiStore } from '../../stores/uiStore';
import { ago } from '../../lib/format';
import {
  InfoTip, PageHero, StrataAlert, StrataSkeletons, StrataVoid,
} from '../ui/Instruments';

// ── Count tile (reuses radar tile styles) ──────────────────────────────────

function CountTile({
  icon: Icon,
  label,
  value,
  color,
  hint,
}: {
  icon: typeof Scale;
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

export function ConflictsView() {
  const { locale, t } = useLocale();
  const { memories } = useMemoryStore();
  const { setActiveView } = useUiStore();
  const {
    conflicts, verdicts, isLoading, error,
    checkConflicts, resolveConflict,
  } = useConflictStore();
  const [busy, setBusy] = useState(false);
  const [done, setDone] = useState(false);
  const doneTimer = useRef<ReturnType<typeof setTimeout> | null>(null);

  // Resolve titles for member ids from the loaded memory pool. A member whose
  // record was deleted falls back to its short id rather than breaking the card.
  const titleOf = useCallback(
    (id: string) => {
      const memory = memories.find((m) => m.id === id);
      return memory ? memory.title : `…${id.slice(0, 8)}`;
    },
    [memories],
  );

  const load = useCallback(async () => {
    setBusy(true);
    try {
      await checkConflicts();
      setDone(true);
      if (doneTimer.current) clearTimeout(doneTimer.current);
      doneTimer.current = setTimeout(() => setDone(false), 1_800);
    } finally {
      setBusy(false);
    }
  }, [checkConflicts]);

  useEffect(() => {
    load();
  }, [load]);

  const openMemory = useCallback(async (memoryId: string) => {
    try {
      const memory = await invoke<Memory | null>('get_memory', { id: memoryId });
      if (!memory) return;
      useMemoryStore.getState().selectMemory(memory);
      setActiveView('memory');
    } catch {
      // The record may have been deleted since the group was built; stay put.
    }
  }, [setActiveView]);

  // Human decides: the chosen member wins, the rest become Superseded.
  const chooseWinner = useCallback(
    async (group: ConflictGroup, winnerId: string) => {
      setBusy(true);
      try {
        await resolveConflict(group.id, winnerId, 'user');
      } finally {
        setBusy(false);
      }
    },
    [resolveConflict],
  );

  const open = conflicts.filter((g) => g.status === 'open');
  const resolved = conflicts.filter((g) => g.status === 'resolved');

  const hero = (
    <PageHero
      kicker={t('conflict.hero.kicker')}
      title={t('conflict.title')}
      copy={t('conflict.hero.sub')}
      accent="var(--rose)"
      secondary="var(--tangerine)"
      stats={[
        { label: t('conflict.stats.open'), value: String(open.length) },
        { label: t('conflict.stats.resolved'), value: String(resolved.length), color: 'var(--mint)' },
      ]}
    />
  );

  const actions = (
    <div className="st-radar-actions">
      <button
        type="button"
        className="st-action-btn"
        disabled={busy}
        onClick={load}
      >
        {done ? <CheckCircle2 size={13} style={{ color: 'var(--mint)' }} /> : <RefreshCw size={13} className={busy ? 'spinning' : undefined} />}
        {done ? t('conflict.refresh.done') : t('conflict.refresh')}
      </button>
    </div>
  );

  if (isLoading && conflicts.length === 0) {
    return <div className="st-page">{hero}{actions}<StrataSkeletons /></div>;
  }

  if (error) {
    return (
      <div className="st-page" style={{ '--st-accent': 'var(--rose)' } as CSSProperties}>
        {hero}{actions}
        <StrataAlert icon={AlertTriangle}>{error}</StrataAlert>
      </div>
    );
  }

  return (
    <div className="st-page" style={{ '--st-accent': 'var(--rose)' } as CSSProperties}>
      {hero}{actions}

      <section className="st-radar-tiles">
        <CountTile
          icon={Zap}
          label={t('conflict.counts.open')}
          value={open.length}
          color="var(--rose)"
          hint={t('conflict.counts.open.hint')}
        />
        <CountTile
          icon={ShieldCheck}
          label={t('conflict.counts.resolved')}
          value={resolved.length}
          color="var(--mint)"
          hint={t('conflict.counts.resolved.hint')}
        />
      </section>

      {/* Open conflicts — the engine's verdict + the human's choice */}
      <section>
        <h2 className="st-section-title" style={{ '--section-color': 'var(--rose)' } as CSSProperties}>
          <Zap size={14} /> {t('conflict.open.title')}
        </h2>
        <p className="st-section-hint">{t('conflict.open.hint')}</p>
        {open.length === 0 ? (
          <StrataVoid icon={ShieldCheck} title={t('conflict.open.empty.title')} accent="var(--rose)">
            {t('conflict.open.empty.desc')}
          </StrataVoid>
        ) : (
          <div className="st-panel st-conflict-list">
            {open.map((group, index) => (
              <div
                key={group.id}
                className="st-conflict-group st-rise"
                style={{ '--st-i': index } as CSSProperties}
              >
                <div className="st-conflict-group-head">
                  <span className="st-conflict-topic">{group.topic}</span>
                  <span className="st-conflict-detected">
                    {t('conflict.detected')} {ago(group.detectedAt, locale)}
                  </span>
                </div>

                {/* Engine verdict, computed live and read-only */}
                <EngineVerdict verdict={verdicts[group.id]} t={t} />

                <div className="st-conflict-members">
                  {group.memberIds.map((memberId) => {
                    const memory = memories.find((m) => m.id === memberId);
                    const state = memory?.memoryState ?? 'Unknown';
                    const isWinner = verdicts[group.id]?.winnerId === memberId;
                    return (
                      <button
                        key={memberId}
                        type="button"
                        className={`st-conflict-member${isWinner ? ' is-verdict' : ''}`}
                        style={{ '--row-color': isWinner ? 'var(--mint)' : 'var(--rose)' } as CSSProperties}
                        onClick={() => openMemory(memberId)}
                      >
                        <span className="st-conflict-member-icon" style={{ color: isWinner ? 'var(--mint)' : 'var(--rose)', background: isWinner ? 'var(--mint)15' : 'var(--rose)15' }}>
                          {isWinner ? <Crown size={14} /> : <AlertTriangle size={14} />}
                        </span>
                        <span style={{ minWidth: 0, flex: 1 }}>
                          <span className="st-conflict-member-title">{titleOf(memberId)}</span>
                          <span className="st-conflict-member-meta">{state}</span>
                        </span>
                        <span
                          className="st-conflict-choose"
                          role="button"
                          tabIndex={0}
                          onClick={(event) => {
                            event.stopPropagation();
                            chooseWinner(group, memberId);
                          }}
                          onKeyDown={(event) => {
                            if (event.key === 'Enter' || event.key === ' ') {
                              event.stopPropagation();
                              chooseWinner(group, memberId);
                            }
                          }}
                        >
                          {t('conflict.choose')}
                        </span>
                      </button>
                    );
                  })}
                </div>
              </div>
            ))}
          </div>
        )}
      </section>

      {/* Resolved conflicts — what the timeline says now */}
      {resolved.length > 0 && (
        <section>
          <h2 className="st-section-title" style={{ '--section-color': 'var(--mint)' } as CSSProperties}>
            <ShieldCheck size={14} /> {t('conflict.resolved.title')}
          </h2>
          <p className="st-section-hint">{t('conflict.resolved.hint')}</p>
          <div className="st-panel st-conflict-list">
            {resolved.map((group, index) => (
              <div
                key={group.id}
                className="st-conflict-group st-rise st-conflict-group--resolved"
                style={{ '--st-i': index } as CSSProperties}
              >
                <div className="st-conflict-group-head">
                  <span className="st-conflict-topic">{group.topic}</span>
                  {group.resolution && (
                    <span className="st-conflict-verdict-line">
                      <Crown size={11} style={{ color: 'var(--mint)' }} />
                      {titleOf(group.resolution.winnerId)} · {Math.round(group.resolution.confidence * 100)}%
                      · {group.resolution.by === 'user' ? t('conflict.by.user') : t('conflict.by.engine')}
                    </span>
                  )}
                </div>
                {group.resolution && (
                  <div className="st-conflict-reasons">
                    {group.resolution.reasons.map((reason) => (
                      <span key={reason} className="st-conflict-reason">{reason}</span>
                    ))}
                  </div>
                )}
                <div className="st-conflict-members">
                  {group.memberIds.map((memberId) => {
                    const isWinner = group.resolution?.winnerId === memberId;
                    return (
                      <button
                        key={memberId}
                        type="button"
                        className={`st-conflict-member${isWinner ? ' is-verdict' : ''}`}
                        style={{ '--row-color': isWinner ? 'var(--mint)' : 'var(--steel)' } as CSSProperties}
                        onClick={() => openMemory(memberId)}
                      >
                        <span className="st-conflict-member-icon" style={{ color: isWinner ? 'var(--mint)' : 'var(--steel)', background: isWinner ? 'var(--mint)15' : 'var(--steel)15' }}>
                          {isWinner ? <Crown size={14} /> : <CheckCircle2 size={14} />}
                        </span>
                        <span style={{ minWidth: 0, flex: 1 }}>
                          <span className="st-conflict-member-title">{titleOf(memberId)}</span>
                          <span className="st-conflict-member-meta">
                            {isWinner ? t('conflict.winner') : t('conflict.superseded')}
                          </span>
                        </span>
                      </button>
                    );
                  })}
                </div>
              </div>
            ))}
          </div>
        </section>
      )}
    </div>
  );
}

// ── Engine verdict block ────────────────────────────────────────────────────

function EngineVerdict({
  verdict,
  t,
}: {
  verdict: TruthVerdict | undefined;
  t: (key: string) => string;
}) {
  if (!verdict) {
    return (
      <div className="st-conflict-verdict st-conflict-verdict--empty">
        <Scale size={11} /> {t('conflict.truth.none')}
      </div>
    );
  }
  return (
    <div className="st-conflict-verdict">
      <Scale size={11} style={{ color: 'var(--mint)' }} />
      <span className="st-conflict-verdict-label">{t('conflict.truth.label')}</span>
      <span className="st-conflict-verdict-winner">{verdict.winnerId.slice(0, 8)}</span>
      <span className="st-conflict-verdict-confidence">
        {Math.round(verdict.confidence * 100)}%
      </span>
      <span className="st-conflict-reasons">
        {verdict.reasons.map((reason) => (
          <span key={reason} className="st-conflict-reason">{reason}</span>
        ))}
      </span>
    </div>
  );
}
