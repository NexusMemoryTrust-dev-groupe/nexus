/**
 * Per-language editor behaviour: indentation width and comment syntax.
 *
 * Kept separate from the tokenizer because these settings drive *editing*
 * (what Tab inserts, what Ctrl+/ toggles) while the tokenizer drives display.
 * Mixing them made `FileEditor.tsx` hard to read and impossible to test.
 */

export interface LangConfig {
  /** Visual tab width. */
  tabSize: number;
  /** What the Tab key inserts. */
  indent: string;
  /** Prefix used for line comments. Empty when the language has none. */
  commentPrefix: string;
}

const LANG_CONFIG: Record<string, LangConfig> = {
  Rust:      { tabSize: 4, indent: '    ', commentPrefix: '//' },
  Python:    { tabSize: 4, indent: '    ', commentPrefix: '#' },
  JavaScript:{ tabSize: 2, indent: '  ', commentPrefix: '//' },
  TypeScript:{ tabSize: 2, indent: '  ', commentPrefix: '//' },
  TSX:       { tabSize: 2, indent: '  ', commentPrefix: '//' },
  JSX:       { tabSize: 2, indent: '  ', commentPrefix: '//' },
  Go:        { tabSize: 4, indent: '    ', commentPrefix: '//' },
  Java:      { tabSize: 4, indent: '    ', commentPrefix: '//' },
  C:         { tabSize: 4, indent: '    ', commentPrefix: '//' },
  'C++':     { tabSize: 4, indent: '    ', commentPrefix: '//' },
  'C Header':{ tabSize: 4, indent: '    ', commentPrefix: '//' },
  CSS:       { tabSize: 2, indent: '  ', commentPrefix: '/*' },
  HTML:      { tabSize: 2, indent: '  ', commentPrefix: '<!--' },
  JSON:      { tabSize: 2, indent: '  ', commentPrefix: '' },
  YAML:      { tabSize: 2, indent: '  ', commentPrefix: '#' },
  TOML:      { tabSize: 4, indent: '    ', commentPrefix: '#' },
  XML:       { tabSize: 2, indent: '  ', commentPrefix: '<!--' },
  SVG:       { tabSize: 2, indent: '  ', commentPrefix: '<!--' },
  Shell:     { tabSize: 2, indent: '  ', commentPrefix: '#' },
  Bash:      { tabSize: 2, indent: '  ', commentPrefix: '#' },
  PowerShell:{ tabSize: 4, indent: '    ', commentPrefix: '#' },
  Batch:     { tabSize: 2, indent: '  ', commentPrefix: 'REM ' },
  SQL:       { tabSize: 2, indent: '  ', commentPrefix: '--' },
  Ruby:      { tabSize: 2, indent: '  ', commentPrefix: '#' },
  PHP:       { tabSize: 4, indent: '    ', commentPrefix: '//' },
  Swift:     { tabSize: 4, indent: '    ', commentPrefix: '//' },
  Kotlin:    { tabSize: 4, indent: '    ', commentPrefix: '//' },
  'Plain Text': { tabSize: 4, indent: '    ', commentPrefix: '' },
};

/** Config for a language, falling back to plain text for anything unknown. */
export function getLangConfig(lang: string): LangConfig {
  return LANG_CONFIG[lang] || LANG_CONFIG['Plain Text'];
}
