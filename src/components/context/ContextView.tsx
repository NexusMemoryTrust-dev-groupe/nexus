import { useCallback, useMemo, useState } from 'react';
import type { CSSProperties, ReactNode } from 'react';
import {
  AlertTriangle, Archive, ArrowRight, Brain, Check, CircleDot,
  ClipboardCopy, Download, FileJson, FileText, GitBranch, Hash,
  Layers, Link2, Loader2, Network, PackageCheck, Search, Sparkles,
  Target, Type, XCircle,
} from 'lucide-react';
import { useContextStore } from '../../stores/contextStore';
import { useGraphStore } from '../../stores/graphStore';
import { useUiStore } from '../../stores/uiStore';
import { useLocale } from '../../stores/localeStore';
import { compact } from '../../lib/format';
import type { ContextTrace } from '../../types';
import {
  InfoTip, PageHero, StrataAlert, StrataVoid,
} from '../ui/Instruments';

const SEEDS_EN = ['What did I decide about auth?', 'Recent work on the parser', 'Everything about Nexus MCP'];
const SEEDS_RU = ['Что я решил про авторизацию?', 'Последняя работа над парсером', 'Всё про Nexus MCP'];

type StageProps = {
  number: string;
  icon: typeof Search;
  color: string;
  name: string;
  description: string;
  ready: boolean;
  running?: boolean;
  children?: ReactNode;
};

function Stage({ number, icon: Icon, color, name, description, ready, running, children }: StageProps) {
  const { t } = useLocale();
  return (
    <section
      className={`st-stage${ready ? ' is-ready' : ''}${running ? ' is-running' : ''}`}
      style={{ '--stage-color': color } as CSSProperties}
    >
      <div className="st-stage-rail">
        <span className="st-stage-node">{number}</span>
      </div>
      <div className="st-stage-card">
        <div className="st-stage-head">
          <Icon size={14} style={{ color, flexShrink: 0, marginTop: 2 }} />
          <div className="st-stage-title-wrap">
            <div className="st-stage-title">{name}</div>
            <div className="st-stage-desc">{description}</div>
          </div>
          <span className="st-stage-state">
            {running ? <Loader2 size={10} className="spinning" /> : ready ? <Check size={10} /> : <CircleDot size={10} />}
            {running ? t('ctx.state.working') : ready ? t('ctx.state.ready') : t('ctx.state.waiting')}
          </span>
        </div>
        {children && <div className="st-stage-body">{children}</div>}
      </div>
    </section>
  );
}

function SourceCard({ icon: Icon, label, value, note, color }: { icon: typeof Network; label: string; value: number; note: string; color: string }) {
  return (
    <div className="st-source" style={{ '--source-color': color } as CSSProperties}>
      <div className="st-source-head"><Icon size={12} /> {label}</div>
      <div className="st-source-value">{value}</div>
      <div className="st-source-note">{note}</div>
    </div>
  );
}

function TraceRows({
  traces,
  dropped = false,
}: {
  traces: ContextTrace[];
  dropped?: boolean;
}) {
  const { t } = useLocale();
  const [expandedId, setExpandedId] = useState<string | null>(null);
  if (traces.length === 0) {
    return (
      <div className={`st-prune-summary${dropped ? ' good' : ''}`}>
        {dropped ? <Check size={12} /> : <Hash size={12} />}
        {dropped ? t('ctx.prune.none') : t('ctx.rank.none')}
      </div>
    );
  }

  const scoreLabel = (component: string) => {
    const key = `ctx.score.${component}`;
    const known = ['titleMatch', 'keywordMatch', 'contentMatch', 'importance', 'recency', 'confidence', 'base'];
    return known.includes(component) ? t(key) : component;
  };

  return (
    <div className="st-rank-list">
      {traces.map((trace) => {
        const color = trace.kind === 'entity' ? 'var(--cyan)' : 'var(--tangerine)';
        const score = Math.max(trace.score ?? 0, 0);
        const key = `${trace.kind}-${trace.id}`;
        const expanded = expandedId === key;
        const parts = trace.scoreParts ?? [];
        return (
          <div
            key={key}
            className={`st-rank-row${dropped ? ' is-dropped' : ''}`}
            style={{ '--row-color': color, '--score': Math.min(score, 1) } as CSSProperties}
          >
            <span className="st-rank-kind">
              {trace.kind === 'entity' ? <Network size={12} /> : <Brain size={12} />}
            </span>
            <div className="st-rank-main">
              <div className="st-rank-title">{trace.title || trace.id.slice(0, 8)}</div>
              <div className="st-rank-reasons">
                {trace.reasons.slice(0, 3).map((reason, index) => (
                  <span
                    key={`${reason.kind}-${index}`}
                    className="st-reason"
                    style={{ '--reason-color': color } as CSSProperties}
                  >
                    {reason.kind === 'queryMatch' ? t('ctx.reason.query') :
                      reason.kind === 'keywordMatch' ? `${t('ctx.reason.keyword')}: ${reason.keyword}` :
                        reason.kind === 'graphExpansion' ? `${t('ctx.reason.graph')} +${reason.hops}` :
                          reason.kind === 'memorySearch' ? t('ctx.reason.memory') :
                            reason.kind === 'recentActivity' ? t('ctx.reason.recent') : t('ctx.reason.important')}
                  </span>
                ))}
                {trace.dropped && (
                  <span className="st-reason" style={{ '--reason-color': 'var(--rose)' } as CSSProperties}>
                    {trace.dropped.kind === 'tokenBudget' ? t('ctx.drop.budget') : trace.dropped.kind === 'entityCap' ? t('ctx.drop.cap') : t('ctx.drop.relevance')}
                  </span>
                )}
              </div>
              {parts.length > 0 && (
                <div className="st-rank-arithmetic">
                  <button
                    type="button"
                    className="st-rank-arithmetic-toggle"
                    onClick={() => setExpandedId(expanded ? null : key)}
                    aria-expanded={expanded}
                  >
                    {expanded ? '−' : '+'} {expanded ? t('ctx.rank.breakdown') : t('ctx.rank.expand')}
                  </button>
                  {expanded && (
                    <div className="st-rank-parts">
                      {parts.map((part, index) => (
                        <div className="st-rank-part" key={`${part.component}-${index}`}>
                          <span className="st-rank-part-name">{scoreLabel(part.component)}</span>
                          <span className="st-rank-part-bar">
                            <span className="st-rank-part-fill" style={{ '--fill': `${Math.min(Math.max(part.points, 0), 1) * 100}%` } as CSSProperties} />
                          </span>
                          <span className="st-rank-part-points">{part.points.toFixed(2)}</span>
                        </div>
                      ))}
                    </div>
                  )}
                </div>
              )}
            </div>
            <span className="st-score-track"><span className="st-score-fill" /></span>
            <span className="st-score-value">{trace.score === null ? '—' : trace.score.toFixed(2)}</span>
            <span className="st-token-cost">{trace.tokens}t</span>
          </div>
        );
      })}
    </div>
  );
}

function ExportStage() {
  const { t } = useLocale();
  const { exportContext, lastExport, isExporting } = useContextStore();
  const [copied, setCopied] = useState(false);
  const formats = [
    { id: 'markdown' as const, icon: FileText, label: t('export.markdown') },
    { id: 'plain' as const, icon: Type, label: t('export.plain') },
    { id: 'json' as const, icon: FileJson, label: t('export.json') },
  ];

  const flash = () => {
    setCopied(true);
    window.setTimeout(() => setCopied(false), 1_800);
  };

  const run = async (format: 'markdown' | 'plain' | 'json') => {
    const result = await exportContext(format);
    if (!result) return;
    try {
      await navigator.clipboard.writeText(result.content);
      flash();
    } catch {
      // The preview still exposes the result if clipboard permissions are off.
    }
  };

  return (
    <div>
      <div className="st-export-actions">
        {formats.map(({ id, icon: Icon, label }) => (
          <button key={id} type="button" className="st-export-button" onClick={() => run(id)} disabled={isExporting}>
            {isExporting ? <Loader2 size={11} className="spinning" /> : <Icon size={11} />}
            {label}
          </button>
        ))}
        {copied && <span style={{ display: 'inline-flex', alignItems: 'center', gap: 5, color: 'var(--mint)', fontSize: 9 }}><Check size={11} /> {t('sheet.copied')}</span>}
      </div>
      {lastExport && (
        <div className="st-export-result">
          <div className="st-export-meta">
            <span>{lastExport.filename}</span>
            <span>{lastExport.tokens} {t('export.tokens')}</span>
            <button type="button" className="st-icon-button" style={{ marginLeft: 'auto' }} onClick={() => { navigator.clipboard.writeText(lastExport.content).then(flash, () => undefined); }} aria-label={t('sheet.copy')}>
              <ClipboardCopy size={11} />
            </button>
          </div>
          <pre className="st-export-preview">{lastExport.content}</pre>
        </div>
      )}
    </div>
  );
}

export function ContextView() {
  const { t, locale } = useLocale();
  const { context, isLoading, error, buildContext, clearContext } = useContextStore();
  const { requestFocus } = useGraphStore();
  const { setActiveView } = useUiStore();
  const [query, setQuery] = useState('');
  const seeds = locale === 'ru' ? SEEDS_RU : SEEDS_EN;
  const ready = query.trim().length > 0 && !isLoading;

  const run = useCallback((value: string) => {
    const trimmed = value.trim();
    if (trimmed) buildContext(trimmed);
  }, [buildContext]);

  // "Show in graph": jump to the graph view with the package's entities
  // highlighted. Only entity ids travel — memories are not graph nodes, so
  // sending them would silently highlight nothing.
  const showInGraph = useCallback(() => {
    const ids = (context?.entities ?? []).map((entity) => entity.id);
    if (ids.length === 0) return;
    requestFocus(ids);
    setActiveView('graph');
  }, [context, requestFocus, setActiveView]);

  const included = useMemo(() => context?.provenance.filter((trace) => trace.included) ?? [], [context]);
  const dropped = useMemo(() => context?.provenance.filter((trace) => !trace.included) ?? [], [context]);
  const packageItems = context ? context.entities.length + context.memoryRecords.length : 0;
  // Only compare traces to traces. Entities/memories/relationships are not the
  // same unit as ranked candidates, so mixing them made the ratio look precise
  // while saying nothing truthful about selection.
  const candidateTotal = included.length + dropped.length;
  const keptShare = candidateTotal > 0 ? included.length / candidateTotal : 1;

  return (
    <div className="st-page st-page--reading" style={{ '--st-accent': 'var(--gold)' } as CSSProperties}>
      <PageHero
        kicker={t('ctx.hero.kicker')}
        title={t('context.title')}
        copy={t('ctx.hero.sub')}
        accent="var(--gold)"
        secondary="var(--tangerine)"
        stats={context ? [
          { label: t('ctx.pack.tokens'), value: compact(context.tokenCount, locale), color: 'var(--gold)' },
          { label: t('ctx.stats.selected'), value: String(packageItems), color: 'var(--mint)' },
        ] : []}
      />

      <section className="st-ask">
        <div className="st-ask-label"><Sparkles size={12} /> {t('ctx.ask.label')} <InfoTip text={t('ctx.stage.query.desc')} /></div>
        <div className="st-ask-field">
          <span className="st-ask-prompt">›</span>
          <input
            value={query}
            onChange={(event) => setQuery(event.target.value)}
            onKeyDown={(event) => { if (event.key === 'Enter') run(query); }}
            placeholder={t('ctx.ask.placeholder')}
            autoFocus
            aria-label={t('ctx.ask.label')}
          />
          {ready && <span className="st-ask-hint">↵ {t('ctx.ask.hint')}</span>}
          <button type="button" className="st-run" disabled={!ready} onClick={() => run(query)}>
            {isLoading ? <Loader2 size={12} className="spinning" /> : <PackageCheck size={12} />}
            {t('ctx.ask.run')}
          </button>
        </div>
        {!context && (
          <div className="st-seeds">
            <span className="st-seed-label">{t('ctx.seeds')}</span>
            {seeds.map((seed) => <button key={seed} type="button" className="st-seed" onClick={() => { setQuery(seed); run(seed); }}>{seed}</button>)}
          </div>
        )}
      </section>

      {error && <StrataAlert icon={AlertTriangle}>{error}</StrataAlert>}

      <div className="st-pipeline-intro">
        <span className="st-pipeline-intro-icon"><GitBranch size={15} /></span>
        <div>
          <div className="st-section-title">{t('ctx.pipeline.title')} <InfoTip text={t('ctx.pipeline.idle')} /></div>
          <p className="st-section-hint">{t('ctx.pipeline.idle')}</p>
        </div>
      </div>

      <div className="st-flow">
        <Stage
          number="01"
          icon={Search}
          color="var(--gold)"
          name={t('ctx.stage.query.name')}
          description={t('ctx.stage.query.desc')}
          ready={Boolean(context || query.trim())}
        >
          <div className="st-stage-question">{context?.query || query.trim() || t('ctx.empty.desc')}</div>
        </Stage>

        <Stage number="02" icon={Target} color="var(--periwinkle)" name={t('ctx.stage.intent.name')} description={t('ctx.stage.intent.desc')} ready={Boolean(context)} running={isLoading}>
          {context ? (
            <div className="st-intent-grid">
              <div className="st-value-card"><div className="st-value-label">{t('ctx.intent.type')}</div><div className="st-value-main" style={{ color: 'var(--gold)' }}>{context.intentType}</div><div className="st-value-note">{t('ctx.intent.type.note')}</div></div>
              <div className="st-value-card"><div className="st-value-label">{t('ctx.intent.confidence')}</div><div className="st-value-main" style={{ color: 'var(--periwinkle)' }}>{Math.round(context.confidence * 100)}%</div><div className="st-value-note">{t('ctx.intent.confidence.note')}</div></div>
            </div>
          ) : <StagePlaceholder />}
        </Stage>

        <Stage number="03" icon={Network} color="var(--cyan)" name={t('ctx.stage.gather.name')} description={t('ctx.stage.gather.desc')} ready={Boolean(context)} running={isLoading}>
          {context ? (
            <div className="st-source-grid">
              <SourceCard icon={Network} label={t('ctx.gather.entities')} value={context.entities.length} note={t('ctx.source.entities.note')} color="var(--cyan)" />
              <SourceCard icon={Brain} label={t('ctx.gather.memories')} value={context.memoryRecords.length} note={t('ctx.source.memories.note')} color="var(--tangerine)" />
              <SourceCard icon={Link2} label={t('ctx.gather.links')} value={context.relationships.length} note={t('ctx.source.links.note')} color="var(--periwinkle)" />
              {context.entities.length > 0 && (
                <button type="button" className="st-show-in-graph" onClick={showInGraph}>
                  <Network size={11} /> {t('ctx.showInGraph')}
                </button>
              )}
            </div>
          ) : <StagePlaceholder />}
        </Stage>

        <Stage number="04" icon={Hash} color="var(--steel)" name={t('ctx.stage.rank.name')} description={t('ctx.stage.rank.desc')} ready={Boolean(context)}>
          {context ? <TraceRows traces={included} /> : <StagePlaceholder />}
        </Stage>

        <Stage number="05" icon={XCircle} color="var(--rose)" name={t('ctx.stage.prune.name')} description={t('ctx.stage.prune.desc')} ready={Boolean(context)}>
          {context ? <TraceRows traces={dropped} dropped /> : <StagePlaceholder />}
        </Stage>

        <Stage number="06" icon={Archive} color="var(--mint)" name={t('ctx.stage.pack.name')} description={t('ctx.stage.pack.desc')} ready={Boolean(context)}>
          {context ? (
            <div className="st-budget">
              <div className="st-budget-total"><div className="st-budget-value">{compact(context.tokenCount, locale)}</div><div className="st-budget-label">{t('ctx.pack.tokens')}</div></div>
              <div>
                <div className="st-budget-bar">
                  <span className="st-budget-part" style={{ '--part': `${Math.max(keptShare, .12) * 100}%`, '--part-color': 'var(--mint)' } as CSSProperties} />
                  <span className="st-budget-part" style={{ '--part': `${Math.max(1 - keptShare, .04) * 100}%`, '--part-color': 'var(--raised-2)' } as CSSProperties} />
                </div>
                <div className="st-budget-legend"><span><i style={{ '--part-color': 'var(--mint)' } as CSSProperties} /> {t('ctx.kept')}: {included.length}</span><span><i style={{ '--part-color': 'var(--raised-2)' } as CSSProperties} /> {t('ctx.dropped')}: {dropped.length}</span></div>
              </div>
            </div>
          ) : <StagePlaceholder />}
        </Stage>

        <Stage number="07" icon={Download} color="var(--gold)" name={t('ctx.stage.export.name')} description={t('ctx.stage.export.desc')} ready={Boolean(context)}>
          {context ? <ExportStage /> : <StagePlaceholder />}
        </Stage>
      </div>

      {context ? (
        <button type="button" className="st-expand" style={{ marginTop: 13 }} onClick={clearContext}><ArrowRight size={11} /> {t('ctx.clear')}</button>
      ) : !isLoading && !error ? (
        <StrataVoid icon={Layers} title={t('ctx.empty.title')} accent="var(--gold)">{t('ctx.empty.desc')}</StrataVoid>
      ) : null}
    </div>
  );
}

function StagePlaceholder() {
  const { t } = useLocale();
  return <div style={{ color: 'var(--muted-2)', fontSize: 10, lineHeight: 1.5 }}>{t('ctx.stage.placeholder')}</div>;
}
