/**
 * Display formatting for dates, sizes and counts.
 *
 * The pages used to call `toLocaleDateString('en-US')` with the tag hardcoded,
 * so a Russian UI printed English months. Everything here takes the app locale
 * explicitly instead of reading a global, which keeps the functions pure and
 * testable.
 *
 * Timestamps arrive from the backend as strings and a malformed one is a real
 * possibility, so every entry point checks for an invalid date and returns a
 * neutral dash rather than rendering "Invalid Date" into the layout.
 */

export type Loc = 'en' | 'ru';

const TAGS: Record<Loc, string> = { en: 'en-US', ru: 'ru-RU' };

/** Placeholder for anything unparseable. */
const NIL = '—';

const DAY_MS = 86_400_000;

function parse(iso: string | null | undefined): Date | null {
  if (!iso) return null;
  const d = new Date(iso);
  return Number.isNaN(d.getTime()) ? null : d;
}

/** Local midnight, so "same day" comparisons ignore the clock. */
function startOfDay(d: Date): number {
  return new Date(d.getFullYear(), d.getMonth(), d.getDate()).getTime();
}

/** Whole days between two dates, positive when `then` is in the past. */
function daysApart(then: Date, now: Date): number {
  return Math.round((startOfDay(now) - startOfDay(then)) / DAY_MS);
}

// ── Grouping ────────────────────────────────────────────────────────────────

/**
 * Stable grouping key, built from local calendar fields.
 *
 * `toISOString()` would shift the day for anyone east or west of UTC and split
 * a single evening's work across two timeline groups.
 */
export function dayKey(iso: string): string {
  const d = parse(iso);
  if (!d) return 'unknown';
  const m = String(d.getMonth() + 1).padStart(2, '0');
  const day = String(d.getDate()).padStart(2, '0');
  return `${d.getFullYear()}-${m}-${day}`;
}

/** `2026-08-03` → local `Date` at midnight. Inverse of `dayKey`. */
export function keyToDate(key: string): Date | null {
  const [y, m, d] = key.split('-').map(Number);
  if (!y || !m || !d) return null;
  return new Date(y, m - 1, d);
}

/** Days between two `dayKey` values. */
export function keySpan(a: string, b: string): number {
  const da = keyToDate(a);
  const db = keyToDate(b);
  if (!da || !db) return 0;
  return Math.abs(Math.round((da.getTime() - db.getTime()) / DAY_MS));
}

// ── Labels ──────────────────────────────────────────────────────────────────

/** Full date, e.g. `3 августа 2026` / `August 3, 2026`. */
export function dayLabel(iso: string, loc: Loc): string {
  const d = parse(iso);
  if (!d) return NIL;
  return new Intl.DateTimeFormat(TAGS[loc], {
    day: 'numeric', month: 'long', year: 'numeric',
  }).format(d);
}

/** Compact date for dense rows, e.g. `3 авг` / `Aug 3`. */
export function dayShort(iso: string, loc: Loc): string {
  const d = parse(iso);
  if (!d) return NIL;
  return new Intl.DateTimeFormat(TAGS[loc], { day: 'numeric', month: 'short' }).format(d);
}

/** Weekday, e.g. `понедельник` / `Monday`. */
export function weekday(iso: string, loc: Loc, short = false): string {
  const d = parse(iso);
  if (!d) return NIL;
  return new Intl.DateTimeFormat(TAGS[loc], { weekday: short ? 'short' : 'long' }).format(d);
}

/**
 * `Today` / `Yesterday` when it applies, otherwise the weekday for the past
 * week, otherwise nothing.
 *
 * Returned separately from `dayLabel` so the timeline can show both without
 * splitting a combined string apart again. The literals are here rather than in
 * `localeStore` because they are date grammar, not UI copy.
 */
export function dayTag(iso: string, loc: Loc): string | null {
  const d = parse(iso);
  if (!d) return null;
  const diff = daysApart(d, new Date());
  if (diff === 0) return loc === 'ru' ? 'Сегодня' : 'Today';
  if (diff === 1) return loc === 'ru' ? 'Вчера' : 'Yesterday';
  if (diff > 0 && diff < 7) return weekday(iso, loc);
  return null;
}

/** Wall clock, 24h in both locales — this is a tool, not a consumer app. */
export function clock(iso: string, loc: Loc): string {
  const d = parse(iso);
  if (!d) return NIL;
  return new Intl.DateTimeFormat(TAGS[loc], {
    hour: '2-digit', minute: '2-digit', hour12: false,
  }).format(d);
}

/**
 * Coarse "how long ago". Deliberately low-resolution: on a card the difference
 * between 41 and 43 minutes is noise, and a precise figure would imply the list
 * re-renders every minute, which it does not.
 */
export function ago(iso: string, loc: Loc): string {
  const d = parse(iso);
  if (!d) return NIL;

  const rtf = new Intl.RelativeTimeFormat(TAGS[loc], { numeric: 'auto' });
  const secs = Math.round((d.getTime() - Date.now()) / 1000);
  const mag = Math.abs(secs);

  if (mag < 45) return loc === 'ru' ? 'только что' : 'just now';
  if (mag < 3600) return rtf.format(Math.round(secs / 60), 'minute');
  if (mag < DAY_MS / 1000) return rtf.format(Math.round(secs / 3600), 'hour');

  const days = daysApart(d, new Date());
  if (Math.abs(days) < 30) return rtf.format(-days, 'day');
  if (Math.abs(days) < 365) return rtf.format(Math.round(-days / 30), 'month');
  return rtf.format(Math.round(-days / 365), 'year');
}

/** Absolute date and time, for footers and tooltips where precision is the point. */
export function stamp(iso: string, loc: Loc): string {
  const d = parse(iso);
  if (!d) return NIL;
  return new Intl.DateTimeFormat(TAGS[loc], {
    day: '2-digit', month: '2-digit', year: 'numeric',
    hour: '2-digit', minute: '2-digit', hour12: false,
  }).format(d);
}

// ── Freshness ───────────────────────────────────────────────────────────────

export type Freshness = 'fresh' | 'recent' | 'settled';

/**
 * Age band of a memory.
 *
 * This drives the only looping animation in the UI: `fresh` pulses, the other
 * two do not. That is the whole point of the split — if every card pulsed, the
 * motion would be decoration and would cost frames for nothing. Because only
 * the last day's captures move, the movement itself carries the information.
 */
export function freshness(iso: string): Freshness {
  const d = parse(iso);
  if (!d) return 'settled';
  const age = Date.now() - d.getTime();
  if (age < DAY_MS) return 'fresh';
  if (age < DAY_MS * 7) return 'recent';
  return 'settled';
}

/**
 * Position within the day as 0–1, for placing a dot on a 24-hour axis.
 *
 * Minutes are included, not just the hour: two captures in the same hour should
 * not stack on the same pixel.
 */
export function dayFraction(iso: string): number {
  const d = parse(iso);
  if (!d) return 0;
  return (d.getHours() * 60 + d.getMinutes()) / 1440;
}

// ── Numbers ─────────────────────────────────────────────────────────────────

/**
 * Byte count in binary units. `AttachedFile.sizeBytes` is what the backend
 * reports, and every file manager the app ships beside uses 1024.
 */
export function bytes(n: number): string {
  if (!Number.isFinite(n) || n < 0) return NIL;
  if (n < 1024) return `${n} B`;
  const units = ['KB', 'MB', 'GB', 'TB'];
  let v = n / 1024;
  let i = 0;
  while (v >= 1024 && i < units.length - 1) {
    v /= 1024;
    i += 1;
  }
  return `${v < 10 ? v.toFixed(1) : Math.round(v)} ${units[i]}`;
}

/** Thousands separators, e.g. for token totals. */
export function num(n: number, loc: Loc): string {
  if (!Number.isFinite(n)) return NIL;
  return new Intl.NumberFormat(TAGS[loc]).format(n);
}

/** Short form for axis ticks and badges: `1.2k`, `18k`. */
export function compact(n: number, loc: Loc): string {
  if (!Number.isFinite(n)) return NIL;
  return new Intl.NumberFormat(TAGS[loc], { notation: 'compact', maximumFractionDigits: 1 }).format(n);
}

/** `0.87` → `87`. Scores arrive as 0–1 and are shown as whole percents. */
export function pct(v: number): number {
  if (!Number.isFinite(v)) return 0;
  return Math.round(Math.min(Math.max(v, 0), 1) * 100);
}

/** Clamp to 0–1. Guards every score that reaches a width or an angle. */
export function unit(v: number): number {
  if (!Number.isFinite(v)) return 0;
  return Math.min(Math.max(v, 0), 1);
}

/**
 * Importance as one of five steps.
 *
 * Importance is shown as five discrete blocks rather than a bar, because it is
 * read as a rank — "how much does this matter" — while confidence is read as a
 * proportion. Rendering both as identical bars was why the two were constantly
 * confused for each other.
 */
export function steps(v: number, total = 5): number {
  return Math.max(1, Math.ceil(unit(v) * total));
}
