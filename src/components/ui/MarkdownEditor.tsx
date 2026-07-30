import { useState, useRef, useCallback } from 'react';
import { Bold, Italic, Code, List, Quote, Heading1, Heading2, Link2, Eye, Edit3 } from 'lucide-react';
import { MarkdownRenderer } from './MarkdownRenderer';

interface MarkdownEditorProps {
  value: string;
  onChange: (value: string) => void;
  placeholder?: string;
  minHeight?: number;
  previewMode?: boolean;
}

/**
 * Built-in markdown editor with live preview, toolbar, and keyboard shortcuts.
 */
export function MarkdownEditor({
  value,
  onChange,
  placeholder = 'Write in markdown...',
  minHeight = 200,
  previewMode = false,
}: MarkdownEditorProps) {
  const [isPreview, setIsPreview] = useState(previewMode);
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  const insertAtCursor = useCallback(
    (before: string, after: string = '') => {
      const ta = textareaRef.current;
      if (!ta) return;
      const start = ta.selectionStart;
      const end = ta.selectionEnd;
      const selected = value.substring(start, end);
      const newText = value.substring(0, start) + before + selected + after + value.substring(end);
      onChange(newText);
      // Restore cursor position
      setTimeout(() => {
        ta.focus();
        ta.selectionStart = start + before.length;
        ta.selectionEnd = start + before.length + selected.length;
      }, 0);
    },
    [value, onChange]
  );

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      // Ctrl+B — bold
      if (e.ctrlKey && e.key === 'b') {
        e.preventDefault();
        insertAtCursor('**', '**');
      }
      // Ctrl+I — italic
      if (e.ctrlKey && e.key === 'i') {
        e.preventDefault();
        insertAtCursor('*', '*');
      }
      // Ctrl+` — inline code
      if (e.ctrlKey && e.key === '`') {
        e.preventDefault();
        insertAtCursor('`', '`');
      }
    },
    [insertAtCursor]
  );

  const toolbar = [
    { icon: Heading1, action: () => insertAtCursor('# '), title: 'Heading 1' },
    { icon: Heading2, action: () => insertAtCursor('## '), title: 'Heading 2' },
    { icon: Bold, action: () => insertAtCursor('**', '**'), title: 'Bold (Ctrl+B)' },
    { icon: Italic, action: () => insertAtCursor('*', '*'), title: 'Italic (Ctrl+I)' },
    { icon: Code, action: () => insertAtCursor('`', '`'), title: 'Code' },
    { icon: List, action: () => insertAtCursor('- '), title: 'List' },
    { icon: Quote, action: () => insertAtCursor('> '), title: 'Quote' },
    { icon: Link2, action: () => insertAtCursor('[', '](url)'), title: 'Link' },
  ];

  return (
    <div style={{
      border: '1px solid var(--line)',
      borderRadius: 'var(--radius-sm)',
      overflow: 'hidden',
      background: 'var(--surface)',
    }}>
      {/* Toolbar */}
      <div style={{
        display: 'flex',
        alignItems: 'center',
        gap: '2px',
        padding: '6px 8px',
        borderBottom: '1px solid var(--line)',
        background: 'var(--carbon-soft)',
      }}>
        {toolbar.map(({ icon: Icon, action, title }) => (
          <button
            key={title}
            onClick={action}
            title={title}
            style={{
              display: 'flex',
              alignItems: 'center',
              justifyContent: 'center',
              width: '28px',
              height: '28px',
              borderRadius: '6px',
              border: 'none',
              background: 'transparent',
              color: 'var(--muted)',
              cursor: 'pointer',
              transition: 'all 0.15s',
            }}
            onMouseEnter={(e) => {
              e.currentTarget.style.background = 'var(--raised)';
              e.currentTarget.style.color = 'var(--bone)';
            }}
            onMouseLeave={(e) => {
              e.currentTarget.style.background = 'transparent';
              e.currentTarget.style.color = 'var(--muted)';
            }}
          >
            <Icon size={14} />
          </button>
        ))}

        <div style={{ flex: 1 }} />

        {/* Preview toggle */}
        <button
          onClick={() => setIsPreview(!isPreview)}
          style={{
            display: 'flex',
            alignItems: 'center',
            gap: '4px',
            padding: '4px 10px',
            borderRadius: '6px',
            border: '1px solid var(--line)',
            background: isPreview ? 'var(--tangerine-soft)' : 'transparent',
            color: isPreview ? 'var(--tangerine)' : 'var(--muted)',
            fontSize: '11px',
            fontWeight: 500,
            cursor: 'pointer',
            transition: 'all 0.15s',
          }}
        >
          {isPreview ? <Edit3 size={12} /> : <Eye size={12} />}
          {isPreview ? 'Edit' : 'Preview'}
        </button>
      </div>

      {/* Editor / Preview */}
      {isPreview ? (
        <div style={{ padding: '12px 16px', minHeight, overflow: 'auto' }}>
          {value ? (
            <MarkdownRenderer content={value} />
          ) : (
            <div style={{ color: 'var(--muted-2)', fontStyle: 'italic', fontSize: '13px' }}>
              Nothing to preview
            </div>
          )}
        </div>
      ) : (
        <textarea
          ref={textareaRef}
          value={value}
          onChange={(e) => onChange(e.target.value)}
          onKeyDown={handleKeyDown}
          placeholder={placeholder}
          style={{
            width: '100%',
            minHeight,
            padding: '12px 16px',
            background: 'transparent',
            border: 'none',
            color: 'var(--bone)',
            fontSize: '14px',
            fontFamily: 'var(--mono)',
            lineHeight: 1.7,
            resize: 'vertical',
            outline: 'none',
          }}
        />
      )}
    </div>
  );
}
