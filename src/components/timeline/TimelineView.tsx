import { useCallback, useMemo, useState } from 'react';
import type { CSSProperties } from 'react';
import { CalendarDays, Clock3, History, Paperclip } from 'lucide-react';
import type { Memory } from '../../types';
import { useMemoryStore } from '../../stores/memoryStore';
import { useUiStore } from '../../stores/uiStore';
import { useLocale } from '../../stores/localeStore';
import { LAYER_ORDER, layerVars, layerVisual } from '../../lib/layers';
import {
  clock, dayFraction, dayKey, dayLabel, dayShort, dayTag,
  freshness, num, steps,
} from '../../lib/format';
import {
  FreshPulse, ImpactBlocks, InfoTip, PageHero, SemanticLayerTag,
  StrataAlert, StrataSkeletons, StrataVoid,
} from '../ui/Instruments';

interface TimelineDay {
  key: string;
  iso: string;
  memories: Memory[];
  counts: Map<string, number>;
}

interface DotCluster {
  at: number;
  memories: Memory[];
}

function localKey(date: Date): string {
  const month = String(date.getMonth() + 1).padStart(2, '0');
  const day = String(date.getDate()).padStart(2, '0');
  return `${date.getFullYear()}-${month}-${day}`;
}

function dayAtOffset(offset: number): Date {
  const today = new Date();
  return new Date(today.getFullYear(), today.getMonth(), today.getDate() - offset);
}

/**
 * Cluster dots that land within a hair of each other (same minute slot) into a
 * single bead with a count badge, so a crowded hour reads as one dot instead
 * of a smear of overlapping circles. The line itself stays clean.
 */
const CLUSTER_GAP = 1.15; // percent of the axis width — dots closer than this fuse

function clusterDots(memories: Memory[]): DotCluster[] {
  const sorted = memories
    .map((memory) => ({ memory, at: dayFraction(memory.createdAt) }))
    .sort((a, b) => a.at - b.at);
  const clusters: DotCluster[] = [];
  for (const { memory, at } of sorted) {
    const last = clusters[clusters.length - 1];
    if (last && at - last.at < CLUSTER_GAP) {
      last.memories.push(memory);
    } else {
      clusters.push({ at, memories: [memory] });
    }
  }
  return clusters;
}

function HeatMap({
  days,
  selected,
  onSelect,
}: {
  days: { key: string; count: number; iso: string }[];
  selected: string | null;
  onSelect: (key: string) => void;
}) {
  const { locale, t } = useLocale();
  const max = Math.max(...days.map((day) => day.count), 1);
  return (
    <div className="st-heat" role="grid" aria-label={t('tl.heat.aria')}>
      {days.map((day) => (
        <button
          key={day.key}
          type="button"
          className={`st-heat-cell${day.count === 0 ? ' st-heat-empty' : ''}`}
          style={{ '--heat': day.count / max } as CSSProperties}
          aria-current={selected === day.key ? 'date' : undefined}
          aria-label={`${dayShort(day.iso, locale)}: ${day.count}`}
          title={`${dayShort(day.iso, locale)} · ${day.count}`}
          onClick={() => onSelect(day.key)}
        />
      ))}
    </div>
  );
}

function AxisDot({
  cluster,
  onOpen,
  locale,
  clusterLabel,
}: {
  cluster: DotCluster;
  onOpen: (memory: Memory) => void;
  locale: 'en' | 'ru';
  clusterLabel: string;
}) {
  const [hovered, setHovered] = useState(false);
  const memories = cluster.memories;
  const primary = memories[0];
  const size = 7 + Math.max(...memories.map((m) => steps(m.importanceScore))) * 1.4;
  const count = memories.length;
  return (
    <span
      className="st-axis-dot"
      style={{
        ...layerVars(primary.layer),
        '--at': cluster.at,
        '--dot-size': `${size}px`,
      } as CSSProperties}
      onMouseEnter={() => setHovered(true)}
      onMouseLeave={() => setHovered(false)}
    >
      <button
        type="button"
        className="st-axis-dot-hit"
        onClick={() => onOpen(primary)}
        aria-label={`${count > 1 ? `${count} memories · ` : ''}${primary.title}, ${clock(primary.createdAt, locale)}`}
        title={`${count > 1 ? `${count} memories · ` : ''}${primary.title} · ${clock(primary.createdAt, locale)}`}
      >
        {count > 1 && <span className="st-axis-dot-count">{count}</span>}
      </button>

      {hovered && (
        <div className="st-axis-card">
          <div className="st-axis-card-head">
            <span className="st-axis-card-time">{clock(primary.createdAt, locale)}</span>
            {count > 1 && <span className="st-axis-card-count">{count} · {clusterLabel}</span>}
          </div>
          <div className="st-axis-card-list">
            {memories.map((memory) => (
              <button
                type="button"
                key={memory.id}
                className="st-axis-card-row"
                style={{ '--row-color': layerVisual(memory.layer).color } as CSSProperties}
                onClick={() => onOpen(memory)}
              >
                <span className="st-axis-card-dot" />
                <span className="st-axis-card-title">{memory.title}</span>
                <span className="st-axis-card-time">{clock(memory.createdAt, locale)}</span>
              </button>
            ))}
          </div>
        </div>
      )}
    </span>
  );
}

export function TimelineView() {
  const { memories, isLoading, error, selectMemory } = useMemoryStore();
  const { setActiveView } = useUiStore();
  const { locale, t } = useLocale();
  const [active, setActive] = useState<Set<string>>(new Set());
  const [oldestFirst, setOldestFirst] = useState(false);
  const [selectedDay, setSelectedDay] = useState<string | null>(null);

  const counts = useMemo(() => {
    const map = new Map<string, number>();
    memories.forEach((memory) => {
      const layer = layerVisual(memory.layer).name;
      map.set(layer, (map.get(layer) ?? 0) + 1);
    });
    return map;
  }, [memories]);

  const filtered = useMemo(
    () => active.size === 0
      ? memories
      : memories.filter((memory) => active.has(layerVisual(memory.layer).name)),
    [active, memories],
  );

  const grouped = useMemo(() => {
    const map = new Map<string, TimelineDay>();
    [...filtered]
      .sort((a, b) => {
        const delta = new Date(b.createdAt).getTime() - new Date(a.createdAt).getTime();
        return oldestFirst ? -delta : delta;
      })
      .forEach((memory) => {
        const key = dayKey(memory.createdAt);
        const current = map.get(key) ?? {
          key,
          iso: memory.createdAt,
          memories: [],
          counts: new Map<string, number>(),
        };
        current.memories.push(memory);
        const layer = layerVisual(memory.layer).name;
        current.counts.set(layer, (current.counts.get(layer) ?? 0) + 1);
        map.set(key, current);
      });
    return [...map.values()];
  }, [filtered, oldestFirst]);

  const heatDays = useMemo(() => {
    const byDay = new Map<string, number>();
    memories.forEach((memory) => {
      const key = dayKey(memory.createdAt);
      byDay.set(key, (byDay.get(key) ?? 0) + 1);
    });
    return Array.from({ length: 90 }, (_, offset) => {
      const date = dayAtOffset(89 - offset);
      const key = localKey(date);
      return { key, iso: date.toISOString(), count: byDay.get(key) ?? 0 };
    });
  }, [memories]);

  const open = useCallback((memory: Memory) => {
    selectMemory(memory);
    setActiveView('memory');
  }, [selectMemory, setActiveView]);

  const selectHeatDay = useCallback((key: string) => {
    setSelectedDay(key);
    window.setTimeout(() => {
      document.getElementById(`st-day-${key}`)?.scrollIntoView({ behavior: 'smooth', block: 'start' });
    }, 0);
  }, []);

  const hero = (
    <PageHero
      kicker={t('tl.hero.kicker')}
      title={t('timeline.title')}
      copy={t('tl.hero.sub')}
      accent="var(--cyan)"
      secondary="var(--tangerine)"
      stats={[
        { label: t('tl.stats.entries'), value: num(memories.length, locale) },
        { label: t('tl.stats.days'), value: num(grouped.length, locale), color: 'var(--cyan)' },
      ]}
    />
  );

  if (isLoading) return <div className="st-page">{hero}<StrataSkeletons /></div>;
  if (error) return <div className="st-page">{hero}<StrataAlert icon={Clock3}>{error}</StrataAlert></div>;
  if (memories.length === 0) return <div className="st-page">{hero}<StrataVoid icon={History} title={t('timeline.empty')}>{t('tl.empty.desc')}</StrataVoid></div>;

  return (
    <div className="st-page" style={{ '--st-accent': 'var(--cyan)' } as CSSProperties}>
      {hero}

      <section className="st-heat-panel">
        <div>
          <div style={{ display: 'flex', alignItems: 'center', gap: 7 }}>
            <h2 className="st-section-title" style={{ '--section-color': 'var(--cyan)' } as CSSProperties}>
              <CalendarDays size={15} /> {t('tl.heat.title')}
            </h2>
            <InfoTip text={t('tl.heat.hint')} />
          </div>
          <p className="st-section-hint">{t('tl.heat.hint')}</p>
        </div>
        <HeatMap days={heatDays} selected={selectedDay} onSelect={selectHeatDay} />
      </section>

      <div className="st-rail">
        <div className="st-chips" role="group" aria-label="Layer filters">
          {LAYER_ORDER.map((layer) => {
            const visual = layerVisual(layer);
            return (
              <button
                type="button"
                key={layer}
                className="st-chip"
                style={{ '--chip-color': visual.color } as CSSProperties}
                aria-pressed={active.has(layer)}
                disabled={!counts.get(layer)}
                onClick={() => setActive((previous) => {
                  const next = new Set(previous);
                  if (next.has(layer)) next.delete(layer);
                  else next.add(layer);
                  return next;
                })}
              >
                <span className="st-chip-dot" /> {layer}
                <span className="st-chip-count">{counts.get(layer) ?? 0}</span>
              </button>
            );
          })}
        </div>
        <span className="st-rail-spacer" />
        <div className="st-seg" role="group" aria-label={t('tl.order')}>
          <button type="button" aria-pressed={!oldestFirst} onClick={() => setOldestFirst(false)}>{t('tl.order.newest')}</button>
          <button type="button" aria-pressed={oldestFirst} onClick={() => setOldestFirst(true)}>{t('tl.order.oldest')}</button>
        </div>
        <span className="st-rail-count">{filtered.length}/{memories.length}</span>
      </div>

      <div style={{ display: 'flex', alignItems: 'center', gap: 7, margin: '0 0 10px 3px' }}>
        <Clock3 size={13} style={{ color: 'var(--cyan)' }} />
        <span className="st-section-hint" style={{ margin: 0 }}>{t('tl.axis.hint')}</span>
        <InfoTip text={t('tl.axis.hint')} />
      </div>

      {grouped.length === 0 ? (
        <StrataVoid icon={Clock3} title={t('tl.none.title')}>{t('tl.none.desc')}</StrataVoid>
      ) : (
        <div className="st-timeline-days">
          {grouped.map((day, dayIndex) => {
            const tag = dayTag(day.iso, locale);
            return (
              <section
                key={day.key}
                id={`st-day-${day.key}`}
                className="st-day st-rise"
                style={{ '--st-i': dayIndex } as CSSProperties}
                data-selected={selectedDay === day.key ? 'true' : undefined}
              >
                <header className="st-day-head">
                  <CalendarDays size={14} style={{ color: 'var(--cyan)' }} />
                  <span className="st-day-date">{dayLabel(day.iso, locale)}</span>
                  {tag && <span className="st-day-relative">{tag}</span>}
                  <span className="st-day-count">{day.memories.length} {day.memories.length === 1 ? t('tl.day.one') : t('tl.day.many')}</span>
                </header>

                <div className="st-axis" aria-label={t('tl.axis.hint')}>
                  <div className="st-axis-line" />
                  {[0, 6, 12, 18, 24].map((hour) => (
                    <span key={hour} className="st-axis-tick" style={{ left: `${hour / 24 * 100}%` }}>
                      <span className="st-axis-label">{String(hour).padStart(2, '0')}</span>
                    </span>
                  ))}
                  {clusterDots(day.memories).map((cluster) => (
                    <AxisDot
                      key={cluster.memories[0].id}
                      cluster={cluster}
                      onOpen={open}
                      locale={locale}
                      clusterLabel={t('tl.cluster')}
                    />
                  ))}
                </div>

                <div className="st-day-entries">
                  {day.memories.map((memory) => (
                    <button
                      type="button"
                      key={memory.id}
                      className="st-entry"
                      style={{ ...layerVars(memory.layer), '--row-color': layerVisual(memory.layer).color } as CSSProperties}
                      onClick={() => open(memory)}
                    >
                      <span className="st-entry-time">{clock(memory.createdAt, locale)}</span>
                      <span style={{ minWidth: 0 }}>
                        <span className="st-entry-title">{memory.title}</span>
                        <span style={{ display: 'flex', alignItems: 'center', gap: 7, marginTop: 5 }}>
                          <SemanticLayerTag layer={memory.layer} />
                          <FreshPulse state={freshness(memory.createdAt)} label={t('tl.captured')} />
                        </span>
                      </span>
                      <span className="st-entry-impact">
                        <ImpactBlocks value={memory.importanceScore} label={t('inst.impact')} />
                        {memory.attachedFiles?.length ? <Paperclip size={9} /> : null}
                      </span>
                      <span className="st-sr">{memory.summary || memory.content}</span>
                    </button>
                  ))}
                </div>
              </section>
            );
          })}
        </div>
      )}
    </div>
  );
}
