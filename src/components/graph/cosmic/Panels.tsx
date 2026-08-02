import { useState, useRef, useEffect, useMemo, useCallback, memo } from 'react';
import {
  MessageCircle, Search, X, Brain, Link2, Eye, Focus, ChevronRight, Sparkles,
} from 'lucide-react';
import type { GraphNodeData, GraphEdgeData, SearchSuggestion } from './types';
import { screenPos } from './types';

/**
 * DOM overlays for the cosmic graph: labels, context menu, search, info panel.
 *
 * These live outside the WebGL canvas and are plain React, so they belong apart
 * from the renderer. Splitting them out also makes the label layer's cost
 * visible: it is the only overlay that runs work every frame, and it is the
 * reason the level-of-detail budget in `./lod` exists.
 */
// ═══════════════════════════════════════════════════════════════
// DOM: LABEL LAYER
// ═══════════════════════════════════════════════════════════════
export const LabelLayer = memo(function LabelLayer({
  nodes, hoveredIdRef, highlightedIdsRef, filteredIds, labelIdsRef,
}: {
  nodes: GraphNodeData[]; hoveredIdRef: React.MutableRefObject<string | null>;
  highlightedIdsRef: React.MutableRefObject<Set<string>>; filteredIds: Set<string> | null;
  /**
   * Ids allowed a label this frame, chosen by the projection pass.
   *
   * Labels are the most expensive thing this view does: each one is a real DOM
   * node repositioned every frame, so the layout cost grows linearly with the
   * graph while the screen area does not. Past a hundred or so they also overlap
   * into an unreadable smear, meaning the uncapped version paid its highest cost
   * exactly when the result was least useful.
   */
  labelIdsRef: React.MutableRefObject<Set<string>>;
}) {
  const containerRef = useRef<HTMLDivElement>(null);
  const rafRef = useRef<number>(0);
  useEffect(() => {
    const update = () => {
      const c = containerRef.current;
      if (!c) { rafRef.current = requestAnimationFrame(update); return; }
      const ch = c.children;
      const isSearching = filteredIds !== null;
      const ids = highlightedIdsRef.current;
      const hasHighlight = ids.size > 0;
      const allowed = labelIdsRef.current;
      for (let i = 0; i < nodes.length && i < ch.length; i++) {
        const n = nodes[i]; const el = ch[i] as HTMLElement; const sp = screenPos.get(n.id);
        const isH = hasHighlight ? ids.has(n.id) : hoveredIdRef.current === n.id;
        const isDimmed = hasHighlight && !isH;
        const isCore = n.orbitRadius === 0;
        const isF = isSearching && !filteredIds.has(n.id);
        // Outside the budget: skip the write entirely rather than positioning a
        // hidden element. Touching `left`/`top` on a hidden node still costs
        // style recalculation, which is the whole expense being avoided here.
        if (!sp || !sp.visible || isF || !allowed.has(n.id)) { el.style.visibility = 'hidden'; }
        else {
          el.style.visibility = 'visible'; el.style.left = `${sp.x}px`; el.style.top = `${sp.y}px`;
          el.style.transform = `translate(-50%, -100%) scale(${isH ? 1.15 : 1})`;
          el.style.opacity = isDimmed ? '0.2' : (isCore ? '1' : (isH ? '1' : '0.55'));
          el.style.fontSize = isCore ? '12px' : '10px';
          el.style.fontWeight = isCore ? '700' : (isH ? '600' : '400');
          el.style.color = isH ? n.color : (isDimmed ? '#333' : (isCore ? '#e8edf3' : '#7888a0'));
        }
      }
      rafRef.current = requestAnimationFrame(update);
    };
    rafRef.current = requestAnimationFrame(update);
    return () => cancelAnimationFrame(rafRef.current);
  }, [nodes, hoveredIdRef, highlightedIdsRef, filteredIds]);

  return (
    <div ref={containerRef} style={{ position: 'absolute', inset: 0, pointerEvents: 'none', overflow: 'hidden' }}>
      {nodes.map(n => (
        <div key={n.id} style={{
          position: 'absolute', transform: 'translate(-50%, -100%)',
          fontFamily: 'system-ui, -apple-system, sans-serif',
          textShadow: '0 1px 4px rgba(0,0,0,0.9)', whiteSpace: 'nowrap',
          pointerEvents: 'none', userSelect: 'none', willChange: 'transform',
          transition: 'color 0.2s, opacity 0.2s, font-size 0.15s',
        }}>
          {n.title}
        </div>
      ))}
    </div>
  );
});

// ═══════════════════════════════════════════════════════════════
// DOM: CONTEXT MENU
// ═══════════════════════════════════════════════════════════════
export function ContextMenu({
  data, onClose, onAskCopilot, onFocus, onHide, onViewMemory,
}: {
  data: { x: number; y: number; node: GraphNodeData } | null;
  onClose: () => void; onAskCopilot: (n: GraphNodeData) => void;
  onFocus: (n: GraphNodeData) => void; onHide: (id: string) => void;
  onViewMemory: (n: GraphNodeData) => void;
}) {
  useEffect(() => {
    if (!data) return;
    const h = () => onClose();
    document.addEventListener('click', h);
    document.addEventListener('contextmenu', h);
    return () => { document.removeEventListener('click', h); document.removeEventListener('contextmenu', h); };
  }, [data, onClose]);
  if (!data) return null;
  return (
    <div className="cosmic-context-menu" style={{ left: data.x, top: data.y }} onClick={e => e.stopPropagation()}>
      <div className="cosmic-context-header">
        <div className="cosmic-context-dot" style={{ background: data.node.color }} />
        <span>{data.node.title}</span>
      </div>
      <button onClick={() => { onAskCopilot(data.node); onClose(); }}><Brain size={14} /> Ask Copilot</button>
      <button onClick={() => { onViewMemory(data.node); onClose(); }}><MessageCircle size={14} /> View memories</button>
      <button onClick={() => { onFocus(data.node); onClose(); }}><Focus size={14} /> Focus cluster</button>
      <div className="cosmic-context-separator" />
      <button onClick={() => { onHide(data.node.id); onClose(); }}><Eye size={14} /> Hide node</button>
    </div>
  );
}

// ═══════════════════════════════════════════════════════════════
// DOM: SEARCH BAR with autocomplete
// ═══════════════════════════════════════════════════════════════
export function SearchBar({
  value, onChange, onClear, suggestions, onSelectSuggestion, allEntities,
}: {
  value: string; onChange: (v: string) => void; onClear: () => void;
  suggestions: SearchSuggestion[]; onSelectSuggestion: (s: SearchSuggestion) => void;
  allEntities: SearchSuggestion[];
}) {
  const [focused, setFocused] = useState(false);
  const [activeIdx, setActiveIdx] = useState(-1);
  const dropdownRef = useRef<HTMLDivElement>(null);
  const showDropdown = focused;
  // When empty query: show all entities grouped by type. When typing: show filtered suggestions
  const displayItems = value.length >= 2 ? suggestions : allEntities;
  // Flat list for keyboard nav
  const flatItems = useMemo(() => {
    if (value.length >= 2) return displayItems;
    const flat: SearchSuggestion[] = [];
    displayItems.forEach(s => flat.push(s));
    return flat;
  }, [displayItems, value]);
  // Group by type for empty query
  const grouped = useMemo(() => {
    if (value.length >= 2) return null;
    const groups = new Map<string, SearchSuggestion[]>();
    displayItems.forEach(s => {
      const key = s.kind === 'memory' ? 'Memories' : s.type;
      if (!groups.has(key)) groups.set(key, []);
      groups.get(key)!.push(s);
    });
    return groups;
  }, [displayItems, value]);

  // Reset active index when query changes
  useEffect(() => { setActiveIdx(-1); }, [value]);

  // Scroll active item into view
  useEffect(() => {
    if (activeIdx < 0 || !dropdownRef.current) return;
    const item = dropdownRef.current.querySelector('.cosmic-search-item-active');
    if (item) item.scrollIntoView({ block: 'nearest', behavior: 'smooth' });
  }, [activeIdx]);

  const handleKeyDown = useCallback((e: React.KeyboardEvent) => {
    if (!showDropdown || flatItems.length === 0) return;
    if (e.key === 'ArrowDown') {
      e.preventDefault();
      setActiveIdx(prev => (prev + 1) % flatItems.length);
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      setActiveIdx(prev => (prev - 1 + flatItems.length) % flatItems.length);
    } else if (e.key === 'Enter' && activeIdx >= 0 && activeIdx < flatItems.length) {
      e.preventDefault();
      onSelectSuggestion(flatItems[activeIdx]);
      setActiveIdx(-1);
    } else if (e.key === 'Escape') {
      e.preventDefault();
      setFocused(false);
      setActiveIdx(-1);
    }
  }, [showDropdown, flatItems, activeIdx, onSelectSuggestion]);

  return (
    <div className="cosmic-search">
      <Search size={13} className="cosmic-search-icon" />
      <input type="text" placeholder="Search..." value={value}
        onChange={e => onChange(e.target.value)}
        onFocus={() => setFocused(true)}
        onBlur={() => setTimeout(() => { setFocused(false); setActiveIdx(-1); }, 200)}
        onKeyDown={handleKeyDown}
        className="cosmic-search-input" />
      {value && <button className="cosmic-search-clear" onClick={onClear}><X size={11} /></button>}
      {showDropdown && (
        <div className="cosmic-search-dropdown" ref={dropdownRef}>
          {value.length < 2 && grouped ? (
            // Show grouped categories when no query
            (() => {
              let globalIdx = 0;
              return Array.from(grouped.entries()).map(([category, items]) => (
                <div key={category}>
                  <div className="cosmic-search-category">{category}</div>
                  {items.slice(0, 4).map(s => {
                    const idx = globalIdx++;
                    return <SearchSuggestionItem key={s.id} s={s} onSelect={onSelectSuggestion} isActive={idx === activeIdx} />;
                  })}
                </div>
              ));
            })()
          ) : (
            // Show filtered results when typing
            displayItems.map((s, idx) => <SearchSuggestionItem key={s.id} s={s} onSelect={onSelectSuggestion} isActive={idx === activeIdx} />)
          )}
          {value.length >= 2 && displayItems.length === 0 && (
            <div className="cosmic-search-empty">No results for "{value}"</div>
          )}
        </div>
      )}
    </div>
  );
}

function SearchSuggestionItem({ s, onSelect, isActive }: { s: SearchSuggestion; onSelect: (s: SearchSuggestion) => void; isActive?: boolean }) {
  const entityIcons: Record<string, string> = {
    person: '👤', project: '📁', decision: '⚖️', task: '✓',
    technology: '🔧', file: '📄', organization: '🏢', meeting: '📅',
    concept: '💡', document: '📝', default: '●',
  };
  const icon = entityIcons[s.type.toLowerCase()] || entityIcons.default;
  const descSnippet = s.description ? s.description.slice(0, 60) + (s.description.length > 60 ? '…' : '') : '';
  return (
    <button className={`cosmic-search-item${isActive ? ' cosmic-search-item-active' : ''}`}
      onMouseDown={(e) => {
        e.preventDefault(); // prevent input blur from killing dropdown
        e.stopPropagation();
        onSelect(s); // fire directly in mousedown — safe across all browsers/webview
      }}>
      <span className="cosmic-search-item-icon" style={{ color: s.color }}>{icon}</span>
      <div className="cosmic-search-item-text">
        <div className="cosmic-search-item-title">{s.title}</div>
        <div className="cosmic-search-item-type" style={{ color: s.color }}>
          {s.type}{s.connectionCount != null ? ` · ${s.connectionCount} conn` : ''}
        </div>
        {descSnippet && <div className="cosmic-search-item-desc">{descSnippet}</div>}
      </div>
      <span className="cosmic-search-item-badge" style={{ color: s.color, background: `${s.color}15` }}>
        {s.kind === 'entity' ? 'Entity' : 'Memory'}
      </span>
    </button>
  );
}

// ═══════════════════════════════════════════════════════════════
// DOM: INFO PANEL — rich display with backend data
// ═══════════════════════════════════════════════════════════════
export const InfoPanel = memo(function InfoPanel({
  node, onClose, onAskCopilot, relatedMemoryCount, relatedMemories,
  connectedNodes, allEdges, onSelectNode, onViewMemory,
}: {
  node: GraphNodeData | null; onClose: () => void;
  onAskCopilot: (n: GraphNodeData) => void; relatedMemoryCount: number;
  relatedMemories: Array<{ id: string; title: string; summary: string; confidenceScore: number; importanceScore: number }>;
  connectedNodes: GraphNodeData[]; allEdges: GraphEdgeData[];
  onSelectNode: (n: GraphNodeData) => void; onViewMemory: (id: string) => void;
}) {
  const [expanded, setExpanded] = useState(false);
  if (!node) return null;
  const nodeEdges = allEdges.filter(e => e.source === node.id || e.target === node.id);
  const entityIcons: Record<string, string> = {
    person: '👤', project: '📁', decision: '⚖️', task: '✓',
    technology: '🔧', file: '📄', organization: '🏢', meeting: '📅',
    concept: '💡', document: '📝', default: '●',
  };
  const icon = entityIcons[node.entityType.toLowerCase()] || entityIcons.default;
  return (
    <div className="cosmic-info-panel" style={{ animation: 'slideInRight 0.25s ease-out' }}>
      <div className="cosmic-info-header">
        <div className="cosmic-info-icon" style={{ background: `${node.color}22`, color: node.color, boxShadow: `0 0 16px ${node.color}33`, fontSize: 18 }}>
          {icon}
        </div>
        <div style={{ flex: 1, minWidth: 0 }}>
          <div className="cosmic-info-title">{node.title}</div>
          <div className="cosmic-info-type" style={{ color: node.color }}>{node.entityType}</div>
        </div>
        <button className="cosmic-info-close" onClick={onClose}>×</button>
      </div>
      <div className="cosmic-info-body" style={{ maxHeight: expanded ? 500 : 280, overflowY: 'auto', transition: 'max-height 0.3s ease' }}>
        {node.description && (
          <div className="cosmic-info-desc">{node.description}</div>
        )}
        <div className="cosmic-info-stats">
          <div className="cosmic-info-stat"><Link2 size={12} /><span>{node.connectionCount} connections</span></div>
          {relatedMemoryCount > 0 && <div className="cosmic-info-stat"><Brain size={12} /><span>{relatedMemoryCount} related memories</span></div>}
        </div>

        {/* Connected entities */}
        {nodeEdges.length > 0 && (
          <div className="cosmic-info-section">
            <div className="cosmic-info-section-title"><Link2 size={10} /> Connections</div>
            {nodeEdges.slice(0, expanded ? 12 : 5).map(e => {
              const otherId = e.source === node.id ? e.target : e.source;
              const other = connectedNodes.find(n => n.id === otherId);
              if (!other) return null;
              const relColors: Record<string, string> = {
                RelatedTo: '#63d8d2', Uses: '#ff8a5b', Implements: '#a99cf8',
                DependsOn: '#f472b6', PartOf: '#ddbb65', ConflictsWith: '#ef4444',
              };
              const relColor = relColors[e.relationshipType] || '#6b7280';
              return (
                <div key={e.id} className="cosmic-info-connection" onClick={() => onSelectNode(other)}
                  style={{ cursor: 'pointer' }}>
                  <div style={{ width: 6, height: 6, borderRadius: '50%', background: other.color, flexShrink: 0 }} />
                  <span className="cosmic-info-connection-name">{other.title}</span>
                  <span className="cosmic-info-connection-rel" style={{ color: relColor, background: `${relColor}15` }}>{e.relationshipType}</span>
                </div>
              );
            })}
          </div>
        )}

        {/* Related memories */}
        {relatedMemories.length > 0 && (
          <div className="cosmic-info-section">
            <div className="cosmic-info-section-title"><Brain size={10} /> Related Memories</div>
            {relatedMemories.slice(0, expanded ? 6 : 3).map(m => (
              <div key={m.id} className="cosmic-info-memory-card" onClick={() => onViewMemory(m.id)}
                style={{ cursor: 'pointer' }}>
                <div className="cosmic-info-memory-title">{m.title}</div>
                <div className="cosmic-info-memory-summary">
                  {m.summary?.slice(0, 80)}{m.summary?.length > 80 ? '…' : ''}
                </div>
                <div className="cosmic-info-memory-bars">
                  <div className="cosmic-info-score-bar">
                    <span className="cosmic-info-score-label">Conf</span>
                    <div className="cosmic-info-score-track">
                      <div className="cosmic-info-score-fill" style={{ width: `${Math.round(m.confidenceScore * 100)}%`, background: '#63d8d2' }} />
                    </div>
                    <span className="cosmic-info-score-value">{Math.round(m.confidenceScore * 100)}%</span>
                  </div>
                  <div className="cosmic-info-score-bar">
                    <span className="cosmic-info-score-label">Imp</span>
                    <div className="cosmic-info-score-track">
                      <div className="cosmic-info-score-fill" style={{ width: `${Math.round(m.importanceScore * 100)}%`, background: '#ddbb65' }} />
                    </div>
                    <span className="cosmic-info-score-value">{Math.round(m.importanceScore * 100)}%</span>
                  </div>
                </div>
              </div>
            ))}
          </div>
        )}

        <button onClick={() => setExpanded(!expanded)} className="cosmic-info-expand">
          {expanded ? 'Show less' : `Show more`} <ChevronRight size={10} style={{ transform: expanded ? 'rotate(90deg)' : 'none', transition: 'transform 0.2s' }} />
        </button>

        <button className="cosmic-info-copilot-btn" onClick={() => onAskCopilot(node)}>
          <Sparkles size={14} /> Ask Copilot
        </button>
      </div>
    </div>
  );
});

