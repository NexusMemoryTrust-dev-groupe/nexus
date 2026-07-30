import { Search } from 'lucide-react';
import { useUiStore } from '../../stores/uiStore';
import { useLocale } from '../../stores/localeStore';

const viewLabelKeys: Record<string, string> = {
  memory: 'memory.title',
  graph: 'graph.title',
  timeline: 'timeline.title',
  context: 'context.title',
  settings: 'settings.title',
  projects: 'projects.title',
};

export function TopBar() {
  const { mode, toggleCommandBar, activeView } = useUiStore();
  const { t } = useLocale();

  return (
    <header className="topbar">
      <div className="topbar-title">
        <h1>Nexus</h1>
        <span className="view-label">{t(viewLabelKeys[activeView] || 'topbar.explorer')}</span>
      </div>

      <div className="topbar-actions">
        <div className={`mode-indicator`}>
          <span>{mode === 'explorer' ? t('topbar.explorer') : t('topbar.operator')}</span>
        </div>

        <button className="command-trigger" onClick={toggleCommandBar}>
          <Search size={14} />
          <span>{t('sidebar.commands')}</span>
          <kbd>Ctrl+K</kbd>
        </button>
      </div>
    </header>
  );
}
