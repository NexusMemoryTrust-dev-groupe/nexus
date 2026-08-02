import { useState, useCallback, useRef, useEffect } from 'react';
import {
  ChevronRight, ChevronDown, Folder, FolderOpen, FileText,
  Trash2, Edit3, Copy, MoreHorizontal, XCircle, X, AlertTriangle,
} from 'lucide-react';
import { invoke } from '@tauri-apps/api/core';
import type { FileEntry } from '../../types';
import { useFileUndoRedo } from '../../hooks/useFileUndoRedo';
import {
  isValidFileExtension, getFileIcon,
  DRAG_THRESHOLD, findClosestItem,
} from './treeUtils';
// Shared with the editor's status bar so a file's size never reads differently
// in two places.
import { fmtSize as formatSize } from './syntax/fileTypes';

// ── Shared callbacks ref for mouse-based DnD ──
interface TreeCallbacks {
  projectId: string | undefined;
  onRefresh: () => void;
  onFileSelect: (path: string) => void;
  onFileDeleted?: (path: string, isDir: boolean) => void;
  onFileRenamed?: (oldPath: string, newPath: string) => void;
  undoRedo?: ReturnType<typeof useFileUndoRedo>;
}
const _cb: { current: TreeCallbacks } = { current: { projectId: undefined, onRefresh: () => {}, onFileSelect: () => {} } };

// ── Module-level mouse DnD state ──
const _drag = {
  active: false,
  sourcePath: '',
  startX: 0,
  startY: 0,
  currentTarget: null as HTMLElement | null,
  started: false, // true once movement threshold exceeded
};

interface FileTreeProps {
  entries: FileEntry[];
  basePath: string;
  projectId?: string;
  onFileSelect: (path: string) => void;
  onRefresh: () => void;
  onFileDeleted?: (path: string, isDir: boolean) => void;
  onFileRenamed?: (oldPath: string, newPath: string) => void;
  onBeforeDelete?: (path: string, isDir: boolean) => Promise<void>;
  onBeforeRename?: (oldPath: string, isDir: boolean) => Promise<void>;
  onBeforeCreate?: (path: string, isDir: boolean) => Promise<void>;
  onRemoveFromWorkspace?: (path: string, isDir: boolean) => void;
  selectedFile: string | null;
  depth?: number;
  activeMenuPath?: string | null;
  onMenuChange?: (path: string | null) => void;
  activeFolderPath?: string | null;
  onFolderActivate?: (path: string | null) => void;
  undoRedo?: ReturnType<typeof useFileUndoRedo>;
}

export function FileTree({
  entries, basePath, projectId, onFileSelect, onRefresh, onFileDeleted, onFileRenamed,
  onBeforeDelete, onBeforeRename, onBeforeCreate, onRemoveFromWorkspace,
  selectedFile, depth = 0, activeMenuPath, onMenuChange,
  activeFolderPath, onFolderActivate, undoRedo,
}: FileTreeProps) {
  const containerRef = useRef<HTMLDivElement>(null);

  // Keep callbacks ref fresh
  useEffect(() => {
    _cb.current = { projectId, onRefresh, onFileSelect, onFileDeleted, onFileRenamed, undoRedo };
  });

  // ── Mouse-based DnD via event delegation (root container only) ──
  useEffect(() => {
    if (depth !== 0) return;
    const el = containerRef.current;
    if (!el) return;

    // ── mousedown: record source ──
    const onMouseDown = (e: MouseEvent) => {
      // Only left button
      if (e.button !== 0) return;
      const item = findClosestItem(e.target);
      if (!item) return;
      const path = item.getAttribute('data-path');
      if (!path) return;

      _drag.sourcePath = path;
      _drag.startX = e.clientX;
      _drag.startY = e.clientY;
      _drag.active = true;
      _drag.started = false;
      _drag.currentTarget = null;
    };

    // ── mousemove: detect threshold, highlight target ──
    const onMouseMove = (e: MouseEvent) => {
      if (!_drag.active) return;

      const dx = e.clientX - _drag.startX;
      const dy = e.clientY - _drag.startY;

      // Not yet past threshold
      if (!_drag.started) {
        if (Math.abs(dx) < DRAG_THRESHOLD && Math.abs(dy) < DRAG_THRESHOLD) return;
        _drag.started = true;
        // Mark source element as dragging
        const sourceEl = el.querySelector(`[data-path="${CSS.escape(_drag.sourcePath)}"]`);
        if (sourceEl && sourceEl instanceof HTMLElement) {
          sourceEl.classList.add('dragging');
          sourceEl.style.opacity = '0.4';
        }
      }

      // Find element under cursor
      // Temporarily hide the dragging element so elementFromPoint sees what's underneath
      const sourceEl = el.querySelector(`[data-path="${CSS.escape(_drag.sourcePath)}"]`) as HTMLElement | null;
      if (sourceEl) sourceEl.style.pointerEvents = 'none';
      const elementBelow = document.elementFromPoint(e.clientX, e.clientY);
      if (sourceEl) sourceEl.style.pointerEvents = '';

      const targetItem = findClosestItem(elementBelow);

      // Clear previous highlight
      if (_drag.currentTarget && _drag.currentTarget !== targetItem) {
        _drag.currentTarget.classList.remove('drag-over');
      }

      if (targetItem) {
        const targetIsDir = targetItem.getAttribute('data-is-dir') === 'true';
        const targetPath = targetItem.getAttribute('data-path') || '';

        // Don't allow drop onto self or own children
        const isSelf = targetPath === _drag.sourcePath;
        const isChild = targetPath.startsWith(_drag.sourcePath + '\\') || targetPath.startsWith(_drag.sourcePath + '/');

        if (targetIsDir && !isSelf && !isChild) {
          targetItem.classList.add('drag-over');
          _drag.currentTarget = targetItem;
        } else {
          _drag.currentTarget = null;
        }
      } else {
        _drag.currentTarget = null;
      }
    };

    // ── mouseup: execute drop ──
    const onMouseUp = async () => {
      if (!_drag.active) return;

      // Cleanup source element visual
      const sourceEl = el.querySelector(`[data-path="${CSS.escape(_drag.sourcePath)}"]`) as HTMLElement | null;
      if (sourceEl) {
        sourceEl.classList.remove('dragging');
        sourceEl.style.opacity = '';
      }

      // If we had a valid target, execute the move
      if (_drag.started && _drag.currentTarget) {
        const destPath = _drag.currentTarget.getAttribute('data-path') || '';
        const sourcePath = _drag.sourcePath;
        _drag.currentTarget.classList.remove('drag-over');

        if (sourcePath && destPath && sourcePath !== destPath) {
          const { projectId: pid, onRefresh: refresh, undoRedo: ur } = _cb.current;
          try {
            const lastSep = Math.max(sourcePath.lastIndexOf('\\'), sourcePath.lastIndexOf('/'));
            const originalParent = lastSep > 0 ? sourcePath.substring(0, lastSep) : sourcePath;
            let newPath: string;
            if (pid) {
              newPath = await invoke<string>('move_workspace_entry', {
                projectId: pid,
                sourcePath,
                destDir: destPath,
              });
            } else {
              newPath = await invoke<string>('move_entry', { sourcePath, destDir: destPath });
            }
            if (ur) {
              ur.pushAction({ type: 'move', sourcePath, destPath: newPath, originalParent });
            }
            refresh();
          } catch (err) {
            console.error('Move failed:', err);
          }
        }
      }

      // Reset all drag-over classes
      el.querySelectorAll('.drag-over').forEach((n) => n.classList.remove('drag-over'));
      el.querySelectorAll('.dragging').forEach((n) => {
        n.classList.remove('dragging');
        (n as HTMLElement).style.opacity = '';
      });

      _drag.active = false;
      _drag.sourcePath = '';
      _drag.started = false;
      _drag.currentTarget = null;
    };

    document.addEventListener('mousedown', onMouseDown, false);
    document.addEventListener('mousemove', onMouseMove, false);
    document.addEventListener('mouseup', onMouseUp, false);

    return () => {
      document.removeEventListener('mousedown', onMouseDown, false);
      document.removeEventListener('mousemove', onMouseMove, false);
      document.removeEventListener('mouseup', onMouseUp, false);
    };
  }, [depth]);

  // ── Context menu prevention ──
  useEffect(() => {
    const el = containerRef.current;
    if (!el) return;
    const handler = (e: MouseEvent) => {
      if (e.target && el.contains(e.target as Node)) {
        e.preventDefault();
      }
    };
    el.addEventListener('contextmenu', handler, false);
    return () => el.removeEventListener('contextmenu', handler, false);
  }, []);

  return (
    <div ref={containerRef} style={{ paddingLeft: depth > 0 ? '12px' : 0 }}>
      {entries.map((entry) => (
        <FileTreeNode
          key={entry.path}
          entry={entry}
          basePath={basePath}
          projectId={projectId}
          onFileSelect={onFileSelect}
          onRefresh={onRefresh}
          onFileDeleted={onFileDeleted}
          onFileRenamed={onFileRenamed}
          onBeforeDelete={onBeforeDelete}
          onBeforeRename={onBeforeRename}
          onBeforeCreate={onBeforeCreate}
          onRemoveFromWorkspace={onRemoveFromWorkspace}
          selectedFile={selectedFile}
          depth={depth}
          activeMenuPath={activeMenuPath}
          onMenuChange={onMenuChange}
          activeFolderPath={activeFolderPath}
          onFolderActivate={onFolderActivate}
          undoRedo={undoRedo}
        />
      ))}
    </div>
  );
}

function FileTreeNode({
  entry, basePath, projectId, onFileSelect, onRefresh, onFileDeleted, onFileRenamed,
  onBeforeDelete, onBeforeRename, onBeforeCreate, onRemoveFromWorkspace,
  selectedFile, depth, activeMenuPath, onMenuChange,
  activeFolderPath, onFolderActivate, undoRedo,
}: {
  entry: FileEntry; basePath: string; projectId?: string;
  onFileSelect: (path: string) => void; onRefresh: () => void;
  onFileDeleted?: (path: string, isDir: boolean) => void;
  onFileRenamed?: (oldPath: string, newPath: string) => void;
  onBeforeDelete?: (path: string, isDir: boolean) => Promise<void>;
  onBeforeRename?: (oldPath: string, isDir: boolean) => Promise<void>;
  onBeforeCreate?: (path: string, isDir: boolean) => Promise<void>;
  onRemoveFromWorkspace?: (path: string, isDir: boolean) => void;
  selectedFile: string | null; depth: number;
  activeMenuPath?: string | null;
  onMenuChange?: (path: string | null) => void;
  activeFolderPath?: string | null;
  onFolderActivate?: (path: string | null) => void;
  undoRedo?: ReturnType<typeof useFileUndoRedo>;
}) {
  const isBookmark = depth === 0 && entry.isDir;
  const [expanded, setExpanded] = useState(depth < 1);
  const [menuPos, setMenuPos] = useState<{ x: number; y: number }>({ x: 0, y: 0 });
  const [renaming, setRenaming] = useState(false);
  const [newName, setNewName] = useState(entry.name);
  const [creatingChild, setCreatingChild] = useState<'file' | 'folder' | null>(null);
  const [childName, setChildName] = useState('');
  const [childNameWarning, setChildNameWarning] = useState('');
  const [extensionError, setExtensionError] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);
  const childInputRef = useRef<HTMLInputElement>(null);
  const menuRef = useRef<HTMLDivElement>(null);

  const isSelected = selectedFile === entry.path;
  const isFolderActive = entry.isDir && activeFolderPath === entry.path;
  const fullPath = entry.path;
  const showMenu = activeMenuPath === fullPath;

  // ── Close menu on outside mousedown ──
  useEffect(() => {
    if (!showMenu) return;
    const handleMouseDown = (e: MouseEvent) => {
      if (menuRef.current && !menuRef.current.contains(e.target as Node)) {
        if (onMenuChange) onMenuChange(null);
      }
    };
    const timer = setTimeout(() => {
      document.addEventListener('mousedown', handleMouseDown, true);
    }, 0);
    return () => {
      clearTimeout(timer);
      document.removeEventListener('mousedown', handleMouseDown, true);
    };
  }, [showMenu, onMenuChange]);

  useEffect(() => {
    if (renaming && inputRef.current) {
      inputRef.current.focus();
      inputRef.current.select();
    }
  }, [renaming]);

  useEffect(() => {
    if (creatingChild && childInputRef.current) {
      childInputRef.current.focus();
    }
  }, [creatingChild]);

  const handleRename = useCallback(async () => {
    if (!newName.trim() || newName === entry.name) {
      setRenaming(false);
      return;
    }
    try {
      if (onBeforeRename) await onBeforeRename(fullPath, entry.isDir);
      const newPath = await invoke<string>('rename_workspace_entry', {
        projectId: projectId || '',
        oldPath: fullPath,
        newName: newName.trim(),
      });
      if (undoRedo) {
        undoRedo.pushAction({ type: 'rename', oldPath: fullPath, newPath, isDir: entry.isDir });
      }
      if (onFileRenamed) onFileRenamed(fullPath, newPath);
      onRefresh();
    } catch (e) {
      console.error('Rename failed:', e);
    }
    setRenaming(false);
  }, [newName, entry.name, entry.isDir, fullPath, projectId, onRefresh, onFileRenamed, onBeforeRename, undoRedo]);

  const handleDelete = useCallback(async () => {
    if (!confirm(`Удалить "${entry.name}" с компьютера?`)) return;
    try {
      if (onBeforeDelete) await onBeforeDelete(fullPath, entry.isDir);
      if (entry.isDir) {
        await invoke('delete_folder', { folderPath: fullPath });
      } else {
        await invoke('delete_file', { filePath: fullPath });
      }
      if (projectId) {
        await invoke('remove_from_workspace', { projectId, filePath: fullPath });
      }
      if (undoRedo) {
        undoRedo.pushAction({ type: 'delete', path: fullPath, isDir: entry.isDir });
      }
      if (onFileDeleted) onFileDeleted(fullPath, entry.isDir);
      onRefresh();
    } catch (e) {
      console.error('Delete failed:', e);
    }
    if (onMenuChange) onMenuChange(null);
  }, [entry, fullPath, projectId, onRefresh, onFileDeleted, onBeforeDelete, onMenuChange, undoRedo]);

  const handleCreateChild = useCallback(async () => {
    if (!childName.trim()) {
      setCreatingChild(null);
      setChildName('');
      setChildNameWarning('');
      setExtensionError(false);
      return;
    }
    if (creatingChild === 'file' && !isValidFileExtension(childName.trim())) {
      return;
    }
    const prefix = entry.isDir ? fullPath : basePath;
    const sep = prefix.includes('\\') ? '\\' : '/';
    const childPath = `${prefix}${sep}${childName.trim()}`;

    // Name collision check — in-memory against entry.children (reliable, no IPC)
    const lowerName = childName.trim().toLowerCase();
    let collisionFound = false;
    if (entry.children) {
      collisionFound = entry.children.some(c => c.name.toLowerCase() === lowerName);
    }
    // Fallback: check disk via IPC if in-memory check found nothing
    if (!collisionFound) {
      try {
        const names = await invoke<string[]>('list_dir_names', { dirPath: prefix });
        collisionFound = names.some(n => n.toLowerCase() === lowerName);
      } catch { /* ignore */ }
    }
    if (collisionFound) {
      setChildNameWarning(`«${childName.trim()}» уже существует`);
      return;
    }

    try {
      const isDir = creatingChild === 'folder';
      if (onBeforeCreate) await onBeforeCreate(childPath, isDir);
      if (projectId) {
        await invoke('create_workspace_entry', {
          projectId,
          parentPath: prefix,
          name: childName.trim(),
          isDir,
        });
      } else {
        if (isDir) {
          await invoke('create_folder', { folderPath: childPath });
        } else {
          await invoke('create_file', { filePath: childPath, content: '' });
        }
      }
      if (undoRedo) {
        undoRedo.pushAction({ type: 'create', path: childPath, isDir });
      }
      onRefresh();
      if (entry.isDir) setExpanded(true);
    } catch (e) {
      console.error('Create failed:', e);
    }
    setCreatingChild(null);
    setChildName('');
    setChildNameWarning('');
    setExtensionError(false);
  }, [childName, creatingChild, entry, fullPath, basePath, projectId, onRefresh, onBeforeCreate, undoRedo]);

  const handleCancelCreate = useCallback(() => {
    setCreatingChild(null);
    setChildName('');
    setChildNameWarning('');
    setExtensionError(false);
  }, []);

  const handleChildKeyDown = useCallback((e: React.KeyboardEvent) => {
    if (e.key === 'Enter') { e.preventDefault(); handleCreateChild(); }
    if (e.key === 'Escape') { e.preventDefault(); handleCancelCreate(); }
  }, [handleCreateChild, handleCancelCreate]);

  const handleChildNameChange = useCallback((val: string) => {
    setChildName(val);
    setChildNameWarning('');
    if (creatingChild === 'file' && val.includes('.') && !isValidFileExtension(val)) {
      setExtensionError(true);
    } else {
      setExtensionError(false);
    }
  }, [creatingChild]);

  const iconSize = 14;
  const FileIcon = getFileIcon(entry.name);

  // ── Bookmark render (depth === 0, directory) ──
  if (isBookmark) {
    return (
      <div style={{ marginBottom: '4px' }}>
        <div
          className={`file-tree-item file-tree-bookmark ${expanded ? 'expanded' : ''} ${isFolderActive ? 'active' : ''}`}
          data-path={fullPath}
          data-is-dir="true"
          style={{
            display: 'flex', alignItems: 'center', gap: '8px',
            padding: '8px 12px', margin: '0 8px',
            borderRadius: 'var(--radius-xs)', cursor: 'pointer', fontSize: '13px',
            fontWeight: 600, color: isFolderActive ? 'var(--gold)' : 'var(--bone)',
            background: isFolderActive ? 'rgba(221, 187, 101, 0.06)' : 'var(--raised)',
            border: `1px solid ${isFolderActive ? 'var(--gold)' : 'var(--line)'}`,
            transition: 'all 0.15s ease', position: 'relative',
          }}
          onClick={() => {
            if (showMenu) return;
            const next = !expanded;
            setExpanded(next);
            if (next && onFolderActivate) onFolderActivate(fullPath);
            else if (!next && onFolderActivate) onFolderActivate(null);
          }}
          onMouseEnter={(e) => {
            if (!isFolderActive) {
              e.currentTarget.style.background = 'var(--raised-2)';
              e.currentTarget.style.borderColor = 'var(--tangerine)';
            }
          }}
          onMouseLeave={(e) => {
            if (!isFolderActive) {
              e.currentTarget.style.background = 'var(--raised)';
              e.currentTarget.style.borderColor = 'var(--line)';
            }
          }}
          onContextMenu={(e) => {
            e.preventDefault();
            e.stopPropagation();
            setMenuPos({ x: e.clientX, y: e.clientY });
            if (onMenuChange) onMenuChange(fullPath);
          }}
        >
          <ChevronRight
            size={iconSize}
            style={{
              flexShrink: 0, color: 'var(--muted-2)',
              transition: 'transform 0.2s ease',
              transform: expanded ? 'rotate(90deg)' : 'rotate(0deg)',
            }}
          />
          <FolderOpen
            size={iconSize}
            style={{ flexShrink: 0, color: isFolderActive ? 'var(--gold)' : 'var(--tangerine)' }}
          />
          {renaming ? (
            <input
              ref={inputRef}
              value={newName}
              onChange={(e) => setNewName(e.target.value)}
              onBlur={handleRename}
              onKeyDown={(e) => {
                if (e.key === 'Enter') { e.preventDefault(); handleRename(); }
                if (e.key === 'Escape') { setRenaming(false); setNewName(entry.name); }
              }}
              className="inline-name-input"
              onClick={(e) => e.stopPropagation()}
              style={{ fontWeight: 600 }}
            />
          ) : (
            <span style={{ flex: 1, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap', minWidth: 0 }}>
              {entry.name}
            </span>
          )}

          <button
            onClick={(e) => {
              e.stopPropagation();
              setMenuPos({ x: e.clientX, y: e.clientY });
              if (onMenuChange) onMenuChange(showMenu ? null : fullPath);
            }}
            style={{
              display: 'flex', alignItems: 'center', justifyContent: 'center',
              width: '22px', height: '22px', borderRadius: '4px',
              border: 'none', background: 'transparent', color: 'var(--muted-2)',
              cursor: 'pointer', opacity: showMenu ? 1 : 0, transition: 'opacity 0.15s', flexShrink: 0,
            }}
            onMouseEnter={(e) => e.currentTarget.style.background = 'var(--surface)'}
            onMouseLeave={(e) => e.currentTarget.style.background = 'transparent'}
          >
            <MoreHorizontal size={14} />
          </button>

          {showMenu && (
            <div
              ref={menuRef}
              className="ctx-menu"
              style={{ position: 'fixed', left: menuPos.x, top: menuPos.y, zIndex: 100 }}
            >
              <div className="ctx-menu-header">Создать</div>
              <button className="ctx-menu-item" onClick={() => { setCreatingChild('file'); if (onMenuChange) onMenuChange(null); }}>
                <FileText size={12} /><span>Новый файл</span>
              </button>
              <button className="ctx-menu-item" onClick={() => { setCreatingChild('folder'); if (onMenuChange) onMenuChange(null); }}>
                <Folder size={12} /><span>Новая папка</span>
              </button>
              <div className="ctx-menu-sep" />
              <button className="ctx-menu-item" onClick={() => { setRenaming(true); if (onMenuChange) onMenuChange(null); }}>
                <Edit3 size={12} /><span>Переименовать</span>
              </button>
              <button className="ctx-menu-item" onClick={() => { navigator.clipboard.writeText(fullPath); if (onMenuChange) onMenuChange(null); }}>
                <Copy size={12} /><span>Копировать путь</span>
              </button>
              <div className="ctx-menu-sep" />
              <button className="ctx-menu-item" onClick={() => { if (onRemoveFromWorkspace) onRemoveFromWorkspace(fullPath, entry.isDir); if (onMenuChange) onMenuChange(null); }}>
                <XCircle size={12} /><span>Убрать из области</span>
              </button>
              <button className="ctx-menu-item danger" onClick={() => handleDelete()}>
                <Trash2 size={12} /><span>Удалить папку</span>
              </button>
            </div>
          )}
        </div>

        {creatingChild && (
          <div className="inline-create-row" style={{
            display: 'flex', flexDirection: 'column',
            padding: '3px 8px', paddingLeft: '32px',
          }}>
            <div style={{ display: 'flex', alignItems: 'center', gap: '6px' }}>
              {creatingChild === 'file'
                ? <FileText size={12} style={{ color: extensionError ? 'var(--rose)' : 'var(--periwinkle)', flexShrink: 0 }} />
                : <Folder size={12} style={{ color: 'var(--gold)', flexShrink: 0 }} />}
              <input
                ref={childInputRef}
                value={childName}
                onChange={(e) => handleChildNameChange(e.target.value)}
                onBlur={handleCancelCreate}
                onKeyDown={handleChildKeyDown}
                placeholder={creatingChild === 'file' ? 'filename.ext' : 'folder name'}
                className="inline-name-input"
                style={extensionError ? { borderColor: 'var(--rose)', color: 'var(--rose)' } : childNameWarning ? { borderColor: 'var(--tangerine)' } : undefined}
              />
              <button className="inline-cancel-btn" onClick={handleCancelCreate} title="Отмена (Esc)" type="button">
                <X size={12} />
              </button>
            </div>
            {extensionError && (
              <div style={{ display: 'flex', alignItems: 'center', gap: '4px', fontSize: '11px', color: 'var(--rose)', paddingLeft: '18px', marginTop: '2px' }}>
                <AlertTriangle size={10} /><span>Неподдерживаемое расширение</span>
              </div>
            )}
            {childNameWarning && (
              <div style={{ display: 'flex', alignItems: 'center', gap: '4px', fontSize: '11px', color: 'var(--tangerine)', paddingLeft: '18px', marginTop: '2px' }}>
                <AlertTriangle size={10} /><span>{childNameWarning}</span>
              </div>
            )}
          </div>
        )}

        {expanded && entry.children && (
          <FileTree
            entries={entry.children}
            basePath={fullPath}
            projectId={projectId}
            onFileSelect={onFileSelect}
            onRefresh={onRefresh}
            onFileDeleted={onFileDeleted}
            onFileRenamed={onFileRenamed}
            onBeforeDelete={onBeforeDelete}
            onBeforeRename={onBeforeRename}
            onBeforeCreate={onBeforeCreate}
            onRemoveFromWorkspace={onRemoveFromWorkspace}
            selectedFile={selectedFile}
            depth={depth + 1}
            activeMenuPath={activeMenuPath}
            onMenuChange={onMenuChange}
            activeFolderPath={activeFolderPath}
            onFolderActivate={onFolderActivate}
            undoRedo={undoRedo}
          />
        )}
      </div>
    );
  }

  // ── Regular item render (nested files/folders) ──
  return (
    <div>
      <div
        className={`file-tree-item ${isSelected ? 'selected' : ''}`}
        data-path={fullPath}
        data-is-dir={entry.isDir ? 'true' : 'false'}
        style={{
          display: 'flex', alignItems: 'center', gap: '4px',
          padding: '4px 8px', paddingLeft: `${depth * 12 + 8}px`,
          borderRadius: '6px', cursor: 'pointer', fontSize: '13px',
          color: isSelected ? 'var(--tangerine)' : isFolderActive ? 'var(--gold)' : 'var(--bone)',
          transition: 'background 0.1s, opacity 0.1s', position: 'relative',
          background: isFolderActive && !isSelected
            ? 'rgba(221, 187, 101, 0.04)'
            : undefined,
          borderLeft: isFolderActive && !isSelected ? '2px solid var(--gold)' : '2px solid transparent',
          userSelect: 'none',
        }}
        onClick={() => {
          if (showMenu) return;
          if (entry.isDir) {
            const next = !expanded;
            setExpanded(next);
            if (next && onFolderActivate) onFolderActivate(fullPath);
            else if (!next && onFolderActivate) onFolderActivate(null);
          } else {
            onFileSelect(fullPath);
          }
        }}
        onMouseEnter={(e) => { if (!isSelected && !isFolderActive) e.currentTarget.style.background = 'var(--raised)'; }}
        onMouseLeave={(e) => { if (!isSelected && !isFolderActive) e.currentTarget.style.background = ''; }}
        onContextMenu={(e) => {
          e.preventDefault();
          e.stopPropagation();
          setMenuPos({ x: e.clientX, y: e.clientY });
          if (onMenuChange) onMenuChange(fullPath);
        }}
      >
        {entry.isDir ? (
          expanded
            ? <ChevronDown size={iconSize} style={{ flexShrink: 0, color: 'var(--muted-2)' }} />
            : <ChevronRight size={iconSize} style={{ flexShrink: 0, color: 'var(--muted-2)' }} />
        ) : (
          <span style={{ width: iconSize, flexShrink: 0 }} />
        )}

        {entry.isDir ? (
          expanded
            ? <FolderOpen size={iconSize} style={{ flexShrink: 0, color: isFolderActive ? 'var(--gold)' : 'var(--muted-2)' }} />
            : <Folder size={iconSize} style={{ flexShrink: 0, color: isFolderActive ? 'var(--gold)' : 'var(--muted-2)' }} />
        ) : (
          <FileIcon size={iconSize} style={{ flexShrink: 0, color: 'var(--periwinkle)' }} />
        )}

        {renaming ? (
          <input
            ref={inputRef}
            value={newName}
            onChange={(e) => setNewName(e.target.value)}
            onBlur={handleRename}
            onKeyDown={(e) => {
              if (e.key === 'Enter') { e.preventDefault(); handleRename(); }
              if (e.key === 'Escape') { setRenaming(false); setNewName(entry.name); }
            }}
            className="inline-name-input"
            onClick={(e) => e.stopPropagation()}
          />
        ) : (
          <span style={{ flex: 1, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap', minWidth: 0 }}>
            {entry.name}
          </span>
        )}

        {!entry.isDir && entry.sizeBytes > 0 && (
          <span style={{ fontSize: '10px', color: 'var(--muted-2)', flexShrink: 0 }}>
            {formatSize(entry.sizeBytes)}
          </span>
        )}

        <button
          onClick={(e) => {
            e.stopPropagation();
            setMenuPos({ x: e.clientX, y: e.clientY });
            if (onMenuChange) onMenuChange(showMenu ? null : fullPath);
          }}
          style={{
            display: 'flex', alignItems: 'center', justifyContent: 'center',
            width: '20px', height: '20px', borderRadius: '4px',
            border: 'none', background: 'transparent', color: 'var(--muted-2)',
            cursor: 'pointer', opacity: showMenu ? 1 : 0, transition: 'opacity 0.15s', flexShrink: 0,
          }}
          onMouseEnter={(e) => e.currentTarget.style.background = 'var(--raised)'}
          onMouseLeave={(e) => e.currentTarget.style.background = 'transparent'}
        >
          <MoreHorizontal size={12} />
        </button>

        {showMenu && (
          <div
            ref={menuRef}
            className="ctx-menu"
            style={{ position: 'fixed', left: menuPos.x, top: menuPos.y, zIndex: 100 }}
          >
            {entry.isDir && (
              <>
                <div className="ctx-menu-header">Создать</div>
                <button className="ctx-menu-item" onClick={() => { setCreatingChild('file'); if (onMenuChange) onMenuChange(null); }}>
                  <FileText size={12} /><span>Новый файл</span>
                </button>
                <button className="ctx-menu-item" onClick={() => { setCreatingChild('folder'); if (onMenuChange) onMenuChange(null); }}>
                  <Folder size={12} /><span>Новая папка</span>
                </button>
                <div className="ctx-menu-sep" />
              </>
            )}
            <button className="ctx-menu-item" onClick={() => { setRenaming(true); if (onMenuChange) onMenuChange(null); }}>
              <Edit3 size={12} /><span>Переименовать</span>
            </button>
            <button className="ctx-menu-item" onClick={() => { navigator.clipboard.writeText(fullPath); if (onMenuChange) onMenuChange(null); }}>
              <Copy size={12} /><span>Копировать путь</span>
            </button>
            <div className="ctx-menu-sep" />
            <button className="ctx-menu-item" onClick={() => { if (onRemoveFromWorkspace) onRemoveFromWorkspace(fullPath, entry.isDir); if (onMenuChange) onMenuChange(null); }}>
              <XCircle size={12} /><span>Убрать из области</span>
            </button>
            <button className="ctx-menu-item danger" onClick={() => handleDelete()}>
              <Trash2 size={12} /><span>Удалить с компьютера</span>
            </button>
          </div>
        )}
      </div>

      {creatingChild && (
        <div className="inline-create-row" style={{
          display: 'flex', flexDirection: 'column',
          padding: '3px 8px', paddingLeft: `${(depth + 1) * 12 + 8}px`,
        }}>
          <div style={{ display: 'flex', alignItems: 'center', gap: '6px' }}>
            {creatingChild === 'file'
              ? <FileText size={12} style={{ color: extensionError ? 'var(--rose)' : 'var(--periwinkle)', flexShrink: 0 }} />
              : <Folder size={12} style={{ color: 'var(--gold)', flexShrink: 0 }} />}
            <input
              ref={childInputRef}
              value={childName}
              onChange={(e) => handleChildNameChange(e.target.value)}
              onBlur={handleCancelCreate}
              onKeyDown={handleChildKeyDown}
              placeholder={creatingChild === 'file' ? 'filename.ext' : 'folder name'}
              className="inline-name-input"
              style={extensionError ? { borderColor: 'var(--rose)', color: 'var(--rose)' } : childNameWarning ? { borderColor: 'var(--tangerine)' } : undefined}
            />
            <button className="inline-cancel-btn" onClick={handleCancelCreate} title="Отмена (Esc)" type="button">
              <X size={12} />
            </button>
          </div>
          {extensionError && (
            <div style={{ display: 'flex', alignItems: 'center', gap: '4px', fontSize: '11px', color: 'var(--rose)', paddingLeft: '18px', marginTop: '2px' }}>
              <AlertTriangle size={10} /><span>Неподдерживаемое расширение</span>
            </div>
          )}
          {childNameWarning && (
            <div style={{ display: 'flex', alignItems: 'center', gap: '4px', fontSize: '11px', color: 'var(--tangerine)', paddingLeft: '18px', marginTop: '2px' }}>
              <AlertTriangle size={10} /><span>{childNameWarning}</span>
            </div>
          )}
        </div>
      )}

      {entry.isDir && expanded && entry.children && (
        <FileTree
          entries={entry.children}
          basePath={fullPath}
          projectId={projectId}
          onFileSelect={onFileSelect}
          onRefresh={onRefresh}
          onFileDeleted={onFileDeleted}
          onFileRenamed={onFileRenamed}
          onBeforeDelete={onBeforeDelete}
          onBeforeRename={onBeforeRename}
          onBeforeCreate={onBeforeCreate}
          onRemoveFromWorkspace={onRemoveFromWorkspace}
          selectedFile={selectedFile}
          depth={depth + 1}
          activeMenuPath={activeMenuPath}
          onMenuChange={onMenuChange}
          activeFolderPath={activeFolderPath}
          onFolderActivate={onFolderActivate}
          undoRedo={undoRedo}
        />
      )}
    </div>
  );
}


