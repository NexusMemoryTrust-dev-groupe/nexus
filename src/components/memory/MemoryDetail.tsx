import { useCallback, useEffect, useState } from 'react';
import type { CSSProperties } from 'react';
import {
  AlertTriangle, ArrowLeft, Check, CircleHelp, Copy, Eye, FileText, Fingerprint, Link2,
  Paperclip, Quote, RefreshCw, SendHorizontal, ShieldCheck, Terminal, ThumbsDown, ThumbsUp,
  User, UserCheck, Wand2, X,
} from 'lucide-react';
import { useMemoryStore } from '../../stores/memoryStore';
import { useLocale } from '../../stores/localeStore';
import { LAYER_LIST, layerKey, layerVars, layerVisual } from '../../lib/layers';
import { ago, bytes, num, stamp } from '../../lib/format';
import {
  ImpactBlocks, InfoTip, LayerGlyph, SemanticLayerTag, SignalRing,
} from '../ui/Instruments';

const CLIP_CHARS = 1_000;

/** Feedback kind — mirrors the backend enum. */
type FeedbackKind = 'useful' | 'irrelevant' | 'wrong';

/** Lifecycle state → locale key + badge colour. Unknown states fall back to
 *  the muted badge rather than crashing, so a state added on the backend later
 *  degrades gracefully. */
const STATE_META: Record<string, { label: string; hint: string; badge: string }> = {
  Current: { label: 'sheet.state.current', hint: 'sheet.state.hint.current', badge: 'st-state-badge--current' },
  Inferred: { label: 'sheet.state.inferred', hint: 'sheet.state.hint.inferred', badge: 'st-state-badge--inferred' },
  Conflicted: { label: 'sheet.state.conflicted', hint: 'sheet.state.hint.conflicted', badge: 'st-state-badge--conflicted' },
  Superseded: { label: 'sheet.state.superseded', hint: 'sheet.state.hint.superseded', badge: 'st-state-badge--superseded' },
  UserConfirmed: { label: 'sheet.state.userConfirmed', hint: 'sheet.state.hint.userConfirmed', badge: 'st-state-badge--userConfirmed' },
};

export function MemoryDetail() {
  const { selectedMemory, selectMemory, memoryConfirm, memoryFeedback, reclassifyMemory } = useMemoryStore();
  const { locale, t } = useLocale();
  const [expanded, setExpanded] = useState(false);
  const [copied, setCopied] = useState(false);
  const [acting, setActing] = useState(false);
  const [pendingKind, setPendingKind] = useState<FeedbackKind | null>(null);
  const [noteDraft, setNoteDraft] = useState('');
  const [noteSending, setNoteSending] = useState(false);
  const [noteSent, setNoteSent] = useState(false);
  const [reclassified, setReclassified] = useState(false);

  // Sync the draft with the saved explanation when switching memories, so
  // reopening the panel shows what was already written. Keyed only on the id:
  // after confirming a vote, the store refresh changes feedback.voted/note and
  // must not wipe the "Saved" flash or the draft the user just typed.
  useEffect(() => {
    setPendingKind(null);
    setNoteDraft(selectedMemory?.feedback?.note ?? '');
    setNoteSent(false);
    setReclassified(false);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [selectedMemory?.id]);

  // Close the "why" panel when the user presses Escape or clicks outside of it
  // (and outside the verdict buttons — those have their own handler). The vote
  // is not recorded yet, so dismissing the panel simply forgets the selection.
  useEffect(() => {
    if (!pendingKind) return;
    const onPointerDown = (event: PointerEvent) => {
      const target = event.target as Element | null;
      if (target?.closest('.st-feedback-note, .st-feedback-btn')) return;
      setPendingKind(null);
    };
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') setPendingKind(null);
    };
    document.addEventListener('pointerdown', onPointerDown);
    document.addEventListener('keydown', onKeyDown);
    return () => {
      document.removeEventListener('pointerdown', onPointerDown);
      document.removeEventListener('keydown', onKeyDown);
    };
  }, [pendingKind]);

  const copy = useCallback((value: string) => {
    navigator.clipboard.writeText(value).then(
      () => {
        setCopied(true);
        window.setTimeout(() => setCopied(false), 1_800);
      },
      () => undefined,
    );
  }, []);

  const act = useCallback(async (run: () => Promise<void>) => {
    setActing(true);
    try {
      await run();
    } finally {
      setActing(false);
    }
  }, []);

  /** Re-run the classifier, then flash "Reclassified" for a beat. */
  const runReclassify = useCallback(async () => {
    await act(() => reclassifyMemory(selectedMemory?.id ?? ''));
    setReclassified(true);
    window.setTimeout(() => setReclassified(false), 1_800);
  }, [act, reclassifyMemory, selectedMemory?.id]);

  /** Open the "why" panel for a verdict. The vote itself is NOT sent to the
   *  backend here — it is only recorded when the user confirms with an
   *  explanation. Clicking the same verdict again closes the panel and forgets
   *  the selection; clicking another verdict moves the panel to it. */
  const openFeedback = useCallback((kind: FeedbackKind) => {
    setPendingKind((current) => (current === kind ? null : kind));
  }, []);

  /** Confirm the verdict and send the explanation (if any) to the backend. */
  const sendNote = useCallback(
    async (kind: FeedbackKind) => {
      const text = noteDraft.trim();
      if (!text) return;
      setNoteSending(true);
      try {
        await memoryFeedback(memory?.id ?? '', kind, text);
        setNoteSent(true);
        window.setTimeout(() => setNoteSent(false), 1_800);
      } finally {
        setNoteSending(false);
      }
    },
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [noteDraft, memoryFeedback],
  );

  if (!selectedMemory) return null;
  const memory = selectedMemory;
  const layer = layerVisual(memory.layer);
  const files = memory.attachedFiles ?? [];
  const linked = memory.linkedEntityIds?.length ?? 0;
  const clipped = !expanded && memory.content.length > CLIP_CHARS;
  const state = STATE_META[memory.memoryState] ?? STATE_META.Current;
  const feedback = memory.feedback ?? { useful: 0, irrelevant: 0, wrong: 0 };
  const votedKind = (feedback.voted ?? null) as FeedbackKind | null;
  const isConfirmed = memory.memoryState === 'UserConfirmed';

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
            {t('layer.stage')} {layer.rank + 1}/6
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

          <div className="st-layer-prov">
            <div className="st-layer-prov-head">
              <span className="st-layer-prov-title">{t('layer.assigned')}</span>
              <InfoTip text={t('layer.assigned.hint')} />
              <span className="st-layer-prov-spacer" />
              <button
                type="button"
                className="st-layer-reclassify"
                disabled={acting}
                onClick={runReclassify}
                title={t('layer.reclassify.hint')}
              >
                {reclassified ? <Check size={12} /> : <RefreshCw size={12} />}
                {reclassified ? t('layer.reclassify.done') : t('layer.reclassify')}
              </button>
            </div>

            <div className="st-layer-prov-row">
              <span
                className="st-layer-prov-chip"
                style={{ '--chip-color': layer.color, '--chip-soft': layer.soft } as CSSProperties}
              >
                {layer.name}
              </span>
              <span className="st-layer-prov-conf" title={t('layer.confidence.hint')}>
                {t('layer.confidence')}: {Math.round((memory.layerConfidence ?? 0) * 100)}%
              </span>
              {memory.layerReason && (
                <span className="st-layer-prov-reason" title={t('layer.reason.hint')}>
                  {memory.layerReason}
                </span>
              )}
            </div>

            <div className="st-layer-history">
              <div className="st-layer-history-head">
                <span className="st-layer-history-title">{t('layer.history')}</span>
                <InfoTip text={t('layer.history.hint')} />
              </div>
              {memory.layerHistory && memory.layerHistory.length > 0 ? (
                <ul className="st-layer-history-list">
                  {memory.layerHistory.map((entry, index) => {
                    const EntryIcon = layerVisual(entry.layer).icon;
                    return (
                      <li
                        key={`${entry.at}-${index}`}
                        className="st-layer-history-item"
                        style={layerVars(entry.layer)}
                      >
                        <EntryIcon size={11} />
                        <span className="st-layer-history-layer">{layerVisual(entry.layer).name}</span>
                        <span className="st-layer-history-by">
                          {t(`layer.by.${entry.by}`)}
                        </span>
                        <span className="st-layer-history-conf">
                          {Math.round(entry.confidence * 100)}%
                        </span>
                        <span className="st-layer-history-when" title={stamp(entry.at, locale)}>
                          {ago(entry.at, locale)}
                        </span>
                      </li>
                    );
                  })}
                </ul>
              ) : (
                <span className="st-layer-history-empty">{t('layer.no.history')}</span>
              )}
            </div>
          </div>
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

        <section className="st-sheet-section">
          <div className="st-sheet-section-head">
            <ShieldCheck size={11} style={{ color: 'var(--mint)' }} />
            <span className="st-sheet-section-title">{t('sheet.lifecycle')}</span>
            <InfoTip text={t('sheet.lifecycle.hint')} />
            <span className="st-sheet-section-line" />
          </div>

          <div className="st-lifecycle">
            <div className="st-lifecycle-state">
              <span className={`st-state-badge ${state.badge}`}>
                <ShieldCheck /> {t(state.label)}
              </span>
              <span className="st-lifecycle-hint">{t(state.hint)}</span>
            </div>

            {(memory.confirmedAt || memory.confirmedBy) && (
              <div className="st-lifecycle-meta">
                {memory.confirmedBy && (
                  <span><UserCheck size={10} /> {t('sheet.confirmed.by')}: {memory.confirmedBy}</span>
                )}
                {memory.confirmedAt && (
                  <span title={stamp(memory.confirmedAt, locale)}>{ago(memory.confirmedAt, locale)}</span>
                )}
              </div>
            )}
            {memory.supersededById && (
              <div className="st-lifecycle-meta">
                <span><Fingerprint size={10} /> {t('sheet.superseded.by')}: {memory.supersededById.slice(0, 8)}</span>
              </div>
            )}
            {memory.supersedesId && (
              <div className="st-lifecycle-meta">
                <span><Fingerprint size={10} /> {t('sheet.supersedes')}: {memory.supersedesId.slice(0, 8)}</span>
              </div>
            )}

            <div className="st-lifecycle-actions">
              {!isConfirmed && (
                <button
                  type="button"
                  className="st-lifecycle-confirm"
                  disabled={acting}
                  onClick={() => act(() => memoryConfirm(memory.id))}
                  title={t('sheet.confirm.hint')}
                >
                  <UserCheck size={12} /> {t('sheet.confirm')}
                </button>
              )}
              <div className="st-lifecycle-feedback">
                <span className="st-feedback-label">{t('sheet.feedback.title')}</span>
                <button
                  type="button"
                  className={`st-feedback-btn st-feedback-btn--useful${votedKind === 'useful' ? ' is-voted' : ''}${pendingKind === 'useful' && votedKind !== 'useful' ? ' is-pending' : ''}`}
                  onClick={() => openFeedback('useful')}
                  title={t('sheet.feedback.hint')}
                >
                  {votedKind === 'useful' ? <Check /> : pendingKind === 'useful' ? <CircleHelp /> : <ThumbsUp />}
                  {t('sheet.feedback.useful')}
                </button>
                <button
                  type="button"
                  className={`st-feedback-btn st-feedback-btn--irrelevant${votedKind === 'irrelevant' ? ' is-voted' : ''}${pendingKind === 'irrelevant' && votedKind !== 'irrelevant' ? ' is-pending' : ''}`}
                  onClick={() => openFeedback('irrelevant')}
                  title={t('sheet.feedback.hint')}
                >
                  {votedKind === 'irrelevant' ? <Check /> : pendingKind === 'irrelevant' ? <CircleHelp /> : <ThumbsDown />}
                  {t('sheet.feedback.irrelevant')}
                </button>
                <button
                  type="button"
                  className={`st-feedback-btn st-feedback-btn--wrong${votedKind === 'wrong' ? ' is-voted' : ''}${pendingKind === 'wrong' && votedKind !== 'wrong' ? ' is-pending' : ''}`}
                  onClick={() => openFeedback('wrong')}
                  title={t('sheet.feedback.hint')}
                >
                  {votedKind === 'wrong' ? <Check /> : pendingKind === 'wrong' ? <CircleHelp /> : <AlertTriangle />}
                  {t('sheet.feedback.wrong')}
                </button>
              </div>
            </div>

            {pendingKind && (
              <div className="st-feedback-note">
                <div className="st-feedback-note-head">
                  <span className="st-feedback-note-title">{t('sheet.feedback.note.title')}</span>
                  <InfoTip text={t('sheet.feedback.note.hint')} />
                  <button
                    type="button"
                    className="st-feedback-note-close"
                    onClick={() => setPendingKind(null)}
                    aria-label={t('sheet.feedback.note.close')}
                    title={t('sheet.feedback.note.close')}
                  >
                    <X size={12} />
                  </button>
                </div>
                <textarea
                  className="st-feedback-note-input"
                  value={noteDraft}
                  onChange={(event) => setNoteDraft(event.target.value)}
                  placeholder={t('sheet.feedback.note.placeholder')}
                  rows={3}
                  maxLength={600}
                />
                <div className="st-feedback-note-actions">
                  <button
                    type="button"
                    className="st-feedback-note-send"
                    disabled={noteSending || !noteDraft.trim()}
                    onClick={() => sendNote(pendingKind)}
                  >
                    {noteSent ? <Check size={12} /> : <SendHorizontal size={12} />}
                    {noteSent ? t('sheet.feedback.note.sent') : t('sheet.feedback.note.send')}
                  </button>
                  {feedback.note && !noteSent && (
                    <span className="st-feedback-note-saved">{t('sheet.feedback.note.saved')}</span>
                  )}
                </div>
              </div>
            )}
          </div>
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
