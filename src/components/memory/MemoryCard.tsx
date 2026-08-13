import { useCallback } from 'react';
import type { CSSProperties } from 'react';
import { ArrowUpRight, Paperclip } from 'lucide-react';
import type { Memory } from '../../types';
import { useMemoryStore } from '../../stores/memoryStore';
import { useLocale } from '../../stores/localeStore';
import { ago, freshness } from '../../lib/format';
import { layerVars } from '../../lib/layers';
import {
  FreshPulse,
  ImpactBlocks,
  LayerGlyph,
  SemanticLayerTag,
  SignalRing,
} from '../ui/Instruments';

export type MemoryLayout = 'bento' | 'list';

interface MemoryCardProps {
  memory: Memory;
  layout?: MemoryLayout;
  index?: number;
}

export function MemoryCard({ memory, layout = 'bento', index = 0 }: MemoryCardProps) {
  const { selectMemory } = useMemoryStore();
  const { locale, t } = useLocale();
  const fresh = freshness(memory.createdAt);
  const files = memory.attachedFiles?.length ?? 0;

  // Cursor wash stays outside React state: this is visual feedback at pointer
  // frequency, not application data. Re-rendering a tile on every mouse move
  // would turn the effect itself into the performance problem.
  const onMove = useCallback((event: React.MouseEvent<HTMLButtonElement>) => {
    const rect = event.currentTarget.getBoundingClientRect();
    event.currentTarget.style.setProperty('--mx', `${((event.clientX - rect.left) / rect.width) * 100}%`);
    event.currentTarget.style.setProperty('--my', `${((event.clientY - rect.top) / rect.height) * 100}%`);
  }, []);

  const freshLabel = fresh === 'fresh'
    ? t('mem.fresh')
    : fresh === 'recent'
      ? t('mem.recent')
      : t('mem.settled');

  return (
    <button
      type="button"
      className={`st-memory-tile st-rise${layout === 'list' ? ' st-memory-row' : ''}`}
      data-size="medium"
      style={{ ...layerVars(memory.layer), '--st-i': index } as CSSProperties}
      onClick={() => selectMemory(memory)}
      onMouseMove={onMove}
      aria-label={`${memory.title}, ${memory.layer}`}
    >
      <span className="st-tile-sheen" aria-hidden="true" />

      <div className="st-tile-top">
        <LayerGlyph layer={memory.layer} size={layout === 'list' ? 30 : 34} />
        <SemanticLayerTag layer={memory.layer} />
        <span className="st-tile-fresh">
          <FreshPulse
            state={fresh}
            label={freshLabel}
            hint={fresh === 'fresh' ? t('mem.pulse.hint') : undefined}
          />
        </span>
      </div>

      <div className="st-tile-body">
        <h3 className="st-tile-title">{memory.title}</h3>
        <p className="st-tile-summary">{memory.summary || memory.content}</p>
      </div>

      <div className="st-tile-bottom">
        <div className="st-tile-signals">
          <SignalRing
            value={memory.confidenceScore}
            label={t('inst.trust')}
            size={layout === 'list' ? 43 : 54}
          />
          <div className="st-tile-impact">
            <span className="st-tile-signal-label">{t('inst.impact')}</span>
            <ImpactBlocks value={memory.importanceScore} label={t('inst.impact')} />
          </div>
        </div>

        <div className="st-tile-meta">
          {files > 0 && (
            <span style={{ display: 'inline-flex', alignItems: 'center', gap: 4 }}>
              <Paperclip size={9} /> {files}
            </span>
          )}
          <div className="st-tile-source">{memory.source}</div>
          <span className="st-tile-time">{ago(memory.createdAt, locale)}</span>
          <span className="st-tile-open">
            {t('mem.open')} <ArrowUpRight size={10} />
          </span>
        </div>
      </div>
    </button>
  );
}
