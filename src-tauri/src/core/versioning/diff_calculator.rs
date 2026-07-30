use async_trait::async_trait;
use serde::Serialize;

use crate::core::result::Result;

/// Trait for calculating diffs between old and new states.
/// Supports text diffs, structured (field-level) diffs, and JSON diffs.
#[async_trait]
pub trait DiffCalculator: Send + Sync {
    /// Calculate a text diff between two strings (Myers algorithm).
    fn calculate_text_diff(&self, old: &str, new: &str) -> String;

    /// Calculate a structured (field-level) diff between two serializable values.
    fn calculate_structured_diff<T: Serialize>(&self, old: &T, new: &T) -> Result<String>;

    /// Calculate a JSON diff between two JSON values.
    fn calculate_json_diff(
        &self,
        old: &serde_json::Value,
        new: &serde_json::Value,
    ) -> String;
}

/// Simple diff calculator using text-based diff.
pub struct SimpleDiffCalculator;

#[async_trait]
impl DiffCalculator for SimpleDiffCalculator {
    fn calculate_text_diff(&self, old: &str, new: &str) -> String {
        let old_lines: Vec<&str> = old.lines().collect();
        let new_lines: Vec<&str> = new.lines().collect();
        let mut diff = String::new();

        let max = old_lines.len().max(new_lines.len());
        for i in 0..max {
            let old_line = old_lines.get(i).copied().unwrap_or("");
            let new_line = new_lines.get(i).copied().unwrap_or("");

            if old_line != new_line {
                if !old_line.is_empty() {
                    diff.push_str(&format!("- {}\n", old_line));
                }
                if !new_line.is_empty() {
                    diff.push_str(&format!("+ {}\n", new_line));
                }
            }
        }

        if diff.is_empty() {
            "(no changes)".to_string()
        } else {
            diff
        }
    }

    fn calculate_structured_diff<T: Serialize>(&self, old: &T, new: &T) -> Result<String> {
        let old_json =
            serde_json::to_value(old).map_err(|e| crate::core::result::AppError::Internal(e.to_string()))?;
        let new_json =
            serde_json::to_value(new).map_err(|e| crate::core::result::AppError::Internal(e.to_string()))?;
        Ok(self.calculate_json_diff(&old_json, &new_json))
    }

    fn calculate_json_diff(
        &self,
        old: &serde_json::Value,
        new: &serde_json::Value,
    ) -> String {
        if old == new {
            return "(no changes)".to_string();
        }

        let mut diff = String::new();

        match (old, new) {
            (serde_json::Value::Object(old_map), serde_json::Value::Object(new_map)) => {
                // Keys in old but not in new → removed
                for key in old_map.keys() {
                    if !new_map.contains_key(key) {
                        diff.push_str(&format!("- {}: {}\n", key, old_map[key]));
                    }
                }
                // Keys in new but not in old → added; keys in both → changed
                for (key, new_val) in new_map {
                    match old_map.get(key) {
                        Some(old_val) if old_val != new_val => {
                            diff.push_str(&format!(
                                "~ {}: {} → {}\n",
                                key, old_val, new_val
                            ));
                        }
                        None => {
                            diff.push_str(&format!("+ {}: {}\n", key, new_val));
                        }
                        _ => {}
                    }
                }
            }
            _ => {
                diff.push_str(&format!("- {}\n", old));
                diff.push_str(&format!("+ {}\n", new));
            }
        }

        if diff.is_empty() {
            "(no changes)".to_string()
        } else {
            diff
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_diff_same() {
        let calc = SimpleDiffCalculator;
        let result = calc.calculate_text_diff("hello", "hello");
        assert_eq!(result, "(no changes)");
    }

    #[test]
    fn text_diff_changed() {
        let calc = SimpleDiffCalculator;
        let result = calc.calculate_text_diff("line1\nline2", "line1\nline3");
        assert!(result.contains("- line2"));
        assert!(result.contains("+ line3"));
    }

    #[test]
    fn text_diff_added_line() {
        let calc = SimpleDiffCalculator;
        let result = calc.calculate_text_diff("a", "a\nb");
        assert!(result.contains("+ b"));
    }

    #[test]
    fn text_diff_removed_line() {
        let calc = SimpleDiffCalculator;
        let result = calc.calculate_text_diff("a\nb", "a");
        assert!(result.contains("- b"));
    }

    #[test]
    fn json_diff_same() {
        let calc = SimpleDiffCalculator;
        let a = serde_json::json!({"key": "value"});
        let b = serde_json::json!({"key": "value"});
        assert_eq!(calc.calculate_json_diff(&a, &b), "(no changes)");
    }

    #[test]
    fn json_diff_added_key() {
        let calc = SimpleDiffCalculator;
        let a = serde_json::json!({"a": 1});
        let b = serde_json::json!({"a": 1, "b": 2});
        let result = calc.calculate_json_diff(&a, &b);
        assert!(result.contains("+ b: 2"));
    }

    #[test]
    fn json_diff_removed_key() {
        let calc = SimpleDiffCalculator;
        let a = serde_json::json!({"a": 1, "b": 2});
        let b = serde_json::json!({"a": 1});
        let result = calc.calculate_json_diff(&a, &b);
        assert!(result.contains("- b: 2"));
    }

    #[test]
    fn json_diff_changed_value() {
        let calc = SimpleDiffCalculator;
        let a = serde_json::json!({"x": 10});
        let b = serde_json::json!({"x": 20});
        let result = calc.calculate_json_diff(&a, &b);
        assert!(result.contains("~ x:"));
        assert!(result.contains("10"));
        assert!(result.contains("20"));
    }

    #[test]
    fn structured_diff_serializable() {
        let calc = SimpleDiffCalculator;
        #[derive(Serialize)]
        struct Point {
            x: i32,
            y: i32,
        }
        let old = Point { x: 1, y: 2 };
        let new = Point { x: 1, y: 3 };
        let result = calc.calculate_structured_diff(&old, &new).unwrap();
        assert!(result.contains("~ y:"));
    }
}
