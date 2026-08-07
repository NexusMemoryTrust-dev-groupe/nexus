import { useCallback, useEffect, useRef, useState } from 'react';
import type { CSSProperties } from 'react';
import { invoke } from '@tauri-apps/api/core';
import {
  AlertTriangle, ClipboardList, GitBranch, History, Scale,
  Search, ShieldCheck, Sparkles, X,
} from 'lucide-react';
import type { AuditTrail } from '../../types';
import { useLocale } from '../../stores/localeStore';
import {
  InfoTip, PageHero, StrataAlert, StrataVoid,
} from '../ui/Instruments';

/** Lightweight memory reference for the picker (what search_memories returns). */
interface MemoryRef {
  id: string;
  title: string;
  summary: string;
  layer: string;
  memory_state?: string;
}

const STATE_COLORS: Record<string, string> = {
  UserConfirmed: 'var(--mint)',
  Current: 'var(--cyan)',
  Inferred: 'var(--gold)',
  Superseded: 'var(--bone)',
  Conflicted: 'var(--rose)',
};

function stateColor(state: string) {
  return STATE_COLORS[state] ?? 'var(--bone)';
}

// ── One decision-journal event ─────────────────────────────────────────────

function EventRow({ event, accent }: { event: { eventType: string; actor: string; detail: string | null; createdAt: string; relatedMemoryId: string | null }; accent: string }) {
  const icon =
    event.eventType === 'Confirmed' ? <ShieldCheck size={13} /> :
    event.eventType === 'Superseded' ? <GitBranch size={13} /> :
    event.eventType === 'Created' ? <Sparkles size={13} /> :
    <ClipboardList size={13} />;
  return (
    <div className="st-audit-event">
      <span className="st-audit-event-icon" style={{ color: accent, background: `${accent}15` }}>
        {icon}
      </span>
      <span style={{ minWidth: 0, flex: 1 }}>
        <span className="st-audit-event-head">
          <span className="st-audit-event-type" style={{ color: accent }}>{event.eventType}</span>
          <span className="st-audit-event-meta">
            {event.actor} · {event.createdAt}
          </span>
        </span>
        {event.detail && <span className="st-audit-event-detail">{event.detail}</span>}
        {event.relatedMemoryId && (
          <span className="st-audit-event-related">→ {event.relatedMemoryId}</span>
        )}
      </span>
    </div>
  );
}

// ── Main view ──────────────────────────────────────────────────────────────

export function AuditView() {
  const { t } = useLocale();
  const [memoryId, setMemoryId] = useState('');
  const [trail, setTrail] = useState<AuditTrail | null>(null);
  const [error, setError] = useState<string | null>(null);

  // Picker state: the search box, the debounced results and the dropdown.
  const [query, setQuery] = useState('');
  const [results, setResults] = useState<MemoryRef[]>([]);
  const [pickerOpen, setPickerOpen] = useState(false);
  const [pickerBusy, setPickerBusy] = useState(false);
  const debounceRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const runSearch = useCallback(async (q: string) => {
    if (q.trim().length < 2) {
      setResults([]);
      setPickerBusy(false);
      return;
    }
    setPickerBusy(true);
    try {
      const hits = await invoke<MemoryRef[]>('search_memories', { query: q.trim() });
      // search_memories ranks by title/summary/content relevance; the picker
      // shows every hit so the user can reconstruct any decision chain.
      setResults(hits);
    } catch {
      setResults([]);
    } finally {
      setPickerBusy(false);
    }
  }, []);

  // Debounced search-as-you-type for the picker dropdown.
  useEffect(() => {
    if (debounceRef.current) clearTimeout(debounceRef.current);
    debounceRef.current = setTimeout(() => { runSearch(query); }, 220);
    return () => {
      if (debounceRef.current) clearTimeout(debounceRef.current);
    };
  }, [query, runSearch]);

  const load = useCallback(async (id: string) => {
    if (!id.trim()) return;
    setError(null);
    try {
      const result = await invoke<AuditTrail>('get_audit_trail', { memoryId: id.trim() });
      setTrail(result);
    } catch (err) {
      setTrail(null);
      setError(String(err));
    }
  }, []);

  const pick = useCallback(async (id: string) => {
    setMemoryId(id);
    setPickerOpen(false);
    setQuery('');
    setResults([]);
    await load(id);
  }, [load]);

  const clear = useCallback(() => {
    setTrail(null);
    setMemoryId('');
    setQuery('');
    setResults([]);
    setPickerOpen(false);
    setError(null);
  }, []);

  const hero = (
    <PageHero
      kicker={t('audit.hero.kicker')}
      title={t('audit.title')}
      copy={t('audit.hero.sub')}
      accent="var(--periwinkle)"
      secondary="var(--cyan)"
      stats={[
        { label: t('audit.stats.alternatives'), value: trail ? String(trail.alternatives.length) : '—' },
        { label: t('audit.stats.versions'), value: trail ? String(trail.versions.length) : '—', color: 'var(--cyan)' },
      ]}
    />
  );

  return (
    <div className="st-page" style={{ '--st-accent': 'var(--periwinkle)' } as CSSProperties}>
      {hero}

      {/* Memory picker */}
      <section className="st-audit-picker">
        <h2 className="st-section-title" style={{ '--section-color': 'var(--periwinkle)' } as CSSProperties}>
          <Scale size={14} /> {t('audit.picker.title')}
          <InfoTip text={t('audit.picker.hint')} />
        </h2>
        <p className="st-section-hint">{t('audit.picker.sub')}</p>

        <div className="st-audit-search">
          <Search size={14} className="st-audit-search-icon" />
          <input
            value={query}
            onChange={(e) => { setQuery(e.target.value); setPickerOpen(true); }}
            onFocus={() => setPickerOpen(true)}
            onBlur={() => setTimeout(() => setPickerOpen(false), 150)}
            placeholder={t('audit.search.placeholder')}
            spellCheck={false}
            autoComplete="off"
          />
          {pickerBusy && <span className="st-audit-search-spinner" aria-hidden="true" />}
          {trail && (
            <button
              type="button"
              className="btn-icon"
              title={t('audit.search.clear')}
              onClick={clear}
            >
              <X size={14} />
            </button>
          )}
        </div>

        {pickerOpen && (
          <div className="st-audit-dropdown" role="listbox">
            {query.trim().length < 2 ? (
              <div className="st-audit-dropdown-empty">
                {t('audit.picker.type')}
              </div>
            ) : results.length === 0 ? (
              <div className="st-audit-dropdown-empty">
                {pickerBusy ? t('audit.picker.searching') : t('audit.picker.noresults')}
              </div>
            ) : (
              results.map((m) => (
                <button
                  key={m.id}
                  type="button"
                  className="st-audit-dropdown-item"
                  role="option"
                  onMouseDown={(e) => { e.preventDefault(); pick(m.id); }}
                >
                  <span className="st-audit-dropdown-item-title">{m.title}</span>
                  <span className="st-audit-dropdown-item-meta">
                    {m.layer}
                    {m.memory_state ? ` · ${m.memory_state}` : ''}
                    {m.summary ? ` · ${m.summary.slice(0, 60)}` : ''}
                  </span>
                </button>
              ))
            )}
          </div>
        )}

        {memoryId && trail && (
          <p className="st-audit-selected">
            {t('audit.picker.selected')} <code>{memoryId}</code>
          </p>
        )}
      </section>

      {error && (
        <StrataAlert icon={AlertTriangle}>{error}</StrataAlert>
      )}

      {!trail && !error && (
        <StrataVoid icon={Scale} title={t('audit.empty.title')} accent="var(--periwinkle)">
          {t('audit.empty.desc')}
        </StrataVoid>
      )}

      {trail && (
        <>
          {/* The decision at a glance */}
          <section className="st-audit-headline">
            <div className="st-audit-title-row">
              <h2 className="st-section-title" style={{ '--section-color': 'var(--periwinkle)' } as CSSProperties}>
                {trail.title}
              </h2>
              <span className="st-team-role" style={{ color: stateColor(trail.state), borderColor: `${stateColor(trail.state)}40`, background: `${stateColor(trail.state)}10` }}>
                {trail.state}
              </span>
            </div>
            <p className="st-section-hint">
              {t('audit.by')} {trail.author} · {trail.createdAt}
            </p>
            {trail.reason && (
              <div className="st-audit-reason">
                <span className="st-audit-reason-label">{t('audit.reason')}</span>
                {trail.reason}
              </div>
            )}
            <div className="st-audit-facts">
              {trail.confirmedBy && (
                <span className="st-audit-fact" style={{ color: 'var(--mint)', borderColor: 'var(--mint)40', background: 'var(--mint)10' }}>
                  <ShieldCheck size={12} /> {t('audit.confirmedBy')} {trail.confirmedBy}
                  {trail.confirmedAt ? ` · ${trail.confirmedAt}` : ''}
                </span>
              )}
              {trail.supersedes && (
                <span className="st-audit-fact" style={{ color: 'var(--gold)', borderColor: 'var(--gold)40', background: 'var(--gold)10' }}>
                  <GitBranch size={12} /> {t('audit.supersedes')} {trail.supersedes}
                </span>
              )}
              {trail.supersededBy && (
                <span className="st-audit-fact" style={{ color: 'var(--bone)', borderColor: 'var(--bone)40', background: 'var(--bone)10' }}>
                  <GitBranch size={12} /> {t('audit.supersededBy')} {trail.supersededBy}
                </span>
              )}
            </div>
          </section>

          {/* Alternatives considered */}
          <section>
            <h2 className="st-section-title" style={{ '--section-color': 'var(--periwinkle)' } as CSSProperties}>
              <Scale size={14} /> {t('audit.alternatives.title')}
              <InfoTip text={t('audit.alternatives.hint')} />
            </h2>
            {trail.alternatives.length === 0 ? (
              <StrataVoid icon={Scale} title={t('audit.alternatives.empty.title')} accent="var(--periwinkle)">
                {t('audit.alternatives.empty.desc')}
              </StrataVoid>
            ) : (
              <div className="st-panel">
                <div className="st-audit-alternatives">
                  {trail.alternatives.map((alt, index) => (
                    <div key={`${alt.title}-${index}`} className="st-audit-alternative st-rise" style={{ '--st-i': index } as CSSProperties}>
                      <span className="st-audit-alt-title">{alt.title}</span>
                      <span className="st-audit-alt-reason">
                        {t('audit.alternatives.rejected')} {alt.reason}
                      </span>
                    </div>
                  ))}
                </div>
              </div>
            )}
          </section>

          {/* Decision journal */}
          <section>
            <h2 className="st-section-title" style={{ '--section-color': 'var(--periwinkle)' } as CSSProperties}>
              <ClipboardList size={14} /> {t('audit.journal.title')}
              <InfoTip text={t('audit.journal.hint')} />
            </h2>
            {trail.events.length === 0 ? (
              <StrataVoid icon={ClipboardList} title={t('audit.journal.empty.title')} accent="var(--periwinkle)">
                {t('audit.journal.empty.desc')}
              </StrataVoid>
            ) : (
              <div className="st-panel">
                <div className="st-audit-journal">
                  {trail.events.map((event) => (
                    <EventRow
                      key={event.id}
                      event={event}
                      accent={
                        event.eventType === 'Confirmed' ? 'var(--mint)' :
                        event.eventType === 'Superseded' ? 'var(--gold)' :
                        event.eventType === 'Created' ? 'var(--cyan)' :
                        'var(--periwinkle)'
                      }
                    />
                  ))}
                </div>
              </div>
            )}
          </section>

          {/* Version history */}
          <section>
            <h2 className="st-section-title" style={{ '--section-color': 'var(--periwinkle)' } as CSSProperties}>
              <History size={14} /> {t('audit.versions.title')}
              <InfoTip text={t('audit.versions.hint')} />
            </h2>
            {trail.versions.length === 0 ? (
              <StrataVoid icon={History} title={t('audit.versions.empty.title')} accent="var(--periwinkle)">
                {t('audit.versions.empty.desc')}
              </StrataVoid>
            ) : (
              <div className="st-panel">
                <div className="st-audit-versions">
                  {trail.versions.map((v) => (
                    <div key={`v${v.version}`} className="st-audit-version">
                      <span className="st-audit-version-badge" style={{ color: 'var(--cyan)', borderColor: 'var(--cyan)40', background: 'var(--cyan)10' }}>
                        v{v.version}
                      </span>
                      <span style={{ minWidth: 0, flex: 1 }}>
                        <span className="st-audit-event-head">
                          <span className="st-audit-event-type">{v.changeType}</span>
                          <span className="st-audit-event-meta">{v.by} · {v.at}</span>
                        </span>
                        {v.reason && <span className="st-audit-event-detail">{v.reason}</span>}
                      </span>
                    </div>
                  ))}
                </div>
              </div>
            )}
          </section>
        </>
      )}
    </div>
  );
}
