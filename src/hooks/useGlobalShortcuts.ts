import { useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { useUiStore } from '../stores/uiStore';
import { useMemoryStore } from '../stores/memoryStore';

/**
 * Global keyboard shortcuts for the application.
 * All shortcuts use Ctrl (Windows) — detected via e.ctrlKey.
 *
 * Layout independence: letter shortcuts use e.code (physical key position),
 * not e.key (which changes with OS keyboard layout). On a Russian layout,
 * e.key === 'л' for the K key, so e.key === 'k' would silently fail.
 * e.code === 'KeyK' is layout-independent and works everywhere.
 *
 * Non-letter keys (digits, comma, F5, Escape) remain as e.key comparisons
 * because their e.key value is layout-stable.
 */
export function useGlobalShortcuts() {
  const { toggleCommandBar, setActiveView, commandBarOpen } = useUiStore();
  const { fetchMemories } = useMemoryStore();

  useEffect(() => {
    function handleKeyDown(e: KeyboardEvent) {
      // Don't trigger shortcuts when typing in inputs (unless it's a known combo)
      const target = e.target as HTMLElement;
      const isInput = target.tagName === 'INPUT' || target.tagName === 'TEXTAREA' || target.isContentEditable;

      // Ctrl+K — Command bar (always works, even in inputs)
      if (e.ctrlKey && e.code === 'KeyK') {
        e.preventDefault();
        toggleCommandBar();
        return;
      }

      // Escape — close command bar
      if (e.key === 'Escape' && commandBarOpen) {
        e.preventDefault();
        toggleCommandBar();
        return;
      }

      // Don't process other shortcuts if typing in an input or command bar is open
      if (isInput || commandBarOpen) return;

      // Ctrl+1 — Memories
      if (e.ctrlKey && e.key === '1') {
        e.preventDefault();
        setActiveView('memory');
        return;
      }

      // Ctrl+2 — Graph
      if (e.ctrlKey && e.key === '2') {
        e.preventDefault();
        setActiveView('graph');
        return;
      }

      // Ctrl+3 — Timeline
      if (e.ctrlKey && e.key === '3') {
        e.preventDefault();
        setActiveView('timeline');
        return;
      }

      // Ctrl+4 — Projects
      if (e.ctrlKey && e.key === '4') {
        e.preventDefault();
        setActiveView('projects');
        return;
      }

      // Ctrl+5 — Savings
      if (e.ctrlKey && e.key === '5') {
        e.preventDefault();
        setActiveView('savings');
        return;
      }

      // Ctrl+6 — Context
      if (e.ctrlKey && e.key === '6') {
        e.preventDefault();
        setActiveView('context');
        return;
      }

      // Ctrl+, (comma) — Settings. e.key === ',' is layout-stable.
      if (e.ctrlKey && e.key === ',') {
        e.preventDefault();
        setActiveView('settings');
        return;
      }

      // Ctrl+N — New memory
      if (e.ctrlKey && e.code === 'KeyN') {
        e.preventDefault();
        invoke('create_memory', {
          title: 'New Memory',
          content: 'Created via keyboard shortcut',
          author: 'user',
        }).then(() => {
          fetchMemories();
          setActiveView('memory');
        }).catch(console.error);
        return;
      }

      // Ctrl+R or F5 — Refresh memories
      if ((e.ctrlKey && e.code === 'KeyR') || e.key === 'F5') {
        e.preventDefault();
        fetchMemories();
        return;
      }
    }

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [toggleCommandBar, setActiveView, commandBarOpen, fetchMemories]);
}
