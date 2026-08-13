import { useCallback, useEffect, useState } from 'react';
import type { CSSProperties } from 'react';
import { invoke } from '@tauri-apps/api/core';
import {
  AlertTriangle, CheckCircle2, ClipboardCopy, HeartPulse,
  RefreshCw, ShieldCheck, XCircle,
} from 'lucide-react';
import type { DiagnosticCheck, DiagnosticsExport, DiagnosticsReport } from '../../types';
import { useLocale } from '../../stores/localeStore';
import {
  InfoTip, PageHero, StrataAlert, StrataSkeletons, StrataVoid,
} from '../ui/Instruments';

const STATUS_META: Record<string, { icon: typeof CheckCircle2; color: string }> = {
  ok: { icon: CheckCircle2, color: 'var(--mint)' },
  warning: { icon: AlertTriangle, color: 'var(--gold)' },
  error: { icon: XCircle, color: 'var(--rose)' },
};

function statusMeta(status: string) {
  return STATUS_META[status] ?? { icon: AlertTriangle, color: 'var(--bone)' };
}

// ── One health check row ────────────────────────────────────────────────────

function CheckRow({ check, index }: { check: DiagnosticCheck; index: number }) {
  const { icon: Icon, color } = statusMeta(check.status);
  return (
    <div
      className="st-dx-check st-rise"
      style={{ '--st-dx-color': color, '--st-i': index } as CSSProperties}
    >
      <span className="st-dx-check-icon" style={{ color, background: `${color}15` }}>
        <Icon size={14} />
      </span>
      <span className="st-dx-check-name">{check.name}</span>
      <span
        className={`st-dx-check-status st-dx-check-status--${check.status}`}
      >
        {check.status}
      </span>
      <span className="st-dx-check-message">{check.message}</span>
    </div>
  );
}

// ── Main view ──────────────────────────────────────────────────────────────

export function DiagnosticsView() {
  const { t } = useLocale();
  const [report, setReport] = useState<DiagnosticsReport | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);
  // Bumped on every run so the check rows remount and replay their entrance
  // animation — a real "re-run" feel instead of in-place value swaps.
  const [runId, setRunId] = useState(0);

  const load = useCallback(async () => {
    setBusy(true);
    setError(null);
    try {
      const data = await invoke<DiagnosticsReport>('get_diagnostics_report');
      setReport(data);
      setRunId((n) => n + 1);
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }, []);

  useEffect(() => {
    load();
  }, [load]);

  const exportReport = useCallback(async () => {
    try {
      const data = await invoke<DiagnosticsExport>('export_diagnostics_report');
      await navigator.clipboard.writeText(data.content);
      setCopied(true);
      setTimeout(() => setCopied(false), 1600);
    } catch (e) {
      setError(String(e));
    }
  }, []);

  const counts = report
    ? {
        ok: report.checks.filter((c) => c.status === 'ok').length,
        warning: report.checks.filter((c) => c.status === 'warning').length,
        error: report.checks.filter((c) => c.status === 'error').length,
      }
    : { ok: 0, warning: 0, error: 0 };

  return (
    <div className="st-page" style={{ '--st-accent': 'var(--cyan)' } as CSSProperties}>
      <PageHero
        kicker={t('diagnostics.hero.kicker')}
        title={t('diagnostics.title')}
        copy={t('diagnostics.hero.sub')}
        accent="var(--cyan)"
        secondary="var(--periwinkle)"
        stats={[
          { label: t('diagnostics.stats.ok'), value: report ? String(counts.ok) : '—', color: 'var(--mint)' },
          { label: t('diagnostics.stats.warning'), value: report ? String(counts.warning) : '—', color: 'var(--gold)' },
          { label: t('diagnostics.stats.error'), value: report ? String(counts.error) : '—', color: 'var(--rose)' },
        ]}
      />

      <section className="st-section">
        <div className="st-section-head">
          <h2 className="st-section-title" style={{ '--section-color': 'var(--cyan)' } as CSSProperties}>
            <HeartPulse size={14} /> {t('diagnostics.checks.title')}
          </h2>
          <InfoTip text={t('diagnostics.checks.hint')} />
        </div>
        <p className="st-section-desc">{t('diagnostics.checks.sub')}</p>

        <div className="st-dx-toolbar">
          <button
            type="button"
            className="st-btn st-dx-run"
            onClick={load}
            disabled={busy}
            title={t('diagnostics.run')}
          >
            {busy ? <RefreshCw size={14} className="spinning" /> : <HeartPulse size={14} />}
            {t('diagnostics.run')}
          </button>
          <button
            type="button"
            className="st-btn st-dx-export"
            onClick={exportReport}
            disabled={!report}
            title={t('diagnostics.export.hint')}
          >
            <ClipboardCopy size={14} />
            {copied ? t('diagnostics.export.copied') : t('diagnostics.export')}
          </button>
        </div>

        {error && (
          <StrataAlert icon={AlertTriangle}>{error}</StrataAlert>
        )}

        {busy && !report && <StrataSkeletons />}

        {!busy && !report && !error && (
          <StrataVoid icon={HeartPulse} title={t('diagnostics.empty.title')} accent="var(--cyan)">
            {t('diagnostics.empty.desc')}
          </StrataVoid>
        )}

        {report && (
          <div className="st-dx-list" key={runId}>
            {report.checks.map((check, i) => (
              <CheckRow key={check.name} check={check} index={i} />
            ))}
          </div>
        )}

        {report && report.healthy && (
          <div className="st-dx-healthy">
            <ShieldCheck size={15} />
            {t('diagnostics.healthy')}
          </div>
        )}
      </section>
    </div>
  );
}
