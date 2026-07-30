import { create } from 'zustand';
import type { AppMode, ActiveView } from '../types';

interface UiState {
  mode: AppMode;
  sidebarOpen: boolean;
  commandBarOpen: boolean;
  activeView: ActiveView;
  // Copilot panel state
  copilotOpen: boolean;
  copilotX: number;
  copilotY: number;
  toggleMode: () => void;
  toggleSidebar: () => void;
  toggleCommandBar: () => void;
  setActiveView: (view: ActiveView) => void;
  toggleCopilot: () => void;
  setCopilotPosition: (x: number, y: number) => void;
}

export const useUiStore = create<UiState>((set) => ({
  mode: 'explorer',
  sidebarOpen: true,
  commandBarOpen: false,
  activeView: 'memory',
  copilotOpen: false,
  copilotX: -1, // -1 = auto-position (right edge)
  copilotY: -1,
  toggleMode: () => set((state) => ({ mode: state.mode === 'explorer' ? 'operator' : 'explorer' })),
  toggleSidebar: () => set((state) => ({ sidebarOpen: !state.sidebarOpen })),
  toggleCommandBar: () => set((state) => ({ commandBarOpen: !state.commandBarOpen })),
  setActiveView: (view) => set({ activeView: view }),
  toggleCopilot: () => set((state) => ({ copilotOpen: !state.copilotOpen })),
  setCopilotPosition: (x, y) => set({ copilotX: x, copilotY: y }),
}));
