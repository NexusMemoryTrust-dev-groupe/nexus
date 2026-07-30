import { create } from 'zustand';
import { invoke } from '@tauri-apps/api/core';
import type { Memory } from '../types';

interface MemoryState {
  memories: Memory[];
  selectedMemory: Memory | null;
  isLoading: boolean;
  error: string | null;
  fetchMemories: () => Promise<void>;
  selectMemory: (memory: Memory | null) => void;
}

export const useMemoryStore = create<MemoryState>((set) => ({
  memories: [],
  selectedMemory: null,
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
}));
