import { create } from 'zustand';
import { invoke } from '@tauri-apps/api/core';
import type { GraphNode, GraphEdge } from '../types';

interface GraphState {
  nodes: GraphNode[];
  edges: GraphEdge[];
  selectedNode: GraphNode | null;
  isLoading: boolean;
  error: string | null;
  fetchGraph: () => Promise<void>;
  selectNode: (node: GraphNode | null) => void;
}

export const useGraphStore = create<GraphState>((set) => ({
  nodes: [],
  edges: [],
  selectedNode: null,
  isLoading: false,
  error: null,
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
}));
