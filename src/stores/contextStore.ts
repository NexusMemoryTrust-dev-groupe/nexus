import { create } from 'zustand';
import { invoke } from '@tauri-apps/api/core';
import type {
  ContextPackage,
  GraphNode,
  GraphEdge,
  Memory,
  ContextTrace,
  ContextExport,
} from '../types';

/// Raw shape returned by the `build_context` command.
interface ContextDto {
  id: string;
  entities: GraphNode[];
  relationships: GraphEdge[];
  memory_records: Memory[];
  user_intent: { query: string; intent_type: string; confidence: number };
  created_at: string;
  token_count: number;
  // Serde emits `{ traces: [...] }`. Optional so an older backend does not
  // crash the view - the panel simply stays hidden.
  provenance?: { traces: ContextTrace[] };
}

interface ContextState {
  context: ContextPackage | null;
  isLoading: boolean;
  error: string | null;
  /// Last export produced, kept so the UI can show what was copied.
  lastExport: ContextExport | null;
  isExporting: boolean;
  buildContext: (query: string) => Promise<void>;
  exportContext: (
    format: 'markdown' | 'json' | 'plain',
  ) => Promise<ContextExport | null>;
  clearContext: () => void;
}

export const useContextStore = create<ContextState>((set, get) => ({
  context: null,
  isLoading: false,
  error: null,
  lastExport: null,
  isExporting: false,

  buildContext: async (query: string) => {
    set({ isLoading: true, error: null });
    try {
      const result = await invoke<ContextDto>('build_context', { query });

      const context: ContextPackage = {
        query: result.user_intent.query,
        intentType: result.user_intent.intent_type,
        confidence: result.user_intent.confidence,
        entities: result.entities,
        memoryRecords: result.memory_records,
        relationships: result.relationships,
        tokenCount: result.token_count,
        provenance: result.provenance?.traces ?? [],
      };

      set({ context, isLoading: false, lastExport: null });
    } catch (error) {
      set({ error: String(error), isLoading: false });
    }
  },

  exportContext: async (format) => {
    const { context } = get();
    if (!context) return null;

    set({ isExporting: true, error: null });
    try {
      // Rebuilt on the backend rather than serialised from this store: the
      // store holds a lossy view (no metadata, no scores), and an export that
      // silently omitted fields would be worse than no export at all.
      const result = await invoke<ContextExport>('export_context', {
        query: context.query,
        format,
      });
      set({ lastExport: result, isExporting: false });
      return result;
    } catch (error) {
      set({ error: String(error), isExporting: false });
      return null;
    }
  },

  clearContext: () => set({ context: null, error: null, lastExport: null }),
}));
