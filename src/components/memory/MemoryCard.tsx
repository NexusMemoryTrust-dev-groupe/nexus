import { useCallback } from 'react';
import type { Memory } from '../../types';
import { useMemoryStore } from '../../stores/memoryStore';
import { FileText } from 'lucide-react';

const layerConfig: Record<string, { className: string; glow: string }> = {
  Raw:      { className: 'badge blue',     glow: 'rgba(120, 169, 255, 0.3)' },
  Knowledge:{ className: 'badge cyan',     glow: 'rgba(99, 216, 210, 0.3)' },
  Decision: { className: 'badge periwinkle',glow: 'rgba(169, 156, 248, 0.3)' },
  Wisdom:   { className: 'badge gold',     glow: 'rgba(221, 187, 101, 0.3)' },
};

interface MemoryCardProps {
  memory: Memory;
}

export function MemoryCard({ memory }: MemoryCardProps) {
  const { selectMemory } = useMemoryStore();
  const layer = layerConfig[memory.layer] || layerConfig.Raw;

  // Track mouse position for radial glow effect
  const handleMouseMove = useCallback((e: React.MouseEvent<HTMLDivElement>) => {
    const rect = e.currentTarget.getBoundingClientRect();
    const x = ((e.clientX - rect.left) / rect.width) * 100;
    const y = ((e.clientY - rect.top) / rect.height) * 100;
    e.currentTarget.style.setProperty('--mouse-x', `${x}%`);
    e.currentTarget.style.setProperty('--mouse-y', `${y}%`);
  }, []);

  return (
    <div
      className="memory-card"
      onClick={() => selectMemory(memory)}
      onMouseMove={handleMouseMove}
    >
      {/* Layer badge with glow */}
      <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', marginBottom: '10px' }}>
        <span
          className={layer.className}
          style={{ boxShadow: `0 0 12px ${layer.glow}` }}
        >
          {memory.layer}
        </span>
        {memory.attachedFiles && memory.attachedFiles.length > 0 && (
          <span style={{
            display: 'flex', alignItems: 'center', gap: '4px',
            fontSize: 'var(--text-2xs)', color: 'var(--muted-2)',
          }}>
            <FileText size={11} />
            {memory.attachedFiles.length}
          </span>
        )}
      </div>

      {/* Title */}
      <div className="memory-card-title">{memory.title}</div>

      {/* Summary */}
      {memory.summary && (
        <div className="memory-card-summary">{memory.summary}</div>
      )}

      {/* Source */}
      <div className="memory-card-meta">
        <span style={{ fontSize: 'var(--text-2xs)', color: 'var(--muted-2)' }}>
          {memory.source}
        </span>
      </div>

      {/* Score bars */}
      <div className="memory-card-scores">
        <div className="memory-card-score">
          <span>Confidence</span>
          <div className="memory-card-score-bar">
            <div
              className="memory-card-score-fill confidence"
              style={{ width: `${memory.confidenceScore * 100}%` }}
            />
          </div>
          <span>{(memory.confidenceScore * 100).toFixed(0)}%</span>
        </div>
        <div className="memory-card-score">
          <span>Importance</span>
          <div className="memory-card-score-bar">
            <div
              className="memory-card-score-fill importance"
              style={{ width: `${memory.importanceScore * 100}%` }}
            />
          </div>
          <span>{(memory.importanceScore * 100).toFixed(0)}%</span>
        </div>
      </div>
    </div>
  );
}
