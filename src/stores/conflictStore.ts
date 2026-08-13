import { create } from 'zustand';
import { invoke } from '@tauri-apps/api/core';
import type { ConflictGroup, TruthVerdict } from '../types';

interface ConflictState {
  conflicts: ConflictGroup[];
  selectedConflict: ConflictGroup | null;
  verdicts: Record<string, TruthVerdict>;
  isLoading: boolean;
  error: string | null;
  /** Reconcile groups with Conflicted records, then refresh the list. */
  checkConflicts: () => Promise<void>;
  fetchConflicts: (status?: string) => Promise<void>;
  selectConflict: (group: ConflictGroup | null) => void;
  /** Run the Current Truth Engine over one group (read-only). */
  fetchTruth: (groupId: string) => Promise<TruthVerdict | null>;
  /** Settle a conflict: winner becomes Current, losers Superseded. */
  resolveConflict: (
    groupId: string,
    winnerId: string,
    by: string,
    reason?: string,
  ) => Promise<void>;
}

export const useConflictStore = create<ConflictState>((set, get) => ({
  conflicts: [],
  selectedConflict: null,
  verdicts: {},
  isLoading: false,
  error: null,
  checkConflicts: async () => {
    set({ isLoading: true, error: null });
    try {
      await invoke<number>('sync_conflict_groups');
      const conflicts = await invoke<ConflictGroup[]>('get_conflicts', {
        status: null,
      });
      // Precompute the engine's verdict for every open group so the list can
      // show "engine thinks X wins at Y%" without N+1 round trips.
      const verdicts: Record<string, TruthVerdict> = {};
      for (const group of conflicts) {
        if (group.status === 'open') {
          const verdict = await invoke<TruthVerdict | null>('get_conflict_truth', {
            id: group.id,
          });
          if (verdict) verdicts[group.id] = verdict;
        }
      }
      set({ conflicts, verdicts, isLoading: false });
    } catch (error) {
      set({ error: String(error), isLoading: false });
    }
  },
  fetchConflicts: async (status) => {
    set({ isLoading: true, error: null });
    try {
      const conflicts = await invoke<ConflictGroup[]>('get_conflicts', {
        status: status ?? null,
      });
      set({ conflicts, isLoading: false });
    } catch (error) {
      set({ error: String(error), isLoading: false });
    }
  },
  selectConflict: (group) => set({ selectedConflict: group }),
  fetchTruth: async (groupId) => {
    try {
      const verdict = await invoke<TruthVerdict>('get_conflict_truth', {
        id: groupId,
      });
      set((state) => ({ verdicts: { ...state.verdicts, [groupId]: verdict } }));
      return verdict;
    } catch (error) {
      set({ error: String(error) });
      return null;
    }
  },
  resolveConflict: async (groupId, winnerId, by, reason) => {
    try {
      await invoke<ConflictGroup['resolution']>(
        'resolve_conflict',
        {
          id: groupId,
          winnerId,
          by,
          reason: reason ?? null,
        },
      );
      // Refresh the whole list so the resolved group shows its new status and
      // the winners/losers re-render everywhere they appear.
      await get().checkConflicts();
      set({ error: null });
      return;
    } catch (error) {
      set({ error: String(error) });
    }
  },
}));
