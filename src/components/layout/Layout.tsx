import { Sidebar } from './Sidebar';
import { TopBar } from './TopBar';
import { StatusBar } from './StatusBar';
import { useUiStore } from '../../stores/uiStore';

interface LayoutProps {
  children: React.ReactNode;
}

export function Layout({ children }: LayoutProps) {
  const { sidebarOpen } = useUiStore();

  return (
    <div className={`app-shell ${!sidebarOpen ? 'sidebar-collapsed' : ''}`}>
      <Sidebar />
      <div className="workspace-shell">
        <TopBar />
        <main style={{ flex: 1, overflow: 'auto', padding: 'var(--gutter)', display: 'flex', flexDirection: 'column', position: 'relative' }}>
          {children}
        </main>
      </div>
      <StatusBar />
    </div>
  );
}
