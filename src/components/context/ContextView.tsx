import { useState, useCallback } from 'react';
import { useContextStore } from '../../stores/contextStore';
import { useLocale } from '../../stores/localeStore';
import {
  Brain, Layers, Sparkles, Network, Link2,
  Hash, ArrowRight, Loader2, Search, ChevronRight, ChevronDown,
} from 'lucide-react';

const entityColors: Record<string, string> = {
  person: '#63d8d2', project: '#ff8a5b', decision: '#818cf8',
  task: '#f472b6', technology: '#60a5fa', file: '#a99cf8',
  organization: '#f59e0b', meeting: '#34d399', document: '#c084fc',
  concept: '#ddbb65', default: '#93c5fd',
};

function getEntityColor(type: string): string {
  const lower = type.toLowerCase();
  for (const [key, val] of Object.entries(entityColors)) {
    if (lower.includes(key)) return val;
  }
  return entityColors.default;
}

export function ContextView() {
  const { t } = useLocale();
  const { context, isLoading, error, buildContext, clearContext } = useContextStore();
  const [query, setQuery] = useState('');
  const [showEntities, setShowEntities] = useState(true);
  const [showMemories, setShowMemories] = useState(true);
  const [showRelationships, setShowRelationships] = useState(true);

  const handleBuild = useCallback(() => {
    if (query.trim()) {
      buildContext(query.trim());
    }
  }, [query, buildContext]);

  const handleKeyDown = useCallback((e: React.KeyboardEvent) => {
    if (e.key === 'Enter' && query.trim()) {
      handleBuild();
    }
  }, [query, handleBuild]);

  return (
    <div style={{ maxWidth: '800px', margin: '0 auto' }}>
      {/* Header */}
      <div style={{ marginBottom: '24px' }}>
        <div style={{ display: 'flex', alignItems: 'center', gap: '10px', marginBottom: '6px' }}>
          <div style={{
            width: '36px', height: '36px', display: 'flex', alignItems: 'center', justifyContent: 'center',
            background: 'var(--gold-soft)', borderRadius: '10px', color: 'var(--gold)',
          }}>
            <Layers size={18} />
          </div>
          <div>
            <h2 style={{
              fontFamily: 'var(--brand)', fontSize: '20px', fontWeight: 700,
              color: 'var(--bone)', letterSpacing: '-0.02em', margin: 0,
            }}>
              {t('context.title')}
            </h2>
            <p style={{ fontSize: '12px', color: 'var(--muted)', margin: 0 }}>
              Build a context package from your knowledge graph and memories
            </p>
          </div>
        </div>
      </div>

      {/* Query input */}
      <div style={{
        background: 'var(--surface)', border: '1px solid var(--line)',
        borderRadius: 'var(--radius)', padding: '16px', marginBottom: '20px',
      }}>
        <div className="context-label" style={{ marginBottom: '8px' }}>
          <Search size={12} style={{ marginRight: '4px', verticalAlign: 'middle' }} />
          {t('context.query')}
        </div>
        <div style={{ display: 'flex', gap: '8px' }}>
          <input
            type="text"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            onKeyDown={handleKeyDown}
            placeholder="Enter a query to build context for..."
            autoFocus
            style={{
              flex: 1, padding: '10px 14px',
              background: 'var(--carbon)', border: '1px solid var(--line)',
              borderRadius: 'var(--radius-xs)', color: 'var(--bone)', fontSize: '14px',
              fontFamily: 'var(--sans)', outline: 'none',
            }}
          />
          <button
            onClick={handleBuild}
            disabled={!query.trim() || isLoading}
            style={{
              display: 'flex', alignItems: 'center', gap: '6px',
              padding: '10px 20px', background: 'var(--gold)', border: 'none',
              borderRadius: 'var(--radius-xs)', color: '#000', fontSize: '13px',
              fontWeight: 600, cursor: query.trim() && !isLoading ? 'pointer' : 'not-allowed',
              opacity: query.trim() && !isLoading ? 1 : 0.5,
              transition: 'opacity 0.15s',
            }}
          >
            {isLoading ? <Loader2 size={14} className="spinning" /> : <Sparkles size={14} />}
            Build
          </button>
        </div>
      </div>

      {/* Error */}
      {error && (
        <div style={{
          padding: '12px 16px', marginBottom: '20px',
          background: 'rgba(255, 112, 133, 0.08)',
          border: '1px solid rgba(255, 112, 133, 0.2)',
          borderRadius: 'var(--radius-xs)',
          color: 'var(--rose)', fontSize: '13px',
        }}>
          {error}
        </div>
      )}

      {/* Results */}
      {context && (
        <div>
          {/* Stats bar */}
          <div style={{
            display: 'grid', gridTemplateColumns: 'repeat(4, 1fr)', gap: '10px',
            marginBottom: '20px',
          }}>
            <StatCard icon={Hash} label="Tokens" value={String(context.tokenCount)} color="var(--gold)" />
            <StatCard icon={Network} label="Entities" value={String(context.entities.length)} color="var(--cyan)" />
            <StatCard icon={Brain} label="Memories" value={String(context.memoryRecords.length)} color="var(--tangerine)" />
            <StatCard icon={Link2} label="Relationships" value={String(context.relationships.length)} color="var(--periwinkle)" />
          </div>

          {/* Intent */}
          <div style={{
            background: 'var(--surface)', border: '1px solid var(--line)',
            borderRadius: 'var(--radius)', padding: '14px 16px', marginBottom: '16px',
          }}>
            <div style={{ display: 'flex', alignItems: 'center', gap: '16px' }}>
              <div style={{ flex: 1 }}>
                <div className="context-label" style={{ marginBottom: '4px' }}>Query</div>
                <div style={{ color: 'var(--bone)', fontSize: '14px', fontWeight: 500 }}>
                  {context.query}
                </div>
              </div>
              <div style={{ textAlign: 'right' }}>
                <div className="context-label" style={{ marginBottom: '4px' }}>{t('context.intent')}</div>
                <div style={{
                  padding: '2px 8px', borderRadius: '999px', fontSize: '12px', fontWeight: 600,
                  background: 'var(--gold-soft)', color: 'var(--gold)',
                }}>
                  {context.intentType}
                </div>
              </div>
              <div style={{ textAlign: 'right' }}>
                <div className="context-label" style={{ marginBottom: '4px' }}>{t('context.confidence')}</div>
                <div style={{ fontSize: '14px', fontWeight: 600, color: 'var(--bone)' }}>
                  {(context.confidence * 100).toFixed(0)}%
                </div>
              </div>
            </div>
          </div>

          {/* Entities */}
          {context.entities.length > 0 && (
            <CollapsibleSection
              title="Entities"
              icon={Network}
              count={context.entities.length}
              color="var(--cyan)"
              open={showEntities}
              onToggle={() => setShowEntities(!showEntities)}
            >
              <div style={{ display: 'flex', flexDirection: 'column', gap: '6px' }}>
                {context.entities.map((entity) => (
                  <div key={entity.id} style={{
                    display: 'flex', alignItems: 'center', gap: '10px',
                    padding: '10px 12px', background: 'var(--carbon)',
                    borderRadius: 'var(--radius-xs)',
                  }}>
                    <div style={{
                      width: '8px', height: '8px', borderRadius: '50%',
                      background: getEntityColor(entity.entityType), flexShrink: 0,
                    }} />
                    <div style={{ flex: 1, minWidth: 0 }}>
                      <div style={{ fontSize: '13px', fontWeight: 600, color: 'var(--bone)' }}>
                        {entity.title}
                      </div>
                      <div style={{ fontSize: '11px', color: 'var(--muted-2)', marginTop: '1px' }}>
                        {entity.entityType}
                      </div>
                    </div>
                  </div>
                ))}
              </div>
            </CollapsibleSection>
          )}

          {/* Memories */}
          {context.memoryRecords.length > 0 && (
            <CollapsibleSection
              title="Memories"
              icon={Brain}
              count={context.memoryRecords.length}
              color="var(--tangerine)"
              open={showMemories}
              onToggle={() => setShowMemories(!showMemories)}
            >
              <div style={{ display: 'flex', flexDirection: 'column', gap: '6px' }}>
                {context.memoryRecords.map((mem) => (
                  <div key={mem.id} style={{
                    padding: '10px 12px', background: 'var(--carbon)',
                    borderRadius: 'var(--radius-xs)',
                  }}>
                    <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', marginBottom: '4px' }}>
                      <div style={{ fontSize: '13px', fontWeight: 600, color: 'var(--bone)' }}>
                        {mem.title}
                      </div>
                      <span style={{
                        padding: '1px 6px', borderRadius: '4px', fontSize: '10px', fontWeight: 600,
                        background: 'var(--tangerine-soft)', color: 'var(--tangerine)',
                      }}>
                        {mem.layer}
                      </span>
                    </div>
                    {mem.summary && (
                      <div style={{
                        fontSize: '11px', color: 'var(--muted)', lineHeight: 1.4,
                        display: '-webkit-box', WebkitLineClamp: 2,
                        WebkitBoxOrient: 'vertical', overflow: 'hidden',
                      }}>
                        {mem.summary}
                      </div>
                    )}
                  </div>
                ))}
              </div>
            </CollapsibleSection>
          )}

          {/* Relationships */}
          {context.relationships.length > 0 && (
            <CollapsibleSection
              title="Relationships"
              icon={Link2}
              count={context.relationships.length}
              color="var(--periwinkle)"
              open={showRelationships}
              onToggle={() => setShowRelationships(!showRelationships)}
            >
              <div style={{ display: 'flex', flexDirection: 'column', gap: '4px' }}>
                {context.relationships.map((rel) => {
                  const srcEntity = context.entities.find(e => e.id === rel.sourceEntityId);
                  const tgtEntity = context.entities.find(e => e.id === rel.targetEntityId);
                  const srcLabel = srcEntity?.title || rel.sourceEntityId.slice(0, 8);
                  const tgtLabel = tgtEntity?.title || rel.targetEntityId.slice(0, 8);
                  const srcColor = srcEntity ? getEntityColor(srcEntity.entityType) : 'var(--muted-2)';
                  const tgtColor = tgtEntity ? getEntityColor(tgtEntity.entityType) : 'var(--muted-2)';
                  return (
                    <div key={rel.id} style={{
                      display: 'flex', alignItems: 'center', gap: '8px',
                      padding: '8px 12px', background: 'var(--carbon)',
                      borderRadius: 'var(--radius-xs)', fontSize: '12px',
                    }}>
                      <span style={{ color: srcColor, fontWeight: 500, fontSize: '11px', maxWidth: '140px', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
                        {srcLabel}
                      </span>
                      <ArrowRight size={12} style={{ color: 'var(--muted-3)', flexShrink: 0 }} />
                      <span style={{
                        padding: '1px 6px', borderRadius: '4px',
                        background: 'var(--periwinkle-soft)', color: 'var(--periwinkle)',
                        fontSize: '10px', fontWeight: 600,
                      }}>
                        {rel.relationshipType}
                      </span>
                      <ArrowRight size={12} style={{ color: 'var(--muted-3)', flexShrink: 0 }} />
                      <span style={{ color: tgtColor, fontWeight: 500, fontSize: '11px', maxWidth: '140px', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
                        {tgtLabel}
                      </span>
                      <span style={{ marginLeft: 'auto', color: 'var(--muted-3)', fontSize: '10px' }}>
                        w: {rel.weight.toFixed(2)}
                      </span>
                    </div>
                  );
                })}
              </div>
            </CollapsibleSection>
          )}

          {/* Clear */}
          <button
            onClick={clearContext}
            style={{
              marginTop: '16px', padding: '8px 16px',
              background: 'none', border: '1px solid var(--line)',
              borderRadius: 'var(--radius-xs)', color: 'var(--muted)',
              fontSize: '12px', cursor: 'pointer',
            }}
          >
            Clear context
          </button>
        </div>
      )}

      {/* Empty state */}
      {!context && !isLoading && (
        <div style={{
          padding: '60px 20px', textAlign: 'center',
          background: 'var(--surface)', border: '1px solid var(--line)',
          borderRadius: 'var(--radius)',
        }}>
          <Layers size={48} style={{ color: 'var(--muted-3)', marginBottom: '16px' }} />
          <div style={{ fontSize: '15px', fontWeight: 600, color: 'var(--bone)', marginBottom: '8px' }}>
            Build Context Package
          </div>
          <div style={{ fontSize: '13px', color: 'var(--muted)', maxWidth: '400px', margin: '0 auto', lineHeight: 1.5 }}>
            Enter a query above to aggregate relevant entities, memories, and relationships from your knowledge graph into a unified context package.
          </div>
        </div>
      )}
    </div>
  );
}

// ── Stat Card ──
function StatCard({ icon: Icon, label, value, color }: {
  icon: typeof Brain; label: string; value: string; color: string;
}) {
  return (
    <div style={{
      background: 'var(--surface)', border: '1px solid var(--line)',
      borderRadius: 'var(--radius-sm)', padding: '14px 16px',
    }}>
      <div style={{ display: 'flex', alignItems: 'center', gap: '6px', marginBottom: '6px' }}>
        <Icon size={13} style={{ color, flexShrink: 0 }} />
        <span style={{ fontSize: '10px', color: 'var(--muted-2)', textTransform: 'uppercase', letterSpacing: '0.05em' }}>
          {label}
        </span>
      </div>
      <div style={{ fontSize: '22px', fontWeight: 700, color: 'var(--bone)', fontFamily: 'var(--brand)' }}>
        {value}
      </div>
    </div>
  );
}

// ── Collapsible Section ──
function CollapsibleSection({ title, icon: Icon, count, color, open, onToggle, children }: {
  title: string; icon: typeof Brain; count: number; color: string;
  open: boolean; onToggle: () => void; children: React.ReactNode;
}) {
  return (
    <div style={{
      background: 'var(--surface)', border: '1px solid var(--line)',
      borderRadius: 'var(--radius)', marginBottom: '12px', overflow: 'hidden',
    }}>
      <button
        onClick={onToggle}
        style={{
          display: 'flex', alignItems: 'center', gap: '8px',
          width: '100%', padding: '12px 16px', background: 'none',
          border: 'none', cursor: 'pointer', textAlign: 'left',
        }}
      >
        <Icon size={14} style={{ color, flexShrink: 0 }} />
        <span style={{ fontSize: '13px', fontWeight: 600, color: 'var(--bone)', flex: 1 }}>
          {title}
        </span>
        <span style={{
          padding: '1px 6px', borderRadius: '999px', fontSize: '10px', fontWeight: 600,
          background: `${color}15`, color,
        }}>
          {count}
        </span>
        {open
          ? <ChevronDown size={14} style={{ color: 'var(--muted-3)' }} />
          : <ChevronRight size={14} style={{ color: 'var(--muted-3)' }} />
        }
      </button>
      {open && (
        <div style={{ padding: '0 16px 12px' }}>
          {children}
        </div>
      )}
    </div>
  );
}
