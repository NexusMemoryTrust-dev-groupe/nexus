import { useEffect, useCallback, useRef } from 'react';
import { useEditor, EditorContent } from '@tiptap/react';
import StarterKit from '@tiptap/starter-kit';
import Placeholder from '@tiptap/extension-placeholder';
import TaskList from '@tiptap/extension-task-list';
import TaskItem from '@tiptap/extension-task-item';
import { Table } from '@tiptap/extension-table';
import TableRow from '@tiptap/extension-table-row';
import TableCell from '@tiptap/extension-table-cell';
import TableHeader from '@tiptap/extension-table-header';
import Image from '@tiptap/extension-image';
import Link from '@tiptap/extension-link';
import HorizontalRule from '@tiptap/extension-horizontal-rule';
import Typography from '@tiptap/extension-typography';
import Highlight from '@tiptap/extension-highlight';
import CodeBlock from '@tiptap/extension-code-block';
import { Markdown } from 'tiptap-markdown';
import {
  Bold, Italic, Strikethrough, Code, Heading1, Heading2, Heading3,
  List, ListOrdered, Quote, Minus, Image as ImageIcon, Link2,
  CheckSquare, Table as TableIcon, CodeSquare, Undo2, Redo2,
  Highlighter,
} from 'lucide-react';

interface TiptapEditorProps {
  content: string;
  onChange: (markdown: string) => void;
  editable?: boolean;
}

export function TiptapEditor({ content, onChange, editable = true }: TiptapEditorProps) {
  const editorRef = useRef<HTMLDivElement>(null);

  const editor = useEditor({
    extensions: [
      StarterKit.configure({
        heading: { levels: [1, 2, 3, 4, 5, 6] },
      }),
      Placeholder.configure({
        placeholder: 'Start writing...',
      }),
      TaskList,
      TaskItem.configure({ nested: true }),
      Table.configure({ resizable: true }),
      TableRow,
      TableCell,
      TableHeader,
      Image,
      Link.configure({
        openOnClick: false,
        HTMLAttributes: { class: 'nx-link' },
      }),
      HorizontalRule,
      Typography,
      Highlight.configure({ multicolor: true }),
      CodeBlock.configure({
        HTMLAttributes: { class: 'nx-code-block' },
      }),
      Markdown.configure({
        html: true,
        transformPastedText: true,
        transformCopiedText: true,
      }),
    ],
    content: content,
    editable: editable,
    onUpdate: ({ editor: ed }) => {
      // tiptap-markdown stores markdown getter in storage
      const md = ((ed.storage as unknown) as Record<string, { getMarkdown: () => string }>).markdown?.getMarkdown() || ed.getText();
      onChange(md);
    },
    editorProps: {
      attributes: {
        class: 'nx-editor-content',
      },
    },
  });

  // Sync external content changes
  useEffect(() => {
    if (editor && !editor.isFocused) {
      const currentMd = ((editor.storage as unknown) as Record<string, { getMarkdown: () => string }>).markdown?.getMarkdown() || editor.getText();
      if (currentMd !== content) {
        editor.commands.setContent(content);
      }
    }
  }, [content, editor]);

  const ToolbarButton = ({ onClick, active, disabled, children, title }: {
    onClick: () => void; active?: boolean; disabled?: boolean;
    children: React.ReactNode; title: string;
  }) => (
    <button
      className={`nx-toolbar-btn ${active ? 'active' : ''}`}
      onClick={onClick}
      disabled={disabled}
      title={title}
      type="button"
    >
      {children}
    </button>
  );

  const ToolbarSep = () => <div className="nx-toolbar-sep" />;

  const addImage = useCallback(() => {
    const url = window.prompt('Image URL:');
    if (url && editor) {
      editor.chain().focus().setImage({ src: url }).run();
    }
  }, [editor]);

  const addLink = useCallback(() => {
    if (!editor) return;
    const url = window.prompt('Link URL:');
    if (url) {
      editor.chain().focus().setLink({ href: url }).run();
    }
  }, [editor]);

  const addTable = useCallback(() => {
    if (editor) {
      editor.chain().focus().insertTable({ rows: 3, cols: 3, withHeaderRow: true }).run();
    }
  }, [editor]);

  if (!editor) return null;

  return (
    <div className="nx-tiptap-wrapper">
      {/* Toolbar */}
      <div className="nx-toolbar">
        <ToolbarButton onClick={() => editor.chain().focus().toggleBold().run()} active={editor.isActive('bold')} title="Bold">
          <Bold size={14} />
        </ToolbarButton>
        <ToolbarButton onClick={() => editor.chain().focus().toggleItalic().run()} active={editor.isActive('italic')} title="Italic">
          <Italic size={14} />
        </ToolbarButton>
        <ToolbarButton onClick={() => editor.chain().focus().toggleStrike().run()} active={editor.isActive('strike')} title="Strikethrough">
          <Strikethrough size={14} />
        </ToolbarButton>
        <ToolbarButton onClick={() => editor.chain().focus().toggleHighlight().run()} active={editor.isActive('highlight')} title="Highlight">
          <Highlighter size={14} />
        </ToolbarButton>
        <ToolbarButton onClick={() => editor.chain().focus().toggleCode().run()} active={editor.isActive('code')} title="Inline Code">
          <Code size={14} />
        </ToolbarButton>
        <ToolbarButton onClick={() => editor.chain().focus().toggleCodeBlock().run()} active={editor.isActive('codeBlock')} title="Code Block">
          <CodeSquare size={14} />
        </ToolbarButton>

        <ToolbarSep />

        <ToolbarButton onClick={() => editor.chain().focus().toggleHeading({ level: 1 }).run()} active={editor.isActive('heading', { level: 1 })} title="Heading 1">
          <Heading1 size={14} />
        </ToolbarButton>
        <ToolbarButton onClick={() => editor.chain().focus().toggleHeading({ level: 2 }).run()} active={editor.isActive('heading', { level: 2 })} title="Heading 2">
          <Heading2 size={14} />
        </ToolbarButton>
        <ToolbarButton onClick={() => editor.chain().focus().toggleHeading({ level:  3 }).run()} active={editor.isActive('heading', { level: 3 })} title="Heading 3">
          <Heading3 size={14} />
        </ToolbarButton>

        <ToolbarSep />

        <ToolbarButton onClick={() => editor.chain().focus().toggleBulletList().run()} active={editor.isActive('bulletList')} title="Bullet List">
          <List size={14} />
        </ToolbarButton>
        <ToolbarButton onClick={() => editor.chain().focus().toggleOrderedList().run()} active={editor.isActive('orderedList')} title="Ordered List">
          <ListOrdered size={14} />
        </ToolbarButton>
        <ToolbarButton onClick={() => editor.chain().focus().toggleTaskList().run()} active={editor.isActive('taskList')} title="Task List">
          <CheckSquare size={14} />
        </ToolbarButton>

        <ToolbarSep />

        <ToolbarButton onClick={() => editor.chain().focus().toggleBlockquote().run()} active={editor.isActive('blockquote')} title="Quote">
          <Quote size={14} />
        </ToolbarButton>
        <ToolbarButton onClick={() => editor.chain().focus().setHorizontalRule().run()} title="Horizontal Rule">
          <Minus size={14} />
        </ToolbarButton>
        <ToolbarButton onClick={addTable} title="Insert Table">
          <TableIcon size={14} />
        </ToolbarButton>
        <ToolbarButton onClick={addImage} title="Insert Image">
          <ImageIcon size={14} />
        </ToolbarButton>
        <ToolbarButton onClick={addLink} active={editor.isActive('link')} title="Insert Link">
          <Link2 size={14} />
        </ToolbarButton>

        <ToolbarSep />

        <ToolbarButton onClick={() => editor.chain().focus().undo().run()} disabled={!editor.can().undo()} title="Undo">
          <Undo2 size={14} />
        </ToolbarButton>
        <ToolbarButton onClick={() => editor.chain().focus().redo().run()} disabled={!editor.can().redo()} title="Redo">
          <Redo2 size={14} />
        </ToolbarButton>
      </div>

      {/* Editor */}
      <div className="nx-editor-scroll" ref={editorRef}>
        <EditorContent editor={editor} />
      </div>
    </div>
  );
}
