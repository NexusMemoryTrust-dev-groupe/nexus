import { useMemoryStore } from '../../stores/memoryStore';
import { useLocale } from '../../stores/localeStore';
import { useUiStore } from '../../stores/uiStore';
import { Clock, ChevronRight, FileText, Eye, User, Shield, BarChart3, Layers } from 'lucide-react';

const layerConfig: Record<string, {
  badgeClass: string;
  color: string;
  glow: string;
  gradient: string;
}> = {
  Raw: {
    badgeClass: 'badge blue',
    color: 'var(--blue)',
    glow: 'rgba(120, 169, 255, 0.4)',
    gradient: 'linear-gradient(135deg, rgba(120, 169, 255, 0.12), transparent)',
  },
  Knowledge: {
    badgeClass: 'badge cyan',
    color: 'var(--cyan)',
    glow: 'rgba(99, 216, 210, 0.4)',
    gradient: 'linear-gradient(135deg, rgba(99, 216, 210, 0.12), transparent)',
  },
  Decision: {
    badgeClass: 'badge periwinkle',
    color: 'var(--periwinkle)',
    glow: 'rgba(169, 156, 248, 0.4)',
    gradient: 'linear-gradient(135deg, rgba(169, 156, 248, 0.12), transparent)',
  },
  Wisdom: {
    badgeClass: 'badge gold',
    color: 'var(--gold)',
    glow: 'rgba(221, 187, 101, 0.4)',
    gradient: 'linear-gradient(135deg, rgba(221, 187, 101, 0.12), transparent)',
  },
};

export function TimelineView() {
  const { memories, isLoading, selectMemory } = useMemoryStore();
  const { setActiveView } = useUiStore();
  const { t } = useLocale();

  if (isLoading) {
    return (
      <div className="empty-state">
        <div className="empty-state-title">{t('timeline.loading')}</div>
      </div>
    );
  }

  if (memories.length === 0) {
    return (
      <div className="empty-state">
        <Clock size={48} className="empty-state-icon" />
        <div className="empty-state-title">{t('timeline.empty')}</div>
        <div className="empty-state-desc">Your memory timeline will appear here.</div>
      </div>
    );
  }

  const sorted = [...memories].sort(
    (a, b) => new Date(b.createdAt).getTime() - new Date(a.createdAt).getTime()
  );

  // Group by date
  const grouped: { date: string; items: typeof sorted }[] = [];
  for (const mem of sorted) {
    const dateStr = new Date(mem.createdAt).toLocaleDateString('en-US', {
      year: 'numeric', month: 'long', day: 'numeric',
    });
    const existing = grouped.find(g => g.date === dateStr);
    if (existing) {
      existing.items.push(mem);
    } else {
      grouped.push({ date: dateStr, items: [mem] });
    }
  }

  const handleCardClick = (memory: typeof memories[0]) => {
    selectMemory(memory);
    setActiveView('memory');
  };

  let globalIdx = 0;

  return (
    <div className="tl-wrapper">
      {/* Header */}
      <div className="tl-header">
        <div className="tl-header-icon">
          <Layers size={20} />
        </div>
        <div>
          <h1 className="tl-header-title">Timeline</h1>
          <p className="tl-header-sub">{memories.length} memories recorded</p>
        </div>
      </div>

      {/* Timeline */}
      <div className="tl-track">
        {/* Animated vertical line */}
        <div className="tl-line">
          <div className="tl-line-glow" />
        </div>

        {grouped.map((group) => (
          <div key={group.date} className="tl-group">
            {/* Date divider */}
            <div className="tl-date">
              <span className="tl-date-dot" />
              <span className="tl-date-text">{group.date}</span>
              <span className="tl-date-line" />
            </div>

            {group.items.map((memory) => {
              const layer = layerConfig[memory.layer] || layerConfig.Raw;
              const idx = globalIdx++;
              const confPct = (memory.confidenceScore * 100).toFixed(0);
              const impPct = (memory.importanceScore * 100).toFixed(0);
              const fileCount = memory.attachedFiles?.length || 0;

              return (
                <div
                  key={memory.id}
                  className="tl-item"
                  style={{ animationDelay: `${0.05 + idx * 0.06}s` }}
                  onClick={() => handleCardClick(memory)}
                >
                  {/* Node */}
                  <div className="tl-node" style={{ '--node-color': layer.color, '--node-glow': layer.glow } as React.CSSProperties}>
                    <div className="tl-node-inner" />
                    <div className="tl-node-ring" />
                  </div>

                  {/* Card */}
                  <div className="tl-card" style={{ '--card-glow': layer.glow, '--card-color': layer.color } as React.CSSProperties}>
                    {/* Left accent bar */}
                    <div className="tl-card-accent" />

                    {/* Ambient glow */}
                    <div className="tl-card-glow" style={{ background: layer.gradient } as React.CSSProperties} />

                    {/* Content */}
                    <div className="tl-card-content">
                      {/* Top row: title + badge */}
                      <div className="tl-card-top">
                        <h3 className="tl-card-title">{memory.title}</h3>
                        <span
                          className={`${layer.badgeClass} tl-card-badge`}
                        >
                          {memory.layer}
                        </span>
                      </div>

                      {/* Summary */}
                      {memory.summary && (
                        <p className="tl-card-summary">{memory.summary}</p>
                      )}

                      {/* Score bars */}
                      <div className="tl-scores">
                        <div className="tl-score">
                          <Shield size={10} style={{ color: 'var(--periwinkle)' }} />
                          <span className="tl-score-label">Confidence</span>
                          <div className="tl-score-track">
                            <div className="tl-score-fill" style={{ width: `${memory.confidenceScore * 100}%`, background: 'var(--periwinkle)' }} />
                          </div>
                          <span className="tl-score-val" style={{ color: 'var(--periwinkle)' }}>{confPct}%</span>
                        </div>
                        <div className="tl-score">
                          <BarChart3 size={10} style={{ color: 'var(--tangerine)' }} />
                          <span className="tl-score-label">Importance</span>
                          <div className="tl-score-track">
                            <div className="tl-score-fill" style={{ width: `${memory.importanceScore * 100}%`, background: 'var(--tangerine)' }} />
                          </div>
                          <span className="tl-score-val" style={{ color: 'var(--tangerine)' }}>{impPct}%</span>
                        </div>
                      </div>

                      {/* Meta */}
                      <div className="tl-card-meta">
                        <span><User size={9} /> {memory.author}</span>
                        <span>{memory.source}</span>
                        {fileCount > 0 && <span><FileText size={9} /> {fileCount}</span>}
                        <span><Eye size={9} /> {memory.visibility}</span>
                        <span className="tl-card-cta">
                          View <ChevronRight size={10} />
                        </span>
                      </div>
                    </div>
                  </div>
                </div>
              );
            })}
          </div>
        ))}
      </div>
    </div>
  );
}
