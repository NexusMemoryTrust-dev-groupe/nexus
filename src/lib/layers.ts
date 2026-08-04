/**
 * Memory layer identity and semantics — single source of truth.
 *
 * Raw → Knowledge → Decision → Wisdom is a maturation ladder: a memory starts as
 * whatever was captured and climbs as it is confirmed, reasoned about and
 * generalised. The palette climbs with it — cool and unprocessed at the bottom,
 * warm and settled at the top — so a wall of cards shows the shape of the
 * collection before a single title is read.
 *
 * The layer name alone told the user nothing. `Wisdom` is not self-explanatory,
 * and nothing on screen said what earns it. So each rung now carries locale key
 * stems for three questions the UI answers inline:
 *
 *   meaning  — what this layer *is*
 *   promotes — what moves a memory to the next rung
 *
 * Copy lives in `localeStore`, not here, so `en` and `ru` stay in one place and
 * this module keeps no strings to translate.
 */

import type { CSSProperties } from 'react';
import type { LucideIcon } from 'lucide-react';
import { BookOpen, GitBranch, Gem, Inbox } from 'lucide-react';

export type LayerName = 'Raw' | 'Knowledge' | 'Decision' | 'Wisdom';

export interface LayerVisual {
  /** Canonical name, safe to render. */
  name: LayerName;
  /** Token stem: `var(--{tint})` and `var(--{tint}-soft)` both resolve. */
  tint: 'blue' | 'cyan' | 'periwinkle' | 'gold';
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
  /** Locale key stem: `layer.raw.meaning`, `.promotes`. */
  key: 'raw' | 'knowledge' | 'decision' | 'wisdom';
  /** 0-based rung, for ordering and the ladder UI. */
  rank: 0 | 1 | 2 | 3;
}

const LAYERS: Record<LayerName, LayerVisual> = {
  Raw: {
    name: 'Raw',
    tint: 'blue',
    color: 'var(--blue)',
    soft: 'var(--blue-soft)',
    glow: 'rgba(120, 169, 255, 0.4)',
    gradient: 'linear-gradient(135deg, rgba(120, 169, 255, 0.13), transparent 68%)',
    icon: Inbox,
    code: 'RW',
    key: 'raw',
    rank: 0,
  },
  Knowledge: {
    name: 'Knowledge',
    tint: 'cyan',
    color: 'var(--cyan)',
    soft: 'var(--cyan-soft)',
    glow: 'rgba(99, 216, 210, 0.4)',
    gradient: 'linear-gradient(135deg, rgba(99, 216, 210, 0.13), transparent 68%)',
    icon: BookOpen,
    code: 'KN',
    key: 'knowledge',
    rank: 1,
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
    rank: 2,
  },
  Wisdom: {
    name: 'Wisdom',
    tint: 'gold',
    color: 'var(--gold)',
    soft: 'var(--gold-soft)',
    glow: 'rgba(221, 187, 101, 0.4)',
    gradient: 'linear-gradient(135deg, rgba(221, 187, 101, 0.13), transparent 68%)',
    icon: Gem,
    code: 'WS',
    key: 'wisdom',
    rank: 3,
  },
};

/** Ladder order. Drives the legend, the strata bar and the filter chips. */
export const LAYER_ORDER: readonly LayerName[] = ['Raw', 'Knowledge', 'Decision', 'Wisdom'];

/** Every layer in ladder order, ready to map over. */
export const LAYER_LIST: readonly LayerVisual[] = LAYER_ORDER.map((n) => LAYERS[n]);

/**
 * Resolve a backend layer string to its visuals.
 *
 * `Memory.layer` is a plain `string`, so an unknown or missing value is a real
 * possibility. Falling back to `Raw` keeps the card painted instead of punching
 * a colourless hole into the grid.
 */
export function layerVisual(layer: string | null | undefined): LayerVisual {
  if (!layer) return LAYERS.Raw;
  return LAYERS[layer as LayerName] ?? LAYERS.Raw;
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

/** Locale key for one of the three explanations. */
export function layerKey(layer: string | null | undefined, field: 'meaning' | 'promotes'): string {
  return `layer.${layerVisual(layer).key}.${field}`;
}
