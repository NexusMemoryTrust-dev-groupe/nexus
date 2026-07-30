import { create } from 'zustand';
import { invoke } from '@tauri-apps/api/core';
import type { ContextPackage, GraphNode, GraphEdge, Memory } from '../types';

interface ContextState {
  context: ContextPackage | null;
  isLoading: boolean;
  error: string | null;
  buildContext: (query: string) => Promise<void>;
  clearContext: () => void;
}

export const useContextStore = create<ContextState>((set) => ({
  context: null,
  isLoading: false,
  error: null,
  buildContext: async (query: string) => {
    set({ isLoading: true, error: null });
    try {
      const result = await invoke<{
        id: string;
        entities: GraphNode[];
        relationships: GraphEdge[];
        memory_records: Memory[];
        user_intent: { query: string; intent_type: string; confidence: number };
        created_at: string;
        token_count: number;
      }>('build_context', { query });

      const context: ContextPackage = {
        query: result.user_intent.query,
        intentType: result.user_intent.intent_type,
        confidence: result.user_intent.confidence,
        entities: result.entities,
        memoryRecords: result.memory_records,
        relationships: result.relationships,
        tokenCount: result.token_count,
      };

      set({ context, isLoading: false });
    } catch (error) {
      set({ error: String(error), isLoading: false });
    }
  },
  clearContext: () => set({ context: null, error: null }),
}));
