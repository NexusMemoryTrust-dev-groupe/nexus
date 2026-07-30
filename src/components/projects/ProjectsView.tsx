import { useEffect, useState, useCallback } from 'react';
import { useProjectStore } from '../../stores/projectStore';
import { useLocale } from '../../stores/localeStore';
import { ProjectDetail } from './ProjectDetail';
import { FolderOpen, Plus, Loader2, X, FolderInput, Trash2, AlertTriangle, Network } from 'lucide-react';
import { invoke } from '@tauri-apps/api/core';

export function ProjectsView() {
  const { projects, selectedProject, isLoading, error, fetchProjects, selectProject, createProject, deleteProject } =
    useProjectStore();
  const { t } = useLocale();
  const [showCreate, setShowCreate] = useState(false);
  const [newTitle, setNewTitle] = useState('');
  const [newDesc, setNewDesc] = useState('');
  const [creating, setCreating] = useState(false);
  const [importPath, setImportPath] = useState<string | null>(null);
  const [importError, setImportError] = useState<string | null>(null);
  const [nameDuplicate, setNameDuplicate] = useState(false);
  const [nameWarning, setNameWarning] = useState('');
  const [projectPaths, setProjectPaths] = useState<Record<string, string>>({});

  // Load project root_folder paths from metadata
  useEffect(() => {
    const loadPaths = async () => {
      const paths: Record<string, string> = {};
      for (const p of projects) {
        try {
          const meta = await invoke<Record<string, unknown>>('get_entity_metadata', { id: p.id });
          if (meta && typeof meta.root_folder === 'string') {
            paths[p.id] = meta.root_folder;
          }
        } catch { /* ignore */ }
      }
      setProjectPaths(paths);
    };
    if (projects.length > 0) loadPaths();
  }, [projects]);

  // Validate project name against existing projects + disk
  const validateProjectName = useCallback(async (title: string) => {
    const trimmed = title.trim();
    if (!trimmed) { setNameDuplicate(false); setNameWarning(''); return; }

    // 1. Check against existing project titles in memory (instant, no IPC)
    const existsInDB = projects.some(p => p.title.toLowerCase() === trimmed.toLowerCase());
    if (existsInDB) {
      setNameDuplicate(true);
      setNameWarning(`Проект «${trimmed}» уже существует`);
      return;
    }

    // 2. Check if folder exists on Desktop (disk check)
    try {
      const desktop = await invoke<string>('get_desktop_dir');
      const folderPath = `${desktop}\\${trimmed}`;
      const exists = await invoke<boolean>('path_exists', { path: folderPath });
      if (exists) {
        setNameDuplicate(true);
        setNameWarning(`Папка «${trimmed}» уже существует на рабочем столе`);
        return;
      }
    } catch { /* disk check unavailable — proceed */ }

    setNameDuplicate(false);
    setNameWarning('');
  }, [projects]);

  // Re-validate name when title changes
  useEffect(() => {
    const timeout = setTimeout(() => { validateProjectName(newTitle); }, 300);
    return () => clearTimeout(timeout);
  }, [newTitle, validateProjectName]);

  // Re-validate when modal opens (user might have created the folder externally)
  useEffect(() => {
    if (showCreate && newTitle.trim()) { validateProjectName(newTitle); }
  }, [showCreate]);

  useEffect(() => {
    fetchProjects();
  }, [fetchProjects]);

  // Auto-delete projects whose folders were deleted from disk
  useEffect(() => {
    const checkAndDeleteStale = async () => {
      try {
        const staleIds = await invoke<string[]>('check_stale_projects');
        for (const pid of staleIds) {
          await deleteProject(pid);
        }
      } catch { /* ignore */ }
    };
    // Run once on mount, then every 5 seconds while on project list
    checkAndDeleteStale();
    const interval = setInterval(checkAndDeleteStale, 5000);
    return () => clearInterval(interval);
  }, [deleteProject]);

  if (selectedProject) {
    return <ProjectDetail />;
  }

  const handleCreate = async () => {
    if (!newTitle.trim()) return;
    setCreating(true);
    try {
      const project = await createProject(newTitle.trim(), newDesc.trim());
      // If importing a folder, store it in the project metadata
      if (importPath && project.id) {
        await invoke('update_entity', {
          id: project.id,
          metadata: { root_folder: importPath },
        });
      }
      setNewTitle('');
      setNewDesc('');
      setImportPath(null);
      setShowCreate(false);
      selectProject(project);
    } catch {
      // error handled by store
    } finally {
      setCreating(false);
    }
  };

  const handleImportFolder = async () => {
    try {
      setImportError(null);
      const path = await invoke<string | null>('pick_folder', {
        title: 'Select project folder',
      });
      if (path) {
        setImportPath(path);
        // Auto-fill title from folder name if empty
        if (!newTitle.trim()) {
          const folderName = path.split(/[/\\]/).pop() || '';
          setNewTitle(folderName);
        }
      }
    } catch (e) {
      setImportError(String(e));
    }
  };

  const handleCloseModal = () => {
    setShowCreate(false);
    setNewTitle('');
    setNewDesc('');
    setImportPath(null);
    setImportError(null);
  };

  return (
    <div style={{ padding: '24px', height: '100%', overflow: 'auto' }}>
      {/* Header */}
      <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', marginBottom: '24px' }}>
        <div>
          <h2 style={{ fontFamily: 'var(--brand)', fontSize: '22px', fontWeight: 700, color: 'var(--bone)', letterSpacing: '-0.02em' }}>
            {t('projects.title')}
          </h2>
          <p style={{ fontSize: '13px', color: 'var(--muted)', marginTop: '4px' }}>
            {t('projects.subtitle')}
          </p>
        </div>
        <button
          className="settings-action-btn"
          onClick={() => setShowCreate(true)}
          style={{ display: 'flex', alignItems: 'center', gap: '6px' }}
        >
          <Plus size={14} />
          {t('projects.new')}
        </button>
      </div>

      {/* Create modal */}
      {showCreate && (
        <div className="project-modal-overlay" onClick={(e) => { if (e.target === e.currentTarget) handleCloseModal(); }}>
          <div className="project-modal" onClick={(e) => e.stopPropagation()}>
            {/* Header */}
            <div className="project-modal-header">
              <div className="project-modal-title">{t('projects.new')}</div>
              <button className="btn-icon" onClick={handleCloseModal} style={{ color: 'var(--muted)' }}>
                <X size={18} />
              </button>
            </div>

            {/* Body */}
            <div className="project-modal-body">
              <div>
                <label style={{ display: 'block', fontSize: '12px', fontWeight: 600, color: 'var(--muted)', marginBottom: '6px', textTransform: 'uppercase', letterSpacing: '0.04em' }}>
                  {t('projects.namePlaceholder')}
                </label>
                <input
                  type="text"
                  className="project-modal-input"
                  placeholder={t('projects.namePlaceholder')}
                  value={newTitle}
                  onChange={(e) => setNewTitle(e.target.value)}
                  onKeyDown={(e) => e.key === 'Enter' && !nameDuplicate && handleCreate()}
                  autoFocus
                  style={nameDuplicate ? { borderColor: 'var(--tangerine)' } : undefined}
                />
                {nameDuplicate && nameWarning && (
                  <div style={{ fontSize: '12px', color: 'var(--tangerine)', padding: '4px 0', display: 'flex', alignItems: 'center', gap: '4px' }}>
                    <AlertTriangle size={13} />
                    {nameWarning}
                  </div>
                )}
              </div>

              <div>
                <label style={{ display: 'block', fontSize: '12px', fontWeight: 600, color: 'var(--muted)', marginBottom: '6px', textTransform: 'uppercase', letterSpacing: '0.04em' }}>
                  {t('projects.descPlaceholder')}
                </label>
                <textarea
                  className="project-modal-textarea"
                  placeholder={t('projects.descPlaceholder')}
                  value={newDesc}
                  onChange={(e) => setNewDesc(e.target.value)}
                  rows={3}
                />
              </div>

              {/* Import divider */}
              <div className="project-modal-divider">
                {t('projects.importOr')}
              </div>

              {/* Import button */}
              <button
                className={`project-modal-import-btn ${importPath ? 'imported' : ''}`}
                onClick={handleImportFolder}
              >
                {importPath ? (
                  <>
                    <FolderOpen size={16} />
                    <span style={{ overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap', maxWidth: '300px' }}>
                      {importPath.split(/[/\\]/).pop()}
                    </span>
                    <span style={{ fontSize: '11px', opacity: 0.7, marginLeft: '4px' }}>
                      ({importPath})
                    </span>
                    <X
                      size={14}
                      style={{ marginLeft: 'auto', flexShrink: 0 }}
                      onClick={(e) => { e.stopPropagation(); setImportPath(null); }}
                    />
                  </>
                ) : (
                  <>
                    <FolderInput size={16} />
                    {t('projects.importFolder')}
                  </>
                )}
              </button>

              {importError && (
                <div style={{ fontSize: '12px', color: 'var(--rose)', padding: '4px 0' }}>
                  {importError}
                </div>
              )}
            </div>

            {/* Footer */}
            <div className="project-modal-footer">
              <button className="project-modal-cancel" onClick={handleCloseModal}>
                {t('common.cancel')}
              </button>
              <button
                className="project-modal-create"
                onClick={handleCreate}
                disabled={!newTitle.trim() || creating || nameDuplicate}
              >
                {creating ? <Loader2 size={14} className="spinning" /> : <Plus size={14} />}
                {t('projects.create')}
              </button>
            </div>
          </div>
        </div>
      )}

      {/* Error */}
      {error && (
        <div style={{
          padding: '12px 16px',
          background: 'rgba(255, 112, 133, 0.08)',
          border: '1px solid rgba(255, 112, 133, 0.2)',
          borderRadius: 'var(--radius-xs)',
          color: 'var(--rose)',
          fontSize: '13px',
          marginBottom: '16px',
        }}>
          {error}
        </div>
      )}

      {/* Loading */}
      {isLoading && (
        <div className="empty-state">
          <Loader2 size={48} className="empty-state-icon spinning" />
          <div className="empty-state-title">{t('common.loading')}</div>
        </div>
      )}

      {/* Empty state */}
      {!isLoading && projects.length === 0 && (
        <div className="empty-state">
          <FolderOpen size={72} className="empty-state-icon" />
          <div className="empty-state-title">{t('projects.empty')}</div>
          <div className="empty-state-desc">{t('projects.emptyDesc')}</div>
        </div>
      )}

      {/* Project grid */}
      {!isLoading && projects.length > 0 && (
        <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fill, minmax(300px, 1fr))', gap: '12px' }}>
          {projects.map((project) => (
            <div
              key={project.id}
              onClick={() => selectProject(project)}
              style={{
                display: 'flex',
                flexDirection: 'column',
                alignItems: 'flex-start',
                gap: '8px',
                padding: '20px',
                background: 'var(--surface)',
                border: '1px solid var(--line)',
                borderRadius: 'var(--radius)',
                cursor: 'pointer',
                textAlign: 'left',
                transition: 'all 0.2s ease',
                position: 'relative',
                overflow: 'hidden',
              }}
              onMouseEnter={(e) => {
                e.currentTarget.style.borderColor = 'rgba(255, 138, 91, 0.3)';
                e.currentTarget.style.boxShadow = '0 4px 20px rgba(255, 138, 91, 0.08)';
                const btn = e.currentTarget.querySelector('.project-delete-btn') as HTMLElement;
                if (btn) btn.style.opacity = '1';
              }}
              onMouseLeave={(e) => {
                e.currentTarget.style.borderColor = 'var(--line)';
                e.currentTarget.style.boxShadow = 'none';
                const btn = e.currentTarget.querySelector('.project-delete-btn') as HTMLElement;
                if (btn) btn.style.opacity = '0';
              }}
            >
              {/* Delete button */}
              <button
                className="project-delete-btn"
                onClick={(e) => {
                  e.stopPropagation();
                  if (confirm(`Delete project "${project.title}"?`)) {
                    deleteProject(project.id);
                  }
                }}
                style={{
                  position: 'absolute',
                  top: '10px',
                  right: '10px',
                  display: 'flex',
                  alignItems: 'center',
                  justifyContent: 'center',
                  width: '28px',
                  height: '28px',
                  borderRadius: '6px',
                  border: 'none',
                  background: 'rgba(255, 112, 133, 0.1)',
                  color: 'var(--rose)',
                  cursor: 'pointer',
                  opacity: 0,
                  transition: 'all 0.2s',
                  zIndex: 2,
                }}
                onMouseEnter={(e) => {
                  e.currentTarget.style.background = 'rgba(255, 112, 133, 0.2)';
                  e.currentTarget.style.transform = 'scale(1.1)';
                }}
                onMouseLeave={(e) => {
                  e.currentTarget.style.background = 'rgba(255, 112, 133, 0.1)';
                  e.currentTarget.style.transform = 'scale(1)';
                }}
                title={t('projects.delete') || 'Delete project'}
              >
                <Trash2 size={14} />
              </button>

              <div style={{
                width: '36px',
                height: '36px',
                display: 'flex',
                alignItems: 'center',
                justifyContent: 'center',
                background: 'var(--tangerine-soft)',
                borderRadius: '10px',
                color: 'var(--tangerine)',
              }}>
                <FolderOpen size={18} />
              </div>
              <div style={{ paddingRight: '24px' }}>
                <div style={{
                  fontFamily: 'var(--brand)',
                  fontSize: '15px',
                  fontWeight: 600,
                  color: 'var(--bone)',
                  marginBottom: '4px',
                }}>
                  {project.title}
                </div>
                {project.description && (
                  <div style={{
                    fontSize: '12px',
                    color: 'var(--muted)',
                    lineHeight: 1.5,
                    display: '-webkit-box',
                    WebkitLineClamp: 2,
                    WebkitBoxOrient: 'vertical',
                    overflow: 'hidden',
                  }}>
                    {project.description}
                  </div>
                )}
              </div>
              {projectPaths[project.id] && (
                <div className="project-card-path">
                  <Network size={11} className="project-card-path-icon" />
                  <span style={{ overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
                    {projectPaths[project.id]}
                  </span>
                </div>
              )}
              <div style={{
                fontSize: '11px',
                color: 'var(--muted-2)',
                marginTop: 'auto',
              }}>
                {new Date(project.createdAt).toLocaleDateString()}
              </div>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
