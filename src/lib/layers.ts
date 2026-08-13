/**
 * Memory layer identity and semantics — single source of truth.
 *
 * Six cognitive layers replace the old four-rung ladder. A memory is no longer
 * just "Raw/Knowledge/Decision/Wisdom" — it is *classified*: the signature
 * classifier (backend) assigns Working / Episodic / Semantic / Procedural /
 * Decision / Strategic and records *why* (layer_reason) and *how sure*
 * (layer_confidence). The UI paints that provenance, not a guess.
 *
 * The palette still climbs with the layer — cool and raw at the bottom, warm
 * and settled at the top — so a wall of cards shows the shape of the
 * collection before a single title is read. Each rung carries locale key stems
 * for two questions the UI answers inline:
 *
 *   meaning  — what this layer *is*
 *   promotes — what moves a memory to the next rung
 *
 * Copy lives in `localeStore`, not here, so `en` and `ru` stay in one place.
 */

import type { CSSProperties } from 'react';
import type { LucideIcon } from 'lucide-react';
import {
  BookOpen,
  Gem,
  GitBranch,
  Inbox,
  ListOrdered,
  Zap,
} from 'lucide-react';

export type LayerName =
  | 'Working'
  | 'Episodic'
  | 'Semantic'
  | 'Procedural'
  | 'Decision'
  | 'Strategic';

export interface LayerVisual {
  /** Canonical name, safe to render. */
  name: LayerName;
  /** Token stem: `var(--{tint})` and `var(--{tint}-soft})` both resolve. */
  tint: 'blue' | 'cyan' | 'mint' | 'steel' | 'periwinkle' | 'gold';
  /** Solid accent. */
  color: string;
  /** ~12% wash of the accent, for fills. */
  soft: string;
  /** Literal rgba — `var()` cannot be interpolated into `rgba()` or a shadow. */
  glow: string;
  /** Directional wash for card ambience. */
  gradient: string;
  /** Icon for the layer glyph. */
  icon: LucideIcon;
  /** Two-letter code, for places too small for an icon. */
  code: string;
  /** Locale key stem: `layer.working.meaning`, `.promotes`. */
  key:
    | 'working'
    | 'episodic'
    | 'semantic'
    | 'procedural'
    | 'decision'
    | 'strategic';
  /** 0-based rung, for ordering and the ladder UI. */
  rank: 0 | 1 | 2 | 3 | 4 | 5;
}

const LAYERS: Record<LayerName, LayerVisual> = {
  Working: {
    name: 'Working',
    tint: 'blue',
    color: 'var(--blue)',
    soft: 'var(--blue-soft)',
    glow: 'rgba(120, 169, 255, 0.4)',
    gradient: 'linear-gradient(135deg, rgba(120, 169, 255, 0.13), transparent 68%)',
    icon: Zap,
    code: 'WK',
    key: 'working',
    rank: 0,
  },
  Episodic: {
    name: 'Episodic',
    tint: 'cyan',
    color: 'var(--cyan)',
    soft: 'var(--cyan-soft)',
    glow: 'rgba(99, 216, 210, 0.4)',
    gradient: 'linear-gradient(135deg, rgba(99, 216, 210, 0.13), transparent 68%)',
    icon: Inbox,
    code: 'EP',
    key: 'episodic',
    rank: 1,
  },
  Semantic: {
    name: 'Semantic',
    tint: 'mint',
    color: 'var(--mint)',
    soft: 'var(--mint-soft)',
    glow: 'rgba(117, 212, 161, 0.4)',
    gradient: 'linear-gradient(135deg, rgba(117, 212, 161, 0.13), transparent 68%)',
    icon: BookOpen,
    code: 'SM',
    key: 'semantic',
    rank: 2,
  },
  Procedural: {
    name: 'Procedural',
    tint: 'steel',
    color: 'var(--steel)',
    soft: 'var(--steel-soft)',
    glow: 'rgba(147, 197, 253, 0.4)',
    gradient: 'linear-gradient(135deg, rgba(147, 197, 253, 0.13), transparent 68%)',
    icon: ListOrdered,
    code: 'PR',
    key: 'procedural',
    rank: 3,
  },
  Decision: {
    name: 'Decision',
    tint: 'periwinkle',
    color: 'var(--periwinkle)',
    soft: 'var(--periwinkle-soft)',
    glow: 'rgba(169, 156, 248, 0.4)',
    gradient: 'linear-gradient(135deg, rgba(169, 156, 248, 0.13), transparent 68%)',
    icon: GitBranch,
    code: 'DC',
    key: 'decision',
    rank: 4,
  },
  Strategic: {
    name: 'Strategic',
    tint: 'gold',
    color: 'var(--gold)',
    soft: 'var(--gold-soft)',
    glow: 'rgba(221, 187, 101, 0.4)',
    gradient: 'linear-gradient(135deg, rgba(221, 187, 101, 0.13), transparent 68%)',
    icon: Gem,
    code: 'ST',
    key: 'strategic',
    rank: 5,
  },
};

/** Ladder order. Drives the legend, the strata bar and the filter chips. */
export const LAYER_ORDER: readonly LayerName[] = [
  'Working',
  'Episodic',
  'Semantic',
  'Procedural',
  'Decision',
  'Strategic',
];

/** Every layer in ladder order, ready to map over. */
export const LAYER_LIST: readonly LayerVisual[] = LAYER_ORDER.map((n) => LAYERS[n]);

/**
 * Legacy ladder names that the V18 migration rewrote in the database. A stale
 * row can still carry them, so the resolver maps them onto their modern
 * cognitive slots before falling back.
 */
const LEGACY_REMAP: Record<string, LayerName> = {
  Raw: 'Episodic',
  Knowledge: 'Semantic',
  Wisdom: 'Strategic',
};

/**
 * Resolve a backend layer string to its visuals.
 *
 * `Memory.layer` is a plain `string`, so an unknown or missing value is a real
 * possibility — including legacy ladder names (Raw/Knowledge/Wisdom) that the
 * V18 migration should have remapped but a stale row could still carry.
 * Falling back to `Episodic` (raw capture is an event) keeps the card painted
 * instead of punching a colourless hole into the grid.
 */
export function layerVisual(layer: string | null | undefined): LayerVisual {
  if (!layer) return LAYERS.Episodic;
  return LAYERS[layer as LayerName] ?? LAYERS[LEGACY_REMAP[layer]] ?? LAYERS.Episodic;
}

/**
 * Custom properties a layered surface needs.
 *
 * Spread onto `style` so descendants inherit the accent through
 * `var(--layer-color)` rather than every child taking an explicit prop.
 */
export function layerVars(layer: string | null | undefined): CSSProperties {
  const v = layerVisual(layer);
  return {
    '--layer-color': v.color,
    '--layer-soft': v.soft,
    '--layer-glow': v.glow,
    '--layer-gradient': v.gradient,
  } as CSSProperties;
}

/** Locale key for one of the two explanations. */
export function layerKey(layer: string | null | undefined, field: 'meaning' | 'promotes'): string {
  return `layer.${layerVisual(layer).key}.${field}`;
}
