import { useUiStore } from '../../stores/uiStore';
import { useMemoryStore } from '../../stores/memoryStore';

export function StatusBar() {
  const { mode } = useUiStore();
  const { memories, isLoading } = useMemoryStore();

  return (
    <footer className="statusbar">
      <div className="statusbar-left">
        <div className="statusbar-item">
          <span className="status-pill online">
            <span className="status-pill-dot" />
            Local
          </span>
        </div>
        <div className="statusbar-item">
          {memories.length} memories
        </div>
      </div>
      <div className="statusbar-right">
        <div className="statusbar-item">
          Mode: {mode}
        </div>
        {isLoading && (
          <div className="statusbar-item" style={{ color: 'var(--tangerine)' }}>
            Loading...
          </div>
        )}
      </div>
    </footer>
  );
}
