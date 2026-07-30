import { useMemo } from 'react';

/**
 * Obsidian-style markdown renderer.
 * Extracts code blocks and tables before escaping, then renders all elements
 * with proper Obsidian-like styling adapted to our design system.
 */
export function MarkdownRenderer({ content }: { content: string }) {
  const html = useMemo(() => renderMarkdown(content), [content]);
  return <div className="markdown-body" dangerouslySetInnerHTML={{ __html: html }} />;
}

function renderMarkdown(text: string): string {
  // 1. Extract fenced code blocks
  const codeBlocks: string[] = [];
  let result = text.replace(/```(\w*)\n([\s\S]*?)```/g, (_m, lang, code) => {
    const id = `%%CB${codeBlocks.length}%%`;
    const langLabel = lang ? `<div class="md-code-lang">${escapeHtml(lang)}</div>` : '';
    codeBlocks.push(
      `<div class="md-code-block"><div class="md-code-header">${langLabel}<button class="md-code-copy" onclick="navigator.clipboard.writeText(this.closest('.md-code-block').querySelector('code').textContent)">Copy</button></div><pre><code>${escapeHtml(code.trimEnd())}</code></pre></div>`
    );
    return id;
  });

  // 2. Extract tables
  const tables: string[] = [];
  result = result.replace(
    /^(\|.+\|)\n(\|[\s:|-]+\|)\n((?:\|.+\|\n?)*)/gm,
    (_m, headerRow, _sep, bodyRows) => {
      const headers = headerRow.split('|').slice(1, -1).map((c: string) => c.trim());
      const alignments = _sep.split('|').slice(1, -1).map((c: string) => {
        const t = c.trim();
        if (t.startsWith(':') && t.endsWith(':')) return 'center';
        if (t.endsWith(':')) return 'right';
        return 'left';
      });
      const rows = bodyRows.trim().split('\n').map((row: string) =>
        row.split('|').slice(1, -1).map((c: string) => c.trim())
      );
      let html = '<div class="md-table-wrap"><table class="md-table"><thead><tr>';
      headers.forEach((h: string, i: number) => {
        html += `<th style="text-align:${alignments[i] || 'left'}">${h}</th>`;
      });
      html += '</tr></thead><tbody>';
      rows.forEach((row: string[]) => {
        html += '<tr>';
        row.forEach((cell: string, i: number) => {
          html += `<td style="text-align:${alignments[i] || 'left'}">${cell}</td>`;
        });
        html += '</tr>';
      });
      html += '</tbody></table></div>';
      const id = `%%TBL${tables.length}%%`;
      tables.push(html);
      return id;
    }
  );

  // 3. Escape HTML
  result = escapeHtml(result);

  // 4. Restore code blocks and tables
  result = result.replace(/%%CB(\d+)%%/g, (_m, i) => codeBlocks[parseInt(i)]);
  result = result.replace(/%%TBL(\d+)%%/g, (_m, i) => tables[parseInt(i)]);

  // 5. Inline code
  result = result.replace(/`([^`]+)`/g, '<code class="md-inline-code">$1</code>');

  // 6. Images (before links)
  result = result.replace(
    /!\[([^\]]*)\]\(([^)]+)\)/g,
    '<div class="md-image-wrap"><img src="$2" alt="$1" class="md-image" />$1</div>'
  );

  // 7. Links
  result = result.replace(
    /\[([^\]]+)\]\(([^)]+)\)/g,
    '<a href="$2" class="md-link" target="_blank" rel="noopener">$1</a>'
  );

  // 8. Obsidian callouts: > [!note] Title → rendered as styled callout block
  result = result.replace(
    /^&gt; \[!(\w+)\]\s*(.*)$/gm,
    (_m, type, title) => {
      const calloutType = type.toLowerCase();
      return `%%CALLOUT:${calloutType}:${title}%%`;
    }
  );

  // 9. Headings
  result = result.replace(/^###### (.+)$/gm, '<h6 class="md-h6">$1</h6>');
  result = result.replace(/^##### (.+)$/gm, '<h5 class="md-h5">$1</h5>');
  result = result.replace(/^#### (.+)$/gm, '<h4 class="md-h4">$1</h4>');
  result = result.replace(/^### (.+)$/gm, '<h3 class="md-h3">$1</h3>');
  result = result.replace(/^## (.+)$/gm, '<h2 class="md-h2">$1</h2>');
  result = result.replace(/^# (.+)$/gm, '<h1 class="md-h1">$1</h1>');

  // 10. Bold + Italic
  result = result.replace(/\*\*\*(.+?)\*\*\*/g, '<strong><em>$1</em></strong>');
  result = result.replace(/\*\*(.+?)\*\*/g, '<strong>$1</strong>');
  result = result.replace(/\*(.+?)\*/g, '<em>$1</em>');

  // 11. Strikethrough
  result = result.replace(/~~(.+?)~~/g, '<del>$1</del>');

  // 12. Blockquotes (regular)
  result = result.replace(
    /^&gt; (.+)$/gm,
    '<blockquote class="md-quote"><p>$1</p></blockquote>'
  );
  result = result.replace(
    /<\/blockquote>\n<blockquote class="md-quote">/g, '\n'
  );

  // 13. Restore callout blocks
  result = result.replace(
    /%%CALLOUT:(\w+):(.*)%%/g,
    (_m, type, title) => {
      const icons: Record<string, string> = {
        note: '📝', tip: '💡', info: 'ℹ️', success: '✅', warning: '⚠️',
        danger: '🚨', bug: '🐛', example: '📋', quote: '💬', question: '❓',
      };
      const icon = icons[type] || '📝';
      const titleText = title.trim() || type.charAt(0).toUpperCase() + type.slice(1);
      return `<div class="md-callout md-callout-${type}"><div class="md-callout-title">${icon} ${titleText}</div><div class="md-callout-content">`;
    }
  );
  // Close callout blocks (they consume content until next double newline or block element)
  result = result.replace(
    /(<div class="md-callout[^"]*">[\s\S]*?<div class="md-callout-content">)([\s\S]*?)(?=<div class="md-callout|<\/?h[1-6]|<div class="md-code-block|<div class="md-table-wrap|<div class="md-image-wrap|<hr|<ul|<ol|<blockquote|$)/g,
    '$1$2</div></div>'
  );

  // 14. Horizontal rules
  result = result.replace(/^---+$/gm, '<hr class="md-hr" />');
  result = result.replace(/^\*\*\*+$/gm, '<hr class="md-hr" />');

  // 15. Task lists
  result = result.replace(
    /^- \[x\] (.+)$/gm,
    '<li class="md-li md-task done"><input type="checkbox" checked disabled /> <span>$1</span></li>'
  );
  result = result.replace(
    /^- \[ \] (.+)$/gm,
    '<li class="md-li md-task"><input type="checkbox" disabled /> <span>$1</span></li>'
  );

  // 16. Unordered lists
  result = result.replace(/^- (.+)$/gm, '<li class="md-li">$1</li>');
  result = result.replace(
    /(<li class="md-li(?:\s+md-task)?(?:\s+done)?">.*<\/li>\n?)+/g,
    (match) => `<ul class="md-ul">${match}</ul>`
  );

  // 17. Ordered lists
  result = result.replace(/^\d+\. (.+)$/gm, '<li class="md-oli">$1</li>');
  result = result.replace(
    /(<li class="md-oli">.*<\/li>\n?)+/g,
    (match) => `<ol class="md-ol">${match}</ol>`
  );

  // 18. Paragraphs
  result = result.replace(/\n\n+/g, '</p><p class="md-p">');
  result = `<p class="md-p">${result}</p>`;

  // 19. Single newlines to br
  result = result.replace(/\n/g, '<br/>');

  // 20. Clean up paragraphs around block elements
  result = result.replace(/<p class="md-p"><\/p>/g, '');
  const blockEls = ['h[1-6]', 'pre', 'div', 'ul', 'ol', 'blockquote', 'hr', 'table'];
  for (const tag of blockEls) {
    result = result.replace(new RegExp(`<p class="md-p">(<${tag})`, 'g'), '$1');
    result = result.replace(new RegExp(`(<\/${tag}>)<\\/p>`, 'g'), '$1');
  }

  return result;
}

function escapeHtml(text: string): string {
  return text
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;');
}
