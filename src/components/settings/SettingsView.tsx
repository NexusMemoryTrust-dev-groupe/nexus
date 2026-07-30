import { useState, useEffect, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import {
  Database, HardDrive, Brain, GitBranch, Layers, Clock,
  Settings2, Save, RotateCcw, RefreshCw, Check, AlertCircle,
  Key, Shield,
} from 'lucide-react';
import { useUiStore } from '../../stores/uiStore';
import { useLocale } from '../../stores/localeStore';

interface ConfigEntry {
  key: string;
  value: string;
}

interface DbStats {
  memory_count: number;
  entity_count: number;
  relationship_count: number;
  commit_count: number;
  snapshot_count: number;
  db_size_bytes: number;
}

function formatBytes(bytes: number): string {
  if (bytes === 0) return '0 B';
  const k = 1024;
  const sizes = ['B', 'KB', 'MB', 'GB'];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return parseFloat((bytes / Math.pow(k, i)).toFixed(1)) + ' ' + sizes[i];
}

interface SettingDef {
  key: string;
  labelKey: string;
  defaultValue: string;
  type: 'toggle' | 'text' | 'number' | 'select';
  options?: string[];
  apply?: (value: string) => void;
}

const defaultSettings: SettingDef[] = [
  {
    key: 'app.language',
    labelKey: 'settings.language',
    defaultValue: 'en',
    type: 'select',
    options: ['en', 'ru'],
    apply: (v) => {
      document.documentElement.setAttribute('lang', v);
    },
  },
  {
    key: 'memory.auto_archive',
    labelKey: 'settings.autoArchive',
    defaultValue: 'false',
    type: 'toggle',
  },
  {
    key: 'graph.show_labels',
    labelKey: 'settings.showLabels',
    defaultValue: 'true',
    type: 'toggle',
  },
  {
    key: 'versioning.auto_commit',
    labelKey: 'settings.autoCommit',
    defaultValue: 'true',
    type: 'toggle',
  },
];

export function SettingsView() {
  const { setActiveView } = useUiStore();
  const { t, setLocale } = useLocale();
  const [config, setConfig] = useState<ConfigEntry[]>([]);
  const [stats, setStats] = useState<DbStats | null>(null);
  const [editingKey, setEditingKey] = useState<string | null>(null);
  const [editValue, setEditValue] = useState('');
  const [savedKey, setSavedKey] = useState<string | null>(null);
  const [errorKey, setErrorKey] = useState<string | null>(null);

  const loadConfig = useCallback(async () => {
    try {
      const entries = await invoke<ConfigEntry[]>('get_all_config');
      const merged = defaultSettings.map((def) => {
        const found = entries.find((e) => e.key === def.key);
        return { key: def.key, value: found ? found.value : def.defaultValue };
      });
      setConfig(merged);

      // Apply all side effects on load
      for (const entry of merged) {
        const def = defaultSettings.find((d) => d.key === entry.key);
        if (def?.apply) def.apply(entry.value);

        // Special handling for language
        if (entry.key === 'app.language') {
          setLocale(entry.value as 'en' | 'ru');
        }
      }
    } catch {
      setConfig(defaultSettings.map((d) => ({ key: d.key, value: d.defaultValue })));
    }
  }, [setLocale]);

  const loadStats = useCallback(async () => {
    try {
      const s = await invoke<DbStats>('get_db_stats');
      setStats(s);
    } catch {
      // Stats unavailable
    }
  }, []);

  useEffect(() => {
    loadConfig();
    loadStats();
  }, [loadConfig, loadStats]);

  const saveConfig = useCallback(async (key: string, value: string) => {
    try {
      await invoke('set_config', { key, value });
      setConfig((prev) => prev.map((e) => (e.key === key ? { ...e, value } : e)));

      // Apply side effects
      const def = defaultSettings.find((d) => d.key === key);
      if (def?.apply) def.apply(value);

      // Special handling for language
      if (key === 'app.language') {
        setLocale(value as 'en' | 'ru');
      }

      // Show success feedback
      setSavedKey(key);
      setTimeout(() => setSavedKey(null), 1500);
    } catch (err) {
      console.error('Failed to save config:', err);
      setErrorKey(key);
      setTimeout(() => setErrorKey(null), 2000);
    }
  }, [setLocale]);

  const resetToDefaults = useCallback(async () => {
    for (const def of defaultSettings) {
      await saveConfig(def.key, def.defaultValue);
    }
  }, [saveConfig]);

  const getValue = (key: string): string => {
    return config.find((c) => c.key === key)?.value || '';
  };

  const sections = [
    { titleKey: 'settings.application', icon: Settings2, prefix: 'app.' },
    { titleKey: 'settings.memory', icon: Brain, prefix: 'memory.' },
    { titleKey: 'settings.graph', icon: Layers, prefix: 'graph.' },
    { titleKey: 'settings.versioning', icon: GitBranch, prefix: 'versioning.' },
  ];

  const [aiHealth, setAiHealth] = useState<string | null>(null);
  const [checkingHealth, setCheckingHealth] = useState(false);

  // ── Model selector state ──
  interface ModelInfo {
    id: string;
    name: string;
    provider: string;
    is_free: boolean;
  }
  const [models, setModels] = useState<ModelInfo[]>([]);
  const [modelsLoading, setModelsLoading] = useState(false);
  const [modelsError, setModelsError] = useState<string | null>(null);
  const [showFreeOnly, setShowFreeOnly] = useState(true);

  const loadModels = useCallback(async () => {
    setModelsLoading(true);
    setModelsError(null);
    try {
      const result = await invoke<ModelInfo[]>('ai_list_models', { freeOnly: showFreeOnly });
      setModels(result);
    } catch (err) {
      setModelsError(String(err));
      setModels([]);
    } finally {
      setModelsLoading(false);
    }
  }, [showFreeOnly]);

  useEffect(() => {
    loadModels();
  }, [loadModels]);

  const checkAiHealth = useCallback(async (modelOverride?: string) => {
    setCheckingHealth(true);
    try {
      const result = await invoke<string>('ai_health_check', { model: modelOverride || null });
      setAiHealth(result);
    } catch {
      setAiHealth('Error: unable to check');
    } finally {
      setCheckingHealth(false);
    }
  }, []);

  const statItems = stats
    ? [
        { labelKey: 'settings.memories', value: stats.memory_count, icon: Brain },
        { labelKey: 'settings.entities', value: stats.entity_count, icon: Layers },
        { labelKey: 'settings.relationships', value: stats.relationship_count, icon: GitBranch },
        { labelKey: 'settings.commits', value: stats.commit_count, icon: Clock },
        { labelKey: 'settings.snapshots', value: stats.snapshot_count, icon: Database },
        { labelKey: 'settings.dbSize', value: formatBytes(stats.db_size_bytes), icon: HardDrive },
      ]
    : [];

  return (
    <div style={{ maxWidth: '720px', margin: '0 auto', padding: '32px 24px' }}>
      {/* Header */}
      <div style={{ display: 'flex', alignItems: 'flex-start', justifyContent: 'space-between', marginBottom: '32px' }}>
        <div>
          <h1 style={{
            fontFamily: 'var(--brand)',
            fontSize: '28px',
            fontWeight: 700,
            color: 'var(--bone)',
            letterSpacing: '-0.02em',
            margin: 0,
          }}>
            {t('settings.title')}
          </h1>
          <p style={{
            fontSize: 'var(--text)',
            color: 'var(--muted)',
            marginTop: '6px',
          }}>
            {t('settings.subtitle')}
          </p>
        </div>
        <div style={{ display: 'flex', gap: '8px' }}>
          <button
            className="settings-action-btn"
            onClick={loadConfig}
          >
            <RefreshCw size={13} className="settings-action-icon" /> {t('settings.reload')}
          </button>
          <button
            className="settings-action-btn"
            onClick={resetToDefaults}
          >
            <RotateCcw size={13} className="settings-action-icon" /> {t('settings.reset')}
          </button>
        </div>
      </div>

      {/* Database Stats */}
      {statItems.length > 0 && (
        <div style={{ marginBottom: '32px' }}>
          <h2 style={{
            fontSize: '13px', fontWeight: 600, color: 'var(--muted-2)',
            textTransform: 'uppercase', letterSpacing: '0.08em', marginBottom: '12px',
          }}>
            {t('settings.database')}
          </h2>
          <div style={{ display: 'grid', gridTemplateColumns: 'repeat(3, 1fr)', gap: '10px' }}>
            {statItems.map((item) => (
              <div key={item.labelKey} style={{
                background: 'var(--surface)', border: '1px solid var(--line)',
                borderRadius: 'var(--radius-sm)', padding: '14px 16px',
              }}>
                <div style={{ display: 'flex', alignItems: 'center', gap: '8px', marginBottom: '6px' }}>
                  <item.icon size={14} style={{ color: 'var(--muted-2)' }} />
                  <span style={{ fontSize: '10px', color: 'var(--muted-2)', textTransform: 'uppercase', letterSpacing: '0.05em' }}>
                    {t(item.labelKey)}
                  </span>
                </div>
                <div style={{ fontSize: '22px', fontWeight: 700, color: 'var(--bone)', fontFamily: 'var(--brand)' }}>
                  {item.value}
                </div>
              </div>
            ))}
          </div>
        </div>
      )}

      {/* Config Sections */}
      {sections.map((section) => {
        const items = defaultSettings.filter((d) => d.key.startsWith(section.prefix));
        return (
          <div key={section.titleKey} style={{ marginBottom: '28px' }}>
            <h2 style={{
              fontSize: '13px', fontWeight: 600, color: 'var(--muted-2)',
              textTransform: 'uppercase', letterSpacing: '0.08em',
              marginBottom: '10px', display: 'flex', alignItems: 'center', gap: '8px',
            }}>
              <section.icon size={14} />
              {t(section.titleKey)}
            </h2>
            <div style={{
              background: 'var(--surface)', border: '1px solid var(--line)',
              borderRadius: 'var(--radius-sm)', overflow: 'hidden',
            }}>
              {items.map((def, idx) => {
                const value = getValue(def.key);
                const isEditing = editingKey === def.key;
                const justSaved = savedKey === def.key;
                const hasError = errorKey === def.key;

                return (
                  <div key={def.key} style={{
                    display: 'flex', alignItems: 'center', justifyContent: 'space-between',
                    padding: '12px 16px',
                    borderBottom: idx < items.length - 1 ? '1px solid var(--line)' : 'none',
                    transition: 'background 0.2s',
                    background: justSaved ? 'rgba(117, 212, 161, 0.05)' : hasError ? 'rgba(255, 112, 133, 0.05)' : 'transparent',
                  }}>
                    <div>
                      <div style={{ fontSize: '14px', fontWeight: 500, color: 'var(--bone)' }}>
                        {t(def.labelKey)}
                      </div>
                      <div style={{ fontSize: '10px', color: 'var(--muted-2)', marginTop: '2px', fontFamily: 'var(--mono)' }}>
                        {def.key}
                      </div>
                    </div>
                    <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
                      {/* Success/error indicator */}
                      {justSaved && <Check size={14} style={{ color: 'var(--mint)' }} />}
                      {hasError && <AlertCircle size={14} style={{ color: 'var(--rose)' }} />}

                      {isEditing && def.type === 'text' ? (
                        /* Text edit mode */
                        <>
                          <input
                            type="text"
                            value={editValue}
                            onChange={(e) => setEditValue(e.target.value)}
                            onKeyDown={(e) => {
                              if (e.key === 'Enter') {
                                saveConfig(def.key, editValue);
                                setEditingKey(null);
                              }
                              if (e.key === 'Escape') setEditingKey(null);
                            }}
                            onBlur={() => {
                              if (editValue !== value) saveConfig(def.key, editValue);
                              setEditingKey(null);
                            }}
                            autoFocus
                            style={{
                              background: 'var(--carbon)', border: '1px solid var(--tangerine)',
                              borderRadius: 'var(--radius-xs)', padding: '4px 8px',
                              fontSize: '13px', color: 'var(--bone)', outline: 'none',
                              width: '140px', fontFamily: 'var(--mono)',
                            }}
                          />
                          <button
                            onClick={() => { saveConfig(def.key, editValue); setEditingKey(null); }}
                            style={{
                              background: 'var(--tangerine)', border: 'none',
                              borderRadius: 'var(--radius-xs)', padding: '4px 8px',
                              cursor: 'pointer', display: 'flex', alignItems: 'center',
                              color: 'var(--carbon)',
                            }}
                          >
                            <Save size={14} />
                          </button>
                        </>
                      ) : isEditing && def.type === 'select' ? (
                        /* Select dropdown */
                        <select
                          value={value}
                          onChange={(e) => {
                            saveConfig(def.key, e.target.value);
                            setEditingKey(null);
                          }}
                          onBlur={() => setEditingKey(null)}
                          autoFocus
                          style={{
                            background: 'var(--carbon)', border: '1px solid var(--tangerine)',
                            borderRadius: 'var(--radius-xs)', padding: '4px 8px',
                            fontSize: '13px', color: 'var(--bone)', outline: 'none',
                            cursor: 'pointer',
                          }}
                        >
                          {def.options?.map((opt) => (
                            <option key={opt} value={opt}>{opt}</option>
                          ))}
                        </select>
                      ) : def.type === 'toggle' ? (
                        /* Toggle button */
                        <button
                          onClick={() => {
                            const next = value === 'true' ? 'false' : 'true';
                            saveConfig(def.key, next);
                          }}
                          style={{
                            background: value === 'true' ? 'var(--mint-soft)' : 'var(--raised)',
                            border: '1px solid',
                            borderColor: value === 'true' ? 'var(--mint)' : 'var(--line)',
                            borderRadius: '999px', padding: '5px 14px',
                            cursor: 'pointer', fontSize: '12px', fontWeight: 600,
                            color: value === 'true' ? 'var(--mint)' : 'var(--muted-2)',
                            transition: 'all 0.2s',
                            minWidth: '52px',
                          }}
                        >
                          {value === 'true' ? t('settings.on') : t('settings.off')}
                        </button>
                      ) : (
                        /* Clickable value display — opens editor */
                        <button
                          onClick={() => {
                            setEditingKey(def.key);
                            setEditValue(value);
                          }}
                          style={{
                            background: 'var(--raised)', border: '1px solid var(--line)',
                            borderRadius: 'var(--radius-xs)', padding: '5px 12px',
                            cursor: 'pointer', fontSize: '13px', color: 'var(--bone)',
                            fontFamily: 'var(--mono)', minWidth: '60px', textAlign: 'center',
                            transition: 'border-color 0.15s',
                          }}
                          onMouseEnter={(e) => (e.currentTarget.style.borderColor = 'var(--muted-2)')}
                          onMouseLeave={(e) => (e.currentTarget.style.borderColor = 'var(--line)')}
                        >
                          {value}
                        </button>
                      )}
                    </div>
                  </div>
                );
              })}
            </div>
          </div>
        );
      })}

      {/* AI Integration */}
      <div style={{ marginBottom: '28px' }}>
        <h2 style={{
          fontSize: '13px', fontWeight: 600, color: 'var(--muted-2)',
          textTransform: 'uppercase', letterSpacing: '0.08em',
          marginBottom: '10px', display: 'flex', alignItems: 'center', gap: '8px',
        }}>
          <Shield size={14} />
          {t('settings.ai')}
        </h2>
        <div style={{
          background: 'var(--surface)', border: '1px solid var(--line)',
          borderRadius: 'var(--radius-sm)', overflow: 'hidden',
        }}>
          {/* API Key */}
          <div style={{
            display: 'flex', alignItems: 'center', justifyContent: 'space-between',
            padding: '14px 18px', borderBottom: '1px solid var(--line)',
          }}>
            <div>
              <div style={{ fontSize: '14px', fontWeight: 500, color: 'var(--bone)', display: 'flex', alignItems: 'center', gap: '8px' }}>
                <Key size={14} style={{ color: 'var(--periwinkle)' }} />
                {t('settings.apiKey')}
              </div>
              <div style={{ fontSize: '10px', color: 'var(--muted-2)', marginTop: '3px' }}>
                {t('settings.apiKeyHint')}
              </div>
            </div>
            <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
              {savedKey === 'ai.opencode_api_key' && <Check size={14} style={{ color: 'var(--mint)' }} />}
              {errorKey === 'ai.opencode_api_key' && <AlertCircle size={14} style={{ color: 'var(--rose)' }} />}
              {editingKey === 'ai.opencode_api_key' ? (
                <>
                  <input
                    type="password"
                    value={editValue}
                    onChange={(e) => setEditValue(e.target.value)}
                    onKeyDown={(e) => {
                      if (e.key === 'Enter') {
                        saveConfig('ai.opencode_api_key', editValue);
                        setEditingKey(null);
                      }
                      if (e.key === 'Escape') setEditingKey(null);
                    }}
                    onBlur={() => {
                      if (editValue !== getValue('ai.opencode_api_key')) {
                        saveConfig('ai.opencode_api_key', editValue);
                      }
                      setEditingKey(null);
                    }}
                    autoFocus
                    placeholder="sk-..."
                    style={{
                      background: 'var(--carbon)', border: '1px solid var(--tangerine)',
                      borderRadius: 'var(--radius-xs)', padding: '5px 10px',
                      fontSize: '13px', color: 'var(--bone)', outline: 'none',
                      width: '220px', fontFamily: 'var(--mono)',
                    }}
                  />
                  <button
                    onClick={() => { saveConfig('ai.opencode_api_key', editValue); setEditingKey(null); }}
                    style={{
                      background: 'var(--tangerine)', border: 'none',
                      borderRadius: 'var(--radius-xs)', padding: '5px 10px',
                      cursor: 'pointer', display: 'flex', alignItems: 'center',
                      color: 'var(--carbon)',
                    }}
                  >
                    <Save size={14} />
                  </button>
                </>
              ) : (
                <button
                  onClick={() => { setEditingKey('ai.opencode_api_key'); setEditValue(getValue('ai.opencode_api_key')); }}
                  style={{
                    background: 'var(--raised)', border: '1px solid var(--line)',
                    borderRadius: 'var(--radius-xs)', padding: '5px 14px',
                    cursor: 'pointer', fontSize: '13px', color: 'var(--bone)',
                    fontFamily: 'var(--mono)', minWidth: '80px', textAlign: 'center',
                    transition: 'border-color 0.15s',
                  }}
                  onMouseEnter={(e) => (e.currentTarget.style.borderColor = 'var(--muted-2)')}
                  onMouseLeave={(e) => (e.currentTarget.style.borderColor = 'var(--line)')}
                >
                  {getValue('ai.opencode_api_key') ? t('settings.apiKeyMasked') : t('settings.apiKeyEmpty')}
                </button>
              )}
            </div>
          </div>

          {/* Status badge */}
          {aiHealth && (
            <div style={{
              padding: '10px 18px', borderBottom: '1px solid var(--line)',
              background: aiHealth.includes('connected') ? 'rgba(117, 212, 161, 0.04)' : 'rgba(255, 112, 133, 0.04)',
            }}>
              <div style={{
                display: 'inline-flex', alignItems: 'center', gap: '6px',
                fontSize: '12px', fontFamily: 'var(--mono)',
                padding: '4px 10px', borderRadius: '999px',
                background: aiHealth.includes('connected') ? 'rgba(117, 212, 161, 0.1)' : 'rgba(255, 112, 133, 0.1)',
                color: aiHealth.includes('connected') ? 'var(--mint)' : 'var(--rose)',
                border: '1px solid',
                borderColor: aiHealth.includes('connected') ? 'rgba(117, 212, 161, 0.2)' : 'rgba(255, 112, 133, 0.2)',
              }}>
                <span style={{ fontSize: '8px' }}>{aiHealth.includes('connected') ? '●' : '○'}</span>
                {aiHealth}
              </div>
            </div>
          )}

          {/* Model + Check */}
          <div style={{
            display: 'flex', alignItems: 'center', justifyContent: 'space-between',
            padding: '14px 18px',
          }}>
            <div>
              <div style={{ fontSize: '14px', fontWeight: 500, color: 'var(--bone)' }}>
                {t('settings.aiModel')}
              </div>
              <div style={{ fontSize: '10px', color: 'var(--muted-2)', marginTop: '3px' }}>
                {t('settings.aiModelHint')}
              </div>
            </div>
            <div style={{ display: 'flex', alignItems: 'center', gap: '10px' }}>
              {/* Model selector dropdown */}
              <div style={{ position: 'relative' }}>
                <select
                  value={getValue('ai.model') || 'opencode/deepseek-v4-flash-free'}
                  onChange={(e) => saveConfig('ai.model', e.target.value)}
                  disabled={modelsLoading}
                  style={{
                    background: 'var(--carbon)', border: '1px solid var(--line)',
                    borderRadius: 'var(--radius-xs)', padding: '5px 28px 5px 10px',
                    fontSize: '12px', color: 'var(--bone)', outline: 'none',
                    fontFamily: 'var(--mono)', cursor: modelsLoading ? 'wait' : 'pointer',
                    minWidth: '200px', maxWidth: '240px', appearance: 'none',
                    transition: 'border-color 0.15s',
                  }}
                  onMouseEnter={(e) => (e.currentTarget.style.borderColor = 'var(--tangerine)')}
                  onMouseLeave={(e) => (e.currentTarget.style.borderColor = 'var(--line)')}
                >
                  {models.length === 0 && !modelsLoading && (
                    <option value="opencode/deepseek-v4-flash-free">
                      opencode/deepseek-v4-flash-free (default)
                    </option>
                  )}
                  {models.map((m) => (
                    <option key={m.id} value={m.id}>
                      {m.id} {m.is_free ? '(free)' : ''}
                    </option>
                  ))}
                </select>
                {/* Dropdown arrow */}
                <div style={{
                  position: 'absolute', right: '8px', top: '50%', transform: 'translateY(-50%)',
                  pointerEvents: 'none', color: 'var(--muted-2)', fontSize: '10px',
                }}>▼</div>
              </div>
              {/* Free-only filter toggle */}
              <button
                onClick={() => setShowFreeOnly((p) => !p)}
                title={showFreeOnly ? 'Showing free models only' : 'Showing all models'}
                style={{
                  background: showFreeOnly ? 'var(--tangerine-soft)' : 'var(--carbon)',
                  border: '1px solid',
                  borderColor: showFreeOnly ? 'var(--tangerine)' : 'var(--line)',
                  borderRadius: '999px', padding: '4px 10px',
                  cursor: 'pointer', fontSize: '10px', fontWeight: 600,
                  color: showFreeOnly ? 'var(--tangerine)' : 'var(--muted-2)',
                  transition: 'all 0.2s', whiteSpace: 'nowrap',
                }}
              >
                FREE
              </button>
              {/* Reload models */}
              <button
                onClick={loadModels}
                disabled={modelsLoading}
                className="settings-action-btn"
                title="Refresh model list"
                style={modelsLoading ? { opacity: 0.5, pointerEvents: 'none' as const } : undefined}
              >
                <RefreshCw size={13} className={modelsLoading ? 'spinning' : 'settings-action-icon'} />
              </button>
              {/* Health check */}
              <button
                onClick={() => checkAiHealth()}
                disabled={checkingHealth}
                className="settings-action-btn"
                style={checkingHealth ? { opacity: 0.5, pointerEvents: 'none' as const } : undefined}
              >
                <RefreshCw size={13} className={checkingHealth ? 'spinning' : 'settings-action-icon'} /> Check
              </button>
            </div>
          </div>
          {/* Models loading/error indicator */}
          {(modelsLoading || modelsError) && (
            <div style={{
              padding: '6px 18px 10px', borderTop: modelsError ? '1px solid var(--line)' : 'none',
            }}>
              {modelsLoading && (
                <span style={{ fontSize: '11px', color: 'var(--muted-2)', fontFamily: 'var(--mono)' }}>
                  Loading models...
                </span>
              )}
              {modelsError && (
                <span style={{ fontSize: '11px', color: 'var(--rose)', fontFamily: 'var(--mono)' }}>
                  {modelsError}
                </span>
              )}
            </div>
          )}
          {/* Models count */}
          {!modelsLoading && models.length > 0 && (
            <div style={{
              padding: '0px 18px 10px',
            }}>
              <span style={{ fontSize: '10px', color: 'var(--muted-2)', fontFamily: 'var(--mono)' }}>
                {models.length} model{models.length !== 1 ? 's' : ''} available
              </span>
            </div>
          )}
        </div>
      </div>

      {/* Back to memories */}
      <button
        className="btn btn-secondary"
        onClick={() => setActiveView('memory')}
        style={{ marginTop: '8px' }}
      >
        {t('settings.back')}
      </button>
    </div>
  );
}
