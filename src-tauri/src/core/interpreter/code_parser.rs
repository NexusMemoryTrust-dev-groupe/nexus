use crate::core::graph::entity::Entity;
use crate::core::graph::entity_types::EntityType;

/// Result of parsing a code file
pub struct ParsedCode {
    pub entities: Vec<Entity>,
    pub summary: String,
}

// ═══════════════════════════════════════════════════════════════
//  Python Parser
// ═══════════════════════════════════════════════════════════════

pub fn parse_python(content: &str) -> ParsedCode {
    let mut entities = Vec::new();
    let mut class_count = 0;
    let mut func_count = 0;
    let mut import_count = 0;

    for line in content.lines() {
        let trimmed = line.trim();

        // Classes: class ClassName(Base):
        if trimmed.starts_with("class ") {
            if let Some(name) = extract_word_after(trimmed, "class ", "(").or_else(|| extract_word_after(trimmed, "class ", ":")) {
                let desc = extract_class_description(trimmed);
                let mut e = Entity::new(EntityType::Document, name, desc);
                e.metadata.insert("kind".into(), serde_json::json!("class"));
                e.metadata.insert("language".into(), serde_json::json!("python"));
                entities.push(e);
                class_count += 1;
            }
        }

        // Functions: def function_name(
        if trimmed.starts_with("def ") || trimmed.starts_with("async def ") {
            let prefix = if trimmed.starts_with("async def ") { "async def " } else { "def " };
            if let Some(name) = extract_word_after(trimmed, prefix, "(") {
                let params = extract_params(trimmed);
                let desc = format!("Python function: {}({})", name, params);
                let mut e = Entity::new(EntityType::Document, name, desc);
                e.metadata.insert("kind".into(), serde_json::json!("function"));
                e.metadata.insert("language".into(), serde_json::json!("python"));
                entities.push(e);
                func_count += 1;
            }
        }

        // Imports: import x / from x import y
        if trimmed.starts_with("import ") || trimmed.starts_with("from ") {
            import_count += 1;
        }
    }

    let summary = format!("Python: {} classes, {} functions, {} imports", class_count, func_count, import_count);
    ParsedCode { entities, summary }
}

// ═══════════════════════════════════════════════════════════════
//  JavaScript / TypeScript Parser
// ═══════════════════════════════════════════════════════════════

pub fn parse_javascript(content: &str) -> ParsedCode {
    parse_js_ts(content, "JavaScript")
}

pub fn parse_typescript(content: &str) -> ParsedCode {
    parse_js_ts(content, "TypeScript")
}

fn parse_js_ts(content: &str, lang: &str) -> ParsedCode {
    let mut entities = Vec::new();
    let mut class_count = 0;
    let mut func_count = 0;
    let mut export_count = 0;
    let mut import_count = 0;

    for line in content.lines() {
        let trimmed = line.trim();

        // Classes: class ClassName {
        if trimmed.starts_with("class ") || trimmed.starts_with("export class ") {
            let clean = trimmed.trim_start_matches("export ").trim_start_matches("default ");
            if let Some(name) = extract_word_after(clean, "class ", "{").or_else(|| extract_word_after(clean, "class ", "extends").or_else(|| extract_word_after(clean, "class ", "implements"))) {
                let mut e = Entity::new(EntityType::Document, name, format!("{} class", lang));
                e.metadata.insert("kind".into(), serde_json::json!("class"));
                e.metadata.insert("language".into(), serde_json::json!(lang));
                entities.push(e);
                class_count += 1;
            }
        }

        // Functions: function name( / const name = ( / export function name(
        if trimmed.starts_with("function ") || trimmed.starts_with("export function ") {
            let clean = trimmed.trim_start_matches("export ");
            if let Some(name) = extract_word_after(clean, "function ", "(") {
                let mut e = Entity::new(EntityType::Document, name, format!("{} function", lang));
                e.metadata.insert("kind".into(), serde_json::json!("function"));
                e.metadata.insert("language".into(), serde_json::json!(lang));
                entities.push(e);
                func_count += 1;
            }
        }

        // Arrow functions: const name = (
        if (trimmed.starts_with("const ") || trimmed.starts_with("let ") || trimmed.starts_with("var "))
            && trimmed.contains("= (") || trimmed.contains("= async (")
        {
            if let Some(name) = extract_const_name(trimmed) {
                let mut e = Entity::new(EntityType::Document, name, format!("{} arrow function", lang));
                e.metadata.insert("kind".into(), serde_json::json!("function"));
                e.metadata.insert("language".into(), serde_json::json!(lang));
                entities.push(e);
                func_count += 1;
            }
        }

        // Imports
        if trimmed.starts_with("import ") || trimmed.starts_with("from ") {
            import_count += 1;
        }

        // Exports
        if trimmed.starts_with("export ") {
            export_count += 1;
        }
    }

    let summary = format!("{}: {} classes, {} functions, {} imports, {} exports",
        lang, class_count, func_count, import_count, export_count);
    ParsedCode { entities, summary }
}

// ═══════════════════════════════════════════════════════════════
//  Rust Parser
// ═══════════════════════════════════════════════════════════════

pub fn parse_rust(content: &str) -> ParsedCode {
    let mut entities = Vec::new();
    let mut struct_count = 0;
    let mut enum_count = 0;
    let mut fn_count = 0;
    let mut impl_count = 0;
    let mut trait_count = 0;
    let mut mod_count = 0;

    for line in content.lines() {
        let trimmed = line.trim();

        // Structs: pub struct Name / struct Name
        if trimmed.contains("struct ") && !trimmed.starts_with("//") {
            let clean = trimmed.trim_start_matches("pub ").trim_start_matches("pub(crate) ");
            if let Some(name) = extract_word_after(clean, "struct ", "{").or_else(|| extract_word_after(clean, "struct ", "<").or_else(|| extract_word_after(clean, "struct ", ";"))) {
                let mut e = Entity::new(EntityType::Document, name, "Rust struct".into());
                e.metadata.insert("kind".into(), serde_json::json!("struct"));
                e.metadata.insert("language".into(), serde_json::json!("rust"));
                entities.push(e);
                struct_count += 1;
            }
        }

        // Enums: pub enum Name / enum Name
        if trimmed.contains("enum ") && !trimmed.starts_with("//") {
            let clean = trimmed.trim_start_matches("pub ").trim_start_matches("pub(crate) ");
            if let Some(name) = extract_word_after(clean, "enum ", "{").or_else(|| extract_word_after(clean, "enum ", "<")) {
                let mut e = Entity::new(EntityType::Document, name, "Rust enum".into());
                e.metadata.insert("kind".into(), serde_json::json!("enum"));
                e.metadata.insert("language".into(), serde_json::json!("rust"));
                entities.push(e);
                enum_count += 1;
            }
        }

        // Functions: pub fn name / fn name
        if trimmed.contains("fn ") && !trimmed.starts_with("//") {
            let clean = trimmed.trim_start_matches("pub ").trim_start_matches("pub(crate) ").trim_start_matches("pub async ").trim_start_matches("async ");
            if let Some(name) = extract_word_after(clean, "fn ", "(") {
                let mut e = Entity::new(EntityType::Document, name, "Rust function".into());
                e.metadata.insert("kind".into(), serde_json::json!("function"));
                e.metadata.insert("language".into(), serde_json::json!("rust"));
                entities.push(e);
                fn_count += 1;
            }
        }

        // Impl blocks: impl Name
        if trimmed.starts_with("impl") && trimmed.contains("for ") {
            // impl Trait for Type
            impl_count += 1;
        } else if trimmed.starts_with("impl ") || trimmed.starts_with("pub impl ") {
            impl_count += 1;
        }

        // Traits: pub trait Name
        if trimmed.contains("trait ") && !trimmed.starts_with("//") {
            let clean = trimmed.trim_start_matches("pub ");
            if let Some(name) = extract_word_after(clean, "trait ", "{").or_else(|| extract_word_after(clean, "trait ", "<")) {
                let mut e = Entity::new(EntityType::Document, name, "Rust trait".into());
                e.metadata.insert("kind".into(), serde_json::json!("trait"));
                e.metadata.insert("language".into(), serde_json::json!("rust"));
                entities.push(e);
                trait_count += 1;
            }
        }

        // Modules: pub mod name
        if trimmed.contains("mod ") && !trimmed.starts_with("//") && !trimmed.starts_with("#[") {
            let clean = trimmed.trim_start_matches("pub ");
            if let Some(_name) = extract_word_after(clean, "mod ", "{").or_else(|| extract_word_after(clean, "mod ", ";")) {
                mod_count += 1;
            }
        }
    }

    let summary = format!("Rust: {} structs, {} enums, {} fns, {} impls, {} traits, {} mods",
        struct_count, enum_count, fn_count, impl_count, trait_count, mod_count);
    ParsedCode { entities, summary }
}

// ═══════════════════════════════════════════════════════════════
//  Go Parser
// ═══════════════════════════════════════════════════════════════

pub fn parse_go(content: &str) -> ParsedCode {
    let mut entities = Vec::new();
    let mut func_count = 0;
    let mut struct_count = 0;
    let mut interface_count = 0;

    for line in content.lines() {
        let trimmed = line.trim();

        // Functions: func Name( / func (r Receiver) Name(
        if trimmed.starts_with("func ") {
            let clean = trimmed.trim_start_matches("func ");
            // Skip receiver
            let name_part = if clean.starts_with("(") {
                match clean.splitn(2, ')').nth(1) {
                    Some(s) => s.trim(),
                    None => continue,
                }
            } else {
                clean
            };
            if let Some(name) = extract_word_after(name_part, "", "(") {
                let mut e = Entity::new(EntityType::Document, name, "Go function".into());
                e.metadata.insert("kind".into(), serde_json::json!("function"));
                e.metadata.insert("language".into(), serde_json::json!("go"));
                entities.push(e);
                func_count += 1;
            }
        }

        // Structs: type Name struct
        if trimmed.contains("struct {") || trimmed.contains("struct{") {
            if let Some(name) = extract_word_before(trimmed, "struct") {
                let mut e = Entity::new(EntityType::Document, name, "Go struct".into());
                e.metadata.insert("kind".into(), serde_json::json!("struct"));
                e.metadata.insert("language".into(), serde_json::json!("go"));
                entities.push(e);
                struct_count += 1;
            }
        }

        // Interfaces: type Name interface
        if trimmed.contains("interface {") || trimmed.contains("interface{") {
            if let Some(name) = extract_word_before(trimmed, "interface") {
                let mut e = Entity::new(EntityType::Document, name, "Go interface".into());
                e.metadata.insert("kind".into(), serde_json::json!("interface"));
                e.metadata.insert("language".into(), serde_json::json!("go"));
                entities.push(e);
                interface_count += 1;
            }
        }
    }

    let summary = format!("Go: {} functions, {} structs, {} interfaces", func_count, struct_count, interface_count);
    ParsedCode { entities, summary }
}

// ═══════════════════════════════════════════════════════════════
//  Java Parser
// ═══════════════════════════════════════════════════════════════

pub fn parse_java(content: &str) -> ParsedCode {
    let mut entities = Vec::new();
    let mut class_count = 0;
    let mut method_count = 0;
    let mut import_count = 0;

    for line in content.lines() {
        let trimmed = line.trim();

        // Classes: public class Name / class Name
        if trimmed.contains("class ") && !trimmed.starts_with("//") && !trimmed.starts_with("*") {
            let clean = trimmed.trim_start_matches("public ").trim_start_matches("abstract ").trim_start_matches("final ");
            if let Some(name) = extract_word_after(clean, "class ", "{").or_else(|| extract_word_after(clean, "class ", "extends").or_else(|| extract_word_after(clean, "class ", "implements"))) {
                let mut e = Entity::new(EntityType::Document, name, "Java class".into());
                e.metadata.insert("kind".into(), serde_json::json!("class"));
                e.metadata.insert("language".into(), serde_json::json!("java"));
                entities.push(e);
                class_count += 1;
            }
        }

        // Methods: public void name( / private String name(
        if !trimmed.starts_with("//") && !trimmed.starts_with("*")
            && (trimmed.contains("void ") || trimmed.contains("String ") || trimmed.contains("int ")
                || trimmed.contains("boolean ") || trimmed.contains("long ") || trimmed.contains("double "))
            && trimmed.contains("(") && trimmed.contains(")")
        {
            // Heuristic: if it looks like a method declaration
            if let Some(name) = extract_method_name(trimmed) {
                let mut e = Entity::new(EntityType::Document, name, "Java method".into());
                e.metadata.insert("kind".into(), serde_json::json!("method"));
                e.metadata.insert("language".into(), serde_json::json!("java"));
                entities.push(e);
                method_count += 1;
            }
        }

        // Imports
        if trimmed.starts_with("import ") {
            import_count += 1;
        }
    }

    let summary = format!("Java: {} classes, {} methods, {} imports", class_count, method_count, import_count);
    ParsedCode { entities, summary }
}

// ═══════════════════════════════════════════════════════════════
//  C/C++ Parser
// ═══════════════════════════════════════════════════════════════

pub fn parse_c_cpp(content: &str, ext: &str) -> ParsedCode {
    let lang = if ext == "c" || ext == "h" { "C" } else { "C++" };
    let mut entities = Vec::new();
    let mut func_count = 0;
    let mut struct_count = 0;
    let mut class_count = 0;
    let mut include_count = 0;

    for line in content.lines() {
        let trimmed = line.trim();

        // Includes
        if trimmed.starts_with("#include") {
            include_count += 1;
        }

        // Functions: return_type name( — heuristic: line starts with type and contains (
        if !trimmed.starts_with("//") && !trimmed.starts_with("#") && !trimmed.starts_with("*")
            && trimmed.contains("(") && trimmed.contains(")")
            && !trimmed.contains("=")  // Skip assignments
            && !trimmed.contains("if ") && !trimmed.contains("for ") && !trimmed.contains("while ")
        {
            if let Some(name) = extract_word_before(trimmed, "(") {
                // Skip keywords
                if !["if", "for", "while", "switch", "return", "sizeof", "typeof"].contains(&name.as_str()) {
                    let mut e = Entity::new(EntityType::Document, name, format!("{} function", lang));
                    e.metadata.insert("kind".into(), serde_json::json!("function"));
                    e.metadata.insert("language".into(), serde_json::json!(lang));
                    entities.push(e);
                    func_count += 1;
                }
            }
        }

        // Structs: struct Name {
        if trimmed.contains("struct ") && trimmed.contains("{") {
            if let Some(name) = extract_word_after(trimmed, "struct ", "{").or_else(|| extract_word_after(trimmed, "struct ", ":")) {
                let mut e = Entity::new(EntityType::Document, name, format!("{} struct", lang));
                e.metadata.insert("kind".into(), serde_json::json!("struct"));
                e.metadata.insert("language".into(), serde_json::json!(lang));
                entities.push(e);
                struct_count += 1;
            }
        }

        // C++ classes: class Name {
        if ext == "cpp" || ext == "hpp" {
            if trimmed.contains("class ") && trimmed.contains("{") {
                let clean = trimmed.trim_start_matches("class ");
                if let Some(name) = extract_word_after(&clean, "", "{").or_else(|| extract_word_after(&clean, "", ":")) {
                    let mut e = Entity::new(EntityType::Document, name, "C++ class".into());
                    e.metadata.insert("kind".into(), serde_json::json!("class"));
                    e.metadata.insert("language".into(), serde_json::json!("cpp"));
                    entities.push(e);
                    class_count += 1;
                }
            }
        }
    }

    let summary = format!("{}: {} functions, {} structs, {} classes, {} includes",
        lang, func_count, struct_count, class_count, include_count);
    ParsedCode { entities, summary }
}

// ═══════════════════════════════════════════════════════════════
//  Helper Functions
// ═══════════════════════════════════════════════════════════════

/// Extract word after a keyword until a delimiter
fn extract_word_after(text: &str, keyword: &str, _until: &str) -> Option<String> {
    let after_keyword = text.strip_prefix(keyword)?.trim();
    let word: String = after_keyword.chars()
        .take_while(|c| *c != '(' && *c != '{' && *c != '<' && *c != ':' && *c != ';' && !c.is_whitespace())
        .collect();
    let word = word.trim();
    if word.is_empty() || ["pub", "pub(crate)", "pub(super)", "async", "static", "const", "let", "var", "fn", "struct", "enum", "trait", "impl", "mod", "class", "function", "type", "interface", "func"].contains(&word) {
        return None;
    }
    Some(word.to_string())
}

/// Extract word before a delimiter
fn extract_word_before(text: &str, before: &str) -> Option<String> {
    let before_idx = text.find(before)?;
    let before_part = &text[..before_idx].trim_end();
    let word: String = before_part.chars().rev()
        .take_while(|c| *c != ' ' && *c != '\t')
        .collect::<String>()
        .chars().rev()
        .collect();
    let word = word.trim();
    if word.is_empty() || ["pub", "pub(crate)", "pub(super)", "async", "static", "const", "let", "var", "fn", "struct", "enum", "trait", "impl", "mod", "class", "function", "type", "interface", "func", "return", "if", "else", "for", "while", "loop", "match", "switch", "case"].contains(&word) {
        return None;
    }
    Some(word.to_string())
}

/// Extract parameter list from function signature
fn extract_params(text: &str) -> String {
    if let Some(start) = text.find('(') {
        if let Some(end) = text.rfind(')') {
            if end <= start {
                return String::new();
            }
            let params = text[start + 1..end].trim();
            crate::core::text::truncate_with_suffix(params, 40, "...")
        } else {
            String::new()
        }
    } else {
        String::new()
    }
}

/// Extract const/let name from declaration
fn extract_const_name(text: &str) -> Option<String> {
    let clean = text.trim_start_matches("const ").trim_start_matches("let ").trim_start_matches("var ").trim_start_matches("export ");
    let name: String = clean.chars().take_while(|c| *c != '=' && *c != ':' && !c.is_whitespace()).collect();
    let name = name.trim();
    if name.is_empty() || name.contains("require") || name.contains("import") {
        return None;
    }
    Some(name.to_string())
}

/// Extract class description from Python class definition
fn extract_class_description(text: &str) -> String {
    if text.contains('(') {
        if let Some(paren_content) = text.splitn(2, '(').nth(1) {
            let base = paren_content.trim_end_matches(':').trim_end_matches(')').trim();
            if !base.is_empty() {
                return format!("Python class (extends: {})", base);
            }
        }
    }
    "Python class".into()
}

/// Extract method name from Java method declaration
fn extract_method_name(text: &str) -> Option<String> {
    // Find the opening parenthesis
    let paren_idx = text.find('(')?;
    let before_paren = &text[..paren_idx].trim_end();
    // The method name is the last word before (
    let name: String = before_paren.chars().rev()
        .take_while(|c| *c != ' ' && *c != '\t' && *c != '<' && *c != '>')
        .collect::<String>()
        .chars().rev()
        .collect();
    let name = name.trim();
    if name.is_empty() || ["void", "String", "int", "boolean", "long", "double", "float", "byte", "char", "short", "public", "private", "protected", "static", "final", "abstract", "synchronized", "native", "strictfp"].contains(&name) {
        return None;
    }
    Some(name.to_string())
}
