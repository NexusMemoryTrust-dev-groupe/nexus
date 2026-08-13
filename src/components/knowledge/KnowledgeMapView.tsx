import { useCallback, useMemo, useState } from 'react';
import type { CSSProperties } from 'react';
import { invoke } from '@tauri-apps/api/core';
import {
  AlertTriangle, BookMarked, CircleDot, History, Layers, Map as MapIcon,
  RefreshCw, Sparkles, Target,
} from 'lucide-react';
import type { GraphNode, KnowledgeMap, MapItem } from '../../types';
import {
  InfoTip, PageHero, StrataAlert, StrataVoid,
} from '../ui/Instruments';

// ── Ring metadata ───────────────────────────────────────────────────────────

const RING_META: Record<string, { icon: typeof Target; color: string; label: string; hint: string }> = {
  mission: { icon: Target, color: 'var(--rose)', label: 'Mission', hint: 'What is in active work right now — the current focus.' },
  relevant: { icon: Sparkles, color: 'var(--gold)', label: 'Relevant', hint: 'What is directly relevant to the mission right now.' },
  supporting: { icon: Layers, color: 'var(--cyan)', label: 'Supporting', hint: 'Stable background knowledge that supports the mission.' },
  historical: { icon: History, color: 'var(--muted-2)', label: 'Historical', hint: 'What has aged out — kept for provenance, no longer competing for context.' },
};

function ringMeta(ring: string) {
  return RING_META[ring] ?? RING_META.supporting;
}

// ── Ring section ────────────────────────────────────────────────────────────

function RingSection({ ring, items }: { ring: string; items: MapItem[] }) {
  const meta = ringMeta(ring);
  const Icon = meta.icon;
  return (
    <div className="st-map-ring" style={{ '--ring-color': meta.color } as CSSProperties}>
      <div className="st-map-ring-head">
        <span className="st-map-ring-icon" style={{ color: meta.color, background: `${meta.color}15` }}>
          <Icon size={13} />
        </span>
        <span className="st-map-ring-label">{meta.label}</span>
        <InfoTip text={meta.hint} />
        <span className="st-map-ring-count">{items.length}</span>
      </div>
      {items.length === 0 ? (
        <p className="st-map-ring-empty">nothing here</p>
      ) : (
        <div className="st-map-ring-list">
          {items.map((item) => (
            <div key={item.id} className="st-map-item">
              <span className="st-map-item-kind">{item.kind}</span>
              <span className="st-map-item-title">{item.title}</span>
              <span className="st-map-item-meta">
                {item.layer}{item.owner ? ` · ${item.owner}` : ''}
              </span>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

// ── Main view ───────────────────────────────────────────────────────────────

export function KnowledgeMapView() {
  const [nodes, setNodes] = useState<GraphNode[]>([]);
  const [map, setMap] = useState<KnowledgeMap | null>(null);
  const [query, setQuery] = useState('');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const loadNodes = useCallback(async () => {
    setBusy(true);
    try {
      const data = await invoke<{ nodes: GraphNode[] }>('get_graph');
      setNodes(data.nodes ?? []);
      setError(null);
    } catch (err) {
      setError(String(err));
    } finally {
      setBusy(false);
    }
  }, []);

  const openMap = useCallback(async (entityId: string) => {
    setBusy(true);
    try {
      const result = await invoke<KnowledgeMap>('knowledge_map', { entityId, depth: 1 });
      setMap(result);
      setError(null);
    } catch (err) {
      setError(String(err));
    } finally {
      setBusy(false);
    }
  }, []);

  // Lazy-load the entity index on first focus.
  const onFocus = useCallback(() => {
    if (nodes.length === 0) void loadNodes();
  }, [nodes.length, loadNodes]);

  const matches = useMemo(() => {
    const q = query.trim().toLowerCase();
    if (!q) return nodes.slice(0, 12);
    return nodes.filter((n) => n.title.toLowerCase().includes(q)).slice(0, 12);
  }, [nodes, query]);

  const hero = (
    <PageHero
      kicker="System 10 · AI Universe"
      title="Knowledge Map"
      copy="Around any entity, Nexus draws four concentric rings: what is in active work (Mission), what matters now (Relevant), what supports it (Supporting), and what has aged out (Historical). Pilot inside the map instead of staring at a raw graph."
      accent="var(--tangerine)"
      secondary="var(--cyan)"
      stats={map ? [
        { label: 'Total', value: String(map.total), color: 'var(--bone)' },
        { label: 'Mission', value: String(map.mission.length), color: 'var(--rose)' },
        { label: 'Relevant', value: String(map.relevant.length), color: 'var(--gold)' },
        { label: 'Supporting', value: String(map.supporting.length), color: 'var(--cyan)' },
      ] : []}
    />
  );

  const actions = (
    <div className="st-radar-actions">
      <button type="button" className="st-action-btn" disabled={busy} onClick={loadNodes}>
        <RefreshCw size={13} className={busy ? 'spinning' : undefined} />
        Load entities
      </button>
    </div>
  );

  if (error && !map) {
    return (
      <div className="st-page" style={{ '--st-accent': 'var(--tangerine)' } as CSSProperties}>
        {hero}{actions}
        <StrataAlert icon={AlertTriangle}>{error}</StrataAlert>
      </div>
    );
  }

  return (
    <div className="st-page" style={{ '--st-accent': 'var(--tangerine)' } as CSSProperties}>
      {hero}{actions}

      {/* Entity picker */}
      <section className="st-section-head" style={{ margin: '4px 0 10px' }}>
        <h2 className="st-section-title" style={{ '--section-color': 'var(--tangerine)' } as CSSProperties}>
          <CircleDot size={14} /> Pick an entity
        </h2>
        <InfoTip text="Search the knowledge graph and open the universe around an entity. The rings update instantly." />
      </section>
      <p className="st-section-desc">Search the knowledge graph and open the universe around an entity — the four rings update instantly.</p>
      <div className="st-sys-probe">
        <input
          className="st-sys-input"
          placeholder="Search entities…"
          value={query}
          onFocus={onFocus}
          onChange={(e) => setQuery(e.target.value)}
        />
        {matches.length > 0 && (
          <div className="st-map-picker">
            {matches.map((n) => (
              <button
                key={n.id}
                type="button"
                className="st-map-picker-item"
                onClick={() => {
                  setQuery(n.title);
                  void openMap(n.id);
                }}
              >
                <CircleDot size={12} style={{ color: 'var(--tangerine)' }} />
                <span style={{ minWidth: 0, flex: 1 }}>
                  <span className="st-map-picker-title">{n.title}</span>
                  <span className="st-map-picker-meta">{n.entityType}</span>
                </span>
              </button>
            ))}
          </div>
        )}
      </div>

      {/* Map */}
      {!map ? (
        <StrataVoid icon={MapIcon} title="No map open yet">
          Pick an entity above to see its universe. Or ask Copilot: <code>/map &lt;entity&gt;</code>.
        </StrataVoid>
      ) : (
        <div className="st-map">
          <div className="st-map-head">
            <BookMarked size={15} style={{ color: 'var(--tangerine)' }} />
            <span className="st-map-title">{map.entityTitle}</span>
            <span className="st-sys-meta">{map.entityId}</span>
          </div>
          <div className="st-map-rings">
            <RingSection ring="mission" items={map.mission} />
            <RingSection ring="relevant" items={map.relevant} />
            <RingSection ring="supporting" items={map.supporting} />
            <RingSection ring="historical" items={map.historical} />
          </div>
          {map.rendered && (
            <details className="st-map-rendered">
              <summary>Rendered map (copilot view)</summary>
              <pre>{map.rendered}</pre>
            </details>
          )}
        </div>
      )}
    </div>
  );
}
