import { useMemo, useState } from 'react';
import type { CSSProperties } from 'react';
import {
  AlertTriangle, ArrowDownAZ, ArrowDownWideNarrow, Brain,
  Grid3X3, List, Search, ShieldCheck, SlidersHorizontal, X,
} from 'lucide-react';
import { useMemoryStore } from '../../stores/memoryStore';
import { useLocale } from '../../stores/localeStore';
import { LAYER_ORDER, layerVisual } from '../../lib/layers';
import { pct } from '../../lib/format';
import { MemoryCard, type MemoryLayout } from './MemoryCard';
import { MemoryDetail } from './MemoryDetail';
import {
  InfoTip, LayerLegend, PageHero, StrataAlert, StrataBar,
  StrataSkeletons, StrataVoid,
} from '../ui/Instruments';

type Sort = 'recent' | 'importance' | 'confidence' | 'title';

const SORTS: { id: Sort; labelKey: string; icon: typeof ArrowDownAZ }[] = [
  { id: 'recent', labelKey: 'mem.sort.recent', icon: ArrowDownWideNarrow },
  { id: 'importance', labelKey: 'mem.sort.impact', icon: SlidersHorizontal },
  { id: 'confidence', labelKey: 'mem.sort.trust', icon: ShieldCheck },
  { id: 'title', labelKey: 'mem.sort.title', icon: ArrowDownAZ },
];

/**
 * Memories is a collection map, not a folder of identical cards.
 *
 * The distribution bar tells users what the six colours mean, the legend keeps
 * the definitions visible, and the bento wall encodes impact spatially. Search,
 * sort and filters remain client-side because the complete memory set is already
 * in Zustand and the interaction should feel instant.
 */
export function MemoryExplorer() {
  const { memories, selectedMemory, isLoading, error } = useMemoryStore();
  const { t } = useLocale();
  const [query, setQuery] = useState('');
  const [active, setActive] = useState<Set<string>>(new Set());
  const [sort, setSort] = useState<Sort>('recent');
  const [layout, setLayout] = useState<MemoryLayout>('bento');

  const counts = useMemo(() => {
    const map = new Map<string, number>();
    memories.forEach((memory) => {
      const layer = layerVisual(memory.layer).name;
      map.set(layer, (map.get(layer) ?? 0) + 1);
    });
    return map;
  }, [memories]);

  const filtered = useMemo(() => {
    const needle = query.trim().toLowerCase();
    const result = memories.filter((memory) => {
      if (active.size > 0 && !active.has(layerVisual(memory.layer).name)) return false;
      if (!needle) return true;
      return [memory.title, memory.summary, memory.content, memory.source]
        .some((value) => value.toLowerCase().includes(needle));
    });

    result.sort((a, b) => {
      switch (sort) {
        case 'importance': return b.importanceScore - a.importanceScore;
        case 'confidence': return b.confidenceScore - a.confidenceScore;
        case 'title': return a.title.localeCompare(b.title);
        default: return new Date(b.createdAt).getTime() - new Date(a.createdAt).getTime();
      }
    });
    return result;
  }, [active, memories, query, sort]);

  const averageImpact = useMemo(() => {
    if (memories.length === 0) return 0;
    return pct(memories.reduce((sum, memory) => sum + memory.importanceScore, 0) / memories.length);
  }, [memories]);

  if (selectedMemory) return <MemoryDetail />;

  const toggleLayer = (layer: string) => {
    setActive((previous) => {
      const next = new Set(previous);
      if (next.has(layer)) next.delete(layer);
      else next.add(layer);
      return next;
    });
  };

  const hero = (
    <PageHero
      kicker={t('mem.hero.kicker')}
      title={t('memory.title')}
      copy={t('mem.hero.sub')}
      accent="var(--tangerine)"
      secondary="var(--periwinkle)"
      stats={[
        { label: t('mem.stats.records'), value: String(memories.length) },
        { label: t('mem.stats.avgImpact'), value: `${averageImpact}%`, color: 'var(--tangerine)' },
      ]}
    />
  );

  if (isLoading) return <div className="st-page">{hero}<StrataSkeletons /></div>;

  if (error) {
    return (
      <div className="st-page">
        {hero}
        <StrataAlert icon={AlertTriangle}>{error}</StrataAlert>
      </div>
    );
  }

  if (memories.length === 0) {
    return (
      <div className="st-page" style={{ '--st-accent': 'var(--tangerine)' } as CSSProperties}>
        {hero}
        <StrataVoid icon={Brain} title={t('memory.empty')}>
          {t('mem.empty.desc')}
        </StrataVoid>
      </div>
    );
  }

  return (
    <div className="st-page" style={{ '--st-accent': 'var(--tangerine)' } as CSSProperties}>
      {hero}

      <section className="st-strata-panel">
        <div className="st-strata-copy">
          <div style={{ display: 'flex', alignItems: 'center', gap: 7 }}>
            <h2 className="st-section-title" style={{ '--section-color': 'var(--tangerine)' } as CSSProperties}>
              <Brain size={15} /> {t('mem.strata.title')}
            </h2>
            <InfoTip text={t('mem.strata.hint')} />
          </div>
          <p className="st-section-hint">{t('layer.ladder.hint')}</p>
        </div>
        <StrataBar counts={counts} total={memories.length} active={active} onToggle={toggleLayer} />
      </section>

      <section style={{ marginBottom: 14 }}>
        <div style={{ display: 'flex', alignItems: 'center', gap: 8, marginBottom: 9 }}>
          <h2 className="st-section-title">{t('mem.legend.title')}</h2>
          <InfoTip text={t('layer.ladder.hint')} />
        </div>
        <LayerLegend />
      </section>

      <div className="st-rail">
        <label className="st-search">
          <Search size={13} />
          <input
            type="search"
            value={query}
            onChange={(event) => setQuery(event.target.value)}
            placeholder={t('memory.search')}
            aria-label={t('memory.search')}
          />
          {query && (
            <button type="button" className="st-icon-button" onClick={() => setQuery('')} aria-label="Clear search">
              <X size={11} />
            </button>
          )}
        </label>

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
                onClick={() => toggleLayer(layer)}
              >
                <span className="st-chip-dot" /> {layer}
                <span className="st-chip-count">{counts.get(layer) ?? 0}</span>
              </button>
            );
          })}
        </div>

        <span className="st-rail-spacer" />

        <div className="st-seg" role="group" aria-label={t('mem.sort')}>
          {SORTS.map(({ id, labelKey, icon: Icon }) => (
            <button key={id} type="button" aria-pressed={sort === id} onClick={() => setSort(id)}>
              <Icon size={10} /> {t(labelKey)}
            </button>
          ))}
        </div>

        <div className="st-seg" role="group" aria-label={t('mem.view')}>
          <button type="button" aria-pressed={layout === 'bento'} onClick={() => setLayout('bento')} title={t('mem.view.hint')}>
            <Grid3X3 size={11} />
          </button>
          <button type="button" aria-pressed={layout === 'list'} onClick={() => setLayout('list')} title={t('mem.view.hint')}>
            <List size={11} />
          </button>
        </div>

        <span className="st-rail-count">{filtered.length}/{memories.length}</span>
      </div>

      {filtered.length === 0 ? (
        <StrataVoid icon={Search} title={t('mem.none.title')}>
          {t('mem.none.desc')}
          <button type="button" className="st-expand" style={{ marginTop: 14 }} onClick={() => { setQuery(''); setActive(new Set()); }}>
            {t('mem.clear')}
          </button>
        </StrataVoid>
      ) : (
        <div className={`st-bento${layout === 'list' ? ' st-bento--list' : ''}`}>
          {filtered.map((memory, index) => (
            <MemoryCard key={memory.id} memory={memory} layout={layout} index={index} />
          ))}
        </div>
      )}
    </div>
  );
}
