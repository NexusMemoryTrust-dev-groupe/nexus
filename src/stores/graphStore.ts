import { create } from 'zustand';
import { invoke } from '@tauri-apps/api/core';
import type { GraphNode, GraphEdge } from '../types';

interface GraphState {
  nodes: GraphNode[];
  edges: GraphEdge[];
  selectedNode: GraphNode | null;
  isLoading: boolean;
  error: string | null;
  /** One-shot focus request: entity ids the graph should highlight and select
   *  when it mounts/is visible (set from ContextView's "show in graph"). The
   *  view consumes it and clears it, so it never re-applies on unrelated
   *  re-renders. */
  focusRequest: string[] | null;
  fetchGraph: () => Promise<void>;
  selectNode: (node: GraphNode | null) => void;
  requestFocus: (entityIds: string[]) => void;
  clearFocus: () => void;
}

export const useGraphStore = create<GraphState>((set) => ({
  nodes: [],
  edges: [],
  selectedNode: null,
  isLoading: false,
  error: null,
  focusRequest: null,
  fetchGraph: async () => {
    set({ isLoading: true, error: null });
    try {
      const { nodes, edges } = await invoke<{ nodes: GraphNode[]; edges: GraphEdge[] }>('get_graph');
      set({ nodes, edges, isLoading: false });
    } catch (error) {
      set({ error: String(error), isLoading: false });
    }
  },
  selectNode: (node) => set({ selectedNode: node }),
  requestFocus: (entityIds) => set({ focusRequest: entityIds }),
  clearFocus: () => set({ focusRequest: null }),
}));
