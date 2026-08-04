import { useCallback } from 'react';
import type { CSSProperties, ReactNode } from 'react';
import type { LucideIcon } from 'lucide-react';
import { LAYER_LIST, layerKey, layerVars, layerVisual } from '../../lib/layers';
import type { Freshness } from '../../lib/format';
import { steps, unit } from '../../lib/format';
import { useCountUp } from '../../hooks/useMotion';
import { useLocale } from '../../stores/localeStore';

/**
 * Shared readouts for the three content pages.
 *
 * A memory is a measured object — it has a confidence, an importance, a rung on
 * the Raw→Wisdom ladder — and the previous UI printed those numbers as plain
 * text next to a label. These components render each measurement as an
 * instrument instead, and because Memories, Timeline and Context all import the
 * same ones, a confidence of 0.5 looks identical wherever it appears.
 *
 * Every animated component honours `prefers-reduced-motion`: the CSS cancels
 * the entrance, and `useCountUp` skips straight to the final value.
 */

// Shared Strata / Assembly primitives. Legacy `nx-*` instruments were removed
// with the old page stylesheet; every content page now consumes this single
// semantic vocabulary.

/**
 * Plain-language explanation attached to a technical label.
 *
 * Two details are load-bearing.
 *
 * The mark is a text glyph, not `HelpCircle`. That icon draws its own ring, so
 * inside a bordered circle it rendered as a circle within a circle and the
 * question mark inherited the icon's internal padding — reading as off-centre no
 * matter what the flex box did. A bare `?` centred by grid is exactly centred.
 *
 * The bubble is positioned on hover from JS because CSS alone cannot save it.
 * These triggers sit inside `.st-ask`, `.st-day` and `.st-hero`, all of which
 * need `overflow: hidden` for their own rounded corners and ambient washes — and
 * an ancestor's overflow clips a descendant regardless of `z-index` or
 * `position: absolute`. Only a fixed-position element escapes the clip, and a
 * fixed element cannot be placed relative to its parent, so the trigger measures
 * itself and hands the bubble viewport coordinates.
 */
export function InfoTip({ text, label }: { text: string; label?: string }) {
  const { t } = useLocale();

  const place = useCallback((event: React.SyntheticEvent<HTMLButtonElement>) => {
    const button = event.currentTarget;
    const tip = button.querySelector<HTMLElement>('.st-tip');
    if (!tip) return;

    const anchor = button.getBoundingClientRect();
    const width = tip.offsetWidth;
    const height = tip.offsetHeight;
    const margin = 10;

    // Centre under the trigger, then pull back inside the viewport if that would
    // overhang either edge.
    const half = width / 2;
    const centre = anchor.left + anchor.width / 2;
    const clamped = Math.min(
      Math.max(centre, margin + half),
      window.innerWidth - margin - half,
    );

    // Below by default, as asked. Flips above only when there is genuinely no
    // room underneath, which beats rendering the text off-screen.
    const below = anchor.bottom + margin;
    const fitsBelow = below + height <= window.innerHeight - margin;

    tip.style.setProperty('--tip-x', `${clamped}px`);
    tip.style.setProperty('--tip-y', `${fitsBelow ? below : anchor.top - margin - height}px`);
    tip.dataset.flip = fitsBelow ? 'below' : 'above';
  }, []);

  return (
    <button
      type="button"
      className="st-info"
      aria-label={label ?? t('inst.explain')}
      onMouseEnter={place}
      onFocus={place}
    >
      <span className="st-info-mark" aria-hidden="true">?</span>
      <span className="st-tip" role="tooltip">{text}</span>
    </button>
  );
}

/**
 * Confidence signal. A continuous value gets a continuous ring; it is never
 * reused for importance, which is a rank and therefore uses discrete blocks.
 */
export function SignalRing({
  value,
  label,
  color = 'var(--periwinkle)',
  soft = 'var(--periwinkle-soft)',
  size = 58,
  explain,
}: {
  value: number;
  label: string;
  color?: string;
  soft?: string;
  size?: number;
  explain?: string;
}) {
  const safe = unit(value);
  const shown = useCountUp(Math.round(safe * 100));
  const ring = (
    <span
      className="st-ring-wrap"
      style={{
        '--ring-size': `${size}px`,
        '--ring-color': color,
        '--ring-soft': soft,
      } as CSSProperties}
    >
      <span
        className="st-ring"
        style={{ '--st-ring': safe, '--ring-value': safe } as CSSProperties}
        role="meter"
        aria-label={label}
        aria-valuemin={0}
        aria-valuemax={100}
        aria-valuenow={Math.round(safe * 100)}
      />
      <span className="st-ring-value">{shown}</span>
      <span className="st-ring-label">{label}</span>
    </span>
  );

  return explain ? (
    <span style={{ display: 'inline-flex', alignItems: 'center', gap: 7 }}>
      {ring}
      <InfoTip text={explain} />
    </span>
  ) : ring;
}

/** Importance as one to five stable blocks. */
export function ImpactBlocks({
  value,
  color = 'var(--tangerine)',
  label,
}: {
  value: number;
  color?: string;
  label: string;
}) {
  const active = steps(value);
  return (
    <span
      className="st-blocks"
      style={{ '--block-color': color } as CSSProperties}
      role="meter"
      aria-label={label}
      aria-valuemin={1}
      aria-valuemax={5}
      aria-valuenow={active}
    >
      {[1, 2, 3, 4, 5].map((n) => (
        <span key={n} className={`st-block${n <= active ? ' on' : ''}`} />
      ))}
    </span>
  );
}

/** The only looping motion on a memory: recent capture, not decoration. */
export function FreshPulse({
  state,
  label,
  hint,
  color = 'var(--mint)',
}: {
  state: Freshness;
  label: string;
  hint?: string;
  color?: string;
}) {
  const dot = (
    <span
      className={`st-pulse${state === 'fresh' ? '' : ' is-still'}`}
      style={{ '--pulse-color': color } as CSSProperties}
      aria-hidden="true"
    />
  );
  return hint ? (
    // This component frequently lives inside a clickable memory tile. A nested
    // InfoTip button would create invalid button-inside-button markup, so the
    // compact card affordance uses the native title while full explanatory
    // surfaces use InfoTip directly.
    <span style={{ display: 'inline-flex', alignItems: 'center', gap: 6 }} title={hint}>
      {dot}
      <span>{label}</span>
    </span>
  ) : (
    <span style={{ display: 'inline-flex', alignItems: 'center', gap: 6 }}>
      {dot}
      <span>{label}</span>
    </span>
  );
}

/** Layer icon. Colour and icon always come from the same source of truth. */
export function LayerGlyph({ layer, size = 34 }: { layer: string; size?: number }) {
  const visual = layerVisual(layer);
  const Icon = visual.icon;
  return (
    <span
      className="st-glyph"
      style={{ ...layerVars(layer), '--glyph-size': `${size}px` } as CSSProperties}
      aria-label={visual.name}
    >
      <Icon size={Math.round(size * .42)} strokeWidth={1.8} />
    </span>
  );
}

export function SemanticLayerTag({ layer }: { layer: string }) {
  const visual = layerVisual(layer);
  const Icon = visual.icon;
  return (
    <span className="st-layer-tag" style={layerVars(layer)}>
      <Icon size={12} />
      {visual.name}
    </span>
  );
}

/** The four definitions stay visible: layer meaning must never require memory. */
export function LayerLegend() {
  const { t } = useLocale();
  return (
    <div className="st-legend">
      {LAYER_LIST.map((layer) => {
        const Icon = layer.icon;
        return (
          <div
            key={layer.name}
            className="st-legend-item"
            style={layerVars(layer.name)}
          >
            <div className="st-legend-head">
              <Icon size={12} />
              {layer.name}
            </div>
            <div className="st-legend-text">{t(layerKey(layer.name, 'meaning'))}</div>
            <span className="st-legend-next">{t(layerKey(layer.name, 'promotes'))}</span>
          </div>
        );
      })}
    </div>
  );
}

export function StrataBar({
  counts,
  total,
  active,
  onToggle,
}: {
  counts: Map<string, number>;
  total: number;
  active: Set<string>;
  onToggle: (layer: string) => void;
}) {
  const { t } = useLocale();
  return (
    <div className="st-strata" role="group" aria-label={t('mem.strata.title')}>
      {LAYER_LIST.map((layer) => {
        const count = counts.get(layer.name) ?? 0;
        const Icon = layer.icon;
        return (
          <button
            key={layer.name}
            type="button"
            className="st-strata-segment"
            style={{
              ...layerVars(layer.name),
              '--segment-share': Math.max(count / Math.max(total, 1) * 10, .75),
            } as CSSProperties}
            aria-pressed={active.has(layer.name)}
            disabled={count === 0}
            onClick={() => onToggle(layer.name)}
          >
            <span className="st-strata-top">
              <Icon size={12} />
              <span className="st-strata-name">{layer.name}</span>
              <span className="st-strata-count">{count}</span>
            </span>
            <span className="st-strata-meaning">{t(layerKey(layer.name, 'meaning'))}</span>
          </button>
        );
      })}
    </div>
  );
}

export interface HeroStat {
  label: string;
  value: string;
  color?: string;
}

export function PageHero({
  kicker,
  title,
  copy,
  stats = [],
  accent = 'var(--tangerine)',
  secondary = 'var(--periwinkle)',
}: {
  kicker: string;
  title: string;
  copy: string;
  stats?: HeroStat[];
  accent?: string;
  secondary?: string;
}) {
  return (
    <header
      className="st-hero"
      style={{ '--st-accent': accent, '--st-secondary': secondary } as CSSProperties}
    >
      <div className="st-hero-main">
        <div className="st-hero-kicker">{kicker}</div>
        <h1 className="st-hero-title">{title}</h1>
        <p className="st-hero-copy">{copy}</p>
      </div>
      {stats.length > 0 && (
        <div className="st-hero-stats">
          {stats.map((stat) => (
            <div key={stat.label} className="st-hero-stat">
              <span
                className="st-hero-stat-value"
                style={{ '--stat-color': stat.color ?? 'var(--bone)' } as CSSProperties}
              >
                {stat.value}
              </span>
              <span className="st-hero-stat-label">{stat.label}</span>
            </div>
          ))}
        </div>
      )}
    </header>
  );
}

export function StrataVoid({
  icon: Icon,
  title,
  children,
  accent = 'var(--tangerine)',
}: {
  icon: LucideIcon;
  title: string;
  children?: ReactNode;
  accent?: string;
}) {
  return (
    <div className="st-void" style={{ '--st-accent': accent } as CSSProperties}>
      <span className="st-void-icon"><Icon size={25} /></span>
      <div className="st-void-title">{title}</div>
      {children && <div className="st-void-copy">{children}</div>}
    </div>
  );
}

export function StrataAlert({ icon: Icon, children }: { icon: LucideIcon; children: ReactNode }) {
  return (
    <div className="st-alert" role="alert">
      <Icon size={14} />
      <div>{children}</div>
    </div>
  );
}

export function StrataSkeletons() {
  return (
    <div className="st-skeleton-grid" aria-hidden="true">
      {[0, 1, 2, 3, 4, 5].map((i) => (
        <div className="st-skeleton" key={i}>
          <div className="st-skeleton-bar" style={{ width: '35%' }} />
          <div className="st-skeleton-bar" style={{ width: '82%', height: 11 }} />
          <div className="st-skeleton-bar" style={{ width: '65%' }} />
        </div>
      ))}
    </div>
  );
}
