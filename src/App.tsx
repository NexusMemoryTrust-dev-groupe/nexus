import { lazy, Suspense, useEffect, useState } from 'react';
import { Layout } from './components/layout/Layout';
import { CommandBar } from './components/command/CommandBar';
import { FloatingCopilot } from './components/ai/FloatingCopilot';
import { useUiStore } from './stores/uiStore';
import { useMemoryStore } from './stores/memoryStore';
import { useLocale } from './stores/localeStore';
import { useGlobalShortcuts } from './hooks/useGlobalShortcuts';
import { invoke } from '@tauri-apps/api/core';

// The first-run wizard is only ever needed once, so it is split out of the
// main bundle rather than loaded on every launch.
const SetupWizard = lazy(() =>
  import('./components/setup/SetupWizard').then((m) => ({ default: m.SetupWizard }))
);

// Code splitting: lazy load route components
const MemoryExplorer = lazy(() => import('./components/memory/MemoryExplorer').then(m => ({ default: m.MemoryExplorer })));
const GraphView = lazy(() => import('./components/graph/GraphView').then(m => ({ default: m.GraphView })));
const TimelineView = lazy(() => import('./components/timeline/TimelineView').then(m => ({ default: m.TimelineView })));
const ContextView = lazy(() => import('./components/context/ContextView').then(m => ({ default: m.ContextView })));
const SettingsView = lazy(() => import('./components/settings/SettingsView').then(m => ({ default: m.SettingsView })));
const ProjectsView = lazy(() => import('./components/projects/ProjectsView').then(m => ({ default: m.ProjectsView })));
const SavingsView = lazy(() => import('./components/savings/SavingsView').then(m => ({ default: m.SavingsView })));
const RadarView = lazy(() => import('./components/radar/RadarView').then(m => ({ default: m.RadarView })));
const TeamView = lazy(() => import('./components/team/TeamView').then(m => ({ default: m.TeamView })));
const AuditView = lazy(() => import('./components/audit/AuditView').then(m => ({ default: m.AuditView })));
const ConflictsView = lazy(() => import('./components/conflict/ConflictsView').then(m => ({ default: m.ConflictsView })));
const FlightView = lazy(() => import('./components/flight/FlightView').then(m => ({ default: m.FlightView })));
const RehearsalView = lazy(() => import('./components/rehearsal/RehearsalView').then(m => ({ default: m.RehearsalView })));
const FirewallView = lazy(() => import('./components/firewall/FirewallView').then(m => ({ default: m.FirewallView })));
const ContextLabView = lazy(() => import('./components/contextlab/ContextLabView').then(m => ({ default: m.ContextLabView })));
const PassportView = lazy(() => import('./components/passport/PassportView').then(m => ({ default: m.PassportView })));
const SkillsView = lazy(() => import('./components/skills/SkillsView').then(m => ({ default: m.SkillsView })));
const PredictiveView = lazy(() => import('./components/predictive/PredictiveView').then(m => ({ default: m.PredictiveView })));
const KnowledgeMapView = lazy(() => import('./components/knowledge/KnowledgeMapView').then(m => ({ default: m.KnowledgeMapView })));
const DiagnosticsView = lazy(() => import('./components/diagnostics/DiagnosticsView').then(m => ({ default: m.DiagnosticsView })));

function ViewSpinner() {
  return (
    <div style={{ display: 'flex', justifyContent: 'center', alignItems: 'center', height: '100%', opacity: 0.5 }}>
      <div className="loading-spinner" />
    </div>
  );
}

function AppContent() {
  const { activeView } = useUiStore();

  return (
    <Suspense fallback={<ViewSpinner />}>
      {(() => {
        switch (activeView) {
          case 'memory':    return <MemoryExplorer />;
          case 'graph':     return <GraphView />;
          case 'timeline':  return <TimelineView />;
          case 'context':   return <ContextView />;
          case 'settings':  return <SettingsView />;
          case 'projects':  return <ProjectsView />;
          case 'savings':   return <SavingsView />;
          case 'radar':     return <RadarView />;
          case 'team':      return <TeamView />;
          case 'audit':     return <AuditView />;
          case 'conflict':  return <ConflictsView />;
          case 'flight':    return <FlightView />;
          case 'rehearsal': return <RehearsalView />;
          case 'firewall':  return <FirewallView />;
          case 'contextlab': return <ContextLabView />;
          case 'passport':  return <PassportView />;
          case 'skills':    return <SkillsView />;
          case 'predictive': return <PredictiveView />;
          case 'knowledge':  return <KnowledgeMapView />;
          case 'diagnostics': return <DiagnosticsView />;
          default:          return <MemoryExplorer />;
        }
      })()}
    </Suspense>
  );
}

export function App() {
  const { fetchMemories } = useMemoryStore();
  const { setLocale } = useLocale();
  useGlobalShortcuts();

  // `null` = we have not asked the backend yet, so neither the app nor the
  // wizard is rendered. Guessing `false` here would flash the main UI before
  // the wizard on a fresh install.
  const [needsSetup, setNeedsSetup] = useState<boolean | null>(null);

  useEffect(() => {
    let cancelled = false;
    invoke<boolean>('setup_needed')
      .then((needed) => {
        if (!cancelled) setNeedsSetup(needed);
      })
      .catch(() => {
        // If the check itself fails, fall through to the app rather than
        // trapping the user in onboarding they cannot dismiss.
        if (!cancelled) setNeedsSetup(false);
      });
    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => {
    fetchMemories();
  }, [fetchMemories]);

  // Load theme and language from backend on startup
  useEffect(() => {
    async function loadSettings() {
      try {
        const entries = await invoke<Array<{ key: string; value: string }>>('get_all_config');
        const theme = entries.find((e) => e.key === 'app.theme');
        const lang = entries.find((e) => e.key === 'app.language');
        if (theme) {
          document.documentElement.setAttribute('data-theme', theme.value);
        }
        if (lang) {
          setLocale(lang.value as 'en' | 'ru');
        }
      } catch {
        // Config not available yet — use defaults
      }
    }
    loadSettings();
  }, [setLocale]);

  // Hold the shell back until the setup check answers, so a first-run user
  // never sees the empty app behind the wizard.
  if (needsSetup === null) {
    return <ViewSpinner />;
  }

  if (needsSetup) {
    return (
      <Suspense fallback={<ViewSpinner />}>
        <SetupWizard onClose={() => setNeedsSetup(false)} />
      </Suspense>
    );
  }

  return (
    <Layout>
      <div style={{ flex: 1, minHeight: 0 }}>
        <AppContent />
      </div>
      <FloatingCopilot />
      <CommandBar />
    </Layout>
  );
}

export default App;
