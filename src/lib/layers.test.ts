import { describe, it, expect } from 'vitest';
import {
  LAYER_LIST,
  LAYER_ORDER,
  layerKey,
  layerVars,
  layerVisual,
} from './layers';

// ── Ladder shape ─────────────────────────────────────────────────────────────

describe('layer ladder', () => {
  it('has exactly six cognitive layers in maturation order', () => {
    expect(LAYER_ORDER).toEqual([
      'Working',
      'Episodic',
      'Semantic',
      'Procedural',
      'Decision',
      'Strategic',
    ]);
  });

  it('keeps rank, list and order mutually consistent', () => {
    expect(LAYER_LIST).toHaveLength(6);
    LAYER_LIST.forEach((layer, index) => {
      expect(layer.rank).toBe(index);
      expect(LAYER_ORDER[index]).toBe(layer.name);
    });
  });

  it('assigns every layer a unique two-letter code', () => {
    const codes = new Set(LAYER_LIST.map((layer) => layer.code));
    expect(codes.size).toBe(6);
    expect(LAYER_LIST.map((layer) => layer.code)).toEqual([
      'WK', 'EP', 'SM', 'PR', 'DC', 'ST',
    ]);
  });

  it('gives every layer visuals that resolve through layerVars', () => {
    for (const layer of LAYER_LIST) {
      const vars = layerVars(layer.name) as Record<string, string>;
      expect(vars['--layer-color']).toBe(layer.color);
      expect(vars['--layer-soft']).toBe(layer.soft);
      expect(vars['--layer-glow']).toBe(layer.glow);
      expect(vars['--layer-gradient']).toBe(layer.gradient);
    }
  });
});

// ── Resolution and fallback ──────────────────────────────────────────────────

describe('layerVisual fallback', () => {
  it('resolves every canonical layer name to itself', () => {
    for (const name of LAYER_ORDER) {
      expect(layerVisual(name).name).toBe(name);
    }
  });

  it('falls back to Episodic on missing or unknown values', () => {
    expect(layerVisual(null).name).toBe('Episodic');
    expect(layerVisual(undefined).name).toBe('Episodic');
    expect(layerVisual('').name).toBe('Episodic');
    expect(layerVisual('nonsense').name).toBe('Episodic');
  });

  it('maps legacy ladder names (V18 remap safety) onto their modern slots', () => {
    // The V18 migration rewrites these in the database; the fallback is only a
    // stale-row safety net, and must not punch a colourless hole into the grid.
    expect(layerVisual('Raw').name).toBe('Episodic');
    expect(layerVisual('Knowledge').name).toBe('Semantic');
    expect(layerVisual('Wisdom').name).toBe('Strategic');
  });
});

// ── Locale keys ──────────────────────────────────────────────────────────────

describe('layerKey', () => {
  it('builds meaning and promotes keys from the resolved layer', () => {
    expect(layerKey('Working', 'meaning')).toBe('layer.working.meaning');
    expect(layerKey('Strategic', 'promotes')).toBe('layer.strategic.promotes');
  });

  it('routes unknown layers through the same fallback as layerVisual', () => {
    expect(layerKey('nonsense', 'meaning')).toBe('layer.episodic.meaning');
    expect(layerKey(null, 'promotes')).toBe('layer.episodic.promotes');
  });
});
