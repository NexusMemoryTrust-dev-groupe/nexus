import { useCallback, useEffect, useLayoutEffect, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
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
 * the Working→Strategic ladder — and the previous UI printed those numbers as plain
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
 * The bubble is rendered through a React portal directly into `document.body`
 * and positioned with `position: fixed` viewport coordinates. This is the only
 * layout that is immune to every ancestor hazard: Radar/Team cards and heroes
 * use transforms, animations and `overflow: hidden` for their rounded corners
 * and ambient washes, and any of those turns an ancestor into a containing
 * block for `position: fixed` (or clips a descendant regardless of z-index).
 * With the bubble outside the component tree, the trigger measures itself in
 * *viewport* coordinates (getBoundingClientRect is already viewport-relative)
 * and the bubble is placed with those exact pixels — always directly under the
 * question mark, never shifted, never clipped, never affecting layout.
 *
 * The reveal is JS-driven, not a pure `:hover` rule: the bubble mounts hidden,
 * is measured with `useLayoutEffect` (before paint, so no flash at wrong
 * coordinates), and only then gets `.is-open`. It closes on leave, blur,
 * Escape, scroll and resize.
 */
export function InfoTip({ text, label }: { text: string; label?: string }) {
  const { t } = useLocale();
  const buttonRef = useRef<HTMLButtonElement>(null);
  const tipRef = useRef<HTMLDivElement>(null);
  const [open, setOpen] = useState(false);
  const [coords, setCoords] = useState<{ x: number; y: number; flip: 'below' | 'above'; arrow: number } | null>(null);

  // Measure once the bubble exists in the portal. It mounts with
  // visibility:hidden (so it is measurable but invisible), layout is computed
  // before paint, and only then do we reveal it with real coordinates.
  useLayoutEffect(() => {
    if (!open) return;
    const button = buttonRef.current;
    const tip = tipRef.current;
    if (!button || !tip) return;

    const anchor = button.getBoundingClientRect();
    const width = tip.offsetWidth;
    const height = tip.offsetHeight;
    const margin = 10;

    // Centre under the trigger, then pull back inside the viewport if that
    // would overhang either edge.
    const half = width / 2;
    const centre = anchor.left + anchor.width / 2;
    const clamped = Math.min(
      Math.max(centre, margin + half),
      window.innerWidth - margin - half,
    );

    // Below by default. Flips above only when there is genuinely no room
    // underneath, which beats rendering the text off-screen.
    const below = anchor.bottom + margin;
    const fitsBelow = below + height <= window.innerHeight - margin;

    setCoords({
      x: clamped,
      y: fitsBelow ? below : anchor.top - margin - height,
      flip: fitsBelow ? 'below' : 'above',
      // The arrow tracks the trigger horizontally so the bubble never points
      // at empty space after clamping.
      arrow: clamped - anchor.left - anchor.width / 2,
    });
  }, [open]);

  const hide = useCallback(() => {
    setOpen(false);
    setCoords(null);
  }, []);

  // Any of these should dismiss the tooltip: leaving, blur, Escape, scrolling
  // or resizing (after which the old coordinates are stale anyway).
  useEffect(() => {
    if (!open) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') hide();
    };
    const onMove = () => hide();
    window.addEventListener('keydown', onKey);
    window.addEventListener('scroll', onMove, true);
    window.addEventListener('resize', onMove);
    return () => {
      window.removeEventListener('keydown', onKey);
      window.removeEventListener('scroll', onMove, true);
      window.removeEventListener('resize', onMove);
    };
  }, [open, hide]);

  return (
    <>
      <button
        ref={buttonRef}
        type="button"
        className={`st-info${open ? ' is-open' : ''}`}
        aria-label={label ?? t('inst.explain')}
        aria-expanded={open}
        onMouseEnter={() => setOpen(true)}
        onFocus={() => setOpen(true)}
        onMouseLeave={hide}
        onBlur={hide}
      >
        <span className="st-info-mark" aria-hidden="true">?</span>
      </button>
      {open && createPortal(
        <div
          ref={tipRef}
          className={`st-tip${coords ? ' is-open' : ''}`}
          role="tooltip"
          data-flip={coords?.flip ?? 'below'}
          style={
            {
              left: coords ? `${coords.x}px` : 0,
              top: coords ? `${coords.y}px` : 0,
              '--tip-arrow': coords ? `${coords.arrow}px` : '0px',
            } as CSSProperties
          }
        >
          {text}
        </div>,
        document.body,
      )}
    </>
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

/** All six definitions stay visible: layer meaning must never require memory. */
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

/**
 * Custom dropdown. The platform `<select>` is unstylable beyond the trigger
 * itself — its listbox pops up in the OS chrome with a foreign font and look.
 * This renders the same semantics (listbox / option) with the app's typography
 * and a smooth pop, and closes on outside click, Escape, blur or scroll.
 */
export function StrataSelect({
  value,
  onChange,
  options,
  ariaLabel,
  onClose,
}: {
  value: string;
  onChange: (value: string) => void;
  options: { value: string; label: string }[];
  ariaLabel: string;
  /** Called when the dropdown closes without committing a value (outside click, Escape, scroll). */
  onClose?: () => void;
}) {
  const [open, setOpen] = useState(false);
  const rootRef = useRef<HTMLDivElement>(null);
  const selected = options.find((option) => option.value === value) ?? options[0];

  const close = useCallback(() => {
    setOpen(false);
    onClose?.();
  }, [onClose]);

  useEffect(() => {
    if (!open) return;
    const onPointerDown = (event: MouseEvent) => {
      if (!rootRef.current?.contains(event.target as Node)) close();
    };
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') close();
    };
    // Scrolling the menu itself (it is a scrollable listbox) must not dismiss
    // it — only scrolls outside the select close the dropdown.
    const onScroll = (event: Event) => {
      const target = event.target as Node | null;
      if (target && rootRef.current?.contains(target)) return;
      close();
    };
    document.addEventListener('mousedown', onPointerDown);
    document.addEventListener('keydown', onKeyDown);
    document.addEventListener('scroll', onScroll, true);
    return () => {
      document.removeEventListener('mousedown', onPointerDown);
      document.removeEventListener('keydown', onKeyDown);
      document.removeEventListener('scroll', onScroll, true);
    };
  }, [open, close]);

  return (
    <div className="st-select" ref={rootRef}>
      <button
        type="button"
        className="st-select-trigger"
        aria-haspopup="listbox"
        aria-expanded={open}
        aria-label={ariaLabel}
        onClick={() => setOpen((previous) => !previous)}
      >
        <span className="st-select-value">{selected?.label ?? value}</span>
        <span className={`st-select-chevron${open ? ' st-select-chevron--open' : ''}`} aria-hidden="true">
          ▾
        </span>
      </button>
      {open && (
        <ul className="st-select-menu" role="listbox" aria-label={ariaLabel}>
          {options.map((option) => (
            <li key={option.value} role="option" aria-selected={option.value === value}>
              <button
                type="button"
                className={`st-select-option${option.value === value ? ' is-selected' : ''}`}
                onClick={() => {
                  onChange(option.value);
                  setOpen(false);
                }}
              >
                {option.label}
              </button>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}

/**
 * App-modal dialog rendered through a portal into `document.body`. This is the
 * replacement for every native `prompt`/`confirm`: same fixed positioning, a
 * blurred backdrop, Escape / backdrop / ✕ to close, and the app's own chrome —
 * nothing foreign, nothing in the OS window chrome.
 */
export function StrataModal({
  title,
  icon: Icon,
  accent = 'var(--tangerine)',
  onClose,
  children,
  footer,
}: {
  title: string;
  icon?: LucideIcon;
  accent?: string;
  onClose: () => void;
  children: ReactNode;
  footer?: ReactNode;
}) {
  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key === 'Escape') onClose();
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [onClose]);

  return createPortal(
    <div
      className="st-modal"
      onMouseDown={(event) => {
        if (event.target === event.currentTarget) onClose();
      }}
    >
      <div className="st-modal-panel" role="dialog" aria-modal="true" aria-label={title}>
        <div className="st-modal-head">
          <h3 className="st-modal-title" style={{ '--section-color': accent } as CSSProperties}>
            {Icon && <Icon size={14} />} {title}
          </h3>
          <button type="button" className="st-modal-x" aria-label="Close" onClick={onClose}>
            ✕
          </button>
        </div>
        <div className="st-modal-body">{children}</div>
        {footer && <div className="st-modal-foot">{footer}</div>}
      </div>
    </div>,
    document.body,
  );
}
