//! Filesystem sandbox for AI-driven file operations.
//!
//! # Why this exists
//!
//! The MCP server exposes `nexus_write_file`, `nexus_delete_file` and friends to
//! whatever model is driving OpenCode. Before this module those tools accepted
//! *any* absolute path, so a hallucinated argument could overwrite or delete
//! anything the user's account can reach — `C:\Windows\System32`, an SSH key, a
//! whole source tree. That is not an acceptable risk for a shipped product.
//!
//! Every path now has to resolve inside an explicitly allowed root:
//!
//! * folders the user added to a Nexus project workspace,
//! * extra roots the user listed in settings (`sandbox.extra_roots`),
//! * the Nexus data directory itself.
//!
//! # Why canonicalisation matters
//!
//! A naive `path.starts_with(root)` check is trivially bypassed:
//!
//! ```text
//! C:\Projects\mine\..\..\Windows\System32\drivers\etc\hosts
//! ```
//!
//! textually starts with an allowed root yet escapes it. Symlinks and NTFS
//! junctions do the same without any `..` at all. So we resolve the real
//! location first (via [`std::fs::canonicalize`], which follows links) and only
//! then compare. For paths that do not exist yet — the normal case for "create
//! this file" — we canonicalise the nearest existing ancestor and re-attach the
//! remaining components, refusing any `..` in that tail.
//!
//! Comparison is component-wise, never string prefix: `C:\Proj` must not be
//! considered a parent of `C:\Project2`.

use std::fmt;
use std::path::{Component, Path, PathBuf};

/// What the caller intends to do with the path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Access {
    Read,
    Write,
    Delete,
}

impl Access {
    fn verb(self) -> &'static str {
        match self {
            Access::Read => "read",
            Access::Write => "write to",
            Access::Delete => "delete",
        }
    }
}

/// Why a path was rejected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SandboxError {
    /// No roots configured at all — nothing can be touched yet.
    NoRoots { attempted: String, access: Access },
    /// The path resolves outside every allowed root.
    Outside {
        attempted: String,
        resolved: String,
        access: Access,
        roots: Vec<String>,
    },
    /// The path could not be resolved (bad drive, permission denied, ...).
    Unresolvable { attempted: String, reason: String },
    /// Relative paths are ambiguous for a background process; require absolute.
    NotAbsolute { attempted: String },
    /// A Windows reserved device name (CON, NUL, COM1, ...).
    ReservedName { attempted: String, name: String },
}

impl fmt::Display for SandboxError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SandboxError::NoRoots { attempted, access } => write!(
                f,
                "Refusing to {} '{}': no workspace folders are registered yet. \
                 Add a folder to a Nexus project first — file access is limited to your projects.",
                access.verb(),
                attempted
            ),
            SandboxError::Outside {
                attempted,
                resolved,
                access,
                roots,
            } => {
                write!(
                    f,
                    "Refusing to {} '{}': it resolves to '{}', which is outside your Nexus workspace. \
                     Allowed locations: {}.",
                    access.verb(),
                    attempted,
                    resolved,
                    if roots.is_empty() {
                        "(none)".to_string()
                    } else {
                        roots.join("; ")
                    }
                )
            }
            SandboxError::Unresolvable { attempted, reason } => {
                write!(f, "Cannot resolve path '{}': {}", attempted, reason)
            }
            SandboxError::NotAbsolute { attempted } => write!(
                f,
                "Path '{}' is not absolute. Provide a full path such as C:\\Projects\\app\\main.rs.",
                attempted
            ),
            SandboxError::ReservedName { attempted, name } => write!(
                f,
                "Refusing to use '{}': '{}' is a reserved Windows device name.",
                attempted, name
            ),
        }
    }
}

impl std::error::Error for SandboxError {}

/// Windows device names that are illegal as file names regardless of extension.
const RESERVED: &[&str] = &[
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

/// Strip the `\\?\` verbatim prefix that `canonicalize` adds on Windows so
/// error messages stay readable and comparisons stay consistent.
fn strip_verbatim(path: &Path) -> PathBuf {
    let s = path.to_string_lossy();
    if let Some(rest) = s.strip_prefix(r"\\?\UNC\") {
        return PathBuf::from(format!(r"\\{}", rest));
    }
    if let Some(rest) = s.strip_prefix(r"\\?\") {
        return PathBuf::from(rest);
    }
    path.to_path_buf()
}

/// Resolve a path that may not exist yet.
///
/// Canonicalises the deepest existing ancestor, then re-appends the missing
/// tail. `..` inside the tail is rejected rather than collapsed — collapsing it
/// lexically is exactly the bug this module exists to prevent.
fn resolve(path: &Path) -> std::result::Result<PathBuf, SandboxError> {
    let attempted = path.display().to_string();

    if let Ok(real) = std::fs::canonicalize(path) {
        return Ok(strip_verbatim(&real));
    }

    let mut missing: Vec<std::ffi::OsString> = Vec::new();
    let mut cursor = path;

    loop {
        let parent = match cursor.parent() {
            Some(p) => p,
            None => {
                return Err(SandboxError::Unresolvable {
                    attempted,
                    reason: "no existing parent directory".to_string(),
                })
            }
        };

        match cursor.file_name() {
            Some(name) => missing.push(name.to_os_string()),
            None => {
                return Err(SandboxError::Unresolvable {
                    attempted,
                    reason: "path has no file name component".to_string(),
                })
            }
        }

        if let Ok(real_parent) = std::fs::canonicalize(parent) {
            let mut out = strip_verbatim(&real_parent);
            for name in missing.iter().rev() {
                if name == ".." {
                    return Err(SandboxError::Unresolvable {
                        attempted,
                        reason: "'..' cannot be resolved past a missing directory".to_string(),
                    });
                }
                out.push(name);
            }
            return Ok(out);
        }

        if parent.parent().is_none() {
            return Err(SandboxError::Unresolvable {
                attempted,
                reason: format!("drive or share '{}' is not reachable", parent.display()),
            });
        }
        cursor = parent;
    }
}

/// True when `child` is `root` or lives beneath it, compared per path component.
fn is_within(child: &Path, root: &Path) -> bool {
    let mut c = child.components();
    for rc in root.components() {
        match c.next() {
            Some(cc) if components_match(&cc, &rc) => {}
            _ => return false,
        }
    }
    true
}

/// Windows paths are case-insensitive, so compare accordingly.
fn components_match(a: &Component, b: &Component) -> bool {
    if a == b {
        return true;
    }
    a.as_os_str()
        .to_string_lossy()
        .eq_ignore_ascii_case(&b.as_os_str().to_string_lossy())
}

/// An immutable set of allowed roots plus the policy check.
#[derive(Debug, Clone, Default)]
pub struct Sandbox {
    roots: Vec<PathBuf>,
}

impl Sandbox {
    /// Build from raw path strings, dropping anything that cannot be resolved.
    ///
    /// Unresolvable roots are skipped rather than fatal: a workspace may point
    /// at a USB drive that is currently unplugged, and that must not lock the
    /// user out of their other projects.
    pub fn from_roots<I, S>(roots: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let mut resolved: Vec<PathBuf> = Vec::new();
        for raw in roots {
            let raw = raw.as_ref().trim();
            if raw.is_empty() {
                continue;
            }
            if let Ok(p) = resolve(Path::new(raw)) {
                if !resolved.iter().any(|existing| is_within(&p, existing)) {
                    resolved.retain(|existing| !is_within(existing, &p));
                    resolved.push(p);
                }
            }
        }
        Self { roots: resolved }
    }

    /// Roots currently in effect, for display and diagnostics.
    pub fn roots(&self) -> Vec<String> {
        self.roots.iter().map(|p| p.display().to_string()).collect()
    }

    pub fn is_empty(&self) -> bool {
        self.roots.is_empty()
    }

    /// Validate `path` for `access`, returning the resolved absolute path.
    pub fn check(
        &self,
        path: &str,
        access: Access,
    ) -> std::result::Result<PathBuf, SandboxError> {
        let raw = Path::new(path);

        if !raw.is_absolute() {
            return Err(SandboxError::NotAbsolute {
                attempted: path.to_string(),
            });
        }

        if let Some(name) = raw.file_name() {
            let stem = name
                .to_string_lossy()
                .split('.')
                .next()
                .unwrap_or("")
                .to_ascii_uppercase();
            if RESERVED.contains(&stem.as_str()) {
                return Err(SandboxError::ReservedName {
                    attempted: path.to_string(),
                    name: stem,
                });
            }
        }

        let resolved = resolve(raw)?;

        if self.roots.is_empty() {
            return Err(SandboxError::NoRoots {
                attempted: path.to_string(),
                access,
            });
        }

        if self.roots.iter().any(|root| is_within(&resolved, root)) {
            Ok(resolved)
        } else {
            Err(SandboxError::Outside {
                attempted: path.to_string(),
                resolved: resolved.display().to_string(),
                access,
                roots: self.roots(),
            })
        }
    }
}

// ── Loading the live policy ─────────────────────────────────────────────────

/// Config key holding newline- or semicolon-separated extra roots.
pub const EXTRA_ROOTS_KEY: &str = "sandbox.extra_roots";

/// Collect roots from the database: workspace root folders, user-configured
/// extra roots, and the Nexus data directory.
fn collect_roots() -> Vec<String> {
    let mut roots: Vec<String> = Vec::new();

    if let Some(dir) = crate::db::db_path().parent() {
        roots.push(dir.display().to_string());
    }

    if let Ok(conn) = crate::db::open_connection() {
        // Top-level workspace entries are the folders the user explicitly added.
        if let Ok(mut stmt) = conn.prepare(
            "SELECT DISTINCT native_path FROM workspace_entries WHERE parent_id IS NULL",
        ) {
            if let Ok(rows) = stmt.query_map([], |row| row.get::<_, String>(0)) {
                for row in rows.flatten() {
                    roots.push(row);
                }
            }
        }

        if let Ok(value) = conn.query_row(
            "SELECT value FROM configuration_kv WHERE key = ?1",
            [EXTRA_ROOTS_KEY],
            |row| row.get::<_, String>(0),
        ) {
            for part in value.split(['\n', ';']) {
                let part = part.trim();
                if !part.is_empty() {
                    roots.push(part.to_string());
                }
            }
        }
    }

    roots
}

/// Build the sandbox from the current database state.
pub fn current() -> Sandbox {
    Sandbox::from_roots(collect_roots())
}

/// Convenience: validate a path against the live policy, returning a
/// user-facing error string suitable for an MCP response.
pub fn guard(path: &str, access: Access) -> std::result::Result<PathBuf, String> {
    current().check(path, access).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("nexus-sandbox-{}-{}", name, std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::canonicalize(&dir).map(|p| strip_verbatim(&p)).unwrap_or(dir)
    }

    #[test]
    fn allows_a_file_inside_the_root() {
        let root = tmp("inside");
        let sb = Sandbox::from_roots([root.display().to_string()]);
        let target = root.join("notes.md");
        assert!(sb.check(&target.display().to_string(), Access::Write).is_ok());
    }

    #[test]
    fn allows_the_root_itself() {
        let root = tmp("rootitself");
        let sb = Sandbox::from_roots([root.display().to_string()]);
        assert!(sb.check(&root.display().to_string(), Access::Read).is_ok());
    }

    #[test]
    fn allows_nested_subdirectories() {
        let root = tmp("nested");
        let sb = Sandbox::from_roots([root.display().to_string()]);
        let deep = root.join("a").join("b").join("c.txt");
        assert!(sb.check(&deep.display().to_string(), Access::Write).is_ok());
    }

    #[test]
    fn blocks_dot_dot_traversal() {
        let root = tmp("traversal");
        std::fs::create_dir_all(root.join("sub")).unwrap();
        let sb = Sandbox::from_roots([root.join("sub").display().to_string()]);

        // Escapes to the parent, which is NOT a root.
        let escape = root.join("sub").join("..").join("outside.txt");
        let err = sb
            .check(&escape.display().to_string(), Access::Write)
            .unwrap_err();
        assert!(matches!(err, SandboxError::Outside { .. }), "got {:?}", err);
    }

    #[test]
    fn blocks_absolute_path_outside_roots() {
        let root = tmp("outside");
        let sb = Sandbox::from_roots([root.display().to_string()]);
        let err = sb
            .check(r"C:\Windows\System32\drivers\etc\hosts", Access::Delete)
            .unwrap_err();
        assert!(matches!(err, SandboxError::Outside { .. }), "got {:?}", err);
    }

    #[test]
    fn sibling_prefix_is_not_treated_as_child() {
        // `C:\Proj` must not authorise `C:\Project2` — the classic string-prefix bug.
        let base = tmp("prefix");
        let allowed = base.join("Proj");
        let sibling = base.join("Project2");
        std::fs::create_dir_all(&allowed).unwrap();
        std::fs::create_dir_all(&sibling).unwrap();

        let sb = Sandbox::from_roots([allowed.display().to_string()]);
        let err = sb
            .check(&sibling.join("x.txt").display().to_string(), Access::Write)
            .unwrap_err();
        assert!(matches!(err, SandboxError::Outside { .. }), "got {:?}", err);
    }

    #[test]
    fn empty_policy_denies_everything() {
        let sb = Sandbox::from_roots(Vec::<String>::new());
        assert!(sb.is_empty());
        let err = sb.check(r"C:\anything\at\all.txt", Access::Write).unwrap_err();
        assert!(matches!(err, SandboxError::NoRoots { .. }), "got {:?}", err);
    }

    #[test]
    fn relative_paths_are_rejected() {
        let root = tmp("relative");
        let sb = Sandbox::from_roots([root.display().to_string()]);
        let err = sb.check("notes.md", Access::Write).unwrap_err();
        assert!(matches!(err, SandboxError::NotAbsolute { .. }), "got {:?}", err);
    }

    #[test]
    fn reserved_device_names_are_rejected() {
        let root = tmp("reserved");
        let sb = Sandbox::from_roots([root.display().to_string()]);
        let target = root.join("NUL");
        let err = sb
            .check(&target.display().to_string(), Access::Write)
            .unwrap_err();
        assert!(matches!(err, SandboxError::ReservedName { .. }), "got {:?}", err);
    }

    #[test]
    fn case_differences_still_match_on_windows() {
        let root = tmp("case");
        let sb = Sandbox::from_roots([root.display().to_string()]);
        let shouty = root.display().to_string().to_uppercase();
        let target = format!(r"{}\file.txt", shouty.trim_end_matches('\\'));
        assert!(sb.check(&target, Access::Write).is_ok(), "case-insensitive match failed");
    }

    #[test]
    fn nested_roots_are_collapsed() {
        let root = tmp("collapse");
        let child = root.join("child");
        std::fs::create_dir_all(&child).unwrap();
        let sb = Sandbox::from_roots([
            child.display().to_string(),
            root.display().to_string(),
        ]);
        // The parent subsumes the child, so only one root should remain.
        assert_eq!(sb.roots().len(), 1, "roots: {:?}", sb.roots());
    }

    #[test]
    fn unresolvable_roots_do_not_break_valid_ones() {
        let root = tmp("mixed");
        let sb = Sandbox::from_roots([
            r"Z:\definitely\not\mounted".to_string(),
            root.display().to_string(),
        ]);
        assert_eq!(sb.roots().len(), 1);
        assert!(sb.check(&root.join("ok.txt").display().to_string(), Access::Write).is_ok());
    }

    #[test]
    fn error_messages_name_the_operation_and_the_allowed_roots() {
        let root = tmp("message");
        let sb = Sandbox::from_roots([root.display().to_string()]);
        let msg = sb
            .check(r"C:\Windows\System32\config\SAM", Access::Delete)
            .unwrap_err()
            .to_string();
        assert!(msg.contains("delete"), "missing verb: {}", msg);
        assert!(msg.contains("outside your Nexus workspace"), "missing reason: {}", msg);
        assert!(msg.contains(&root.display().to_string()), "missing roots: {}", msg);
    }

    #[test]
    fn blocks_traversal_through_a_nonexistent_directory() {
        let root = tmp("deeptraversal");
        let sb = Sandbox::from_roots([root.display().to_string()]);
        let sneaky = root.join("nope").join("..").join("..").join("escaped.txt");
        let err = sb
            .check(&sneaky.display().to_string(), Access::Write)
            .unwrap_err();
        // Either unresolvable or outside — both are refusals, never Ok.
        assert!(
            matches!(err, SandboxError::Unresolvable { .. } | SandboxError::Outside { .. }),
            "got {:?}",
            err
        );
    }
}
