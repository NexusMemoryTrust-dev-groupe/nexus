import { Brain, Network, Clock, Settings, PanelLeftClose, PanelLeft, Command, FolderOpen, TrendingDown, Layers, Radar, Users, Scale, Zap, Plane, BookOpenCheck, ShieldAlert, FlaskConical, CreditCard, Wrench, TrendingUp, Map as MapIcon, HeartPulse } from 'lucide-react';
import { useUiStore } from '../../stores/uiStore';
import { useLocale } from '../../stores/localeStore';
import { NexusLogo } from './NexusLogo';
import type { ActiveView } from '../../types';

export function Sidebar() {
  const { sidebarOpen, toggleSidebar, activeView, setActiveView, toggleCommandBar } = useUiStore();
  const { t } = useLocale();

  const navItems: { icon: typeof Brain; labelKey: string; view: ActiveView; shortcut: string }[] = [
    { icon: Brain, labelKey: 'sidebar.memories', view: 'memory', shortcut: 'Ctrl+1' },
    { icon: Network, labelKey: 'sidebar.graph', view: 'graph', shortcut: 'Ctrl+2' },
    { icon: Clock, labelKey: 'sidebar.timeline', view: 'timeline', shortcut: 'Ctrl+3' },
    { icon: Layers, labelKey: 'sidebar.context', view: 'context', shortcut: 'Ctrl+6' },
    { icon: Radar, labelKey: 'sidebar.radar', view: 'radar', shortcut: 'Ctrl+7' },
    { icon: Users, labelKey: 'sidebar.team', view: 'team', shortcut: 'Ctrl+8' },
    { icon: Scale, labelKey: 'sidebar.audit', view: 'audit', shortcut: 'Ctrl+9' },
    { icon: Zap, labelKey: 'sidebar.conflict', view: 'conflict', shortcut: 'Ctrl+0' },
    { icon: Plane, labelKey: 'sidebar.flight', view: 'flight', shortcut: '' },
    { icon: FolderOpen, labelKey: 'sidebar.projects', view: 'projects', shortcut: 'Ctrl+4' },
  ];

  // Systems 2-4, 6-9: rehearsal/consolidation, firewall, context lab,
  // passports, skills and predictive context. Kept compact — these views open
  // a specific system rather than a primary workspace.
  const systemItems: { icon: typeof Brain; labelKey: string; view: ActiveView }[] = [
    { icon: BookOpenCheck, labelKey: 'sidebar.rehearsal', view: 'rehearsal' },
    { icon: ShieldAlert, labelKey: 'sidebar.firewall', view: 'firewall' },
    { icon: FlaskConical, labelKey: 'sidebar.contextlab', view: 'contextlab' },
    { icon: CreditCard, labelKey: 'sidebar.passport', view: 'passport' },
    { icon: Wrench, labelKey: 'sidebar.skills', view: 'skills' },
    { icon: TrendingUp, labelKey: 'sidebar.predictive', view: 'predictive' },
    { icon: MapIcon, labelKey: 'sidebar.knowledge', view: 'knowledge' },
    { icon: HeartPulse, labelKey: 'sidebar.diagnostics', view: 'diagnostics' },
  ];

  return (
    <aside className="sidebar">
      <div className="sidebar-header">
        {sidebarOpen && (
          <div className="sidebar-brand">
            <div className="sidebar-brand-icon">
              <NexusLogo size={45} />
            </div>
            <div className="sidebar-brand-text-block">
              <span className="nexus-brand-wordmark">NEXUS</span>
              <span className="nexus-brand-line" />
              <div className="nexus-brand-subtitle">
                <span className="nexus-sub-dash" />
                <span className="nexus-sub-text">Cognitive Core</span>
                <span className="nexus-sub-dash" />
              </div>
            </div>
          </div>
        )}
        <button className="btn-icon" onClick={toggleSidebar}>
          {sidebarOpen ? <PanelLeftClose size={18} /> : <PanelLeft size={18} />}
        </button>
      </div>

      {sidebarOpen && (
        <>
          {/* Command palette trigger */}
          <button className="sidebar-command-trigger" onClick={toggleCommandBar}>
            <Command size={14} />
            <span>{t('sidebar.commands')}</span>
            <kbd>Ctrl+K</kbd>
          </button>

          <nav className="sidebar-nav">
            <div className="sidebar-section-label">Navigation</div>
            <div className="sidebar-nav-list">
              {navItems.map(({ icon: Icon, labelKey, view, shortcut }) => (
                <button
                  key={view}
                  onClick={() => setActiveView(view)}
                  className={`sidebar-item ${activeView === view ? 'active' : ''}`}
                >
                  <div className={`sidebar-item-indicator ${activeView === view ? 'active' : ''}`} />
                  <Icon size={16} className="sidebar-item-icon" />
                  <span className="sidebar-item-text">{t(labelKey)}</span>
                  <span className="sidebar-item-shortcut">{shortcut}</span>
                </button>
              ))}
            </div>
            <div className="sidebar-section-label">Systems</div>
            <div className="sidebar-nav-list">
              {systemItems.map(({ icon: Icon, labelKey, view }) => (
                <button
                  key={view}
                  onClick={() => setActiveView(view)}
                  className={`sidebar-item ${activeView === view ? 'active' : ''}`}
                >
                  <div className={`sidebar-item-indicator ${activeView === view ? 'active' : ''}`} />
                  <Icon size={16} className="sidebar-item-icon" />
                  <span className="sidebar-item-text">{t(labelKey)}</span>
                </button>
              ))}
            </div>
          </nav>

          <div className="sidebar-footer">
            <button
              className={`sidebar-item ${activeView === 'savings' ? 'active' : ''}`}
              onClick={() => setActiveView('savings')}
            >
              <div className={`sidebar-item-indicator ${activeView === 'savings' ? 'active' : ''}`} />
              <TrendingDown size={16} className="sidebar-item-icon" />
              <span className="sidebar-item-text">{t('savings.title')}</span>
              <span className="sidebar-item-shortcut">Ctrl+5</span>
            </button>
            <button
              className={`sidebar-item ${activeView === 'settings' ? 'active' : ''}`}
              onClick={() => setActiveView('settings')}
            >
              <div className={`sidebar-item-indicator ${activeView === 'settings' ? 'active' : ''}`} />
              <Settings size={16} className="sidebar-item-icon" />
              <span className="sidebar-item-text">{t('sidebar.settings')}</span>
              <span className="sidebar-item-shortcut">Ctrl+,</span>
            </button>
          </div>
        </>
      )}
    </aside>
  );
}
