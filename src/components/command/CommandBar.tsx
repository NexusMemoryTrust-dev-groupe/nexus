import { useState, useEffect, useCallback, useRef, useMemo } from 'react';
import { invoke } from '@tauri-apps/api/core';
import {
  Search, Brain, Network, Clock, Settings, Plus, Database,
  GitBranch, ArrowRight, Layers, Sparkles,
  Zap, FolderOpen, TrendingDown,
} from 'lucide-react';
import { useUiStore } from '../../stores/uiStore';
import { useMemoryStore } from '../../stores/memoryStore';
import { useGraphStore } from '../../stores/graphStore';
import { useContextStore } from '../../stores/contextStore';
import { useLocale } from '../../stores/localeStore';
import type { ActiveView } from '../../types';

interface CommandItem {
  id: string;
  label: string;
  description: string;
  icon: typeof Brain;
  category: string;
  shortcut?: string;
  action: () => void | Promise<void>;
}

/** Highlight matching text segments */
function HighlightedText({ text, query }: { text: string; query: string }) {
  if (!query) return <>{text}</>;
  const lower = text.toLowerCase();
  const q = query.toLowerCase();
  const parts: { text: string; match: boolean }[] = [];
  let lastIndex = 0;

  let idx = lower.indexOf(q, lastIndex);
  while (idx !== -1) {
    if (idx > lastIndex) {
      parts.push({ text: text.slice(lastIndex, idx), match: false });
    }
    parts.push({ text: text.slice(idx, idx + q.length), match: true });
    lastIndex = idx + q.length;
    idx = lower.indexOf(q, lastIndex);
  }
  if (lastIndex < text.length) {
    parts.push({ text: text.slice(lastIndex), match: false });
  }

  return (
    <>
      {parts.map((p, i) =>
        p.match ? (
          <span key={i} style={{ color: 'var(--tangerine)', fontWeight: 600 }}>{p.text}</span>
        ) : (
          <span key={i}>{p.text}</span>
        )
      )}
    </>
  );
}

  const categoryColors: Record<string, string> = {
    Navigation: 'var(--periwinkle)',
    Memories: 'var(--tangerine)',
    Graph: 'var(--cyan)',
    Projects: 'var(--mint)',
    Context: 'var(--gold)',
    System: 'var(--steel)',
  };

export function CommandBar() {
  const [query, setQuery] = useState('');
  const [selectedIndex, setSelectedIndex] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);
  const listRef = useRef<HTMLDivElement>(null);
  const justOpenedRef = useRef(false);
  const { commandBarOpen, toggleCommandBar, setActiveView } = useUiStore();
  const { fetchMemories } = useMemoryStore();
  const { fetchGraph } = useGraphStore();
  const { buildContext } = useContextStore();
  const { t } = useLocale();

  const buildCommands = useCallback((): CommandItem[] => [
    {
      id: 'nav:memory',
      label: 'Go to Memories',
      description: 'Browse and manage your memory records',
      icon: Brain,
      category: 'Navigation',
      shortcut: 'Ctrl+1',
      action: () => { setActiveView('memory'); toggleCommandBar(); },
    },
    {
      id: 'nav:graph',
      label: 'Go to Graph',
      description: 'Explore entity relationships',
      icon: Network,
      category: 'Navigation',
      shortcut: 'Ctrl+2',
      action: () => { setActiveView('graph'); toggleCommandBar(); },
    },
    {
      id: 'nav:timeline',
      label: 'Go to Timeline',
      description: 'View memory timeline',
      icon: Clock,
      category: 'Navigation',
      shortcut: 'Ctrl+3',
      action: () => { setActiveView('timeline'); toggleCommandBar(); },
    },
    {
      id: 'nav:context',
      label: 'Go to Context',
      description: 'Build and view context packages',
      icon: Layers,
      category: 'Navigation',
      shortcut: 'Ctrl+6',
      action: () => { setActiveView('context'); toggleCommandBar(); },
    },
    {
      id: 'nav:savings',
      label: 'Go to Savings',
      description: 'View token savings and cost reports',
      icon: TrendingDown,
      category: 'Navigation',
      shortcut: 'Ctrl+5',
      action: () => { setActiveView('savings'); toggleCommandBar(); },
    },
    {
      id: 'nav:projects',
      label: 'Go to Projects',
      description: 'Organize memories and entities by project',
      icon: FolderOpen,
      category: 'Navigation',
      shortcut: 'Ctrl+4',
      action: () => { setActiveView('projects'); toggleCommandBar(); },
    },
    {
      id: 'nav:settings',
      label: 'Go to Settings',
      description: 'Configure your workspace',
      icon: Settings,
      category: 'Navigation',
      shortcut: 'Ctrl+,',
      action: () => { setActiveView('settings' as ActiveView); toggleCommandBar(); },
    },
    {
      id: 'memory:create',
      label: 'Create Memory',
      description: 'Add a new memory record',
      icon: Plus,
      category: 'Memories',
      shortcut: 'Ctrl+N',
      action: async () => {
        try {
          await invoke('create_memory', {
            title: 'New Memory',
            content: 'Created via command bar',
            author: 'user',
          });
          await fetchMemories();
          setActiveView('memory');
          toggleCommandBar();
        } catch (err) {
          console.error('Failed to create memory:', err);
        }
      },
    },
    {
      id: 'memory:refresh',
      label: 'Refresh Memories',
      description: 'Reload all memory records from database',
      icon: Zap,
      category: 'Memories',
      action: async () => {
        await fetchMemories();
        toggleCommandBar();
      },
    },
    {
      id: 'graph:load',
      label: 'Load Graph',
      description: 'Fetch all entities and relationships',
      icon: Network,
      category: 'Graph',
      action: async () => {
        await fetchGraph();
        setActiveView('graph');
        toggleCommandBar();
      },
    },
    {
      id: 'graph:create-entity',
      label: 'Create Entity',
      description: 'Add a new entity to the knowledge graph',
      icon: Plus,
      category: 'Graph',
      action: async () => {
        try {
          await invoke('create_entity', {
            entityType: 'Project',
            title: 'New Entity',
            description: 'Created via command bar',
          });
          await fetchGraph();
          setActiveView('graph');
          toggleCommandBar();
        } catch (err) {
          console.error('Failed to create entity:', err);
        }
      },
    },
    {
      id: 'context:build',
      label: 'Build Context',
      description: 'Build a context package from memories and graph',
      icon: Sparkles,
      category: 'Context',
      action: async () => {
        await buildContext('general');
        setActiveView('context');
        toggleCommandBar();
      },
    },
    {
      id: 'system:stats',
      label: 'View Database Stats',
      description: 'Show database statistics and sizes',
      icon: Database,
      category: 'System',
      action: async () => {
        setActiveView('settings' as ActiveView);
        toggleCommandBar();
      },
    },
    {
      id: 'system:versions',
      label: 'View Version History',
      description: 'Browse automatic commit history',
      icon: GitBranch,
      category: 'System',
      action: async () => {
        setActiveView('settings' as ActiveView);
        toggleCommandBar();
      },
    },
  ], [setActiveView, toggleCommandBar, fetchMemories, fetchGraph, buildContext]);

  const commands = buildCommands();

  const filtered = useMemo(() => {
    if (!query) return commands;
    const q = query.toLowerCase();
    return commands.filter(
      (cmd) =>
        cmd.label.toLowerCase().includes(q) ||
        cmd.description.toLowerCase().includes(q) ||
        cmd.category.toLowerCase().includes(q)
    );
  }, [commands, query]);

  const grouped = useMemo(() => {
    return filtered.reduce<Record<string, CommandItem[]>>((acc, cmd) => {
      if (!acc[cmd.category]) acc[cmd.category] = [];
      acc[cmd.category].push(cmd);
      return acc;
    }, {});
  }, [filtered]);

  useEffect(() => {
    if (commandBarOpen) {
      justOpenedRef.current = true;
      setQuery('');
      setSelectedIndex(0);
      setTimeout(() => {
        inputRef.current?.focus();
        justOpenedRef.current = false;
      }, 50);
    }
  }, [commandBarOpen]);

  // Arrow key + Enter navigation
  useEffect(() => {
    if (!commandBarOpen) return;

    function onKeyDown(e: KeyboardEvent) {
      if (e.key === 'ArrowDown') {
        e.preventDefault();
        setSelectedIndex((i) => Math.min(i + 1, filtered.length - 1));
      } else if (e.key === 'ArrowUp') {
        e.preventDefault();
        setSelectedIndex((i) => Math.max(i - 1, 0));
      } else if (e.key === 'Enter' && filtered[selectedIndex]) {
        e.preventDefault();
        filtered[selectedIndex].action();
      }
    }

    window.addEventListener('keydown', onKeyDown);
    return () => window.removeEventListener('keydown', onKeyDown);
  }, [commandBarOpen, filtered, selectedIndex]);

  // Auto-scroll selected item into view (skip on initial open)
  useEffect(() => {
    if (justOpenedRef.current) return;
    if (!listRef.current) return;
    const selected = listRef.current.querySelector('[data-selected="true"]');
    if (selected) {
      selected.scrollIntoView({ block: 'nearest' });
    }
  }, [selectedIndex]);

  if (!commandBarOpen) return null;

  let flatIndex = -1;

  return (
    <div
      className="command-overlay"
      onClick={(e) => {
        if (e.target === e.currentTarget) toggleCommandBar();
      }}
    >
      <div className="command-palette">
        {/* Input */}
        <div className="command-input-wrapper">
          <div className="search-icon-wrap">
            <Search size={16} style={{ color: 'var(--tangerine)' }} />
          </div>
          <input
            ref={inputRef}
            type="text"
            value={query}
            onChange={(e) => {
              setQuery(e.target.value);
              setSelectedIndex(0);
            }}
            placeholder={t('command.search')}
            className="command-input"
          />
          {query && (
            <button
              onClick={() => { setQuery(''); inputRef.current?.focus(); }}
              style={{
                background: 'var(--raised)', border: '1px solid var(--line)', borderRadius: '4px',
                padding: '2px 6px', cursor: 'pointer', fontSize: '11px', color: 'var(--muted-2)',
                fontFamily: 'var(--mono)', flexShrink: 0,
              }}
            >
              {t('command.clear')}
            </button>
          )}
          <kbd style={{
            background: 'var(--raised)', border: '1px solid var(--line)', borderRadius: '4px',
            padding: '2px 6px', fontSize: '11px', color: 'var(--muted-2)', fontFamily: 'var(--mono)',
            flexShrink: 0,
          }}>Esc</kbd>
        </div>

        {/* Commands */}
        <div ref={listRef} style={{ maxHeight: '380px', overflowY: 'auto', padding: '6px' }}>
          {Object.entries(grouped).map(([category, items]) => {
            const catColor = categoryColors[category] || 'var(--muted-2)';
            return (
              <div key={category} style={{ marginBottom: '4px' }}>
                <div style={{
                  display: 'flex', alignItems: 'center', gap: '6px',
                  fontSize: '10px', fontWeight: 700, color: catColor,
                  textTransform: 'uppercase', letterSpacing: '0.1em',
                  padding: '10px 12px 4px', userSelect: 'none',
                }}>
                  <div style={{
                    width: '4px', height: '4px', borderRadius: '50%',
                    background: catColor, opacity: 0.6,
                  }} />
                  {category}
                  <span style={{
                    fontSize: '9px', fontWeight: 500, color: 'var(--muted-2)',
                    textTransform: 'none', letterSpacing: 'normal', marginLeft: '2px',
                  }}>
                    {items.length}
                  </span>
                </div>
                {items.map((cmd) => {
                  flatIndex++;
                  // FIX: capture index by value, not by reference
                  const idx = flatIndex;
                  const isSelected = idx === selectedIndex;
                  const Icon = cmd.icon;
                  return (
                    <button
                      key={cmd.id}
                      className="command-item"
                      data-selected={isSelected}
                      data-category={category.toLowerCase()}
                      onClick={cmd.action}
                      onMouseEnter={() => setSelectedIndex(idx)}
                      style={{ '--cat-color': catColor } as React.CSSProperties}
                    >
                      {/* Icon */}
                      <div
                        className="command-item-icon"
                        style={{
                          background: isSelected ? `${catColor}15` : undefined,
                          borderColor: isSelected ? `${catColor}30` : undefined,
                        }}
                      >
                        <Icon size={14} style={{ color: isSelected ? catColor : 'var(--muted-2)' }} />
                      </div>

                      {/* Text */}
                      <div style={{ flex: 1, minWidth: 0 }}>
                        <div style={{ fontSize: '13px', fontWeight: 500, color: 'var(--bone)' }}>
                          <HighlightedText text={cmd.label} query={query} />
                        </div>
                        <div style={{ fontSize: '11px', color: 'var(--muted-2)', marginTop: '2px', lineHeight: 1.3 }}>
                          <HighlightedText text={cmd.description} query={query} />
                        </div>
                      </div>

                      {/* Shortcut */}
                      {cmd.shortcut && (
                        <kbd
                          className="command-item-kbd"
                          style={{
                            borderColor: isSelected ? catColor + '40' : undefined,
                            color: isSelected ? catColor : undefined,
                            fontWeight: isSelected ? 600 : undefined,
                          }}
                        >
                          {cmd.shortcut}
                        </kbd>
                      )}

                      {/* Arrow */}
                      <ArrowRight size={12} style={{
                        color: catColor,
                        opacity: isSelected ? 1 : 0,
                        transform: isSelected ? 'translateX(0)' : 'translateX(-4px)',
                        transition: 'all 0.15s ease',
                        flexShrink: 0,
                      }} />
                    </button>
                  );
                })}
              </div>
            );
          })}

          {filtered.length === 0 && (
            <div style={{
              padding: '40px 16px', textAlign: 'center',
            }}>
              <div style={{
                width: '48px', height: '48px', display: 'flex', alignItems: 'center', justifyContent: 'center',
                background: 'var(--raised)', borderRadius: 'var(--radius-sm)', margin: '0 auto 12px',
              }}>
                <Search size={20} style={{ color: 'var(--muted-2)' }} />
              </div>
              <div style={{ fontSize: '14px', fontWeight: 500, color: 'var(--bone)', marginBottom: '4px' }}>
                {t('command.noResults')}
              </div>
              <div style={{ fontSize: '12px', color: 'var(--muted-2)' }}>
                {t('command.tryAgain')}
              </div>
            </div>
          )}
        </div>

        {/* Footer */}
        <div style={{
          display: 'flex', alignItems: 'center', justifyContent: 'space-between',
          padding: '8px 16px', borderTop: '1px solid var(--line)',
          fontSize: '11px', color: 'var(--muted-2)',
        }}>
          <div style={{ display: 'flex', alignItems: 'center', gap: '14px' }}>
            <span style={{ display: 'flex', alignItems: 'center', gap: '5px' }}>
              <kbd style={kbdStyle}>↑↓</kbd>
              {t('command.navigate')}
            </span>
            <span style={{ display: 'flex', alignItems: 'center', gap: '5px' }}>
              <kbd style={kbdStyle}>Enter</kbd>
              {t('command.select')}
            </span>
            <span style={{ display: 'flex', alignItems: 'center', gap: '5px' }}>
              <kbd style={kbdStyle}>Esc</kbd>
              {t('command.close')}
            </span>
          </div>
          <div style={{ display: 'flex', alignItems: 'center', gap: '6px' }}>
            <div style={{
              width: '6px', height: '6px', borderRadius: '50%',
              background: filtered.length > 0 ? 'var(--mint)' : 'var(--rose)',
            }} />
            {filtered.length} {filtered.length === 1 ? t('command.command') : t('command.commands')}
          </div>
        </div>
      </div>
    </div>
  );
}

const kbdStyle: React.CSSProperties = {
  background: 'var(--raised)',
  border: '1px solid var(--line)',
  borderRadius: '3px',
  padding: '1px 6px',
  fontFamily: 'var(--mono)',
  fontSize: '10px',
};
