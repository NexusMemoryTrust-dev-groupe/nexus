import { ArrowLeft, Shield, Eye, Clock, User, Layers, BarChart3 } from 'lucide-react';
import { useMemoryStore } from '../../stores/memoryStore';

const layerConfig: Record<string, { className: string; glow: string; color: string }> = {
  Raw:      { className: 'badge blue',      glow: 'rgba(120, 169, 255, 0.3)', color: 'var(--blue)' },
  Knowledge:{ className: 'badge cyan',      glow: 'rgba(99, 216, 210, 0.3)', color: 'var(--cyan)' },
  Decision: { className: 'badge periwinkle', glow: 'rgba(169, 156, 248, 0.3)', color: 'var(--periwinkle)' },
  Wisdom:   { className: 'badge gold',      glow: 'rgba(221, 187, 101, 0.3)', color: 'var(--gold)' },
};

export function MemoryDetail() {
  const { selectedMemory, selectMemory } = useMemoryStore();

  if (!selectedMemory) return null;

  const layer = layerConfig[selectedMemory.layer] || layerConfig.Raw;
  const confPct = (selectedMemory.confidenceScore * 100).toFixed(0);
  const impPct = (selectedMemory.importanceScore * 100).toFixed(0);

  return (
    <div className="memory-detail-panel">
      {/* Back button */}
      <button
        onClick={() => selectMemory(null)}
        className="memory-back-btn"
      >
        <ArrowLeft size={15} />
        <span>Back to memories</span>
      </button>

      {/* Card with animated gradient border */}
      <div className="memory-detail-border">
        <div style={{ position: 'relative', zIndex: 1 }}>
          {/* Header */}
          <div style={{ padding: '20px 24px', borderBottom: '1px solid var(--line)' }}>
            <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', marginBottom: '8px' }}>
              <h2 style={{
                fontFamily: 'var(--brand)', fontSize: '20px', fontWeight: 700,
                color: 'var(--bone)', letterSpacing: '-0.02em', margin: 0,
              }}>
                {selectedMemory.title}
              </h2>
              <span
                className={layer.className}
                style={{ boxShadow: `0 0 12px ${layer.glow}` }}
              >
                <Layers size={10} style={{ marginRight: '3px' }} />
                {selectedMemory.layer}
              </span>
            </div>
          </div>

          {/* Stats bar */}
          <div className="memory-stats-bar">
            <div className="memory-stat">
              <div className="memory-stat-icon" style={{ background: 'var(--periwinkle-soft)', color: 'var(--periwinkle)' }}>
                <Shield size={11} />
              </div>
              <span>Confidence</span>
              <span className="memory-stat-value" style={{ color: 'var(--periwinkle)' }}>{confPct}%</span>
            </div>
            <div className="memory-stat">
              <div className="memory-stat-icon" style={{ background: 'var(--tangerine-soft)', color: 'var(--tangerine)' }}>
                <BarChart3 size={11} />
              </div>
              <span>Importance</span>
              <span className="memory-stat-value" style={{ color: 'var(--tangerine)' }}>{impPct}%</span>
            </div>
            <div className="memory-stat">
              <div className="memory-stat-icon" style={{ background: 'var(--cyan-soft)', color: 'var(--cyan)' }}>
                <Eye size={11} />
              </div>
              <span>{selectedMemory.visibility}</span>
            </div>
            <div className="memory-stat">
              <div className="memory-stat-icon" style={{ background: 'var(--gold-soft)', color: 'var(--gold)' }}>
                <User size={11} />
              </div>
              <span>{selectedMemory.author}</span>
            </div>
          </div>

          {/* Body */}
          <div style={{ padding: '20px 24px' }}>
            {/* Summary */}
            {selectedMemory.summary && (
              <div style={{ marginBottom: '20px' }}>
                <div className="context-label" style={{ marginBottom: '8px' }}>Summary</div>
                <div style={{
                  color: 'var(--muted)', lineHeight: '1.6',
                  padding: '12px 16px', background: 'var(--surface-2)',
                  borderRadius: 'var(--radius-xs)', borderLeft: `3px solid ${layer.color}`,
                }}>
                  {selectedMemory.summary}
                </div>
              </div>
            )}

            {/* Content */}
            <div style={{ marginBottom: '20px' }}>
              <div className="context-label" style={{ marginBottom: '8px' }}>Content</div>
              <div style={{
                color: 'var(--bone)', lineHeight: '1.7', whiteSpace: 'pre-wrap',
                padding: '16px', background: 'var(--carbon)',
                borderRadius: 'var(--radius-xs)', border: '1px solid var(--line)',
                fontFamily: 'var(--mono)', fontSize: '13px',
              }}>
                {selectedMemory.content}
              </div>
            </div>

            {/* Footer meta */}
            <div style={{
              paddingTop: '16px', borderTop: '1px solid var(--line)',
              display: 'flex', gap: '20px', fontSize: '11px', color: 'var(--muted-2)',
              fontFamily: 'var(--mono)',
            }}>
              <span style={{ display: 'flex', alignItems: 'center', gap: '4px' }}>
                <Clock size={11} />
                Created {new Date(selectedMemory.createdAt).toLocaleDateString()}
              </span>
              <span style={{ display: 'flex', alignItems: 'center', gap: '4px' }}>
                <Clock size={11} />
                Updated {new Date(selectedMemory.updatedAt).toLocaleDateString()}
              </span>
              <span style={{ opacity: 0.5 }}>
                {selectedMemory.source}
              </span>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
