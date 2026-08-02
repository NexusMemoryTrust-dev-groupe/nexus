import { useState, useEffect, useCallback, useRef } from 'react';
import { useProjectStore } from '../../stores/projectStore';
import { useLocale } from '../../stores/localeStore';
import {
  ArrowLeft, FolderOpen, Brain, Network, Plus, Trash2,
  FileText, Loader2, Unlink, Edit3, Check, X, FolderPlus, FilePlus, RefreshCw, AlertTriangle,
} from 'lucide-react';
import { MarkdownRenderer } from '../ui/MarkdownRenderer';
import { MarkdownEditor } from '../ui/MarkdownEditor';
import { FileTree } from '../files/FileTree';
import { FileEditor } from '../files/FileEditor';
import { invoke } from '@tauri-apps/api/core';
import type { FileEntry } from '../../types';

/** Normalize paths that may have old debug format wrappers like [Path("...")] */
function cleanPath(p: string): string {
  let s = p.trim();
  while (s.startsWith('[') && s.endsWith(']')) s = s.slice(1, -1);
  const m = s.match(/^Path\("(.+)"\)$/);
  if (m) s = m[1];
  if (s.startsWith('"') && s.endsWith('"')) s = s.slice(1, -1);
  return s.replace(/\\\\/g, '\\');
}

const SUPPORTED_EXTS = [
  'md', 'markdown', 'mdx', 'mdown',
  'rs', 'py', 'js', 'ts', 'tsx', 'jsx',
  'go', 'java', 'c', 'cpp', 'h', 'css', 'html',
  'json', 'yaml', 'yml', 'toml', 'xml', 'svg',
  'sh', 'bash', 'ps1', 'bat', 'sql',
  'rb', 'php', 'swift', 'kt', 'txt',
];

function isValidFileExtension(name: string): boolean {
  const ext = name.split('.').pop()?.toLowerCase() || '';
  if (!ext) return false;
  return SUPPORTED_EXTS.includes(ext);
}

export function ProjectDetail() {
  const {
    selectedProject, projectEntities, projectEdges, projectMemories, isLoading,
    selectProject, createProjectMemory, deleteMemory, updateMemory,
    deleteRelationship, updateProject, deleteProject,
  } = useProjectStore();
  const { t } = useLocale();

  const [tab, setTab] = useState<'files' | 'memories' | 'entities'>('files');
  const [showAddMemory, setShowAddMemory] = useState(false);
  const [newMemTitle, setNewMemTitle] = useState('');
  const [newMemContent, setNewMemContent] = useState('');
  const [creating, setCreating] = useState(false);
  const [editingId, setEditingId] = useState<string | null>(null);
  const [editTitle, setEditTitle] = useState('');
  const [editContent, setEditContent] = useState('');
  const [editingProject, setEditingProject] = useState(false);
  const [projectEditTitle, setProjectEditTitle] = useState('');
  const [projectEditDesc, setProjectEditDesc] = useState('');
  const [fileTree, setFileTree] = useState<FileEntry | null>(null);
  const [selectedFile, setSelectedFile] = useState<string | null>(null);
  const [loadingFiles, setLoadingFiles] = useState(false);
  const [creatingInRoot, setCreatingInRoot] = useState<'file' | 'folder' | null>(null);
  const [rootChildName, setRootChildName] = useState('');
  const rootChildInputRef = useRef<HTMLInputElement>(null);
  const [activeMenuPath, setActiveMenuPath] = useState<string | null>(null);
  const [activeFolderPath, setActiveFolderPath] = useState<string | null>(null);
  const [createNameWarning, setCreateNameWarning] = useState('');

// ── Undo stack for file operations ──
interface DeletedItem {
  type: 'delete' | 'rename' | 'create' | 'detach';
  path: string;             // current path (for rename: old path before rename)
  newPath?: string;         // for rename: new path after rename
  name: string;
  isDir: boolean;
  content: string;          // file content (empty for dirs)
  parentPath: string;       // parent directory
  children?: DeletedItem[]; // for dirs: recursively stored children
  detachType?: 'root' | 'attached' | 'workspace'; // for detach: was it root_folder, attached_files, or workspace entry
}
const undoStackRef = useRef<DeletedItem[]>([]);
const redoStackRef = useRef<DeletedItem[]>([]);
// Guard against React StrictMode double-mount
const lastProjectIdRef = useRef<string | null>(null);
const initializedProjectsRef = useRef<Set<string>>(new Set());

  // ── Load file tree and metadata when project changes ──
  useEffect(() => {
    if (selectedProject) {
      // Detect actual project change (not re-render) — reset guard for new project only
      if (lastProjectIdRef.current !== selectedProject.id) {
        initializedProjectsRef.current.delete(selectedProject.id);
        lastProjectIdRef.current = selectedProject.id;
      }
      loadProjectFiles();
    }
  }, [selectedProject?.id]);

  // ── Filesystem sync polling — checks disk every 3s, removes stale entries, adds new files ──
  useEffect(() => {
    if (!selectedProject || tab !== 'files') return;
    const interval = setInterval(async () => {
      try {
        const result = await invoke<{ tree: FileEntry | null; stale_found: boolean }>('sync_workspace', {
          projectId: selectedProject.id,
        });
        // If folders were deleted from disk AND workspace is now empty → delete the project
        if (result.stale_found && (!result.tree || !result.tree.children || result.tree.children.length === 0)) {
          await deleteProject(selectedProject.id);
          return;
        }
        // Only update if tree actually changed (avoid unnecessary re-renders)
        setFileTree(prev => {
          if (!result.tree && !prev) return prev;
          if (!result.tree || !prev) return result.tree;
          if (JSON.stringify(result.tree) !== JSON.stringify(prev)) return result.tree;
          return prev;
        });
      } catch (e) {
        console.error('Sync workspace failed:', e);
      }
    }, 3000);
    return () => clearInterval(interval);
  }, [selectedProject?.id, tab]);

  const loadProjectFiles = useCallback(async () => {
    if (!selectedProject) return;
    const pid = selectedProject.id;
    // SYNCHRONOUS guard — blocks StrictMode second call BEFORE any await
    if (initializedProjectsRef.current.has(pid)) {
      // Already initialized — just refresh tree
      try {
        const tree = await invoke<FileEntry | null>('get_workspace_tree', { projectId: pid });
        setFileTree(tree);
      } catch { /* ignore */ }
      return;
    }
    // Mark IMMEDIATELY (synchronously, no await before this)
    initializedProjectsRef.current.add(pid);

    setLoadingFiles(true);
    try {
      const tree = await invoke<FileEntry | null>('get_workspace_tree', {
        projectId: pid,
      });
      if (!tree || !tree.children || tree.children.length === 0) {
        // Empty workspace — auto-create a Desktop folder named after the project
        const desktop = await invoke<string>('get_desktop_dir');
        const safeName = selectedProject.title.replace(/[<>:"/\\|?*]/g, '_').trim() || 'Project';
        const projectFolder = `${desktop}\\${safeName}`;
        try {
          await invoke('create_folder', { folderPath: projectFolder });
          await invoke('add_to_workspace', {
            projectId: selectedProject.id,
            paths: [projectFolder],
          });
          const tree2 = await invoke<FileEntry | null>('get_workspace_tree', {
            projectId: selectedProject.id,
          });
          setFileTree(tree2);
          setActiveFolderPath(projectFolder);
        } catch (e2) {
          console.error('Auto-create project folder failed:', e2);
          setFileTree(null);
        }
      } else {
        setFileTree(tree);
      }
    } catch (e) {
      console.error('Load workspace failed:', e);
      setFileTree(null);
    } finally {
      setLoadingFiles(false);
    }
  }, [selectedProject]);

  // ── Undo helpers (read file content before delete/rename) ──
  const readForUndo = useCallback(async (filePath: string): Promise<string> => {
    try {
      const info = await invoke<{ content: string }>('read_file', { filePath });
      return info.content;
    } catch { return ''; }
  }, []);

  const readFolderForUndo = useCallback(async (folderPath: string): Promise<DeletedItem[]> => {
    try {
      const tree = await invoke<FileEntry>('scan_folder', { folderPath });
      const items: DeletedItem[] = [];
      for (const entry of tree.children || []) {
        if (entry.isDir) {
          const children = await readFolderForUndo(entry.path);
          items.push({ type: 'delete', path: entry.path, name: entry.name, isDir: true, content: '', parentPath: folderPath, children });
        } else {
          const content = await readForUndo(entry.path);
          items.push({ type: 'delete', path: entry.path, name: entry.name, isDir: false, content, parentPath: folderPath });
        }
      }
      return items;
    } catch { return []; }
  }, [readForUndo]);

  // ── Store operations for undo ──
  const storeDeletedForUndo = useCallback(async (path: string, isDir: boolean): Promise<void> => {
    const name = path.split(/[/\\]/).pop() || path;
    const parentPath = path.split(/[/\\]/).slice(0, -1).join('\\');
    if (isDir) {
      const children = await readFolderForUndo(path);
      undoStackRef.current.push({ type: 'delete', path, name, isDir: true, content: '', parentPath, children });
    } else {
      const content = await readForUndo(path);
      undoStackRef.current.push({ type: 'delete', path, name, isDir: false, content, parentPath });
    }
    if (undoStackRef.current.length > 50) undoStackRef.current.shift();
    redoStackRef.current = []; // Clear redo on new action
  }, [readForUndo, readFolderForUndo]);

  const storeRenamedForUndo = useCallback((_oldPath: string, _isDir: boolean): Promise<void> => {
    // No-op: rename undo is handled in handleFileRenamedInTree after rename succeeds
    return Promise.resolve();
  }, []);

  const storeCreatedForUndo = useCallback((path: string, isDir: boolean): Promise<void> => {
    const name = path.split(/[/\\]/).pop() || path;
    const parentPath = path.split(/[/\\]/).slice(0, -1).join('\\');
    undoStackRef.current.push({ type: 'create', path, name, isDir, content: '', parentPath });
    if (undoStackRef.current.length > 50) undoStackRef.current.shift();
    redoStackRef.current = []; // Clear redo on new action
    return Promise.resolve();
  }, []);

  // ── Restore from undo stack ──
  const restoreFromUndo = useCallback(async () => {
    const item = undoStackRef.current.pop();
    if (!item || !selectedProject) return;
    try {
      switch (item.type) {
        case 'delete': {
          // Recreate on disk
          if (item.isDir) {
            await invoke('create_folder', { folderPath: item.path });
            const restoreChildren = async (children: DeletedItem[]) => {
              for (const child of children) {
                if (child.isDir) {
                  await invoke('create_folder', { folderPath: child.path });
                  if (child.children) await restoreChildren(child.children);
                } else {
                  await invoke('create_file', { filePath: child.path, content: child.content });
                }
              }
            };
            if (item.children) await restoreChildren(item.children);
          } else {
            await invoke('create_file', { filePath: item.path, content: item.content });
          }
          // Re-add to workspace
          await invoke('add_to_workspace', {
            projectId: selectedProject.id,
            paths: [item.path],
          });
          break;
        }
        case 'rename': {
          // Rename back to old name
          await invoke('rename_file', { oldPath: item.newPath, newName: item.name });
          // Update workspace entry
          await invoke('rename_workspace_entry', {
            projectId: selectedProject.id,
            oldPath: item.newPath,
            newName: item.name,
          });
          break;
        }
        case 'create': {
          // Delete what was created
          if (item.isDir) {
            await invoke('delete_folder', { folderPath: item.path });
          } else {
            await invoke('delete_file', { filePath: item.path });
          }
          // Remove from workspace
          await invoke('remove_from_workspace', {
            projectId: selectedProject.id,
            filePath: item.path,
          });
          break;
        }
        case 'detach': {
          // Re-add to workspace
          await invoke('add_to_workspace', {
            projectId: selectedProject.id,
            paths: [item.path],
          });
          break;
        }
      }
      redoStackRef.current.push(item);
      if (redoStackRef.current.length > 50) redoStackRef.current.shift();
      loadProjectFiles();
    } catch (e) {
      console.error('Restore failed:', e);
    }
  }, [loadProjectFiles, selectedProject]);

  // ── Restore from redo stack ──
  const restoreFromRedo = useCallback(async () => {
    const item = redoStackRef.current.pop();
    if (!item || !selectedProject) return;
    try {
      switch (item.type) {
        case 'delete': {
          // Re-delete what was restored
          if (item.isDir) {
            await invoke('delete_folder', { folderPath: item.path });
          } else {
            await invoke('delete_file', { filePath: item.path });
          }
          await invoke('remove_from_workspace', {
            projectId: selectedProject.id,
            filePath: item.path,
          });
          break;
        }
        case 'rename': {
          // Rename to new name again
          const newName = item.newPath?.split(/[/\\]/).pop() || '';
          await invoke('rename_file', { oldPath: item.path, newName });
          await invoke('rename_workspace_entry', {
            projectId: selectedProject.id,
            oldPath: item.path,
            newName,
          });
          break;
        }
        case 'create': {
          // Re-create what was undone
          if (item.isDir) {
            await invoke('create_folder', { folderPath: item.path });
          } else {
            await invoke('create_file', { filePath: item.path, content: item.content });
          }
          await invoke('add_to_workspace', {
            projectId: selectedProject.id,
            paths: [item.path],
          });
          break;
        }
        case 'detach': {
          // Re-detach from workspace
          await invoke('remove_from_workspace', {
            projectId: selectedProject.id,
            filePath: item.path,
          });
          break;
        }
      }
      loadProjectFiles();
    } catch (e) {
      console.error('Redo failed:', e);
    }
  }, [loadProjectFiles, selectedProject]);

  // ── Ctrl+Z/Y handler for file tree undo/redo ──
  // Fires globally: undo stack operations (delete/rename/create/detach) from the
  // file tree context menu. When focus is on a Tiptap/textarea (contenteditable
  // or INPUT/TEXTAREA), we skip — the editor handles its own undo.
  useEffect(() => {
    const handleKey = (e: KeyboardEvent) => {
      const target = e.target as HTMLElement;
      const tag = target?.tagName;
      const isEditable = target?.getAttribute('contenteditable');

      // Skip if focus is inside a text-editing element (let native undo work)
      if (tag === 'INPUT' || tag === 'TEXTAREA' || isEditable === 'true') return;

      // Ctrl+Z — Undo. `e.code` is the physical key position, so this fires on
      // any keyboard layout without needing per-layout character literals.
      const isZ = e.code === 'KeyZ';
      if (e.ctrlKey && !e.shiftKey && isZ) {
        if (undoStackRef.current.length === 0) return;
        e.preventDefault();
        restoreFromUndo();
        return;
      }

      // Ctrl+Y or Ctrl+Shift+Z — Redo
      const isY = e.code === 'KeyY';
      const isShiftZ = e.shiftKey && isZ;
      if (e.ctrlKey && (isY || isShiftZ)) {
        if (redoStackRef.current.length === 0) return;
        e.preventDefault();
        restoreFromRedo();
        return;
      }
    };
    window.addEventListener('keydown', handleKey);
    return () => window.removeEventListener('keydown', handleKey);
  }, [restoreFromUndo, restoreFromRedo]);

  const handleAddFiles = useCallback(async () => {
    if (!selectedProject) return;
    try {
      const paths = await invoke<string[]>('pick_files', {
        title: 'Select files to add to workspace',
        filters: ['All Files|*.*', 'Markdown|*.md', 'Text|*.txt', 'Code|*.rs,*.py,*.js,*.ts,*.tsx'],
      });
      if (paths.length > 0) {
        const cleanPaths = paths.map(cleanPath);
        await invoke<FileEntry | null>('add_to_workspace', {
          projectId: selectedProject.id,
          paths: cleanPaths,
        });
        loadProjectFiles();
      }
    } catch (e) {
      console.error('Add files failed:', e);
    }
  }, [selectedProject, loadProjectFiles]);

  const handleAddFolder = useCallback(async () => {
    if (!selectedProject) return;
    try {
      const path = await invoke<string | null>('pick_folder', {
        title: 'Select a folder to add to workspace',
      });
      if (path) {
        const clean = cleanPath(path);
        await invoke<FileEntry | null>('add_to_workspace', {
          projectId: selectedProject.id,
          paths: [clean],
        });
        loadProjectFiles();
      }
    } catch (e) {
      console.error('Add folder failed:', e);
    }
  }, [selectedProject, loadProjectFiles]);

  const handleRemoveFromWorkspace = useCallback(async (path: string, _isDir: boolean) => {
    if (!selectedProject) return;
    // Clear selection if it was the removed item
    if (selectedFile === path || (selectedFile && selectedFile.startsWith(path + '\\'))) {
      setSelectedFile(null);
    }
    try {
      // Read content for undo before removing (for potential undo/redo)
      let content = '';
      if (!_isDir) {
        try {
          const info = await invoke<{ content: string }>('read_file', { filePath: path });
          content = info.content;
        } catch {
          // Undo only needs the content to restore a file; if it is unreadable
          // (binary, deleted, locked) an empty string still lets the entry be
          // recreated, so this is not worth surfacing to the user.
        }
      }
      undoStackRef.current.push({
        type: 'detach', path, name: path.split(/[/\\]/).pop() || path,
        isDir: _isDir, content, parentPath: '', detachType: 'workspace',
      });
      if (undoStackRef.current.length > 50) undoStackRef.current.shift();
      redoStackRef.current = [];

      await invoke('remove_from_workspace', {
        projectId: selectedProject.id,
        filePath: path,
      });
      loadProjectFiles();
    } catch (e) {
      console.error('Remove from workspace failed:', e);
    }
  }, [selectedProject, selectedFile, loadProjectFiles]);

  // Called when a file/folder is deleted from the tree via context menu
  // FileTree already did delete from disk + remove_from_workspace — just clear selection + refresh
  const handleFileDeletedFromTree = useCallback(async (path: string, _isDir: boolean) => {
    if (selectedFile === path || (selectedFile && selectedFile.startsWith(path + '\\'))) {
      setSelectedFile(null);
    }
    loadProjectFiles();
  }, [selectedFile, loadProjectFiles]);

  // Called when a file/folder is renamed from the tree
  // FileTree already did the rename in workspace DB + disk — just store undo + refresh
  const handleFileRenamedInTree = useCallback(async (oldPath: string, newPath: string) => {
    // Store rename for Ctrl+Z undo
    const name = oldPath.split(/[/\\]/).pop() || oldPath;
    const parentPath = oldPath.split(/[/\\]/).slice(0, -1).join('\\');
    const isDir = oldPath.endsWith('\\') || oldPath.endsWith('/') || (!oldPath.includes('.'));
    undoStackRef.current.push({ type: 'rename', path: oldPath, newPath, name, isDir, content: '', parentPath });
    if (undoStackRef.current.length > 50) undoStackRef.current.shift();
    redoStackRef.current = [];
    if (selectedFile === oldPath) {
      setSelectedFile(newPath);
    }
    loadProjectFiles();
  }, [selectedFile, loadProjectFiles]);

  // Create file/folder inside root folder
  useEffect(() => {
    if (creatingInRoot && rootChildInputRef.current) {
      rootChildInputRef.current.focus();
    }
  }, [creatingInRoot]);

  const handleCreateInRoot = useCallback(async () => {
    if (!rootChildName.trim() || !selectedProject) {
      setCreatingInRoot(null);
      return;
    }
    const isDir = creatingInRoot === 'folder';

    // Markdown-only validation for files
    if (!isDir && !isValidFileExtension(rootChildName.trim())) {
      setCreateNameWarning('Неподдерживаемое расширение');
      return;
    }

    // Resolve parent path: prefer active folder, fallback to tree root
    let parentPath = activeFolderPath || '';
    if (!parentPath && fileTree && fileTree.path) {
      parentPath = fileTree.path;
    } else if (!parentPath && fileTree && fileTree.children && fileTree.children.length > 0) {
      parentPath = fileTree.children[0].path.split(/[/\\]/).slice(0, -1).join('\\');
    }
    if (!parentPath) {
      setCreatingInRoot(null);
      return;
    }

    // Name collision check — verify item doesn't already exist in parent directory
    // Try in-memory workspace tree first (reliable, no IPC needed)
    const lowerName = rootChildName.trim().toLowerCase();
    let collisionFound = false;
    if (fileTree) {
      // Find the parent node in the tree by path
      const findNode = (node: FileEntry, targetPath: string): FileEntry | null => {
        if (node.path === parentPath) return node;
        if (node.children) {
          for (const child of node.children) {
            const found = findNode(child, targetPath);
            if (found) return found;
          }
        }
        return null;
      };
      const parentNode = findNode(fileTree, parentPath);
      if (parentNode?.children) {
        collisionFound = parentNode.children.some(c => c.name.toLowerCase() === lowerName);
      }
    }
    // Fallback: check disk via IPC if in-memory check found nothing
    if (!collisionFound) {
      try {
        const names = await invoke<string[]>('list_dir_names', { dirPath: parentPath });
        collisionFound = names.some(n => n.toLowerCase() === lowerName);
      } catch { /* ignore */ }
    }
    if (collisionFound) {
      setCreateNameWarning(`«${rootChildName.trim()}» уже существует`);
      return;
    }

    try {
      const sep = parentPath.includes('\\') ? '\\' : '/';
      const childPath = `${parentPath}${sep}${rootChildName.trim()}`;
      await storeCreatedForUndo(childPath, isDir);
      await invoke('create_workspace_entry', {
        projectId: selectedProject.id,
        parentPath: parentPath,
        name: rootChildName.trim(),
        isDir: isDir,
      });
      loadProjectFiles();
    } catch (e) {
      console.error('Create in workspace failed:', e);
    }
    setCreatingInRoot(null);
    setRootChildName('');
    setCreateNameWarning('');
  }, [rootChildName, creatingInRoot, fileTree, activeFolderPath, selectedProject, loadProjectFiles, storeCreatedForUndo]);

  if (!selectedProject) return null;

  const startEditProject = () => {
    setProjectEditTitle(selectedProject.title);
    setProjectEditDesc(selectedProject.description);
    setEditingProject(true);
  };

  const saveProjectEdit = async () => {
    if (!projectEditTitle.trim()) return;
    const oldTitle = selectedProject.title;
    const newTitle = projectEditTitle.trim();
    await updateProject(selectedProject.id, newTitle, projectEditDesc.trim());
    setEditingProject(false);
    // Auto-rename managed folder on disk if title changed
    if (oldTitle !== newTitle && fileTree?.children && fileTree.children.length > 0) {
      const firstChild = fileTree.children[0];
      if (firstChild?.isDir) {
        const oldFolderName = oldTitle.replace(/[<>:"/\\|?*]/g, '_').trim() || 'Project';
        const folderPath = firstChild.path;
        // Only rename if folder name matches the old title pattern (managed folder)
        if (folderPath.endsWith('\\' + oldFolderName) || folderPath.endsWith('/' + oldFolderName)) {
          const newFolderName = newTitle.replace(/[<>:"/\\|?*]/g, '_').trim() || 'Project';
          try {
            await invoke('rename_managed_folder', {
              projectId: selectedProject.id,
              oldPath: folderPath,
              newName: newFolderName,
            });
            loadProjectFiles();
          } catch (e) {
            console.error('Auto-rename managed folder failed:', e);
          }
        }
      }
    }
  };

  const handleAddMemory = async () => {
    if (!newMemTitle.trim() || !newMemContent.trim()) return;
    setCreating(true);
    try {
      await createProjectMemory(selectedProject.id, newMemTitle.trim(), newMemContent.trim());
      setNewMemTitle('');
      setNewMemContent('');
      setShowAddMemory(false);
    } catch {
      // error in store
    } finally {
      setCreating(false);
    }
  };

  const startEdit = (mem: { id: string; title: string; content: string }) => {
    setEditingId(mem.id);
    setEditTitle(mem.title);
    setEditContent(mem.content);
  };

  const saveEdit = async (id: string) => {
    await updateMemory(id, editTitle, editContent);
    setEditingId(null);
  };

  return (
    <div style={{ padding: '24px', height: '100%', overflow: 'auto' }}>
      {/* Back + Header */}
      <div style={{ display: 'flex', alignItems: 'center', gap: '12px', marginBottom: '24px' }}>
        <button
          className="btn-icon"
          onClick={() => selectProject(null)}
          style={{ color: 'var(--muted)' }}
        >
          <ArrowLeft size={18} />
        </button>
        <div style={{
          width: '40px', height: '40px', display: 'flex', alignItems: 'center', justifyContent: 'center',
          background: 'var(--tangerine-soft)', borderRadius: '12px', color: 'var(--tangerine)',
        }}>
          <FolderOpen size={20} />
        </div>
        <div style={{ flex: 1 }}>
          {editingProject ? (
            <div style={{ display: 'flex', flexDirection: 'column', gap: '6px' }}>
              <input
                value={projectEditTitle}
                onChange={(e) => setProjectEditTitle(e.target.value)}
                onKeyDown={(e) => e.key === 'Enter' && saveProjectEdit()}
                autoFocus
                style={{
                  padding: '6px 10px', background: 'var(--carbon)', border: '1px solid var(--line)',
                  borderRadius: 'var(--radius-xs)', color: 'var(--bone)', fontSize: '18px',
                  fontFamily: 'var(--brand)', fontWeight: 700, outline: 'none',
                }}
              />
              <input
                value={projectEditDesc}
                onChange={(e) => setProjectEditDesc(e.target.value)}
                placeholder="Description (optional)"
                style={{
                  padding: '4px 10px', background: 'var(--carbon)', border: '1px solid var(--line)',
                  borderRadius: 'var(--radius-xs)', color: 'var(--muted)', fontSize: '13px',
                  fontFamily: 'var(--sans)', outline: 'none',
                }}
              />
              <div style={{ display: 'flex', gap: '6px' }}>
                <button className="btn-icon" onClick={() => setEditingProject(false)}
                  style={{ color: 'var(--muted)' }}>
                  <X size={14} />
                </button>
                <button className="btn-icon" onClick={saveProjectEdit}
                  style={{ color: 'var(--mint)' }}>
                  <Check size={14} />
                </button>
              </div>
            </div>
          ) : (
            <>
              <div className="editable-wrapper" onClick={startEditProject} style={{ position: 'relative' }}>
                <h2
                  style={{
                    fontFamily: 'var(--brand)', fontSize: '20px', fontWeight: 700,
                    color: 'var(--bone)', letterSpacing: '-0.02em', cursor: 'pointer',
                    margin: 0,
                  }}
                >
                  {selectedProject.title}
                </h2>
                {selectedProject.description && (
                  <p style={{ fontSize: '13px', color: 'var(--muted)', marginTop: '2px', cursor: 'pointer', margin: 0 }}>
                    {selectedProject.description}
                  </p>
                )}
                {fileTree?.path && (
                  <div className="project-header-path">
                    <Network size={11} style={{ color: 'var(--tangerine)', opacity: 0.6 }} />
                    <span>{fileTree.path}</span>
                  </div>
                )}
                <div className="edit-indicator">
                  <Edit3 size={11} />
                </div>
              </div>
            </>
          )}
        </div>
      </div>

      {/* Tabs */}
      <div style={{ display: 'flex', gap: '4px', marginBottom: '20px' }}>
        {([
          { key: 'files' as const, icon: FolderOpen, label: t('projects.files') },
          { key: 'memories' as const, icon: Brain, label: t('projects.memories') },
          { key: 'entities' as const, icon: Network, label: t('projects.entities') },
        ]).map(({ key, icon: Icon, label }) => (
          <button
            key={key}
            onClick={() => setTab(key)}
            style={{
              display: 'flex', alignItems: 'center', gap: '6px',
              padding: '8px 16px', borderRadius: 'var(--radius-xs)',
              background: tab === key ? 'var(--tangerine-soft)' : 'transparent',
              border: `1px solid ${tab === key ? 'rgba(255, 138, 91, 0.2)' : 'transparent'}`,
              color: tab === key ? 'var(--tangerine)' : 'var(--muted)',
              fontSize: '13px', fontWeight: 500, cursor: 'pointer',
              transition: 'all 0.15s ease',
            }}
          >
            <Icon size={14} />
            {label}
          </button>
        ))}
      </div>

      {isLoading && (
        <div className="empty-state">
          <Loader2 size={48} className="empty-state-icon spinning" />
          <div className="empty-state-title">{t('common.loading')}</div>
        </div>
      )}

      {/* ── Memories Tab ── */}
      {!isLoading && tab === 'memories' && (
        <div>
          {/* Add memory button */}
          <div style={{ display: 'flex', justifyContent: 'flex-end', marginBottom: '16px' }}>
            <button
              className="settings-action-btn"
              onClick={() => setShowAddMemory(!showAddMemory)}
              style={{ display: 'flex', alignItems: 'center', gap: '6px' }}
            >
              <Plus size={14} />
              {t('projects.addMemory')}
            </button>
          </div>

          {/* Add memory form */}
          {showAddMemory && (
            <div style={{
              background: 'var(--surface)', border: '1px solid var(--line)',
              borderRadius: 'var(--radius)', padding: '20px', marginBottom: '16px',
            }}>
              <div style={{ display: 'flex', flexDirection: 'column', gap: '12px' }}>
                <input
                  type="text"
                  placeholder={t('projects.memoryTitlePlaceholder')}
                  value={newMemTitle}
                  onChange={(e) => setNewMemTitle(e.target.value)}
                  autoFocus
                  style={{
                    padding: '10px 14px', background: 'var(--carbon)', border: '1px solid var(--line)',
                    borderRadius: 'var(--radius-xs)', color: 'var(--bone)', fontSize: '14px',
                    fontFamily: 'var(--sans)', outline: 'none',
                  }}
                />
                <MarkdownEditor
                  value={newMemContent}
                  onChange={setNewMemContent}
                  placeholder={t('projects.memoryContentPlaceholder')}
                  minHeight={120}
                />
                <div style={{ display: 'flex', gap: '8px', justifyContent: 'flex-end' }}>
                  <button className="settings-action-btn" onClick={() => setShowAddMemory(false)}
                    style={{ background: 'var(--raised)', color: 'var(--muted)' }}>
                    {t('common.cancel')}
                  </button>
                  <button className="settings-action-btn" onClick={handleAddMemory}
                    disabled={!newMemTitle.trim() || creating}>
                    {creating ? <Loader2 size={14} className="spinning" /> : <Plus size={14} />}
                    {t('projects.addMemory')}
                  </button>
                </div>
              </div>
            </div>
          )}

          {/* Empty */}
          {projectMemories.length === 0 && !showAddMemory && (
            <div className="empty-state" style={{ minHeight: '200px' }}>
              <Brain size={72} className="empty-state-icon" />
              <div className="empty-state-title">{t('projects.noMemories')}</div>
              <div className="empty-state-desc">{t('projects.noMemoriesDesc')}</div>
            </div>
          )}

          {/* Memory cards */}
          <div style={{ display: 'flex', flexDirection: 'column', gap: '8px' }}>
            {projectMemories.map((mem) => (
              <div
                key={mem.id}
                className="memory-card"
                style={{
                  padding: '16px', background: 'var(--surface)', border: '1px solid var(--line)',
                  borderRadius: 'var(--radius-sm)', position: 'relative',
                }}
              >
                {editingId === mem.id ? (
                  /* Edit mode */
                  <div style={{ display: 'flex', flexDirection: 'column', gap: '8px' }}>
                    <input
                      value={editTitle}
                      onChange={(e) => setEditTitle(e.target.value)}
                      style={{
                        padding: '8px 12px', background: 'var(--carbon)', border: '1px solid var(--line)',
                        borderRadius: 'var(--radius-xs)', color: 'var(--bone)', fontSize: '14px',
                        fontFamily: 'var(--sans)', outline: 'none',
                      }}
                    />
                    <MarkdownEditor
                      value={editContent}
                      onChange={setEditContent}
                      placeholder="Write in markdown..."
                      minHeight={120}
                    />
                    <div style={{ display: 'flex', gap: '6px', justifyContent: 'flex-end' }}>
                      <button className="btn-icon" onClick={() => setEditingId(null)}
                        style={{ color: 'var(--muted)' }}>
                        <X size={14} />
                      </button>
                      <button className="btn-icon" onClick={() => saveEdit(mem.id)}
                        style={{ color: 'var(--mint)' }}>
                        <Check size={14} />
                      </button>
                    </div>
                  </div>
                ) : (
                  /* View mode */
                  <>
                    <div className="editable-wrapper" onClick={() => startEdit(mem)}>
                      <div style={{
                        fontFamily: 'var(--brand)', fontSize: '14px', fontWeight: 600,
                        color: 'var(--bone)', marginBottom: '8px',
                      }}>
                        {mem.title}
                      </div>
                      <div style={{ fontSize: '13px', color: 'var(--muted)' }}>
                        <MarkdownRenderer content={mem.content} />
                      </div>
                      <div className="edit-indicator">
                        <Edit3 size={11} />
                      </div>
                    </div>
                    <div className="edit-actions" style={{ position: 'absolute', top: '12px', right: '12px', display: 'flex', gap: '4px' }}>
                      <button className="btn-icon" onClick={() => startEdit(mem)}
                        style={{ color: 'var(--muted-2)' }}>
                        <Edit3 size={13} />
                      </button>
                      <button className="btn-icon" onClick={() => deleteMemory(mem.id)}
                        style={{ color: 'var(--rose)' }}>
                        <Trash2 size={13} />
                      </button>
                    </div>
                    <div style={{
                      display: 'flex', gap: '12px', marginTop: '8px', fontSize: '11px', color: 'var(--muted-2)',
                    }}>
                      <span>{mem.author}</span>
                      <span>{new Date(mem.createdAt).toLocaleDateString()}</span>
                      <span style={{
                        padding: '1px 6px', borderRadius: '4px',
                        background: 'var(--periwinkle-soft)', color: 'var(--periwinkle)',
                      }}>
                        {mem.layer}
                      </span>
                    </div>
                  </>
                )}
              </div>
            ))}
          </div>
        </div>
      )}

      {/* ── Entities Tab ── */}
      {!isLoading && tab === 'entities' && (
        <div>
          {projectEntities.length === 0 ? (
            <div className="empty-state" style={{ minHeight: '200px' }}>
              <Network size={72} className="empty-state-icon" />
              <div className="empty-state-title">{t('projects.noEntities')}</div>
              <div className="empty-state-desc">{t('projects.noEntitiesDesc')}</div>
            </div>
          ) : (
            <div style={{ display: 'flex', flexDirection: 'column', gap: '8px' }}>
              {projectEntities.map((entity) => {
                // Find the relationship linking this entity to the project
                const linkingEdge = projectEdges.find(
                  (edge: { sourceEntityId: string; targetEntityId: string; id: string; relationshipType: string }) =>
                    (edge.sourceEntityId === selectedProject.id && edge.targetEntityId === entity.id) ||
                    (edge.sourceEntityId === entity.id && edge.targetEntityId === selectedProject.id)
                );
                return (
                  <div
                    key={entity.id}
                    className="entity-row"
                    style={{
                      display: 'flex', alignItems: 'center', gap: '12px',
                      padding: '14px 16px', background: 'var(--surface)',
                      border: '1px solid var(--line)', borderRadius: 'var(--radius-sm)',
                      position: 'relative',
                    }}
                  >
                    <div style={{
                      width: '32px', height: '32px', display: 'flex', alignItems: 'center', justifyContent: 'center',
                      background: 'var(--periwinkle-soft)', borderRadius: '8px', color: 'var(--periwinkle)',
                      flexShrink: 0,
                    }}>
                      <FileText size={16} />
                    </div>
                    <div style={{ flex: 1, minWidth: 0 }}>
                      <div style={{ fontSize: '14px', fontWeight: 600, color: 'var(--bone)' }}>
                        {entity.title}
                      </div>
                      <div style={{ fontSize: '12px', color: 'var(--muted-2)', display: 'flex', gap: '8px', marginTop: '2px' }}>
                        <span style={{
                          padding: '1px 6px', borderRadius: '4px',
                          background: 'var(--steel-soft)', color: 'var(--steel)',
                        }}>
                          {entity.entityType}
                        </span>
                        {linkingEdge && (
                          <span style={{
                            padding: '1px 6px', borderRadius: '4px',
                            background: 'var(--tangerine-soft)', color: 'var(--tangerine)',
                          }}>
                            {linkingEdge.relationshipType}
                          </span>
                        )}
                      </div>
                    </div>
                    {linkingEdge && (
                      <button
                        className="btn-icon entity-actions"
                        onClick={() => deleteRelationship(linkingEdge.id)}
                        style={{ color: 'var(--muted-2)', flexShrink: 0, position: 'absolute', right: '12px', top: '50%', transform: 'translateY(-50%)' }}
                        title={t('projects.unlink')}
                      >
                        <Unlink size={14} />
                      </button>
                    )}
                  </div>
                );
              })}
            </div>
          )}
        </div>
      )}

      {/* ── Files Tab ── */}
      {!isLoading && tab === 'files' && (
        <div style={{ display: 'flex', gap: '16px', height: 'calc(100vh - 200px)' }}>
          {/* File tree panel */}
          <div style={{
            width: '280px', flexShrink: 0, display: 'flex', flexDirection: 'column',
            background: 'var(--surface)', border: '1px solid var(--line)',
            borderRadius: 'var(--radius)', overflow: 'hidden',
          }}>
            {/* File tree header */}
            <div style={{
              display: 'flex', flexDirection: 'column',
              padding: '10px 12px', borderBottom: '1px solid var(--line)', gap: '8px',
            }}>
              <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
                <div style={{ display: 'flex', flexDirection: 'column', minWidth: 0, flex: 1 }}>
                  <span style={{ fontSize: '12px', fontWeight: 600, color: 'var(--muted)', textTransform: 'uppercase', letterSpacing: '0.05em' }}>
                    {t('projects.files')}
                  </span>
                </div>
                <button
                  className="btn-icon"
                  onClick={loadProjectFiles}
                  title="Refresh"
                  style={{ color: 'var(--muted-2)' }}
                >
                  <RefreshCw size={14} />
                </button>
              </div>
              {/* Action buttons row */}
              <div style={{ display: 'flex', gap: '4px', alignItems: 'center' }}>
                <button
                  className="settings-action-btn"
                  onClick={() => { setCreatingInRoot('file'); setRootChildName(''); }}
                  style={{ padding: '4px 8px', display: 'flex', alignItems: 'center', justifyContent: 'center' }}
                  title="New File"
                >
                  <FilePlus size={14} />
                </button>
                <button
                  className="settings-action-btn"
                  onClick={() => { setCreatingInRoot('folder'); setRootChildName(''); }}
                  style={{ padding: '4px 8px', display: 'flex', alignItems: 'center', justifyContent: 'center' }}
                  title="New Folder"
                >
                  <FolderPlus size={14} />
                </button>
                <button
                  className="settings-action-btn"
                  onClick={handleAddFiles}
                  style={{ padding: '4px 8px', display: 'flex', alignItems: 'center', justifyContent: 'center', background: 'var(--raised)', color: 'var(--muted)' }}
                  title="Add Files"
                >
                  <FileText size={14} />
                </button>
                <button
                  className="settings-action-btn"
                  onClick={handleAddFolder}
                  style={{ padding: '4px 8px', display: 'flex', alignItems: 'center', justifyContent: 'center', background: 'var(--raised)', color: 'var(--muted)' }}
                  title="Add Folder"
                >
                  <FolderPlus size={14} />
                </button>
              </div>
            </div>

            {/* Inline create input */}
            {creatingInRoot && (
              <div style={{
                display: 'flex', flexDirection: 'column',
                padding: '6px 12px', borderBottom: '1px solid var(--line)', gap: '4px',
              }}>
                <div style={{ display: 'flex', alignItems: 'center', gap: '6px' }}>
                  {creatingInRoot === 'file'
                    ? <FileText size={12} style={{ color: 'var(--periwinkle)', flexShrink: 0 }} />
                    : <FolderPlus size={12} style={{ color: 'var(--gold)', flexShrink: 0 }} />}
                  <input
                    ref={rootChildInputRef}
                    value={rootChildName}
                    onChange={(e) => { setRootChildName(e.target.value); setCreateNameWarning(''); }}
                    onBlur={() => { setCreatingInRoot(null); setRootChildName(''); setCreateNameWarning(''); }}
                    onKeyDown={(e) => {
                      if (e.key === 'Enter') {
                        e.preventDefault();
                        handleCreateInRoot();
                      }
                      if (e.key === 'Escape') { setCreatingInRoot(null); setRootChildName(''); setCreateNameWarning(''); }
                    }}
                    placeholder={creatingInRoot === 'file' ? 'filename.ext' : 'folder name'}
                    className="inline-name-input"
                  />
                  <button
                    className="inline-cancel-btn"
                    onMouseDown={(e) => { e.preventDefault(); setCreatingInRoot(null); setRootChildName(''); setCreateNameWarning(''); }}
                  >
                    <X size={12} />
                  </button>
                </div>
                {createNameWarning && (
                  <div style={{ display: 'flex', alignItems: 'center', gap: '4px', color: '#f87171', fontSize: '11px' }}>
                    <AlertTriangle size={11} />
                    {createNameWarning}
                  </div>
                )}
              </div>
            )}

            {/* File tree content */}
            <div style={{ flex: 1, overflow: 'auto', padding: '8px 0' }}>
              {loadingFiles ? (
                <div style={{ padding: '20px', textAlign: 'center', color: 'var(--muted)' }}>
                  <Loader2 size={16} className="spinning" />
                </div>
              ) : (fileTree && fileTree.children && fileTree.children.length > 0) ? (
                <FileTree
                  entries={fileTree.children}
                  basePath={fileTree.path || ''}
                  projectId={selectedProject.id}
                  onFileSelect={setSelectedFile}
                  onRefresh={loadProjectFiles}
                  onFileDeleted={handleFileDeletedFromTree}
                  onFileRenamed={handleFileRenamedInTree}
                  onBeforeDelete={storeDeletedForUndo}
                  onBeforeRename={storeRenamedForUndo}
                  onBeforeCreate={storeCreatedForUndo}
                  onRemoveFromWorkspace={handleRemoveFromWorkspace}
                  selectedFile={selectedFile}
                  activeMenuPath={activeMenuPath}
                  onMenuChange={setActiveMenuPath}
                  activeFolderPath={activeFolderPath}
                  onFolderActivate={setActiveFolderPath}
                />
              ) : (
                <div style={{ padding: '20px', textAlign: 'center' }}>
                  <div style={{ color: 'var(--muted)', fontSize: '13px', marginBottom: '8px' }}>
                    {t('projects.noFiles')}
                  </div>
                  <div style={{ color: 'var(--muted-2)', fontSize: '11px' }}>
                    {t('projects.noFilesDesc')}
                  </div>
                </div>
              )}
            </div>
          </div>

          {/* File editor panel */}
          <div style={{
            flex: 1, minWidth: 0, display: 'flex', flexDirection: 'column',
            background: 'var(--surface)', border: '1px solid var(--line)',
            borderRadius: 'var(--radius)', overflow: 'hidden',
          }}>
            {selectedFile ? (
              <FileEditor
                filePath={selectedFile}
                onClose={() => setSelectedFile(null)}
                onSaved={loadProjectFiles}
              />
            ) : (
              <div className="empty-state" style={{ minHeight: '200px' }}>
                <FileText size={72} className="empty-state-icon" />
                <div className="empty-state-title">{t('projects.selectFile')}</div>
                <div className="empty-state-desc">{t('projects.selectFileDesc')}</div>
              </div>
            )}
          </div>
        </div>
      )}
    </div>
  );
}
