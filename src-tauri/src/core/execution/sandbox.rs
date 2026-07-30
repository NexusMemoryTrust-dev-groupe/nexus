use crate::core::result::{AppError, Result};

/// Sandbox constrains which paths and commands an action may use.
/// Every tool execution passes through the sandbox for safety checks.
pub struct Sandbox {
    pub allowed_paths: Vec<String>,
    pub blocked_commands: Vec<String>,
    pub max_file_size: u64,
}

impl Sandbox {
    /// Create a permissive sandbox (no path restrictions, standard blocked commands).
    pub fn new() -> Self {
        Self {
            allowed_paths: Vec::new(),
            blocked_commands: vec!["rm -rf /".to_string(), "sudo".to_string()],
            max_file_size: 10 * 1024 * 1024, // 10 MB
        }
    }

    /// Create a sandbox scoped to a single root directory.
    pub fn scoped(root: &str) -> Self {
        Self {
            allowed_paths: vec![root.to_string()],
            blocked_commands: vec!["rm -rf /".to_string(), "sudo".to_string()],
            max_file_size: 10 * 1024 * 1024,
        }
    }

    /// Validate that a filesystem path is within allowed locations.
    /// If no allowed_paths are configured, all paths are permitted.
    pub fn validate_path(&self, path: &str) -> Result<()> {
        if self.allowed_paths.is_empty() {
            return Ok(());
        }
        let normalised = path.replace('\\', "/");
        if self
            .allowed_paths
            .iter()
            .any(|p| normalised.starts_with(&p.replace('\\', "/")))
        {
            Ok(())
        } else {
            Err(AppError::Security(format!(
                "Path not allowed: {}",
                path
            )))
        }
    }

    /// Validate that a command does not contain blocked substrings.
    pub fn validate_command(&self, command: &str) -> Result<()> {
        if self
            .blocked_commands
            .iter()
            .any(|c| command.contains(c.as_str()))
        {
            Err(AppError::Security(format!(
                "Command blocked: {}",
                command
            )))
        } else {
            Ok(())
        }
    }

    /// Validate that a file size is within the allowed limit.
    pub fn validate_file_size(&self, size: u64) -> Result<()> {
        if size > self.max_file_size {
            Err(AppError::Security(format!(
                "File size {} exceeds limit {}",
                size, self.max_file_size
            )))
        } else {
            Ok(())
        }
    }
}

// ── Tests ──

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sandbox_new_defaults() {
        let sb = Sandbox::new();
        assert!(sb.allowed_paths.is_empty());
        assert_eq!(sb.max_file_size, 10 * 1024 * 1024);
        assert!(sb.blocked_commands.contains(&"rm -rf /".to_string()));
    }

    #[test]
    fn sandbox_scoped() {
        let sb = Sandbox::scoped("/tmp/project");
        assert_eq!(sb.allowed_paths, vec!["/tmp/project".to_string()]);
    }

    #[test]
    fn validate_path_allows_when_empty() {
        let sb = Sandbox::new();
        assert!(sb.validate_path("/any/path").is_ok());
    }

    #[test]
    fn validate_path_rejects_outside_scope() {
        let sb = Sandbox::scoped("/tmp/project");
        assert!(sb.validate_path("/etc/passwd").is_err());
    }

    #[test]
    fn validate_path_accepts_within_scope() {
        let sb = Sandbox::scoped("/tmp/project");
        assert!(sb.validate_path("/tmp/project/file.txt").is_ok());
    }

    #[test]
    fn validate_command_blocks_sudo() {
        let sb = Sandbox::new();
        assert!(sb.validate_command("sudo rm -rf /home").is_err());
    }

    #[test]
    fn validate_command_allows_safe() {
        let sb = Sandbox::new();
        assert!(sb.validate_command("ls -la").is_ok());
    }

    #[test]
    fn validate_file_size_within_limit() {
        let sb = Sandbox::new();
        assert!(sb.validate_file_size(1024).is_ok());
    }

    #[test]
    fn validate_file_size_exceeds_limit() {
        let sb = Sandbox::new();
        assert!(sb.validate_file_size(20 * 1024 * 1024).is_err());
    }

    #[test]
    fn validate_path_windows_backslash() {
        let sb = Sandbox::scoped("C:/Users/project");
        assert!(sb.validate_path("C:\\Users\\project\\file.txt").is_ok());
    }
}
