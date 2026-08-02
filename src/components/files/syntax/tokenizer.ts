/**
 * Syntax highlighting tokenizer.
 *
 * Extracted from `FileEditor.tsx`, which had grown to 742 lines with roughly
 * 280 of them being per-language regex tables. Keeping them next to the React
 * component meant every edit to the editor's UI risked touching the
 * highlighting, and vice versa; worse, the whole table was re-parsed on every
 * hot reload of the component.
 *
 * The tokenizers are pure `(line, lang) -> Token[]` functions, so they are
 * testable without mounting anything.
 *
 * Token classes: kw=keyword, str=string, num=number, cmt=comment, fn=function,
 * type=type, op=operator, tag=html/xml tag, attr=html/xml attribute,
 * imp=import/include, dec=decorator/annotation, br=bracket/paren, arg=argument.
 */
export interface Token { text: string; cls: string; }

export function escHtml(s: string): string {
  return s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;');
}

function wordsPattern(words: string): RegExp {
  return new RegExp('\\b(?:' + words + ')\\b');
}

// Combined regex per-language using new RegExp to avoid TS literal parsing issues
function buildCombined(...sources: RegExp[]): RegExp {
  return new RegExp(sources.map(s => '(' + s.source + ')').join('|'), 'gm');
}

// Generic line tokenizer
function tokenizeGeneric(line: string, patterns: [RegExp, string][]): Token[] {
  const tokens: Token[] = [];
  let pos = 0;
  const combined = buildCombined(...patterns.map(([re]) => re));
  let m: RegExpExecArray | null;
  while ((m = combined.exec(line)) !== null) {
    if (m.index > pos) {
      tokens.push({ text: line.slice(pos, m.index), cls: '' });
    }
    for (let i = 0; i < patterns.length; i++) {
      if (m[i + 1] !== undefined) {
        tokens.push({ text: m[0], cls: patterns[i][1] });
        break;
      }
    }
    pos = m.index + m[0].length;
    if (pos >= line.length) break;
  }
  if (pos < line.length) tokens.push({ text: line.slice(pos), cls: '' });
  return tokens.length ? tokens : [{ text: line, cls: '' }];
}

// ── Shared regex atoms ──
const R_COMMENT_C   = /\/\/.*|\/\*[\s\S]*?\*\//;
const R_COMMENT_H   = /#.*/;
const R_COMMENT_SQL = /--.*/;
const R_STRING_DQ   = /"(?:\\.|[^"\\])*"/;
const R_STRING_SQ   = /'(?:\\.|[^'\\])*'/;
const R_STRING_BT   = /`(?:\\.|[^`\\])*`/;
const R_NUMBER      = /\b\d+\.?\d*\b/;
const R_IDENT_UC    = /\b[A-Z]\w*\b/;
const R_BRACKET     = /[{}()\[\]]/;
const R_COMMA_SEMI  = /[,;]|\.\.\.?|=>|::|->/;
const R_OP          = /[+\-*/%&|^~!<>=:]+/;

// ═══════════════════════════════════════════
//  RUST — warm amber tones
// ═══════════════════════════════════════════
const KW_RUST = wordsPattern('fn|let|mut|pub|pub\\(crate\\)|struct|enum|impl|trait|use|mod|crate|self|super|where|async|await|move|ref|match|if|else|for|while|loop|return|break|continue|type|const|static|unsafe|extern|dyn|as|in|box|yield|try|catch|throw|macro_rules');
const TY_RUST = wordsPattern('String|str|bool|u8|u16|u32|u64|u128|i8|i16|i32|i64|i128|f32|f64|usize|isize|Option|Result|Vec|HashMap|HashSet|Box|Rc|Arc|Cell|RefCell|Cow|Pin|Future|Stream|Mutex|RwLock');
const KV_RUST = wordsPattern('true|false|None|Some|Ok|Err|Self');
const IMP_RUST = /^(?:use|mod|crate)\s/;
function tokenizeRust(line: string): Token[] {
  return tokenizeGeneric(line, [
    [R_COMMENT_C, 'cmt'], [R_STRING_DQ, 'str'], [R_STRING_SQ, 'str'],
    [IMP_RUST, 'imp'], [KW_RUST, 'kw'], [TY_RUST, 'type'], [KV_RUST, 'kw'],
    [R_IDENT_UC, 'type'], [R_NUMBER, 'num'], [R_BRACKET, 'br'], [R_COMMA_SEMI, 'op'], [R_OP, 'op'],
  ]);
}

// ═══════════════════════════════════════════
//  PYTHON — blue + green tones
// ═══════════════════════════════════════════
const KW_PY = wordsPattern('def|class|import|from|as|return|if|elif|else|for|while|try|except|finally|with|yield|lambda|pass|break|continue|raise|del|global|nonlocal|assert|and|or|not|in|is|async|await');
const TY_PY = wordsPattern('None|True|False|int|float|str|bool|list|dict|tuple|set|bytes|type|object|Any|Optional|Union|List|Dict|Tuple|Set|Callable|Iterator|Generator|Coroutine|Type|Self');
const FN_PY = wordsPattern('print|len|range|enumerate|zip|map|filter|isinstance|getattr|setattr|hasattr|super|property|staticmethod|classmethod|open|input|int|float|str|bool|list|dict|tuple|set|type|repr|id|hash|abs|min|max|sum|sorted|reversed|any|all|next|iter|format|Exception|ValueError|TypeError|KeyError|IndexError|RuntimeError|StopIteration|OSError|IOError');
const IMP_PY = /^(?:import|from)\s/;
const DEC_PY = /@\w[\w.]*/;
function tokenizePython(line: string): Token[] {
  return tokenizeGeneric(line, [
    [R_COMMENT_H, 'cmt'], [/"""[\s\S]*?"""|'''[\s\S]*?'''/, 'str'],
    [R_STRING_DQ, 'str'], [R_STRING_SQ, 'str'],
    [IMP_PY, 'imp'], [DEC_PY, 'dec'],
    [KW_PY, 'kw'], [TY_PY, 'type'], [FN_PY, 'fn'],
    [R_NUMBER, 'num'], [R_BRACKET, 'br'], [R_COMMA_SEMI, 'op'], [R_OP, 'op'],
  ]);
}

// ═══════════════════════════════════════════
//  JS/TS — yellow + electric blue
// ═══════════════════════════════════════════
const KW_JS = wordsPattern('const|let|var|function|return|if|else|for|while|do|switch|case|break|continue|new|delete|typeof|instanceof|in|of|class|extends|super|import|from|export|default|as|async|await|yield|try|catch|finally|throw|this|void|static|get|set|satisfies|keyof|type|interface|enum|implements|abstract|declare|module|namespace|readonly|override|infer|require|module');
const TY_JS = wordsPattern('string|number|boolean|any|unknown|never|object|symbol|bigint|undefined|null|void|Promise|Array|Record|Partial|Required|Pick|Omit|Exclude|Extract|ReturnType|Readonly|Map|Set|WeakMap|WeakSet|Error|RegExp|Date|Math|JSON|console|Symbol|Proxy|Reflect');
const KV_JS = wordsPattern('null|undefined|true|false|NaN|Infinity');
const IMP_JS = /^(?:import|from|require)\s/;
const DEC_JS = /@\w[\w.]*/;
function tokenizeJS(line: string): Token[] {
  return tokenizeGeneric(line, [
    [R_COMMENT_C, 'cmt'], [R_STRING_BT, 'str'], [R_STRING_DQ, 'str'], [R_STRING_SQ, 'str'],
    [IMP_JS, 'imp'], [DEC_JS, 'dec'],
    [KW_JS, 'kw'], [TY_JS, 'type'], [KV_JS, 'kw'], [R_IDENT_UC, 'type'],
    [R_NUMBER, 'num'], [R_BRACKET, 'br'], [R_COMMA_SEMI, 'op'], [R_OP, 'op'],
  ]);
}

// ═══════════════════════════════════════════
//  GO — cyan + teal
// ═══════════════════════════════════════════
const KW_GO = wordsPattern('func|return|if|else|for|range|switch|case|default|break|continue|go|defer|select|chan|map|struct|interface|type|var|const|package|import|fallthrough');
const TY_GO = wordsPattern('string|int|int8|int16|int32|int64|uint|uint8|uint16|uint32|uint64|float32|float64|bool|byte|rune|error|any|comparable');
const IMP_GO = /^(?:import)\s/;
function tokenizeGo(line: string): Token[] {
  return tokenizeGeneric(line, [
    [R_COMMENT_C, 'cmt'], [R_STRING_DQ, 'str'], [/`[^`]*`/, 'str'],
    [IMP_GO, 'imp'], [KW_GO, 'kw'], [TY_GO, 'type'],
    [R_NUMBER, 'num'], [R_BRACKET, 'br'], [R_COMMA_SEMI, 'op'], [R_OP, 'op'],
  ]);
}

// ═══════════════════════════════════════════
//  HTML/SVG — green tags + pink attributes
// ═══════════════════════════════════════════
const TAG_HTML = wordsPattern('div|span|p|a|img|ul|ol|li|h[1-6]|table|tr|td|th|form|input|button|select|option|textarea|label|section|article|nav|header|footer|main|aside|details|summary|figure|figcaption|video|audio|source|canvas|svg|path|style|script|link|meta|title|head|body|html|doctype|br|hr|img');
const ATTR_HTML = wordsPattern('class|id|href|src|alt|title|style|type|name|value|placeholder|checked|disabled|required|readonly|hidden|data-\\w+|role|aria-\\w+|xmlns|viewBox|fill|stroke|stroke-width|d|transform|width|height|cx|cy|r|rx|ry|x|y|x1|y1|x2|y2|points|offset|stop-color|opacity|font-family|font-size|text-anchor|dominant-baseline');
const STR_HTML = /"[^"]*"|'[^']*'/;
function tokenizeHTML(line: string): Token[] {
  return tokenizeGeneric(line, [
    [/<!--[\s\S]*?-->/, 'cmt'], [TAG_HTML, 'tag'], [ATTR_HTML, 'attr'],
    [STR_HTML, 'str'], [/<\/?/, 'tag'], [/\/?>/, 'tag'], [/<(?![\/\s])/, 'tag'],
    [R_NUMBER, 'num'], [/@\w[\w-]*/, 'dec'],
    [R_BRACKET, 'br'], [R_COMMA_SEMI, 'op'], [R_OP, 'op'],
  ]);
}

// ═══════════════════════════════════════════
//  CSS — purple + pink
// ═══════════════════════════════════════════
const KW_CSS = wordsPattern('important|inherit|initial|unset|none|auto|normal|bold|italic|transparent|currentColor');
const SEL_CSS = /[#.:][\w-]+|[\w-]+(?=\s*\{)/;
const PROP_CSS = wordsPattern('display|position|top|right|bottom|left|width|height|min-width|max-width|min-height|max-height|margin|padding|border|background|color|font|font-size|font-weight|font-family|line-height|text-align|text-decoration|overflow|z-index|opacity|transform|transition|animation|flex|grid|gap|align-items|justify-content|content|cursor|box-shadow|outline|resize|overflow-wrap|white-space');
function tokenizeCSS(line: string): Token[] {
  return tokenizeGeneric(line, [
    [/\/\*[\s\S]*?\*\//, 'cmt'], [/#[0-9a-fA-F]{3,8}/, 'num'],
    [R_STRING_DQ, 'str'], [R_STRING_SQ, 'str'],
    [KW_CSS, 'kw'], [PROP_CSS, 'type'], [SEL_CSS, 'tag'],
    [/#[\w-]+/, 'attr'], [/\.[\w-]+/, 'attr'],
    [R_NUMBER, 'num'], [R_BRACKET, 'br'], [R_COMMA_SEMI, 'op'], [R_OP, 'op'],
  ]);
}

// ═══════════════════════════════════════════
//  SQL — purple + teal
// ═══════════════════════════════════════════
const KW_SQL = wordsPattern('SELECT|FROM|WHERE|INSERT|INTO|VALUES|UPDATE|SET|DELETE|CREATE|TABLE|DROP|ALTER|INDEX|JOIN|LEFT|RIGHT|INNER|OUTER|CROSS|ON|AND|OR|NOT|IN|LIKE|BETWEEN|EXISTS|HAVING|GROUP|BY|ORDER|ASC|DESC|LIMIT|OFFSET|UNION|ALL|DISTINCT|AS|CASE|WHEN|THEN|ELSE|END|PRIMARY|KEY|FOREIGN|REFERENCES|CONSTRAINT|DEFAULT|NULL|IS|TRUE|FALSE|CHECK|UNIQUE|WITH|RECURSIVE|OVER|PARTITION|ROW|RANK|DENSE|FETCH|NEXT|ROWS|ONLY|FIRST|LAST|PERCENT|TIES|REPLACE|EXPLAIN|ANALYZE|VACUUM|GRANT|REVOKE|COMMIT|ROLLBACK|SAVEPOINT|BEGIN|TRANSACTION|TRIGGER|FUNCTION|PROCEDURE|RETURNS|RETURN|DECLARE|CURSOR|OPEN|CLOSE|FETCH|DEALLOCATE|PREPARE|EXECUTE');
const TY_SQL = wordsPattern('INT|INTEGER|BIGINT|SMALLINT|TINYINT|DECIMAL|NUMERIC|FLOAT|REAL|DOUBLE|CHAR|VARCHAR|TEXT|CLOB|BLOB|DATE|DATETIME|TIMESTAMP|TIME|BOOLEAN|BOOL|BIT|UUID|JSON|JSONB|ARRAY|XML|MONEY|SERIAL|BIGSERIAL');
function tokenizeSQL(line: string): Token[] {
  return tokenizeGeneric(line, [
    [R_COMMENT_SQL, 'cmt'], [R_COMMENT_C, 'cmt'],
    [R_STRING_SQ, 'str'], [R_STRING_DQ, 'str'],
    [KW_SQL, 'kw'], [TY_SQL, 'type'],
    [R_NUMBER, 'num'], [R_BRACKET, 'br'], [R_COMMA_SEMI, 'op'], [R_OP, 'op'],
  ]);
}

// ═══════════════════════════════════════════
//  SHELL / BASH / POWERSHELL
// ═══════════════════════════════════════════
const KW_SH = wordsPattern('if|then|else|elif|fi|for|while|do|done|case|esac|function|return|exit|export|source|alias|unalias|local|declare|typeset|readonly|shift|eval|exec|set|unset|trap|wait|kill|cd|pwd|echo|printf|read|test|true|false|select|in|time|coproc');
function tokenizeShell(line: string): Token[] {
  return tokenizeGeneric(line, [
    [R_COMMENT_H, 'cmt'], [R_STRING_DQ, 'str'], [R_STRING_SQ, 'str'],
    [KW_SH, 'kw'],
    [/\$\w+|\$\{[^}]+\}|\$\([^)]+\)/, 'arg'],
    [R_NUMBER, 'num'], [R_BRACKET, 'br'],
    [/[|&;<>]+/, 'op'], [R_OP, 'op'],
  ]);
}

// ═══════════════════════════════════════════
//  RUBY — red + green
// ═══════════════════════════════════════════
const KW_RB = wordsPattern('def|class|module|require|include|extend|return|if|elsif|else|unless|for|while|until|loop|do|end|yield|raise|begin|rescue|ensure|retry|throw|catch|case|when|then|next|break|redo|nil|self|super|attr_reader|attr_writer|attr_accessor|lambda|proc|block_given?');
const TY_RB = wordsPattern('nil|true|False|Integer|Float|String|Array|Hash|Symbol|Regexp|Range|Exception|NilClass|TrueClass|FalseClass|Numeric|Comparable|Enumerable');
const IMP_RB = /^(?:require|require_relative|load)\s/;
function tokenizeRuby(line: string): Token[] {
  return tokenizeGeneric(line, [
    [R_COMMENT_H, 'cmt'], [R_STRING_DQ, 'str'], [R_STRING_SQ, 'str'], [/:[\w]+/, 'str'],
    [IMP_RB, 'imp'],
    [KW_RB, 'kw'], [TY_RB, 'type'],
    [R_NUMBER, 'num'], [R_BRACKET, 'br'], [R_COMMA_SEMI, 'op'], [R_OP, 'op'],
  ]);
}

// ═══════════════════════════════════════════
//  PHP — blue + green
// ═══════════════════════════════════════════
const KW_PHP = wordsPattern('function|return|if|else|elseif|for|foreach|while|do|switch|case|break|continue|class|interface|trait|extends|implements|new|clone|instanceof|static|final|abstract|private|protected|public|var|let|const|echo|print|die|exit|include|require|include_once|require_once|yield|match|fn|as');
const TY_PHP = wordsPattern('int|float|string|bool|array|object|null|void|never|mixed|self|parent|static|iterable|callable|false|true');
const IMP_PHP = /^(?:use|namespace)\s/;
function tokenizePHP(line: string): Token[] {
  return tokenizeGeneric(line, [
    [/\/\*[\s\S]*?\*\//, 'cmt'], [R_COMMENT_H, 'cmt'], [R_STRING_DQ, 'str'], [R_STRING_SQ, 'str'],
    [/\$[\w]+/, 'arg'], [IMP_PHP, 'imp'],
    [KW_PHP, 'kw'], [TY_PHP, 'type'],
    [R_NUMBER, 'num'], [R_BRACKET, 'br'], [R_COMMA_SEMI, 'op'], [R_OP, 'op'],
  ]);
}

// ═══════════════════════════════════════════
//  SWIFT / KOTLIN — orange + blue
// ═══════════════════════════════════════════
const KW_SWIFT = wordsPattern('func|let|var|return|if|else|for|while|repeat|switch|case|default|break|continue|class|struct|enum|protocol|extension|import|public|private|internal|fileprivate|open|static|final|lazy|weak|unowned|guard|defer|as|is|try|catch|throw|throws|rethrows|async|await|actor|some|any|where|typealias|associatedtype|subscript|init|deinit|self|super|nil|true|false|print');
const TY_SWIFT = wordsPattern('Int|Double|Float|String|Bool|Array|Dictionary|Set|Optional|Result|Character|Any|AnyObject|Void|Error|Range|ClosedRange');
const IMP_SWIFT = /^(?:import)\s/;
function tokenizeSwift(line: string): Token[] {
  return tokenizeGeneric(line, [
    [R_COMMENT_C, 'cmt'], [R_STRING_DQ, 'str'], [R_STRING_SQ, 'str'],
    [IMP_SWIFT, 'imp'],
    [KW_SWIFT, 'kw'], [TY_SWIFT, 'type'],
    [R_NUMBER, 'num'], [R_BRACKET, 'br'], [R_COMMA_SEMI, 'op'], [R_OP, 'op'],
  ]);
}

const KW_KT = wordsPattern('fun|val|var|return|if|else|for|while|do|when|is|in|as|try|catch|finally|throw|class|interface|object|data|sealed|enum|abstract|open|private|protected|public|internal|override|companion|suspend|inline|noinline|crossinline|reified|import|package|typealias|constructor|init|by|lazy|get|set|companion|where|break|continue|this|super|null|true|false');
const TY_KT = wordsPattern('Int|Long|Short|Byte|Float|Double|String|Boolean|Char|Any|Nothing|Unit|Array|List|MutableList|Map|MutableMap|Set|MutableSet|Pair|Triple|Sequence|Iterable|Collection');
const IMP_KT = /^(?:import|package)\s/;
function tokenizeKotlin(line: string): Token[] {
  return tokenizeGeneric(line, [
    [R_COMMENT_C, 'cmt'], [/"[\s\S]*?"/, 'str'], [/'''[\s\S]*?'''/, 'str'],
    [IMP_KT, 'imp'],
    [KW_KT, 'kw'], [TY_KT, 'type'],
    [R_NUMBER, 'num'], [R_BRACKET, 'br'], [R_COMMA_SEMI, 'op'], [R_OP, 'op'],
  ]);
}

// ═══════════════════════════════════════════
//  C / C++ / C HEADER
// ═══════════════════════════════════════════
const KW_C = wordsPattern('if|else|for|while|do|switch|case|default|break|continue|return|goto|typedef|struct|enum|union|const|static|extern|volatile|register|auto|inline|restrict|sizeof|typeof|nullptr|NULL|TRUE|FALSE|void|int|char|short|long|float|double|unsigned|signed|bool|size_t|int8_t|int16_t|int32_t|int64_t|uint8_t|uint16_t|uint32_t|uint64_t');
const TY_C = wordsPattern('int|char|float|double|void|long|short|unsigned|signed|bool|size_t|FILE|NULL|uint8_t|uint16_t|uint32_t|int8_t|int16_t|int32_t|int64_t|uint64_t');
const IMP_C = /^(?:#include|#define|#ifdef|#ifndef|#endif|#undef|#if|#pragma|#error)/;
function tokenizeC(line: string): Token[] {
  return tokenizeGeneric(line, [
    [R_COMMENT_C, 'cmt'], [/\/\*[\s\S]*?\*\//, 'cmt'], [R_STRING_DQ, 'str'], [R_STRING_SQ, 'str'],
    [IMP_C, 'imp'], [KW_C, 'kw'], [TY_C, 'type'],
    [R_NUMBER, 'num'], [R_BRACKET, 'br'], [R_COMMA_SEMI, 'op'], [R_OP, 'op'],
  ]);
}

// ═══════════════════════════════════════════
//  MAIN DISPATCHER
// ═══════════════════════════════════════════
export function tokenizeLine(line: string, lang: string): Token[] {
  switch (lang) {
    case 'Rust':       return tokenizeRust(line);
    case 'Python':     return tokenizePython(line);
    case 'JavaScript': case 'TypeScript': case 'TSX': case 'JSX': return tokenizeJS(line);
    case 'Go':         return tokenizeGo(line);
    case 'HTML': case 'SVG': return tokenizeHTML(line);
    case 'CSS':        return tokenizeCSS(line);
    case 'SQL':        return tokenizeSQL(line);
    case 'Shell': case 'Bash': case 'PowerShell': return tokenizeShell(line);
    case 'Ruby':       return tokenizeRuby(line);
    case 'PHP':        return tokenizePHP(line);
    case 'Swift':      return tokenizeSwift(line);
    case 'Kotlin':     return tokenizeKotlin(line);
    case 'C':          case 'C++': case 'C Header': return tokenizeC(line);
    default:           return [{ text: line, cls: '' }];
  }
}

export function renderTokens(tokens: Token[]): string {
  return tokens.map(t => t.cls ? `<span class="syn-${t.cls}">${escHtml(t.text)}</span>` : escHtml(t.text)).join('');
}
