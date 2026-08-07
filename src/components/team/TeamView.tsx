import { useCallback, useEffect, useRef, useState } from 'react';
import type { CSSProperties } from 'react';
import { invoke } from '@tauri-apps/api/core';
import {
  AlertTriangle, CheckCircle2, Clock3, RefreshCw, ShieldQuestion, UserPlus, Users,
} from 'lucide-react';
import type { Memory, TeamOverview } from '../../types';
import { useLocale } from '../../stores/localeStore';
import { useMemoryStore } from '../../stores/memoryStore';
import { useUiStore } from '../../stores/uiStore';
import {
  InfoTip, PageHero, StrataAlert, StrataSkeletons, StrataVoid,
} from '../ui/Instruments';

const ROLE_COLORS: Record<string, string> = {
  admin: 'var(--rose)',
  member: 'var(--periwinkle)',
  viewer: 'var(--cyan)',
};

function roleColor(role: string) {
  return ROLE_COLORS[role] ?? 'var(--bone)';
}

// ── Count tile (reuses radar tile styles) ──────────────────────────────────

function CountTile({
  icon: Icon,
  label,
  value,
  color,
  hint,
}: {
  icon: typeof Users;
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

export function TeamView() {
  const { t } = useLocale();
  const { selectMemory } = useMemoryStore();
  const { setActiveView } = useUiStore();
  const [overview, setOverview] = useState<TeamOverview | null>(null);
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  // Flashes the refresh button to a success state after a reload.
  const [done, setDone] = useState(false);
  const doneTimer = useRef<ReturnType<typeof setTimeout> | null>(null);

  // Open the underlying memory in its detail sheet — the decision chain
  // (why it exists, who confirmed it, what replaced it) lives on that record.
  const openMemory = useCallback(async (memoryId: string) => {
    try {
      const memory = await invoke<Memory | null>('get_memory', { id: memoryId });
      if (!memory) return;
      selectMemory(memory);
      setActiveView('memory');
    } catch {
      // The record may have been deleted since the overview was built; stay put.
    }
  }, [selectMemory, setActiveView]);

  const load = useCallback(async () => {
    setBusy(true);
    try {
      const result = await invoke<TeamOverview>('get_team_overview');
      setOverview(result);
      setError(null);
      setDone(true);
      if (doneTimer.current) clearTimeout(doneTimer.current);
      doneTimer.current = setTimeout(() => setDone(false), 1_800);
    } catch (err) {
      setError(String(err));
    } finally {
      setBusy(false);
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    load();
  }, [load]);

  const hero = (
    <PageHero
      kicker={t('team.hero.kicker')}
      title={t('team.title')}
      copy={t('team.hero.sub')}
      accent="var(--mint)"
      secondary="var(--periwinkle)"
      stats={[
        { label: t('team.stats.members'), value: overview ? String(overview.totals.members) : '—' },
        { label: t('team.stats.confirmed'), value: overview ? String(overview.totals.confirmed) : '—', color: 'var(--mint)' },
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
        {done ? t('team.refresh.done') : t('team.refresh')}
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

  if (!overview) return null;

  const totals = overview.totals;

  return (
    <div className="st-page" style={{ '--st-accent': 'var(--mint)' } as CSSProperties}>
      {hero}{actions}

      {/* The trusted layer at a glance */}
      <section className="st-radar-tiles">
        <CountTile
          icon={CheckCircle2}
          label={t('team.counts.confirmed')}
          value={totals.confirmed}
          color="var(--mint)"
          hint={t('team.counts.confirmed.hint')}
        />
        <CountTile
          icon={Clock3}
          label={t('team.counts.superseded')}
          value={totals.superseded}
          color="var(--gold)"
          hint={t('team.counts.superseded.hint')}
        />
        <CountTile
          icon={AlertTriangle}
          label={t('team.counts.conflicted')}
          value={totals.conflicted}
          color="var(--rose)"
          hint={t('team.counts.conflicted.hint')}
        />
        <CountTile
          icon={ShieldQuestion}
          label={t('team.counts.authored')}
          value={totals.authored}
          color="var(--periwinkle)"
          hint={t('team.counts.authored.hint')}
        />
      </section>

      {/* Roster */}
      <section>
        <h2 className="st-section-title" style={{ '--section-color': 'var(--mint)' } as CSSProperties}>
          <Users size={14} /> {t('team.roster.title')}
        </h2>
        <p className="st-section-hint">{t('team.roster.hint')}</p>
        {overview.members.length === 0 ? (
          <StrataVoid icon={UserPlus} title={t('team.roster.empty.title')} accent="var(--mint)">
            {t('team.roster.empty.desc')}
          </StrataVoid>
        ) : (
          <div className="st-panel">
            <div className="st-team-roster">
              {overview.members.map((activity, index) => {
                const color = roleColor(activity.member.role);
                return (
                  <div
                    key={activity.member.id}
                    className="st-team-roster-row st-rise"
                    style={{ '--st-i': index, '--row-color': color } as CSSProperties}
                  >
                    <span className="st-team-avatar" style={{ color, background: `${color}15` }}>
                      {activity.member.name.charAt(0).toUpperCase()}
                    </span>
                    <span style={{ minWidth: 0, flex: 1 }}>
                      <span className="st-team-roster-name">
                        {activity.member.name}
                        {!activity.member.active && <span className="st-team-inactive"> · {t('team.inactive')}</span>}
                      </span>
                      <span className="st-team-roster-meta">
                        {t('team.authored')} {activity.authored} · {t('team.confirmed')} {activity.confirmed} · {t('team.updated')} {activity.updated}
                      </span>
                    </span>
                    <span className="st-team-role" style={{ color, borderColor: `${color}40`, background: `${color}10` }}>
                      {activity.member.role}
                    </span>
                  </div>
                );
              })}
            </div>
          </div>
        )}
      </section>

      {/* Trusted decisions */}
      <section>
        <h2 className="st-section-title" style={{ '--section-color': 'var(--mint)' } as CSSProperties}>
          <CheckCircle2 size={14} /> {t('team.decisions.title')}
        </h2>
        <p className="st-section-hint">{t('team.decisions.hint')}</p>
        {overview.confirmedDecisions.length === 0 &&
          overview.supersededDecisions.length === 0 &&
          overview.conflicted.length === 0 ? (
          <StrataVoid icon={ShieldQuestion} title={t('team.decisions.empty.title')} accent="var(--mint)">
            {t('team.decisions.empty.desc')}
          </StrataVoid>
        ) : (
          <div className="st-panel">
            <div className="st-team-decisions">
              {overview.confirmedDecisions.map((d) => (
                <button
                  key={`c-${d.memoryId}`}
                  type="button"
                  className="st-team-decision st-team-decision-confirmed"
                  style={{ '--row-color': 'var(--mint)' } as CSSProperties}
                  onClick={() => openMemory(d.memoryId)}
                >
                  <span className="st-team-decision-icon" style={{ color: 'var(--mint)', background: 'var(--mint)15' }}>
                    <CheckCircle2 size={14} />
                  </span>
                  <span style={{ minWidth: 0, flex: 1 }}>
                    <span className="st-team-decision-title">{d.title}</span>
                    <span className="st-team-decision-meta">
                      {t('team.confirmedBy')} {d.by ?? '—'} {d.at ? ` · ${d.at}` : ''}
                    </span>
                  </span>
                </button>
              ))}
              {overview.supersededDecisions.map((d) => (
                <button
                  key={`s-${d.memoryId}`}
                  type="button"
                  className="st-team-decision st-team-decision-superseded"
                  style={{ '--row-color': 'var(--gold)' } as CSSProperties}
                  onClick={() => openMemory(d.memoryId)}
                >
                  <span className="st-team-decision-icon" style={{ color: 'var(--gold)', background: 'var(--gold)15' }}>
                    <Clock3 size={14} />
                  </span>
                  <span style={{ minWidth: 0, flex: 1 }}>
                    <span className="st-team-decision-title">{d.title}</span>
                    <span className="st-team-decision-meta">
                      {d.detail ?? t('team.superseded')}
                    </span>
                  </span>
                </button>
              ))}
              {overview.conflicted.map((d) => (
                <button
                  key={`x-${d.memoryId}`}
                  type="button"
                  className="st-team-decision st-team-decision-conflicted"
                  style={{ '--row-color': 'var(--rose)' } as CSSProperties}
                  onClick={() => openMemory(d.memoryId)}
                >
                  <span className="st-team-decision-icon" style={{ color: 'var(--rose)', background: 'var(--rose)15' }}>
                    <AlertTriangle size={14} />
                  </span>
                  <span style={{ minWidth: 0, flex: 1 }}>
                    <span className="st-team-decision-title">{d.title}</span>
                    <span className="st-team-decision-meta">
                      {t('team.conflictedBy')} {d.by ?? '—'}
                    </span>
                  </span>
                </button>
              ))}
            </div>
          </div>
        )}
      </section>
    </div>
  );
}
