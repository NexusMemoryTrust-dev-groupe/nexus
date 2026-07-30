import { useState, useEffect, useRef, useMemo, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { Send, Bot, Zap, Brain, Network, Layers, Clock, Settings2, Database, Activity, GripVertical, ChevronDown, ChevronUp } from 'lucide-react';
import { useLocale } from '../../stores/localeStore';
import { useUiStore } from '../../stores/uiStore';
import { useMemoryStore } from '../../stores/memoryStore';
import { useGraphStore } from '../../stores/graphStore';
import { tryExecuteCommand } from '../../utils/commandExecutor';

interface Message {
  role: 'user' | 'assistant';
  content: string;
}

interface AiCommand {
  name: string;
  labelKey: string;
  descKey: string;
  icon: typeof Brain;
  category: string;
  args?: string;
  usesAI?: boolean;
  action: (args: string) => Promise<{ result: string; data?: unknown }>;
}

const PANEL_W = 380;
const PANEL_H_MIN = 200;
const MARGIN = 10;

function clamp(val: number, min: number, max: number) {
  return Math.max(min, Math.min(max, val));
}

export function FloatingCopilot() {
  const [messages, setMessages] = useState<Message[]>([]);
  const [input, setInput] = useState('');
  const [showCommands, setShowCommands] = useState(false);
  const [selectedIndex, setSelectedIndex] = useState(0);
  const [isLoading, setIsLoading] = useState(false);
  const [isDragging, setIsDragging] = useState(false);
  const [thinkingExpanded, setThinkingExpanded] = useState(false);
  const [thinkingText, setThinkingText] = useState('');
  const [streamingText, setStreamingText] = useState('');
  const [isThinking, setIsThinking] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);
  const listRef = useRef<HTMLDivElement>(null);
  const panelRef = useRef<HTMLDivElement>(null);
  const dragState = useRef<{ startX: number; startY: number; origX: number; origY: number; moved: boolean } | null>(null);
  const { t } = useLocale();
  const { setActiveView, copilotOpen, toggleCopilot, copilotX, copilotY, setCopilotPosition } = useUiStore();
  const { fetchMemories } = useMemoryStore();
  const { fetchGraph } = useGraphStore();

  // ── Selected AI model from config ──
  const [selectedModel, setSelectedModel] = useState<string>('');

  useEffect(() => {
    invoke<string | null>('get_config', { key: 'ai.model' }).then((v) => {
      setSelectedModel(v || 'opencode/deepseek-v4-flash-free');
    }).catch(() => {
      setSelectedModel('opencode/deepseek-v4-flash-free');
    });
  }, []);

  // ── Listen for streaming AI events ──
  useEffect(() => {
    const unlisteners: (() => void)[] = [];

    listen<{ chunk: string; full_thinking: string }>('ai-thinking-chunk', (event) => {
      setThinkingText(event.payload.full_thinking);
      setIsThinking(true);
    }).then((unlisten) => unlisteners.push(unlisten));

    listen<{ chunk: string; full_text: string }>('ai-text-chunk', (event) => {
      setStreamingText(event.payload.full_text);
    }).then((unlisten) => unlisteners.push(unlisten));

    listen<{ full_text: string; had_thinking: boolean }>('ai-stream-finish', () => {
      // Final answer received — add to messages
      setIsThinking(false);
    }).then((unlisten) => unlisteners.push(unlisten));

    return () => { unlisteners.forEach((u) => u()); };
  }, []);

  // ── Content area bounds — measure <main> inside .workspace-shell ──
  // Copilot stays strictly within the content area, never over sidebar or topbar.
  const getBounds = useCallback(() => {
    const main = document.querySelector('.workspace-shell main') as HTMLElement | null;
    if (!main) {
      return {
        minX: MARGIN,
        minY: MARGIN,
        maxX: window.innerWidth - PANEL_W - MARGIN,
        maxY: window.innerHeight - 28 - MARGIN,
      };
    }
    const rect = main.getBoundingClientRect();
    return {
      minX: rect.left + MARGIN,
      minY: rect.top + MARGIN,
      maxX: rect.right - PANEL_W - MARGIN,
      maxY: rect.bottom - MARGIN,
    };
  }, []);

  // ── Auto-position on first open / recalculate on sidebar toggle ──
  useEffect(() => {
    if (copilotX === -1 || copilotY === -1) {
      const b = getBounds();
      setCopilotPosition(b.maxX, b.minY);
    }
  }, [copilotX, copilotY, getBounds, setCopilotPosition]);

  // ── Re-clamp position when window resizes or sidebar toggles ──
  useEffect(() => {
    function onResize() {
      if (copilotX === -1 && copilotY === -1) return;
      const b = getBounds();
      const x = clamp(copilotX, b.minX, b.maxX);
      const y = clamp(copilotY, b.minY, b.maxY);
      if (x !== copilotX || y !== copilotY) setCopilotPosition(x, y);
    }
    window.addEventListener('resize', onResize);
    return () => window.removeEventListener('resize', onResize);
  }, [copilotX, copilotY, getBounds, setCopilotPosition]);

  // ── Drag: mousedown on header or tab ──
  const onDragStart = useCallback((e: React.MouseEvent) => {
    if ((e.target as HTMLElement).closest('button')) return;
    e.preventDefault();
    dragState.current = { startX: e.clientX, startY: e.clientY, origX: copilotX, origY: copilotY, moved: false };
    setIsDragging(true);

    const onMove = (ev: MouseEvent) => {
      if (!dragState.current) return;
      const dx = ev.clientX - dragState.current.startX;
      const dy = ev.clientY - dragState.current.startY;
      // Threshold: treat as drag if moved > 4px
      if (Math.abs(dx) > 4 || Math.abs(dy) > 4) {
        dragState.current.moved = true;
      }
      const b = getBounds();
      const newX = clamp(dragState.current.origX + dx, b.minX, b.maxX);
      const newY = clamp(dragState.current.origY + dy, b.minY, b.maxY);
      setCopilotPosition(newX, newY);
    };

    const onUp = () => {
      const wasDrag = dragState.current?.moved ?? false;
      dragState.current = null;
      setIsDragging(false);
      document.removeEventListener('mousemove', onMove);
      document.removeEventListener('mouseup', onUp);
      document.body.style.userSelect = '';
      // If mouse didn't move (pure click), toggle copilot
      if (!wasDrag) {
        toggleCopilot();
      }
    };

    document.body.style.userSelect = 'none';
    document.addEventListener('mousemove', onMove);
    document.addEventListener('mouseup', onUp);
  }, [copilotX, copilotY, getBounds, setCopilotPosition, toggleCopilot]);

  // ── Commands ──
  const commands: AiCommand[] = useMemo(() => [
    {
      name: 'memories', labelKey: 'ai.cmd.memories', descKey: 'ai.cmd.memories', icon: Brain, category: 'Memory', usesAI: true,
      action: async () => {
        const memories = await invoke<Array<{ id: string; title: string; layer: string; importanceScore: number; createdAt: string }>>('get_memories');
        return { result: t('ai.res.memories').replace('{count}', String(memories.length)), data: memories };
      },
    },
    {
      name: 'memory', labelKey: 'ai.cmd.memory', descKey: 'ai.cmd.memory', icon: Brain, category: 'Memory', args: '<id>', usesAI: true,
      action: async (args) => {
        if (!args) return { result: t('ai.res.error').replace('{message}', 'Missing memory ID'), data: null };
        const memory = await invoke<{ id: string; title: string; content: string; layer: string; importanceScore: number; confidenceScore: number } | null>('get_memory', { id: args });
        if (!memory) return { result: t('ai.res.error').replace('{message}', 'Memory not found'), data: null };
        return { result: t('ai.res.memory').replace('{title}', memory.title), data: memory };
      },
    },
    {
      name: 'create-memory', labelKey: 'ai.cmd.create-memory', descKey: 'ai.cmd.create-memory', icon: Zap, category: 'Memory', args: '<title>',
      action: async (args) => {
        const title = args || 'New Memory';
        await invoke('create_memory', { title, content: 'Created via AI Co-Pilot', author: 'ai' });
        await fetchMemories();
        return { result: t('ai.res.created'), data: null };
      },
    },
    {
      name: 'search', labelKey: 'ai.cmd.search', descKey: 'ai.cmd.search', icon: Brain, category: 'Memory', args: '<query>', usesAI: true,
      action: async (args) => {
        if (!args) return { result: t('ai.res.error').replace('{message}', 'Missing search query'), data: null };
        const results = await invoke<Array<{ id: string; title: string; content: string; layer: string }>>('search_memories', { query: args });
        return { result: t('ai.res.search').replace('{count}', String(results.length)).replace('{query}', args), data: results };
      },
    },
    {
      name: 'graph', labelKey: 'ai.cmd.graph', descKey: 'ai.cmd.graph', icon: Network, category: 'Graph', usesAI: true,
      action: async () => {
        const data = await invoke<{ nodes: Array<{ id: string; entityType: string; title: string }>; edges: Array<{ id: string; sourceEntityId: string; targetEntityId: string; relationshipType: string }> }>('get_graph');
        await fetchGraph();
        return { result: t('ai.res.graph').replace('{nodes}', String(data.nodes.length)).replace('{edges}', String(data.edges.length)), data };
      },
    },
    {
      name: 'entity', labelKey: 'ai.cmd.entity', descKey: 'ai.cmd.entity', icon: Network, category: 'Graph', args: '<id>', usesAI: true,
      action: async (args) => {
        if (!args) return { result: t('ai.res.error').replace('{message}', 'Missing entity ID'), data: null };
        const entity = await invoke<{ id: string; title: string; entityType: string; description: string } | null>('get_entity', { id: args });
        if (!entity) return { result: t('ai.res.error').replace('{message}', 'Entity not found'), data: null };
        return { result: t('ai.res.entity').replace('{title}', entity.title).replace('{type}', entity.entityType), data: entity };
      },
    },
    {
      name: 'create-entity', labelKey: 'ai.cmd.create-entity', descKey: 'ai.cmd.create-entity', icon: Network, category: 'Graph', args: '<type> <title>',
      action: async (args) => {
        const parts = args.split(' ');
        const entityType = parts[0] || 'Project';
        const title = parts.slice(1).join(' ') || 'New Entity';
        await invoke('create_entity', { entityType, title, description: 'Created via AI Co-Pilot' });
        await fetchGraph();
        return { result: t('ai.res.created'), data: null };
      },
    },
    {
      name: 'context', labelKey: 'ai.cmd.context', descKey: 'ai.cmd.context', icon: Layers, category: 'Context', args: '<query>', usesAI: true,
      action: async (args) => {
        const query = args || 'general';
        const result = await invoke<{ token_count: number; entities: Array<{ id: string; title: string }>; memory_records: Array<{ id: string; title: string }>; relationships: Array<{ id: string }> }>('build_context', { query });
        return {
          result: t('ai.res.context').replace('{tokens}', String(result.token_count)).replace('{entities}', String(result.entities.length)).replace('{memories}', String(result.memory_records.length)),
          data: result,
        };
      },
    },
    {
      name: 'stats', labelKey: 'ai.cmd.stats', descKey: 'ai.cmd.stats', icon: Database, category: 'System', usesAI: true,
      action: async () => {
        const stats = await invoke<{ memory_count: number; entity_count: number; relationship_count: number; commit_count: number; snapshot_count: number; db_size_bytes: number }>('get_db_stats');
        return { result: t('ai.res.stats'), data: stats };
      },
    },
    {
      name: 'health', labelKey: 'ai.cmd.health', descKey: 'ai.cmd.health', icon: Activity, category: 'System',
      action: async () => {
        const status = await invoke<string>('ai_health_check');
        return { result: t('ai.res.health').replace('{status}', status), data: null };
      },
    },
    {
      name: 'settings', labelKey: 'ai.cmd.settings', descKey: 'ai.cmd.settings', icon: Settings2, category: 'Navigation',
      action: async () => {
        setActiveView('settings');
        return { result: t('ai.res.opened').replace('{view}', 'Settings'), data: null };
      },
    },
    {
      name: 'timeline', labelKey: 'ai.cmd.timeline', descKey: 'ai.cmd.timeline', icon: Clock, category: 'Navigation',
      action: async () => {
        setActiveView('timeline');
        return { result: t('ai.res.opened').replace('{view}', 'Timeline'), data: null };
      },
    },
  ], [t, setActiveView, fetchMemories, fetchGraph]);

  const filtered = useMemo(() => {
    if (!input.startsWith('/')) return [];
    const query = input.slice(1).toLowerCase();
    return commands.filter(
      (cmd) => cmd.name.toLowerCase().includes(query) || t(cmd.labelKey).toLowerCase().includes(query)
    );
  }, [input, commands, t]);

  useEffect(() => {
    setShowCommands(input.startsWith('/') && input.length > 0);
    setSelectedIndex(0);
  }, [input]);

  useEffect(() => {
    if (!showCommands) return;
    function onKeyDown(e: KeyboardEvent) {
      if (e.key === 'ArrowDown') { e.preventDefault(); setSelectedIndex((i) => Math.min(i + 1, filtered.length - 1)); }
      else if (e.key === 'ArrowUp') { e.preventDefault(); setSelectedIndex((i) => Math.max(i - 1, 0)); }
      else if (e.key === 'Enter' && filtered[selectedIndex]) { e.preventDefault(); selectCommand(filtered[selectedIndex]); }
      else if (e.key === 'Escape') { setShowCommands(false); setInput(''); }
    }
    window.addEventListener('keydown', onKeyDown);
    return () => window.removeEventListener('keydown', onKeyDown);
  }, [showCommands, filtered, selectedIndex]);

  useEffect(() => {
    if (!listRef.current) return;
    const selected = listRef.current.querySelector('[data-selected="true"]');
    if (selected) selected.scrollIntoView({ block: 'nearest' });
  }, [selectedIndex]);

  const selectCommand = useCallback((cmd: AiCommand) => {
    setInput(`/${cmd.name} `);
    setShowCommands(false);
    inputRef.current?.focus();
  }, []);

  const handleSend = useCallback(async () => {
    if (!input.trim() || isLoading) return;
    const userMsg = input.trim();
    setMessages((prev) => [...prev, { role: 'user', content: userMsg }]);
    setInput('');
    setShowCommands(false);
    setIsLoading(true);
    setThinkingExpanded(false);
    setThinkingText('');
    setStreamingText('');
    setIsThinking(false);

    // Try to execute as slash command first
    if (userMsg.startsWith('/')) {
      try {
        const result = await tryExecuteCommand(userMsg);
        if (result) {
          setMessages((prev) => [...prev, { role: 'assistant', content: result }]);
          setIsLoading(false);
          return;
        }
      } catch {
        // Fall through to AI chat if command executor fails
      }
    }

    // Send to AI via streaming
    try {
      const chatMessages = [...messages, { role: 'user' as const, content: userMsg }].map((m) => ({ role: m.role, content: m.content }));

      // Start streaming — this triggers Tauri events
      const finalText = await invoke<string>('ai_chat_stream', { messages: chatMessages, model: selectedModel || null });
      setMessages((prev) => [...prev, { role: 'assistant', content: finalText }]);
    } catch (err) {
      // If streaming failed, show error
      setMessages((prev) => [...prev, { role: 'assistant', content: t('ai.res.error').replace('{message}', String(err)) }]);
    }

    setIsLoading(false);
    setThinkingText('');
    setStreamingText('');
    setIsThinking(false);
  }, [input, t, messages, isLoading, selectedModel]);

  const categoryColors: Record<string, string> = {
    Memory: 'var(--tangerine)',
    Graph: 'var(--cyan)',
    Context: 'var(--gold)',
    System: 'var(--steel)',
    Navigation: 'var(--periwinkle)',
  };

  // ── Position: clamp within content area bounds ──
  const bounds = getBounds();
  const posX = copilotX === -1 ? bounds.maxX : clamp(copilotX, bounds.minX, bounds.maxX);
  const posY = copilotY === -1 ? bounds.minY : clamp(copilotY, bounds.minY, bounds.maxY);
  const panelHeight = `min(${bounds.maxY - posY}px, 70vh)`;

  // ── Collapsed tab — same look as panel header, width = PANEL_W ──
  if (!copilotOpen) {
    return (
      <div
        className={`floating-copilot-tab ${isDragging ? 'dragging' : ''}`}
        onMouseDown={onDragStart}
        style={{
          position: 'fixed',
          left: posX,
          top: posY,
          width: PANEL_W,
          zIndex: 9999,
        }}
      >
        <GripVertical size={14} className="copilot-grip" />
        <Bot size={16} style={{ color: 'var(--tangerine)', flexShrink: 0 }} />
        <span className="copilot-tab-label">AI Copilot</span>
        <ChevronUp size={16} style={{ color: 'var(--muted-2)', flexShrink: 0 }} />
      </div>
    );
  }

  // ── Expanded panel — within content area ──
  let flatIndex = -1;

  return (
    <div
      ref={panelRef}
      className={`floating-copilot-panel ${isDragging ? 'dragging' : ''}`}
      style={{
        position: 'fixed',
        left: posX,
        top: posY,
        width: PANEL_W,
        height: panelHeight,
        minHeight: PANEL_H_MIN,
        zIndex: 9999,
      }}
    >
      {/* ── Draggable header ── */}
      <div className="floating-copilot-header" onMouseDown={onDragStart}>
        <GripVertical size={14} className="copilot-grip" />
        <Bot size={16} style={{ color: 'var(--tangerine)', flexShrink: 0 }} />
        <span className="copilot-header-title">{t('ai.title')}</span>
        <button className="floating-copilot-toggle" onClick={toggleCopilot} title="Свернуть">
          <ChevronDown size={16} />
        </button>
      </div>

      {/* ── Messages ── */}
      <div className="ai-messages">
        {messages.length === 0 && !isLoading && (
          <div className="empty-state" style={{ marginTop: '40px' }}>
            <div className="empty-state-desc">{t('ai.subtitle')}</div>
          </div>
        )}
        {messages.map((msg, i) => (
          <div key={i} className={`ai-message ${msg.role}`}>
            {msg.content}
          </div>
        ))}
        {/* Streaming answer (appears as response arrives) */}
        {isLoading && streamingText && (
          <div className="ai-message assistant">
            {streamingText}
          </div>
        )}
        {/* Thinking indicator — expandable, shows real AI reasoning */}
        {isLoading && (
          <div className="ai-thinking-wrapper">
            <button
              className="ai-thinking-btn"
              onClick={() => setThinkingExpanded((p) => !p)}
              type="button"
            >
              <span className={`ai-thinking-spinner ${isThinking ? 'active' : ''}`} />
              <span className="ai-thinking-label">
                {isThinking ? 'Thinking' : (streamingText ? 'Responding' : 'Processing')}
              </span>
              <ChevronDown
                size={14}
                style={{
                  color: 'var(--muted-2)',
                  transition: 'transform 0.2s',
                  transform: thinkingExpanded ? 'rotate(180deg)' : 'rotate(0deg)',
                }}
              />
            </button>
            {thinkingExpanded && (
              <div className="ai-thinking-expanded">
                {thinkingText ? (
                  <div className="ai-thinking-real-text">{thinkingText}</div>
                ) : (
                  <>
                    <div className="ai-thinking-line" />
                    <div className="ai-thinking-line short" />
                    <div className="ai-thinking-line medium" />
                    <div className="ai-thinking-text">Waiting for thinking process...</div>
                  </>
                )}
              </div>
            )}
          </div>
        )}
      </div>

      {/* ── Input ── */}
      <div className="ai-input-wrapper">
        {showCommands && filtered.length > 0 && (
          <div ref={listRef} className="ai-commands-dropdown">
            {filtered.map((cmd) => {
              flatIndex++;
              const idx = flatIndex;
              const isSelected = idx === selectedIndex;
              const Icon = cmd.icon;
              const catColor = categoryColors[cmd.category] || 'var(--muted-2)';
              return (
                <button
                  key={cmd.name}
                  data-selected={isSelected}
                  onClick={() => selectCommand(cmd)}
                  onMouseEnter={() => setSelectedIndex(idx)}
                  className="ai-cmd-item"
                >
                  <div
                    className="ai-cmd-icon"
                    style={{
                      background: isSelected ? `${catColor}15` : undefined,
                      borderColor: isSelected ? `${catColor}30` : undefined,
                    }}
                  >
                    <Icon size={13} style={{ color: isSelected ? catColor : 'var(--muted-2)' }} />
                  </div>
                  <div style={{ flex: 1, minWidth: 0 }}>
                    <div style={{ fontSize: '12px', fontWeight: 600, color: 'var(--bone)', fontFamily: 'var(--mono)' }}>
                      /{cmd.name}
                      {cmd.args && <span style={{ color: 'var(--muted-2)', fontWeight: 400 }}> {cmd.args}</span>}
                    </div>
                    <div style={{ fontSize: '10px', color: 'var(--muted-2)', marginTop: '1px' }}>
                      {t(cmd.descKey)}
                    </div>
                  </div>
                  {cmd.usesAI && (
                    <span style={{
                      fontSize: '9px', fontWeight: 600, color: 'var(--tangerine)',
                      background: 'var(--tangerine-soft)', borderRadius: '4px',
                      padding: '2px 6px', flexShrink: 0,
                    }}>AI</span>
                  )}
                </button>
              );
            })}
          </div>
        )}
        <input
          ref={inputRef}
          type="text"
          value={input}
          onChange={(e) => setInput(e.target.value)}
          onKeyDown={(e) => { if (e.key === 'Enter' && !showCommands) handleSend(); }}
          placeholder={t('ai.placeholder')}
          className="ai-input"
          disabled={isLoading}
        />
        <button className="btn btn-primary" onClick={handleSend} style={{ padding: '10px 14px' }} disabled={isLoading}>
          <Send size={16} />
        </button>
      </div>
    </div>
  );
}
