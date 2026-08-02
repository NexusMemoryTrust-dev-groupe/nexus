/**
 * First-run setup wizard.
 *
 * Replaces the PowerShell `first-run-setup.ps1` script. A buyer who installs a
 * desktop app should never be told to open a terminal, and the old script also
 * never registered the MCP server — the single step that makes the product work
 * at all. Every check here is fixable in place: if Node or OpenCode is missing
 * we say what to do, and where we can act ourselves we do.
 *
 * Copy lives in `stores/setupLocale.ts` (ru + en, enforced by a shared type).
 */

import { useCallback, useEffect, useMemo, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import {
  Check, X, Loader2, ArrowRight, ArrowLeft, ExternalLink,
  Copy, Terminal, KeyRound, Cpu, Plug, PartyPopper, ShieldCheck,
} from 'lucide-react';
import { useLocale } from '../../stores/localeStore';
import { NexusLogo } from '../layout/NexusLogo';

// ── Backend contract ────────────────────────────────────────────────────────

interface CheckResult {
  id: 'node' | 'opencode' | 'apiKey' | 'model' | 'mcp';
  ok: boolean;
  detail: string;
  version: string | null;
  fixable: boolean;
}

interface SetupStatus {
  checks: CheckResult[];
  ready: boolean;
  opencodeConfigPath: string;
  databasePath: string;
  executablePath: string;
  tokenMethod: string;
}

interface ModelInfo {
  id: string;
  name: string;
  provider: string;
  is_free: boolean;
}

type StepId = 'welcome' | 'node' | 'opencode' | 'key' | 'model' | 'mcp' | 'done';

const STEPS: StepId[] = ['welcome', 'node', 'opencode', 'key', 'model', 'mcp', 'done'];

const STEP_ICON: Record<StepId, typeof Check> = {
  welcome: PartyPopper,
  node: Terminal,
  opencode: Cpu,
  key: KeyRound,
  model: Cpu,
  mcp: Plug,
  done: PartyPopper,
};

/** Recommended free model — no card required, tuned for long context. */
const RECOMMENDED_MODEL = 'opencode/deepseek-v4-flash-free';

// ── Small presentational pieces ─────────────────────────────────────────────

function StatusPill({ state, label }: { state: 'ok' | 'bad' | 'busy'; label: string }) {
  const tone =
    state === 'ok' ? 'var(--mint)' : state === 'busy' ? 'var(--steel)' : 'var(--rose)';
  const bg =
    state === 'ok' ? 'var(--mint-soft)' : state === 'busy' ? 'var(--steel-soft)' : 'var(--rose-soft)';

  return (
    <span
      style={{
        display: 'inline-flex', alignItems: 'center', gap: 6,
        padding: '4px 10px', borderRadius: 999, background: bg,
        color: tone, fontSize: 12, fontWeight: 600, whiteSpace: 'nowrap',
      }}
    >
      {state === 'ok' && <Check size={13} />}
      {state === 'bad' && <X size={13} />}
      {state === 'busy' && <Loader2 size={13} className="spin" />}
      {label}
    </span>
  );
}

function Why({ children }: { children: React.ReactNode }) {
  return (
    <p style={{
      margin: '0 0 18px', fontSize: 13, lineHeight: 1.65,
      color: 'var(--muted)', maxWidth: '62ch',
    }}>
      {children}
    </p>
  );
}

function CodeLine({ text, copyLabel, copiedLabel }: {
  text: string; copyLabel: string; copiedLabel: string;
}) {
  const [copied, setCopied] = useState(false);

  const copy = async () => {
    try {
      await navigator.clipboard.writeText(text);
      setCopied(true);
      setTimeout(() => setCopied(false), 1600);
    } catch {
      // Clipboard blocked — the text stays selectable, which is enough.
    }
  };

  return (
    <div style={{
      display: 'flex', alignItems: 'center', gap: 10, marginBottom: 14,
      padding: '10px 12px', borderRadius: 8,
      background: 'var(--carbon-soft)', border: '1px solid var(--line)',
    }}>
      <code style={{
        flex: 1, fontSize: 12.5, fontFamily: 'var(--mono, monospace)',
        color: 'var(--bone)', overflowX: 'auto', whiteSpace: 'nowrap',
      }}>
        {text}
      </code>
      <button
        onClick={copy}
        style={{
          display: 'inline-flex', alignItems: 'center', gap: 6,
          padding: '5px 10px', borderRadius: 6, cursor: 'pointer',
          background: copied ? 'var(--mint-soft)' : 'transparent',
          color: copied ? 'var(--mint)' : 'var(--muted)',
          border: '1px solid var(--line)', fontSize: 11.5, fontWeight: 600,
        }}
      >
        <Copy size={12} />
        {copied ? copiedLabel : copyLabel}
      </button>
    </div>
  );
}

function Steps({ text }: { text: string[] }) {
  return (
    <ol style={{
      margin: '0 0 18px', paddingLeft: 20, fontSize: 13,
      lineHeight: 1.9, color: 'var(--bone)', maxWidth: '62ch',
    }}>
      {text.map((s, i) => <li key={i}>{s}</li>)}
    </ol>
  );
}

function PrimaryButton({ onClick, disabled, busy, children }: {
  onClick: () => void; disabled?: boolean; busy?: boolean; children: React.ReactNode;
}) {
  return (
    <button
      onClick={onClick}
      disabled={disabled || busy}
      style={{
        display: 'inline-flex', alignItems: 'center', gap: 8,
        padding: '10px 18px', borderRadius: 8, border: 'none',
        background: disabled || busy ? 'var(--raised)' : 'var(--tangerine)',
        color: disabled || busy ? 'var(--muted)' : '#1a0d05',
        fontSize: 13.5, fontWeight: 700,
        cursor: disabled || busy ? 'not-allowed' : 'pointer',
        transition: 'background 140ms ease, transform 140ms ease',
      }}
    >
      {busy && <Loader2 size={15} className="spin" />}
      {children}
    </button>
  );
}

function GhostButton({ onClick, children }: { onClick: () => void; children: React.ReactNode }) {
  return (
    <button
      onClick={onClick}
      style={{
        display: 'inline-flex', alignItems: 'center', gap: 7,
        padding: '10px 14px', borderRadius: 8, cursor: 'pointer',
        background: 'transparent', color: 'var(--muted)',
        border: '1px solid var(--line)', fontSize: 13, fontWeight: 600,
      }}
    >
      {children}
    </button>
  );
}

// ── Wizard ──────────────────────────────────────────────────────────────────

export function SetupWizard({ onClose }: { onClose: () => void }) {
  const { t } = useLocale();

  const [stepIndex, setStepIndex] = useState(0);
  const [status, setStatus] = useState<SetupStatus | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const [apiKey, setApiKey] = useState('');
  const [keySaved, setKeySaved] = useState(false);
  const [healthState, setHealthState] = useState<'idle' | 'testing' | 'ok' | 'fail'>('idle');
  const [healthDetail, setHealthDetail] = useState('');

  const [models, setModels] = useState<ModelInfo[]>([]);
  const [modelsLoading, setModelsLoading] = useState(false);
  const [chosenModel, setChosenModel] = useState<string>('');

  const step = STEPS[stepIndex];

  const refresh = useCallback(async () => {
    try {
      const s = await invoke<SetupStatus>('setup_status');
      setStatus(s);
      const model = s.checks.find((c) => c.id === 'model');
      if (model?.version) setChosenModel(model.version);
      return s;
    } catch (e) {
      setError(String(e));
      return null;
    }
  }, []);

  useEffect(() => { void refresh(); }, [refresh]);

  const check = useCallback(
    (id: CheckResult['id']) => status?.checks.find((c) => c.id === id),
    [status],
  );

  /** Translate a backend `detail` token into locale copy. */
  const detailCopy = useCallback((c: CheckResult | undefined): string => {
    if (!c) return t('setup.status.checking');
    const map: Record<string, string> = {
      installed: 'setup.status.ok',
      configured: 'setup.key.present',
      notConfigured: 'setup.key.absent',
      notFound: c.id === 'node' ? 'setup.node.absent' : 'setup.opencode.absent',
      notRunnable: 'setup.opencode.absent',
      tooOld: 'setup.node.absent',
      registered: 'setup.mcp.registered',
      notRegistered: 'setup.mcp.absent',
      stalePath: 'setup.mcp.stale',
      selected: 'setup.model.current',
      notSelected: 'setup.model.none',
      opencodeMissing: 'setup.opencode.absent',
    };
    const key = map[c.detail];
    return key ? t(key) : c.detail;
  }, [t]);

  const pillFor = useCallback((id: CheckResult['id']) => {
    const c = check(id);
    if (busy === id) return <StatusPill state="busy" label={t('setup.status.checking')} />;
    if (!c) return <StatusPill state="busy" label={t('setup.status.checking')} />;
    return (
      <StatusPill
        state={c.ok ? 'ok' : 'bad'}
        label={c.ok ? t('setup.status.ok') : t('setup.status.missing')}
      />
    );
  }, [check, busy, t]);

  // ── Actions ──

  const installOpencode = async () => {
    setBusy('opencode'); setError(null);
    try {
      await invoke<string>('install_opencode');
      await refresh();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(null);
    }
  };

  const saveKey = async () => {
    if (!apiKey.trim()) return;
    setBusy('apiKey'); setError(null);
    try {
      await invoke<string>('save_api_key', { key: apiKey.trim() });
      setKeySaved(true);
      setApiKey('');
      await refresh();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(null);
    }
  };

  const testConnection = async () => {
    setHealthState('testing'); setHealthDetail('');
    try {
      const msg = await invoke<string>('ai_health_check', { model: chosenModel || null });
      setHealthState('ok');
      setHealthDetail(msg);
    } catch (e) {
      setHealthState('fail');
      setHealthDetail(String(e));
    }
  };

  const loadModels = useCallback(async () => {
    setModelsLoading(true);
    try {
      const list = await invoke<ModelInfo[]>('ai_list_models', { freeOnly: false });
      setModels(list);
    } catch {
      setModels([]);
    } finally {
      setModelsLoading(false);
    }
  }, []);

  useEffect(() => {
    if (step === 'model' && models.length === 0 && !modelsLoading) void loadModels();
  }, [step, models.length, modelsLoading, loadModels]);

  const pickModel = async (id: string) => {
    setChosenModel(id);
    try {
      await invoke('select_model', { model: id });
      await refresh();
    } catch (e) {
      setError(String(e));
    }
  };

  const registerMcp = async () => {
    setBusy('mcp'); setError(null);
    try {
      await invoke<unknown>('register_mcp');
      await refresh();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(null);
    }
  };

  const finish = async () => {
    try { await invoke('complete_setup'); } catch { /* non-fatal */ }
    onClose();
  };

  // ── Auto-advance conveniences ──

  const next = () => setStepIndex((i) => Math.min(i + 1, STEPS.length - 1));
  const back = () => setStepIndex((i) => Math.max(i - 1, 0));

  const freeModels = useMemo(() => models.filter((m) => m.is_free), [models]);
  const modelList = useMemo(() => {
    // Recommended first, then the rest of the free tier, then everything else.
    const rec = models.filter((m) => m.id === RECOMMENDED_MODEL);
    const otherFree = freeModels.filter((m) => m.id !== RECOMMENDED_MODEL);
    const paid = models.filter((m) => !m.is_free);
    return [...rec, ...otherFree, ...paid];
  }, [models, freeModels]);

  // ── Step bodies ──

  const body = () => {
    switch (step) {
      case 'welcome':
        return (
          <>
            <div style={{ marginBottom: 20 }}>
              <NexusLogo size={72} />
            </div>
            <h2 style={{ margin: '0 0 12px', fontSize: 24, fontWeight: 800, color: 'var(--bone)', lineHeight: 1.25, maxWidth: '26ch' }}>
              {t('setup.welcome.heading')}
            </h2>
            <Why>{t('setup.welcome.body')}</Why>
            <ul style={{ margin: '0 0 20px', paddingLeft: 18, fontSize: 13, lineHeight: 2, color: 'var(--bone)' }}>
              <li>{t('setup.welcome.point1')}</li>
              <li>{t('setup.welcome.point2')}</li>
              <li>{t('setup.welcome.point3')}</li>
            </ul>
            <p style={{ margin: '0 0 22px', fontSize: 12.5, color: 'var(--muted)' }}>
              {t('setup.welcome.time')}
            </p>
            <PrimaryButton onClick={next}>
              {t('setup.welcome.start')} <ArrowRight size={15} />
            </PrimaryButton>
          </>
        );

      case 'node': {
        const c = check('node');
        return (
          <>
            <h2 style={{ margin: '0 0 8px', fontSize: 20, fontWeight: 700, color: 'var(--bone)' }}>
              {t('setup.node.heading')}
            </h2>
            <div style={{ marginBottom: 14 }}>{pillFor('node')}</div>
            <Why>{t('setup.node.why')}</Why>
            {c?.ok ? (
              <p style={{ fontSize: 13, color: 'var(--mint)', marginBottom: 20 }}>
                {t('setup.node.found')} {c.version ?? ''}
              </p>
            ) : (
              <>
                <Steps text={[t('setup.node.how1'), t('setup.node.how2'), t('setup.node.how3')]} />
                <a
                  href="https://nodejs.org/en/download"
                  target="_blank"
                  rel="noreferrer"
                  style={{
                    display: 'inline-flex', alignItems: 'center', gap: 7,
                    fontSize: 13, fontWeight: 600, color: 'var(--tangerine)',
                    textDecoration: 'none', marginBottom: 20,
                  }}
                >
                  {t('setup.node.download')} <ExternalLink size={13} />
                </a>
              </>
            )}
          </>
        );
      }

      case 'opencode': {
        const c = check('opencode');
        const nodeOk = check('node')?.ok ?? false;
        return (
          <>
            <h2 style={{ margin: '0 0 8px', fontSize: 20, fontWeight: 700, color: 'var(--bone)' }}>
              {t('setup.opencode.heading')}
            </h2>
            <div style={{ marginBottom: 14 }}>{pillFor('opencode')}</div>
            <Why>{t('setup.opencode.why')}</Why>
            {c?.ok ? (
              <p style={{ fontSize: 13, color: 'var(--mint)', marginBottom: 20 }}>
                {t('setup.opencode.found')} {c.version ?? ''}
              </p>
            ) : (
              <>
                <PrimaryButton onClick={installOpencode} busy={busy === 'opencode'} disabled={!nodeOk}>
                  {busy === 'opencode' ? t('setup.opencode.installing') : t('setup.opencode.install')}
                </PrimaryButton>
                <p style={{ fontSize: 12.5, color: 'var(--muted)', margin: '16px 0 8px' }}>
                  {t('setup.opencode.manual')}
                </p>
                <CodeLine
                  text="npm install -g opencode-ai"
                  copyLabel={t('setup.copy')}
                  copiedLabel={t('setup.copied')}
                />
              </>
            )}
          </>
        );
      }

      case 'key': {
        const c = check('apiKey');
        return (
          <>
            <h2 style={{ margin: '0 0 8px', fontSize: 20, fontWeight: 700, color: 'var(--bone)' }}>
              {t('setup.key.heading')}
            </h2>
            <div style={{ marginBottom: 14 }}>{pillFor('apiKey')}</div>
            <Why>{t('setup.key.why')}</Why>
            <p style={{
              margin: '0 0 18px', padding: '10px 12px', borderRadius: 8,
              background: 'var(--mint-soft)', color: 'var(--mint)',
              fontSize: 12.5, lineHeight: 1.6, maxWidth: '62ch',
            }}>
              {t('setup.key.free')}
            </p>

            <h3 style={{ margin: '0 0 10px', fontSize: 14, fontWeight: 700, color: 'var(--bone)' }}>
              {t('setup.key.whereHeading')}
            </h3>
            <Steps text={[
              t('setup.key.where1'), t('setup.key.where2'),
              t('setup.key.where3'), t('setup.key.where4'),
            ]} />

            <div style={{ display: 'flex', gap: 10, marginBottom: 12, maxWidth: 560 }}>
              <input
                type="password"
                value={apiKey}
                onChange={(e) => setApiKey(e.target.value)}
                placeholder={t('setup.key.placeholder')}
                style={{
                  flex: 1, padding: '10px 12px', borderRadius: 8,
                  background: 'var(--carbon-soft)', color: 'var(--bone)',
                  border: '1px solid var(--line)', fontSize: 13,
                  fontFamily: 'var(--mono, monospace)',
                }}
              />
              <PrimaryButton onClick={saveKey} busy={busy === 'apiKey'} disabled={!apiKey.trim()}>
                {t('setup.key.save')}
              </PrimaryButton>
            </div>

            {(keySaved || c?.ok) && (
              <p style={{ fontSize: 12.5, color: 'var(--mint)', marginBottom: 12 }}>
                {t('setup.key.saved')}
              </p>
            )}

            <div style={{ display: 'flex', alignItems: 'center', gap: 12, marginBottom: 10 }}>
              <GhostButton onClick={testConnection}>
                {healthState === 'testing' ? <Loader2 size={14} className="spin" /> : <ShieldCheck size={14} />}
                {healthState === 'testing' ? t('setup.key.testing') : t('setup.key.test')}
              </GhostButton>
              {healthState === 'ok' && <StatusPill state="ok" label={t('setup.key.testOk')} />}
              {healthState === 'fail' && <StatusPill state="bad" label={t('setup.key.testFail')} />}
            </div>
            {healthDetail && (
              <p style={{
                fontSize: 11.5, color: 'var(--muted)', marginBottom: 14,
                fontFamily: 'var(--mono, monospace)', wordBreak: 'break-word', maxWidth: '70ch',
              }}>
                {healthDetail}
              </p>
            )}

            <p style={{ fontSize: 11.5, color: 'var(--muted)', maxWidth: '62ch' }}>
              {t('setup.key.privacy')}
            </p>
          </>
        );
      }

      case 'model':
        return (
          <>
            <h2 style={{ margin: '0 0 8px', fontSize: 20, fontWeight: 700, color: 'var(--bone)' }}>
              {t('setup.model.heading')}
            </h2>
            <div style={{ marginBottom: 14 }}>{pillFor('model')}</div>
            <Why>{t('setup.model.why')}</Why>

            {modelsLoading && (
              <p style={{ fontSize: 13, color: 'var(--muted)' }}>
                <Loader2 size={14} className="spin" /> {t('setup.model.loading')}
              </p>
            )}

            {!modelsLoading && modelList.length === 0 && (
              <>
                <p style={{ fontSize: 13, color: 'var(--muted)', marginBottom: 14 }}>
                  {t('setup.model.none')}
                </p>
                <CodeLine
                  text={RECOMMENDED_MODEL}
                  copyLabel={t('setup.copy')}
                  copiedLabel={t('setup.copied')}
                />
              </>
            )}

            <div style={{ display: 'grid', gap: 8, maxWidth: 620 }}>
              {modelList.slice(0, 12).map((m) => {
                const active = chosenModel === m.id;
                const recommended = m.id === RECOMMENDED_MODEL;
                return (
                  <button
                    key={m.id}
                    onClick={() => void pickModel(m.id)}
                    style={{
                      display: 'flex', alignItems: 'center', gap: 10,
                      padding: '11px 13px', borderRadius: 8, cursor: 'pointer',
                      textAlign: 'left',
                      background: active ? 'var(--tangerine-soft)' : 'var(--carbon-soft)',
                      border: `1px solid ${active ? 'var(--tangerine)' : 'var(--line)'}`,
                    }}
                  >
                    <span style={{
                      width: 16, height: 16, borderRadius: 999, flexShrink: 0,
                      border: `2px solid ${active ? 'var(--tangerine)' : 'var(--line)'}`,
                      background: active ? 'var(--tangerine)' : 'transparent',
                    }} />
                    <span style={{ flex: 1, minWidth: 0 }}>
                      <span style={{
                        display: 'block', fontSize: 13, fontWeight: 600,
                        color: 'var(--bone)', overflow: 'hidden', textOverflow: 'ellipsis',
                      }}>
                        {m.name}
                      </span>
                      <span style={{ fontSize: 11.5, color: 'var(--muted)' }}>{m.provider}</span>
                    </span>
                    {recommended && (
                      <span style={{
                        padding: '3px 8px', borderRadius: 999, fontSize: 11, fontWeight: 700,
                        background: 'var(--tangerine-soft)', color: 'var(--tangerine)',
                      }}>
                        {t('setup.model.recommended')}
                      </span>
                    )}
                    {m.is_free && !recommended && (
                      <span style={{
                        padding: '3px 8px', borderRadius: 999, fontSize: 11, fontWeight: 700,
                        background: 'var(--mint-soft)', color: 'var(--mint)',
                      }}>
                        {t('setup.model.freeBadge')}
                      </span>
                    )}
                  </button>
                );
              })}
            </div>
          </>
        );

      case 'mcp': {
        const c = check('mcp');
        return (
          <>
            <h2 style={{ margin: '0 0 8px', fontSize: 20, fontWeight: 700, color: 'var(--bone)' }}>
              {t('setup.mcp.heading')}
            </h2>
            <div style={{ marginBottom: 14 }}>{pillFor('mcp')}</div>
            <Why>{t('setup.mcp.why')}</Why>

            {c?.ok ? (
              <p style={{ fontSize: 13, color: 'var(--mint)', marginBottom: 16 }}>
                {t('setup.mcp.registered')}
              </p>
            ) : (
              <>
                <p style={{ fontSize: 13, color: 'var(--muted)', marginBottom: 14 }}>
                  {detailCopy(c)}
                </p>
                <div style={{ marginBottom: 18 }}>
                  <PrimaryButton onClick={registerMcp} busy={busy === 'mcp'}>
                    {busy === 'mcp' ? t('setup.mcp.registering') : t('setup.mcp.register')}
                  </PrimaryButton>
                </div>
              </>
            )}

            {status && (
              <p style={{
                fontSize: 11.5, color: 'var(--muted)', marginBottom: 14,
                fontFamily: 'var(--mono, monospace)', wordBreak: 'break-all',
              }}>
                {t('setup.mcp.configAt')}: {status.opencodeConfigPath}
              </p>
            )}

            <p style={{
              margin: 0, padding: '10px 12px', borderRadius: 8,
              background: 'var(--steel-soft)', color: 'var(--steel)',
              fontSize: 12.5, lineHeight: 1.6, maxWidth: '62ch',
            }}>
              <ShieldCheck size={13} style={{ verticalAlign: '-2px', marginRight: 6 }} />
              {t('setup.mcp.sandbox')}
            </p>
          </>
        );
      }

      case 'done':
        return (
          <>
            <div style={{ marginBottom: 18 }}>
              <NexusLogo size={64} />
            </div>
            <h2 style={{ margin: '0 0 12px', fontSize: 22, fontWeight: 800, color: 'var(--bone)' }}>
              {t('setup.done.heading')}
            </h2>
            <Why>{status?.ready ? t('setup.done.body') : t('setup.done.partial')}</Why>
            <ol style={{
              margin: '0 0 22px', paddingLeft: 20, fontSize: 13,
              lineHeight: 1.95, color: 'var(--bone)', maxWidth: '62ch',
            }}>
              <li>{t('setup.done.next1')}</li>
              <li>{t('setup.done.next2')}</li>
              <li>{t('setup.done.next3')}</li>
            </ol>
            <PrimaryButton onClick={finish}>
              {t('setup.done.launch')} <ArrowRight size={15} />
            </PrimaryButton>
          </>
        );
    }
  };

  // ── Shell ──

  const StepIcon = STEP_ICON[step];

  return (
    <div
      style={{
        position: 'fixed', inset: 0, zIndex: 9000,
        display: 'flex', alignItems: 'center', justifyContent: 'center',
        background: 'rgba(6, 7, 10, 0.88)', backdropFilter: 'blur(10px)',
        padding: 24,
      }}
    >
      <div
        style={{
          display: 'flex', width: '100%', maxWidth: 940, maxHeight: '88vh',
          borderRadius: 16, overflow: 'hidden',
          background: 'var(--surface)', border: '1px solid var(--line)',
          boxShadow: '0 28px 80px rgba(0,0,0,0.6)',
        }}
      >
        {/* Rail */}
        <nav
          style={{
            width: 232, flexShrink: 0, padding: '24px 18px',
            background: 'var(--carbon-soft)', borderRight: '1px solid var(--line)',
            display: 'flex', flexDirection: 'column', gap: 4,
          }}
        >
          <div style={{ marginBottom: 18 }}>
            <div style={{ fontSize: 14, fontWeight: 800, color: 'var(--bone)' }}>
              {t('setup.title')}
            </div>
            <div style={{ fontSize: 11.5, color: 'var(--muted)', marginTop: 3 }}>
              {t('setup.subtitle')}
            </div>
          </div>

          {STEPS.map((s, i) => {
            const Icon = STEP_ICON[s];
            const active = i === stepIndex;
            const passed = i < stepIndex;
            const relatedCheck =
              s === 'node' ? check('node')
              : s === 'opencode' ? check('opencode')
              : s === 'key' ? check('apiKey')
              : s === 'model' ? check('model')
              : s === 'mcp' ? check('mcp')
              : undefined;

            return (
              <button
                key={s}
                onClick={() => setStepIndex(i)}
                style={{
                  display: 'flex', alignItems: 'center', gap: 10,
                  padding: '9px 10px', borderRadius: 8, cursor: 'pointer',
                  border: 'none', textAlign: 'left',
                  background: active ? 'var(--tangerine-soft)' : 'transparent',
                  color: active ? 'var(--tangerine)' : passed ? 'var(--bone)' : 'var(--muted)',
                  fontSize: 12.5, fontWeight: active ? 700 : 500,
                }}
              >
                <Icon size={15} style={{ flexShrink: 0 }} />
                <span style={{ flex: 1, minWidth: 0, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
                  {t(`setup.step.${s}`)}
                </span>
                {relatedCheck?.ok && <Check size={13} style={{ color: 'var(--mint)', flexShrink: 0 }} />}
              </button>
            );
          })}

          <div style={{ marginTop: 'auto', paddingTop: 16 }}>
            <GhostButton onClick={() => void refresh()}>{t('setup.recheck')}</GhostButton>
            <button
              onClick={finish}
              style={{
                display: 'block', marginTop: 10, padding: 0,
                background: 'none', border: 'none', cursor: 'pointer',
                color: 'var(--muted)', fontSize: 11.5, textDecoration: 'underline',
              }}
            >
              {t('setup.skip')}
            </button>
          </div>
        </nav>

        {/* Panel */}
        <section style={{ flex: 1, display: 'flex', flexDirection: 'column', minWidth: 0 }}>
          <header style={{
            display: 'flex', alignItems: 'center', gap: 10,
            padding: '16px 24px', borderBottom: '1px solid var(--line)',
          }}>
            <StepIcon size={16} style={{ color: 'var(--tangerine)' }} />
            <span style={{ fontSize: 12.5, color: 'var(--muted)', fontWeight: 600 }}>
              {t('setup.progress')
                .replace('{current}', String(stepIndex + 1))
                .replace('{total}', String(STEPS.length))}
            </span>
            {status && (
              <span style={{ marginLeft: 'auto', fontSize: 11.5, color: 'var(--muted)' }}>
                {status.tokenMethod === 'exact'
                  ? t('setup.tokens.exact')
                  : t('setup.tokens.estimated')}
              </span>
            )}
          </header>

          <div style={{ flex: 1, overflowY: 'auto', padding: '28px 30px' }}>
            {error && (
              <div style={{
                marginBottom: 18, padding: '10px 12px', borderRadius: 8,
                background: 'var(--rose-soft)', color: 'var(--rose)',
                fontSize: 12.5, lineHeight: 1.6, wordBreak: 'break-word',
              }}>
                {error}
              </div>
            )}
            {body()}
          </div>

          <footer style={{
            display: 'flex', alignItems: 'center', gap: 10,
            padding: '14px 24px', borderTop: '1px solid var(--line)',
          }}>
            {stepIndex > 0 && (
              <GhostButton onClick={back}>
                <ArrowLeft size={14} /> {t('setup.back')}
              </GhostButton>
            )}
            <div style={{ marginLeft: 'auto', display: 'flex', gap: 10 }}>
              {step !== 'done' && step !== 'welcome' && (
                <PrimaryButton onClick={next}>
                  {t('setup.next')} <ArrowRight size={15} />
                </PrimaryButton>
              )}
            </div>
          </footer>
        </section>
      </div>
    </div>
  );
}

export default SetupWizard;
