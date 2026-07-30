import { useMemoryStore } from '../../stores/memoryStore';
import { useLocale } from '../../stores/localeStore';
import { MemoryCard } from './MemoryCard';
import { MemoryDetail } from './MemoryDetail';
import { Brain } from 'lucide-react';

export function MemoryExplorer() {
  const { memories, selectedMemory, isLoading, error } = useMemoryStore();
  const { t } = useLocale();

  if (selectedMemory) {
    return <MemoryDetail />;
  }

  if (isLoading) {
    return (
      <div className="empty-state">
        <div className="empty-state-title">Loading memories...</div>
      </div>
    );
  }

  if (error) {
    return (
      <div className="empty-state">
        <div className="empty-state-title" style={{ color: 'var(--rose)' }}>Error</div>
        <div className="empty-state-desc">{error}</div>
      </div>
    );
  }

  if (memories.length === 0) {
    return (
      <div className="empty-state">
        <Brain size={48} className="empty-state-icon" />
        <div className="empty-state-title">{t('memory.empty')}</div>
        <div className="empty-state-desc">Create your first memory to get started.</div>
      </div>
    );
  }

  return (
    <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fill, minmax(320px, 1fr))', gap: '12px' }}>
      {memories.map((memory) => (
        <MemoryCard key={memory.id} memory={memory} />
      ))}
    </div>
  );
}
