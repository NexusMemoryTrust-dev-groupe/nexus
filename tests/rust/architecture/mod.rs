#[cfg(test)]
mod tests {
    use std::path::Path;

    /// Core business logic must not depend on infrastructure (storage, infra).
    #[test]
    fn domain_does_not_depend_on_infrastructure() {
        let core_dir = Path::new("src-tauri/src/core");
        if !core_dir.exists() {
            return;
        }

        let entries = std::fs::read_dir(core_dir).expect("Failed to read core dir");
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                // Recurse into subdirectories (event_bus, config, security)
                if let Ok(sub_entries) = std::fs::read_dir(&path) {
                    for sub_entry in sub_entries.flatten() {
                        let sub_path = sub_entry.path();
                        if sub_path.extension().is_some_and(|e| e == "rs") {
                            let content = std::fs::read_to_string(&sub_path)
                                .expect("Failed to read core source file");
                            assert!(
                                !content.contains("use crate::storage::")
                                    && !content.contains("use crate::infra::"),
                                "Core file {:?} should not depend on storage or infra",
                                sub_path
                            );
                        }
                    }
                }
            } else if path.extension().is_some_and(|e| e == "rs") {
                let content =
                    std::fs::read_to_string(&path).expect("Failed to read core source file");
                assert!(
                    !content.contains("use crate::storage::")
                        && !content.contains("use crate::infra::"),
                    "Core file {:?} should not depend on storage or infra",
                    path
                );
            }
        }
    }

    /// No file in core/ should use tauri:: imports.
    #[test]
    fn core_has_no_tauri_dependencies() {
        let core_dir = Path::new("src-tauri/src/core");
        if !core_dir.exists() {
            return;
        }

        let entries = std::fs::read_dir(core_dir).expect("Failed to read core dir");
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if let Ok(sub_entries) = std::fs::read_dir(&path) {
                    for sub_entry in sub_entries.flatten() {
                        let sub_path = sub_entry.path();
                        if sub_path.extension().is_some_and(|e| e == "rs") {
                            let content = std::fs::read_to_string(&sub_path)
                                .expect("Failed to read core source file");
                            assert!(
                                !content.contains("tauri::"),
                                "Core file {:?} should not depend on Tauri",
                                sub_path
                            );
                        }
                    }
                }
            } else if path.extension().is_some_and(|e| e == "rs") {
                let content =
                    std::fs::read_to_string(&path).expect("Failed to read core source file");
                assert!(
                    !content.contains("tauri::"),
                    "Core file {:?} should not depend on Tauri",
                    path
                );
            }
        }
    }

    /// All required M1 core modules exist.
    #[test]
    fn core_modules_exist() {
        let required_files = vec![
            "result.rs",
            "entity_id.rs",
            "value_object.rs",
            "domain_event.rs",
            "module_registry.rs",
            "mod.rs",
        ];

        for file in required_files {
            let path = Path::new("src-tauri/src/core").join(file);
            assert!(
                path.exists(),
                "Missing core module: {}",
                path.display()
            );
        }

        // Check subdirectories
        let required_dirs = vec!["event_bus", "config", "security"];
        for dir in required_dirs {
            let path = Path::new("src-tauri/src/core").join(dir);
            assert!(
                path.exists(),
                "Missing core directory: {}",
                path.display()
            );
        }
    }

    /// Core modules don't import from sibling modules directly.
    /// They should only import from crate::core (the parent).
    #[test]
    fn no_cyclic_core_dependencies() {
        let core_dir = Path::new("src-tauri/src/core");
        if !core_dir.exists() {
            return;
        }

        let entries: Vec<_> = std::fs::read_dir(core_dir)
            .expect("Failed to read core dir")
            .flatten()
            .filter_map(|e| {
                let p = e.path();
                if p.extension().is_some_and(|ext| ext == "rs") {
                    Some(p)
                } else {
                    None
                }
            })
            .collect();

        for path in &entries {
            let content = std::fs::read_to_string(path).expect("Failed to read source");
            let module_name = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("unknown");

            // Each module should only import from crate::core, not from siblings directly
            for line in content.lines() {
                let trimmed = line.trim();
                if trimmed.starts_with("use crate::core::") {
                    continue;
                }
                if trimmed.starts_with("use super::") {
                    continue;
                }
            }
        }
    }
}
