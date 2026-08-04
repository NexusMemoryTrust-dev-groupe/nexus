import { useCallback, useState } from 'react';
import type { CSSProperties } from 'react';
import {
  ArrowLeft, Check, Copy, Eye, FileText, Fingerprint, Link2,
  Paperclip, Quote, Terminal, User, Wand2,
} from 'lucide-react';
import { useMemoryStore } from '../../stores/memoryStore';
import { useLocale } from '../../stores/localeStore';
import { LAYER_LIST, layerKey, layerVars, layerVisual } from '../../lib/layers';
import { ago, bytes, num, stamp } from '../../lib/format';
import {
  ImpactBlocks, InfoTip, LayerGlyph, SemanticLayerTag, SignalRing,
} from '../ui/Instruments';

const CLIP_CHARS = 1_000;

export function MemoryDetail() {
  const { selectedMemory, selectMemory } = useMemoryStore();
  const { locale, t } = useLocale();
  const [expanded, setExpanded] = useState(false);
  const [copied, setCopied] = useState(false);

  const copy = useCallback((value: string) => {
    navigator.clipboard.writeText(value).then(
      () => {
        setCopied(true);
        window.setTimeout(() => setCopied(false), 1_800);
      },
      () => undefined,
    );
  }, []);

  if (!selectedMemory) return null;
  const memory = selectedMemory;
  const layer = layerVisual(memory.layer);
  const files = memory.attachedFiles ?? [];
  const linked = memory.linkedEntityIds?.length ?? 0;
  const clipped = !expanded && memory.content.length > CLIP_CHARS;

  return (
    <article className="st-sheet" style={layerVars(memory.layer)}>
      <nav className="st-sheet-nav">
        <button type="button" className="st-back" onClick={() => selectMemory(null)}>
          <ArrowLeft size={13} /> {t('sheet.back')}
        </button>
        <span style={{ flex: 1 }} />
        <button
          type="button"
          className={`st-icon-button${copied ? ' is-success' : ''}`}
          onClick={() => copy(`${memory.title}\n\n${memory.content}`)}
          aria-label={t('sheet.copy')}
          title={t('sheet.copy')}
        >
          {copied ? <Check size={13} /> : <Copy size={13} />}
        </button>
      </nav>

      <header className="st-sheet-hero">
        <div className="st-sheet-kicker">
          <LayerGlyph layer={memory.layer} size={38} />
          <SemanticLayerTag layer={memory.layer} />
          <span style={{ color: 'var(--muted-2)', fontFamily: 'var(--mono)', fontSize: 9 }}>
            {t('layer.stage')} {layer.rank + 1}/4
          </span>
          <InfoTip text={t(layerKey(memory.layer, 'meaning'))} />
        </div>
        <h1 className="st-sheet-title">{memory.title}</h1>

        <div className="st-sheet-facts">
          <span className="st-fact st-fact--visibility"><Eye size={11} /> {memory.visibility}</span>
          <span className="st-fact st-fact--author"><User size={11} /> {memory.author}</span>
          <span className="st-fact st-fact--source"><Terminal size={11} /> {memory.source}</span>
          <span className="st-fact st-fact--mode"><Wand2 size={11} /> {memory.captureMode}</span>
          {linked > 0 && <span className="st-fact st-fact--linked"><Link2 size={11} /> {linked} {t('sheet.linked')}</span>}
          {files.length > 0 && <span className="st-fact st-fact--files"><Paperclip size={11} /> {files.length}</span>}
        </div>
      </header>

      <section className="st-sheet-map">
        <div className="st-ladder">
          <div style={{ display: 'flex', alignItems: 'center', gap: 7 }}>
            <h2 className="st-section-title" style={{ '--section-color': layer.color } as CSSProperties}>
              {t('layer.ladder')}
            </h2>
            <InfoTip text={t('layer.ladder.hint')} />
          </div>
          <div className="st-ladder-track">
            {LAYER_LIST.map((step) => {
              const Icon = step.icon;
              const on = step.rank <= layer.rank;
              return (
                <div
                  key={step.name}
                  className={`st-ladder-step${on ? ' on' : ''}`}
                  style={{ '--step-color': step.color } as CSSProperties}
                >
                  <span className="st-ladder-node"><Icon size={13} /></span>
                  <span className="st-ladder-name">{step.name}</span>
                </div>
              );
            })}
          </div>
          <p className="st-ladder-meaning">
            <strong style={{ color: layer.color }}>{layer.name}.</strong>{' '}
            {t(layerKey(memory.layer, 'meaning'))} {t(layerKey(memory.layer, 'promotes'))}
          </p>
        </div>

        <div className="st-sheet-signals">
          <div className="st-signal st-signal--trust">
            <span className="st-signal-title">{t('inst.trust')}</span>
            <SignalRing
              value={memory.confidenceScore}
              label={t('inst.trust')}
              size={72}
            />
            <span className="st-signal-copy">{t('inst.trust.hint')}</span>
          </div>
          <div className="st-signal st-signal--impact">
            <span className="st-signal-title">{t('inst.impact')}</span>
            <div className="st-signal-impact">
              <span className="st-signal-impact-value">{Math.round(memory.importanceScore * 100)}</span>
              <ImpactBlocks value={memory.importanceScore} label={t('inst.impact')} />
            </div>
            <span className="st-signal-copy">{t('inst.impact.hint')}</span>
          </div>
        </div>
      </section>

      <div className="st-sheet-body">
        {memory.summary && (
          <section className="st-sheet-section">
            <div className="st-sheet-section-head">
              <Quote size={11} style={{ color: layer.color }} />
              <span className="st-sheet-section-title">{t('sheet.summary')}</span>
              <InfoTip text={t('sheet.summary.hint')} />
              <span className="st-sheet-section-line" />
            </div>
            <div className="st-quote">{memory.summary}</div>
          </section>
        )}

        <section className="st-sheet-section">
          <div className="st-sheet-section-head">
            <FileText size={11} style={{ color: 'var(--muted-2)' }} />
            <span className="st-sheet-section-title">{t('sheet.content')}</span>
            <InfoTip text={t('sheet.content.hint')} />
            <span className="st-sheet-section-line" />
            <span className="st-sheet-section-note">{num(memory.content.length, locale)} {t('sheet.chars')}</span>
          </div>

          <div className={`st-read${clipped ? ' is-clipped' : ''}`}>
            <div className="st-read-tools">
              <button
                type="button"
                className={`st-icon-button${copied ? ' is-success' : ''}`}
                onClick={() => copy(memory.content)}
                aria-label={t('sheet.copy')}
              >
                {copied ? <Check size={12} /> : <Copy size={12} />}
              </button>
            </div>
            {memory.content}
          </div>
          {memory.content.length > CLIP_CHARS && (
            <button type="button" className="st-expand" onClick={() => setExpanded((value) => !value)}>
              {expanded ? t('sheet.collapse') : t('sheet.expand')}
            </button>
          )}
        </section>

        {files.length > 0 && (
          <section className="st-sheet-section">
            <div className="st-sheet-section-head">
              <Paperclip size={11} style={{ color: layer.color }} />
              <span className="st-sheet-section-title">{t('sheet.files')}</span>
              <span className="st-sheet-section-line" />
              <span className="st-sheet-section-note">{files.length}</span>
            </div>
            <div className="st-files">
              {files.map((file) => (
                <div className="st-file" key={file.path}>
                  <LayerGlyph layer={memory.layer} size={28} />
                  <div className="st-file-main">
                    <div className="st-file-name">{file.name}</div>
                    <div className="st-file-path" title={file.path}>{file.path}</div>
                  </div>
                  <span className="st-file-size">{bytes(file.sizeBytes)}</span>
                </div>
              ))}
            </div>
          </section>
        )}

        <footer className="st-sheet-footer">
          <span title={stamp(memory.createdAt, locale)}>{t('sheet.created')} {ago(memory.createdAt, locale)}</span>
          <span title={stamp(memory.updatedAt, locale)}>{t('sheet.updated')} {ago(memory.updatedAt, locale)}</span>
          <span>{t('sheet.status')}: {memory.status}</span>
          <span className="id"><Fingerprint size={9} /> {memory.id.slice(0, 8)}</span>
        </footer>
      </div>
    </article>
  );
}
