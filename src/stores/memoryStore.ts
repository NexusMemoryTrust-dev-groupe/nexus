import { create } from 'zustand';
import type { StoreApi } from 'zustand';
import { invoke } from '@tauri-apps/api/core';
import type { LayerStat, Memory } from '../types';

interface MemoryState {
  memories: Memory[];
  selectedMemory: Memory | null;
  layerStats: LayerStat[];
  isLoading: boolean;
  error: string | null;
  fetchMemories: () => Promise<void>;
  selectMemory: (memory: Memory | null) => void;
  memorySetState: (id: string, state: string) => Promise<void>;
  memoryConfirm: (id: string) => Promise<void>;
  memoryFeedback: (id: string, kind: string, note?: string) => Promise<void>;
  refreshSelected: () => Promise<void>;
  /** Override the cognitive layer; pins it against future auto-reclassification. */
  setMemoryLayer: (id: string, layer: string, reason?: string) => Promise<void>;
  /** Re-run the classifier on a memory; replaces classifier-owned assignments. */
  reclassifyMemory: (id: string) => Promise<void>;
  /** Refresh the aggregate layer distribution. */
  fetchLayerStats: () => Promise<void>;
}

type MemorySetState = StoreApi<MemoryState>['setState'];

function replaceSelected(set: MemorySetState, updated: Memory) {
  set((state) => ({
    memories: state.memories.map((m) => (m.id === updated.id ? updated : m)),
    selectedMemory:
      state.selectedMemory && state.selectedMemory.id === updated.id
        ? updated
        : state.selectedMemory,
  }));
}

export const useMemoryStore = create<MemoryState>((set, get) => ({
  memories: [],
  selectedMemory: null,
  layerStats: [],
  isLoading: false,
  error: null,
  fetchMemories: async () => {
    set({ isLoading: true, error: null });
    try {
      const memories = await invoke<Memory[]>('get_memories');
      set({ memories, isLoading: false });
    } catch (error) {
      set({ error: String(error), isLoading: false });
    }
  },
  selectMemory: (memory) => set({ selectedMemory: memory }),
  memorySetState: async (id, state) => {
    try {
      const updated = await invoke<Memory>('memory_set_state', { id, state });
      replaceSelected(set, updated);
    } catch (error) {
      set({ error: String(error) });
    }
  },
  memoryConfirm: async (id) => {
    try {
      const updated = await invoke<Memory>('memory_confirm', { id, by: null });
      replaceSelected(set, updated);
    } catch (error) {
      set({ error: String(error) });
    }
  },
  memoryFeedback: async (id, kind, note) => {
    try {
      const updated = await invoke<Memory>('memory_feedback', {
        id,
        kind,
        note: note ?? null,
      });
      replaceSelected(set, updated);
    } catch (error) {
      set({ error: String(error) });
    }
  },
  refreshSelected: async () => {
    const current = get().selectedMemory;
    if (!current) return;
    try {
      const updated = await invoke<Memory | null>('get_memory', { id: current.id });
      if (updated) replaceSelected(set, updated);
    } catch {
      // Keep the current copy if the refresh fails; the detail view stays usable.
    }
  },
  setMemoryLayer: async (id, layer, reason) => {
    try {
      const updated = await invoke<Memory>('set_memory_layer', {
        id,
        layer,
        reason: reason ?? null,
      });
      replaceSelected(set, updated);
    } catch (error) {
      set({ error: String(error) });
    }
  },
  reclassifyMemory: async (id) => {
    try {
      const updated = await invoke<Memory>('reclassify_memory', { id });
      replaceSelected(set, updated);
    } catch (error) {
      set({ error: String(error) });
    }
  },
  fetchLayerStats: async () => {
    try {
      const stats = await invoke<LayerStat[]>('get_layer_stats');
      set({ layerStats: stats });
    } catch {
      // Stats are a summary surface; leave the last-known values on failure.
    }
  },
}));
