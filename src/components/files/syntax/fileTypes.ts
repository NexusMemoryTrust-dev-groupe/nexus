/**
 * File-type detection and size/line/word formatting.
 *
 * Extracted from `FileEditor.tsx`, which had grown to 742 lines by keeping the
 * whole syntax-highlighting engine inline with the React component. These
 * helpers are pure string functions and belong outside the component: they are
 * testable on their own and reusable by the file tree, which needs the same
 * language label.
 */

/** Extensions rendered through the rich Markdown editor rather than the code view. */
export const MARKDOWN_EXTS = ['md', 'markdown', 'mdx', 'mdown'];

/** Extension → display language name. */
export const CODE_EXTS: Record<string, string> = {
  rs: 'Rust', py: 'Python', js: 'JavaScript', ts: 'TypeScript', tsx: 'TSX', jsx: 'JSX',
  go: 'Go', java: 'Java', c: 'C', cpp: 'C++', h: 'C Header', css: 'CSS', html: 'HTML',
  json: 'JSON', yaml: 'YAML', yml: 'YAML', toml: 'TOML', xml: 'XML', svg: 'SVG',
  sh: 'Shell', bash: 'Bash', ps1: 'PowerShell', bat: 'Batch', sql: 'SQL',
  rb: 'Ruby', php: 'PHP', swift: 'Swift', kt: 'Kotlin',
};

/** Lowercased extension without the dot, or `''` when the name has none. */
export function getExt(n: string): string {
  const p = n.split('.');
  return p.length > 1 ? p[p.length - 1].toLowerCase() : '';
}

/** Display language for a filename, falling back to plain text. */
export function getLang(n: string): string {
  return CODE_EXTS[getExt(n)] || 'Plain Text';
}

/** Whether this file should open in the Markdown editor. */
export function isMarkdown(n: string): boolean {
  return MARKDOWN_EXTS.includes(getExt(n));
}

/** Human-readable byte size. */
export function fmtSize(b: number): string {
  if (b < 1024) return `${b}B`;
  if (b < 1048576) return `${(b / 1024).toFixed(1)}KB`;
  return `${(b / 1048576).toFixed(1)}MB`;
}

/** Line count. Empty text is zero lines, not one. */
export function countLines(t: string): number {
  return t ? t.split('\n').length : 0;
}

/** Word count, ignoring runs of whitespace. */
export function countWords(t: string): number {
  return t ? t.trim().split(/\s+/).filter(Boolean).length : 0;
}
