import { File, FileCode, FileText } from 'lucide-react';

/**
 * Pure helpers for the file tree.
 *
 * Extracted from `FileTree.tsx` because none of this needs React: it is
 * extension tables, name validation and hit-testing. Keeping it here means the
 * rules can be read — and corrected — without scrolling past 800 lines of
 * drag-and-drop wiring.
 */

/**
 * Extensions the tree will let a user create.
 *
 * Deliberately a whitelist rather than "anything with a dot": creating a file
 * the editor cannot open is a dead end, and this list is exactly what the
 * syntax highlighter and the markdown renderer between them can handle.
 */
export const SUPPORTED_EXTS = [
  'md', 'markdown', 'mdx', 'mdown',
  'rs', 'py', 'js', 'ts', 'tsx', 'jsx',
  'go', 'java', 'c', 'cpp', 'h', 'css', 'html',
  'json', 'yaml', 'yml', 'toml', 'xml', 'svg',
  'sh', 'bash', 'ps1', 'bat', 'sql',
  'rb', 'php', 'swift', 'kt', 'txt',
] as const;

/**
 * Whether a proposed filename is one we can open afterwards.
 *
 * A name with no extension is rejected: it would land in the tree as a file the
 * editor refuses to load, which reads as a bug rather than as a rule.
 */
export function isValidFileExtension(name: string): boolean {
  const ext = name.split('.').pop()?.toLowerCase() || '';
  if (!ext) return false;
  return (SUPPORTED_EXTS as readonly string[]).includes(ext);
}

/** Icon for a filename, chosen by extension family. */
export function getFileIcon(name: string) {
  const ext = name.split('.').pop()?.toLowerCase() || '';
  if (['md', 'markdown'].includes(ext)) return FileText;
  if (['json', 'yaml', 'yml', 'toml'].includes(ext)) return FileCode;
  if (['rs', 'py', 'js', 'ts', 'tsx', 'jsx', 'go', 'java', 'c', 'cpp', 'h'].includes(ext)) return FileCode;
  if (['html', 'css', 'svg'].includes(ext)) return FileCode;
  return File;
}

/**
 * Pixels of movement before a press becomes a drag.
 *
 * Without a threshold every click registers as a one-pixel drag, so selecting a
 * file by clicking it would silently move it into whatever sat under the cursor.
 */
export const DRAG_THRESHOLD = 5;

/**
 * Nearest draggable tree row for an event target.
 *
 * Bookmarks are excluded: they look like rows but are shortcuts, and dropping a
 * file onto one has no meaning.
 */
export function findClosestItem(target: EventTarget | null): HTMLElement | null {
  if (!target || !(target instanceof HTMLElement)) return null;
  return target.closest('.file-tree-item:not(.file-tree-bookmark)') as HTMLElement | null;
}
