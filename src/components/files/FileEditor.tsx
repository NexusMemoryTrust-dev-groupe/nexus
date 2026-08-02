import { useState, useEffect, useRef, useCallback, useMemo } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { X, Save, Loader2, Eye, Edit3, FileText, Link2, FileX } from 'lucide-react';
import { MarkdownRenderer } from '../ui/MarkdownRenderer';
import { TiptapEditor } from './TiptapEditor';
import {
  MARKDOWN_EXTS, getExt, getLang,
  fmtSize, countLines, countWords,
} from './syntax/fileTypes';
import { getLangConfig } from './syntax/langConfig';
import { tokenizeLine, renderTokens } from './syntax/tokenizer';

interface FileEditorProps {
  filePath: string;
  onClose: () => void;
  onSaved: () => void;
}

interface FileInfo {
  name: string;
  path: string;
  content: string;
  sizeBytes: number;
  mimeType: string;
  isEditable: boolean;
}

export function FileEditor({ filePath, onClose, onSaved }: FileEditorProps) {
  const [file, setFile] = useState<FileInfo | null>(null);
  const [isPreview, setIsPreview] = useState(false);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [isDirty, setIsDirty] = useState(false);
  // Uncontrolled textarea — native Ctrl+Z works
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const highlightRef = useRef<HTMLPreElement>(null);
  const lineNumbersRef = useRef<HTMLDivElement>(null);
  const contentRef = useRef(''); // latest content for save
  const [currentLine, setCurrentLine] = useState(1);
  const [dirtyTick, setDirtyTick] = useState(0); // forces highlightedHtml recompute

  // Context menu state
  const [ctxMenu, setCtxMenu] = useState<{ x: number; y: number; selected: string; start: number; end: number } | null>(null);

  const isMarkdown = file ? MARKDOWN_EXTS.includes(getExt(file.name)) : false;
  const language = file ? getLang(file.name) : 'Plain Text';

  // Line count for line numbers. `dirtyTick` is the recompute trigger: the
  // content lives in a ref, so the memo must re-run on the version bump rather
  // than on dependency identity (the ref itself is not reactive).
  const lineCount = useMemo(() => {
    void dirtyTick;
    return countLines(contentRef.current || file?.content || '');
  }, [dirtyTick, file?.content]);

  // Syntax-highlighted HTML for code overlay
  const highlightedHtml = useMemo(() => {
    void dirtyTick; // recompute trigger for ref-backed content (see lineCount)
    const text = contentRef.current || file?.content || '';
    const lines = text.split('\n');
    return lines.map(line => {
      if (!line) return ''; // empty lines stay empty
      const tokens = tokenizeLine(line, language);
      return renderTokens(tokens);
    }).join('\n');
  }, [dirtyTick, file?.content, language]);

  // Track current line from cursor position
  const trackCurrentLine = useCallback(() => {
    const ta = textareaRef.current;
    if (!ta) return;
    const pos = ta.selectionStart;
    const textBefore = ta.value.substring(0, pos);
    const line = textBefore.split('\n').length;
    setCurrentLine(line);
  }, []);

  // Sync textarea scroll to line numbers gutter + highlight pre
  const syncScroll = useCallback(() => {
    const ta = textareaRef.current;
    const gutter = lineNumbersRef.current;
    const highlight = highlightRef.current;
    if (ta) {
      if (gutter) gutter.scrollTop = ta.scrollTop;
      if (highlight) highlight.scrollTop = ta.scrollTop;
    }
  }, []);

  // Close context menu on any click
  useEffect(() => {
    if (!ctxMenu) return;
    const close = () => setCtxMenu(null);
    window.addEventListener('click', close);
    window.addEventListener('keydown', close);
    return () => { window.removeEventListener('click', close); window.removeEventListener('keydown', close); };
  }, [ctxMenu]);

  const loadFile = useCallback(async () => {
    try {
      const info = await invoke<FileInfo>('read_file', { filePath });
      setFile(info);
      contentRef.current = info.content;
      // Set textarea value directly (uncontrolled)
      if (textareaRef.current) {
        textareaRef.current.value = info.content;
      }
      setIsDirty(false);
      setDirtyTick(0);
      setError(null);
      if (!info.isEditable) setIsPreview(true);
    } catch {
      // File was deleted from disk — show "not found" state
      setError('FILE_NOT_FOUND');
      setFile(null);
    }
  }, [filePath]);

  useEffect(() => { loadFile(); }, [loadFile]);

  const handleSave = useCallback(async () => {
    if (!file) return;
    setSaving(true);
    try {
      await invoke('write_file', { filePath: file.path, content: contentRef.current });
      setIsDirty(false);
      onSaved();
    } catch (e) {
      setError(String(e));
    } finally {
      setSaving(false);
    }
  }, [file, onSaved]);

  // Ctrl+S to save. e.code is the physical key position, so this fires on any
  // keyboard layout; e.key would be 'ы' on a Russian layout and never match.
  useEffect(() => {
    const handleKey = (e: KeyboardEvent) => {
      if (e.ctrlKey && e.code === 'KeyS') {
        e.preventDefault();
        handleSave();
      }
      // Ctrl+Z/Y for Russian layout (physical key check via e.code)
      // Russian Z = physical Z key = e.code 'KeyZ', Russian Y = e.code 'KeyY'
      // This ensures undo/redo works regardless of keyboard layout
      if (e.ctrlKey && !e.shiftKey && e.code === 'KeyZ') {
        // Let browser handle native undo for textarea — do NOT preventDefault
      }
      if (e.ctrlKey && (e.code === 'KeyY' || (e.shiftKey && e.code === 'KeyZ'))) {
        // Let browser handle native redo for textarea — do NOT preventDefault
      }
    };
    window.addEventListener('keydown', handleKey);
    return () => window.removeEventListener('keydown', handleKey);
  }, [handleSave]);

  // Context menu actions — use document.execCommand to preserve undo stack
  const insertAtSelection = useCallback((before: string, after: string = '') => {
    const ta = textareaRef.current;
    if (!ta) return;
    const s = ta.selectionStart, e = ta.selectionEnd;
    const sel = contentRef.current.substring(s, e);
    // Select the range, then replace with before + sel + after
    ta.focus();
    ta.setSelectionRange(s, e);
    document.execCommand('insertText', false, before + sel + after);
    contentRef.current = ta.value;
    setIsDirty(true);
    setDirtyTick(t => t + 1);
    // Restore selection
    setTimeout(() => {
      ta.selectionStart = s + before.length;
      ta.selectionEnd = s + before.length + sel.length;
    }, 0);
  }, []);

  const replaceSelection = useCallback((replacement: string) => {
    const ta = textareaRef.current;
    if (!ta) return;
    const s = ta.selectionStart, e = ta.selectionEnd;
    ta.focus();
    ta.setSelectionRange(s, e);
    document.execCommand('insertText', false, replacement);
    contentRef.current = ta.value;
    setIsDirty(true);
    setDirtyTick(t => t + 1);
  }, []);

  const ctxMakeHeading = useCallback(() => {
    if (!ctxMenu) return;
    insertAtSelection('## ');
    setCtxMenu(null);
  }, [ctxMenu, insertAtSelection]);

  const ctxMakeList = useCallback(() => {
    if (!ctxMenu) return;
    const lines = ctxMenu.selected.split('\n');
    const bulleted = lines.map((l: string) => `- ${l}`).join('\n');
    replaceSelection(bulleted);
    setCtxMenu(null);
  }, [ctxMenu, replaceSelection]);

  const ctxMakeLink = useCallback(() => {
    if (!ctxMenu) return;
    insertAtSelection('[', '](url)');
    setCtxMenu(null);
  }, [ctxMenu, insertAtSelection]);

  const ctxMakeCode = useCallback(() => {
    if (!ctxMenu) return;
    replaceSelection('```\n' + ctxMenu.selected + '\n```');
    setCtxMenu(null);
  }, [ctxMenu, replaceSelection]);

  const ctxMakeQuote = useCallback(() => {
    if (!ctxMenu) return;
    const lines = ctxMenu.selected.split('\n');
    replaceSelection(lines.map((l: string) => `> ${l}`).join('\n'));
    setCtxMenu(null);
  }, [ctxMenu, replaceSelection]);

  // Right-click handler
  const handleContextMenu = useCallback((e: React.MouseEvent<HTMLTextAreaElement>) => {
    const ta = textareaRef.current;
    if (!ta) return;
    const start = ta.selectionStart;
    const end = ta.selectionEnd;
    if (start === end) return;
    e.preventDefault();
    const selected = contentRef.current.substring(start, end);
    setCtxMenu({ x: e.clientX, y: e.clientY, selected, start, end });
  }, []);

  // File not found state
  if (error === 'FILE_NOT_FOUND') {
    return (
      <div style={{ padding: '20px', display: 'flex', flexDirection: 'column', alignItems: 'center', justifyContent: 'center', height: '100%', gap: '12px' }}>
        <FileX size={48} style={{ color: 'var(--muted-2)', opacity: 0.4 }} />
        <div style={{ color: 'var(--muted)', fontSize: '14px', fontWeight: 500 }}>Файл не найден</div>
        <div style={{ color: 'var(--muted-2)', fontSize: '12px', textAlign: 'center', maxWidth: '260px' }}>
          Файл был удалён из файловой системы или недоступен
        </div>
        <button className="settings-action-btn" onClick={onClose} style={{ marginTop: '8px' }}>Закрыть</button>
      </div>
    );
  }

  // Other error state
  if (error && !file) {
    return (
      <div style={{ padding: '20px', display: 'flex', flexDirection: 'column', alignItems: 'center', justifyContent: 'center', height: '100%', gap: '12px' }}>
        <div style={{ color: 'var(--rose)', fontSize: '14px' }}>Error loading file</div>
        <div style={{ color: 'var(--muted)', fontSize: '13px' }}>{error}</div>
        <button className="settings-action-btn" onClick={onClose}>Close</button>
      </div>
    );
  }

  if (!file) {
    return (
      <div style={{ padding: '20px', display: 'flex', alignItems: 'center', justifyContent: 'center', height: '100%' }}>
        <Loader2 size={20} className="spinning" style={{ color: 'var(--muted)' }} />
      </div>
    );
  }

  return (
    <div style={{ display: 'flex', flexDirection: 'column', height: '100%', overflow: 'hidden' }}>
      {/* ── Toolbar ── */}
      <div className="file-editor-toolbar">
        <div style={{ display: 'flex', alignItems: 'center', gap: '6px', flex: 1, minWidth: 0 }}>
          <FileText size={14} style={{ color: 'var(--periwinkle)', flexShrink: 0 }} />
          <span style={{ fontFamily: 'var(--brand)', fontSize: '13px', fontWeight: 600, color: 'var(--bone)', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
            {file.name}
          </span>
          {isDirty && <span style={{ width: '6px', height: '6px', borderRadius: '50%', background: 'var(--tangerine)', flexShrink: 0 }} />}
          <span style={{ fontSize: '11px', color: 'var(--muted-2)', flexShrink: 0 }}>{language}</span>
        </div>

        <div style={{ display: 'flex', alignItems: 'center', gap: '2px' }}>
          {file.isEditable && (
            <button className="toolbar-v-btn" onClick={handleSave} disabled={!isDirty || saving} title="Save (Ctrl+S)">
              <Save size={13} />
              <span>{saving ? '...' : 'Save'}</span>
            </button>
          )}
          <button className="toolbar-v-btn" onClick={() => { navigator.clipboard.writeText(file.path); }} title="Copy path for graph linking">
            <Link2 size={13} />
            <span>Link</span>
          </button>
          {isMarkdown && (
            <button className={`toolbar-v-btn ${isPreview ? 'active' : ''}`} onClick={() => setIsPreview(!isPreview)} title={isPreview ? 'Edit' : 'Preview'}>
              {isPreview ? <Edit3 size={13} /> : <Eye size={13} />}
              <span>{isPreview ? 'Edit' : 'Preview'}</span>
            </button>
          )}
          <div style={{ width: '1px', height: '16px', background: 'var(--line)', margin: '0 4px' }} />
          <button className="toolbar-v-btn" onClick={onClose} title="Close">
            <X size={13} />
          </button>
        </div>
      </div>

      {/* ── Error ── */}
      {error && (
        <div style={{ padding: '6px 16px', background: 'rgba(255, 112, 133, 0.08)', color: 'var(--rose)', fontSize: '12px', flexShrink: 0 }}>
          {error}
        </div>
      )}

      {/* ── Content ── */}
      <div className={`file-editor-content ${isPreview ? 'preview-mode' : ''}`} style={{ position: 'relative', flex: 1, overflow: 'hidden' }}>
        {!file.isEditable ? (
          <div style={{ padding: '40px', textAlign: 'center', color: 'var(--muted)' }}>
            <FileText size={48} style={{ opacity: 0.3, marginBottom: '12px' }} />
            <div style={{ fontSize: '14px', marginBottom: '4px' }}>Binary file</div>
            <div style={{ fontSize: '12px', color: 'var(--muted-2)' }}>{fmtSize(file.sizeBytes)}</div>
          </div>
        ) : isPreview && isMarkdown ? (
          <div style={{ overflow: 'auto', height: '100%' }}>
            <MarkdownRenderer content={contentRef.current} />
          </div>
        ) : isMarkdown ? (
          <TiptapEditor
            content={file.content}
            onChange={(md) => {
              contentRef.current = md;
              setIsDirty(true);
            }}
          />
        ) : (
          <div className="file-editor-code-wrapper">
            {/* Line numbers gutter */}
            <div className="file-editor-line-numbers" ref={lineNumbersRef}>
              {Array.from({ length: lineCount }, (_, i) => (
                <div key={i + 1} className={`file-editor-line-num ${i + 1 === currentLine ? 'active' : ''}`}>
                  {i + 1}
                </div>
              ))}
            </div>
            {/* Syntax highlight overlay + transparent textarea */}
            <div className="file-editor-highlight-wrapper" data-lang={language}>
              <pre
                ref={highlightRef}
                className="file-editor-highlight-pre"
                dangerouslySetInnerHTML={{ __html: highlightedHtml }}
              />
              <textarea
                ref={textareaRef}
                defaultValue={file.content}
                onClick={trackCurrentLine}
                onKeyUp={trackCurrentLine}
                onScroll={syncScroll}
                onChange={() => { setIsDirty(true); contentRef.current = textareaRef.current?.value || ''; setDirtyTick(t => t + 1); trackCurrentLine(); }}
                onContextMenu={handleContextMenu}
                spellCheck={false}
                className="file-editor-textarea"
                style={{ fontSize: '14px' }}
                onKeyDown={(e) => {
                  trackCurrentLine();
                  if (e.key === 'Tab') {
                    e.preventDefault();
                    const ta = textareaRef.current;
                    if (ta) {
                      // Use execCommand to preserve undo stack
                      ta.focus();
                      const langCfg = getLangConfig(language);
                      document.execCommand('insertText', false, langCfg.indent);
                      contentRef.current = ta.value;
                      setIsDirty(true);
                      setDirtyTick(t => t + 1);
                    }
                  }
                }}
              />
            </div>
          </div>
        )}
      </div>

      {/* ── Status bar ── */}
      <div className="file-editor-status">
        <span style={{ color: 'var(--periwinkle)' }}>{language}</span>
        <span>{countLines(contentRef.current)} lines</span>
        <span>{countWords(contentRef.current)} words</span>
        <span>{fmtSize(file.sizeBytes)}</span>
        {isDirty && <span style={{ color: 'var(--tangerine)' }}>Modified</span>}
        <span style={{ marginLeft: 'auto' }}>{file.mimeType}</span>
      </div>

      {/* ── Right-click context menu ── */}
      {ctxMenu && (
        <>
          <div style={{ position: 'fixed', inset: 0, zIndex: 200 }} onClick={() => setCtxMenu(null)} />
          <div className="ctx-menu" style={{ position: 'fixed', left: ctxMenu.x, top: ctxMenu.y, zIndex: 201 }}>
            <div className="ctx-menu-header">Transform selection</div>
            <button className="ctx-menu-item" onClick={ctxMakeHeading}>
              <span className="ctx-icon">H</span>
              <span>Make heading</span>
            </button>
            <button className="ctx-menu-item" onClick={ctxMakeList}>
              <span className="ctx-icon">☰</span>
              <span>Make list</span>
            </button>
            <button className="ctx-menu-item" onClick={ctxMakeQuote}>
              <span className="ctx-icon">"</span>
              <span>Make quote</span>
            </button>
            <button className="ctx-menu-item" onClick={ctxMakeCode}>
              <span className="ctx-icon">&lt;/&gt;</span>
              <span>Make code block</span>
            </button>
            <div className="ctx-menu-sep" />
            <button className="ctx-menu-item" onClick={ctxMakeLink}>
              <Link2 size={12} />
              <span>Link to graph</span>
            </button>
          </div>
        </>
      )}
    </div>
  );
}
