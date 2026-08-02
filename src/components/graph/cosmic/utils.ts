import * as THREE from 'three';
import type { GraphNodeData } from './types';

/**
 * Position and label helpers shared by the scene and the DOM overlay.
 */

/** Orbit position of a satellite at time `t`, around `corePos` when given. */
export function getOrbitPos(
  n: GraphNodeData,
  t: number,
  corePos?: THREE.Vector3,
): THREE.Vector3 {
  const a = t * n.orbitSpeed + n.orbitOffset;
  const base = corePos || n.position;
  return new THREE.Vector3(
    base.x + Math.cos(a) * n.orbitRadius,
    base.y + Math.sin(a * 0.5) * n.orbitRadius * 0.3,
    base.z + Math.sin(a) * n.orbitRadius,
  );
}

/**
 * Resolve the live position of any node, core or satellite.
 *
 * Cores are driven by the physics loop and looked up by their own id;
 * satellites orbit whatever position their parent core currently holds, which
 * is what makes a dragged core carry its satellites with it.
 */
export function resolveNodePosition(
  n: GraphNodeData,
  t: number,
  corePositions: React.MutableRefObject<Map<string, THREE.Vector3>>,
): THREE.Vector3 {
  if (n.orbitRadius > 0 && n.coreId) {
    const corePos = corePositions.current.get(n.coreId);
    return getOrbitPos(n, t, corePos);
  }
  const livePos = corePositions.current.get(n.id);
  return livePos || n.position;
}

/**
 * Shorten a node title into something readable at graph scale.
 *
 * Truncation is done by *character*, never by byte: Cyrillic is two bytes per
 * character, so slicing a byte range would cut a character in half and render
 * a replacement glyph.
 */
export function formatNodeName(title: string, entityType: string): string {
  const lower = entityType.toLowerCase();
  const clip = (s: string, max: number) =>
    Array.from(s).length > max ? Array.from(s).slice(0, max - 2).join('') + '…' : s;

  // Files: keep the filename, ensure it reads like one.
  if (lower.includes('file')) {
    const parts = title.replace(/\\/g, '/').split('/');
    let name = parts[parts.length - 1] || title;
    if (!name.includes('.')) {
      const ext = lower.includes('config')
        ? '.config'
        : lower.includes('test')
          ? '.test'
          : '.file';
      name += ext;
    }
    return clip(name, 28);
  }

  // Code symbols: the identifier is the useful part, not the path.
  if (lower.includes('function') || lower.includes('method') || lower.includes('class')) {
    const parts = title.replace(/\\/g, '/').split('/');
    return clip(parts[parts.length - 1] || title, 28);
  }

  if (lower.includes('task')) {
    return clip(title, 25);
  }

  // Concepts and decisions: filler words crowd out the distinguishing phrase.
  if (lower.includes('concept') || lower.includes('decision')) {
    const cleaned = title
      .replace(
        /\b(implementation|concept|decision|approach|strategy|pattern|architecture)\b/gi,
        '',
      )
      .replace(/\s{2,}/g, ' ')
      .trim();
    if (cleaned.length > 3 && Array.from(cleaned).length <= 28) return cleaned;
    return clip(title, 28);
  }

  return clip(title, 28);
}
