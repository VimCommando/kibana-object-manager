//! YAML field formatter transformer
//!
//! Handles conversion of compact YAML strings to formatted multi-line YAML
//! for better version control and readability in saved JSON files.

use crate::etl::Transformer;
use eyre::Result;
use serde_json::Value;

/// Transformer that formats YAML string fields to multi-line format
///
/// Used during extract (Kibana → Files) to convert compact YAML strings
/// into properly formatted multi-line YAML for better version control.
///
/// This transformer is generic and can be configured with any field paths.
/// Common use cases:
/// - Workflows: `yaml` field
/// - Agents: `definition` field (future)
/// - Custom objects: any field containing YAML content
///
/// # Example
///
/// ```
/// use kibana_object_manager::transform::YamlFormatter;
/// use kibana_object_manager::etl::Transformer;
/// use serde_json::json;
///
/// // Format workflow yaml field
/// let formatter = YamlFormatter::for_workflows();
/// let input = json!({
///     "id": "wf-123",
///     "name": "my-workflow",
///     "yaml": "version: 1.0\nsteps:\n  - name: step1"
/// });
///
/// let output = formatter.transform(input).unwrap();
/// // yaml field is now properly formatted with indentation preserved
/// ```
///
/// # Configuration
///
/// Create formatters with custom field paths:
///
/// ```
/// use kibana_object_manager::transform::YamlFormatter;
///
/// // Single field
/// let formatter = YamlFormatter::new(vec!["yaml"]);
///
/// // Multiple fields
/// let formatter = YamlFormatter::new(vec!["yaml", "definition"]);
///
/// // Nested field paths
/// let formatter = YamlFormatter::new(vec!["spec.yaml", "attributes.definition"]);
/// ```
///
/// # Error Handling
///
/// - **Invalid YAML**: Logs warning, keeps original string value
/// - **Missing field**: Silently skips (no error)
/// - **Null field**: Silently skips (no error)
/// - **Empty string**: Silently skips (no error)
/// - **Non-string field**: Silently skips (no error)
///
/// This ensures the transformer is resilient to edge cases and doesn't
/// break the ETL pipeline for workflows with unusual content.
pub struct YamlFormatter {
    fields: Vec<String>,
}

impl YamlFormatter {
    /// Create a new YAML formatter with custom field paths
    ///
    /// Field paths can be:
    /// - Simple: `"yaml"`, `"definition"`
    /// - Nested: `"spec.yaml"`, `"attributes.definition"`
    ///
    /// # Arguments
    ///
    /// * `fields` - Vector of field paths to format as YAML
    ///
    /// # Example
    ///
    /// ```
    /// use kibana_object_manager::transform::YamlFormatter;
    ///
    /// let formatter = YamlFormatter::new(vec!["yaml", "spec.definition"]);
    /// ```
    pub fn new(fields: Vec<&str>) -> Self {
        Self {
            fields: fields.iter().map(|s| s.to_string()).collect(),
        }
    }

    /// Create formatter configured for workflow objects
    ///
    /// Formats the `yaml` field commonly found in workflow objects.
    ///
    /// # Example
    ///
    /// ```
    /// use kibana_object_manager::transform::YamlFormatter;
    ///
    /// let formatter = YamlFormatter::for_workflows();
    /// ```
    pub fn for_workflows() -> Self {
        Self::new(vec!["yaml"])
    }

    /// Create formatter configured for agent objects (future use)
    ///
    /// Can be extended when agents start using YAML fields.
    ///
    /// # Example
    ///
    /// ```
    /// use kibana_object_manager::transform::YamlFormatter;
    ///
    /// let formatter = YamlFormatter::for_agents();
    /// ```
    pub fn for_agents() -> Self {
        // Placeholder for future agent YAML fields
        // Adjust field names when agent YAML structure is known
        Self::new(vec!["definition"])
    }

    /// Helper to get nested mutable field by path
    ///
    /// Supports dot notation for nested fields (e.g., "attributes.yaml")
    ///
    /// # Arguments
    ///
    /// * `obj` - The JSON object to traverse
    /// * `path` - Dot-separated path to the field
    ///
    /// # Returns
    ///
    /// `Some(&mut Value)` if field exists, `None` otherwise
    fn get_nested_mut<'a>(obj: &'a mut Value, path: &str) -> Option<&'a mut Value> {
        let parts: Vec<&str> = path.split('.').collect();
        let mut current = obj;

        for part in parts {
            current = current.get_mut(part)?;
        }

        Some(current)
    }

    /// Format a YAML string to multi-line format
    ///
    /// Parses the YAML string and re-serializes it with proper formatting.
    /// This ensures the YAML is valid and properly indented.
    ///
    /// # Arguments
    ///
    /// * `yaml_str` - The compact YAML string to format
    ///
    /// # Returns
    ///
    /// - `Ok(formatted_string)` if YAML is valid
    /// - `Err(error_message)` if YAML parsing fails
    ///
    /// # Note
    ///
    /// This function attempts to preserve the original YAML structure,
    /// including key order, but this depends on the serde_yaml implementation.
    fn format_yaml_string(yaml_str: &str) -> Result<String, String> {
        // Parse YAML to validate and get structured data
        let parsed: serde_yaml::Value =
            serde_yaml::from_str(yaml_str).map_err(|e| format!("Invalid YAML: {}", e))?;

        // Re-serialize with proper formatting
        // Note: serde_yaml should preserve key order in most cases,
        // but this is not guaranteed by the YAML 1.2 spec
        serde_yaml::to_string(&parsed).map_err(|e| format!("Failed to serialize YAML: {}", e))
    }
}

impl Transformer for YamlFormatter {
    type Input = Value;
    type Output = Value;

    fn transform(&self, mut input: Self::Input) -> Result<Self::Output> {
        for field_path in &self.fields {
            if let Some(field) = Self::get_nested_mut(&mut input, field_path) {
                // Only process if it's a string
                if let Some(yaml_str) = field.as_str() {
                    // Skip empty strings
                    if yaml_str.trim().is_empty() {
                        log::debug!("Skipping empty YAML field: {}", field_path);
                        continue;
                    }

                    // Try to format the YAML
                    match Self::format_yaml_string(yaml_str) {
                        Ok(formatted) => {
                            log::debug!("Formatted YAML field: {}", field_path);
                            *field = Value::String(formatted);
                        }
                        Err(e) => {
                            log::warn!(
                                "Could not format YAML field '{}': {}. Keeping original value.",
                                field_path,
                                e
                            );
                            // Keep original string value - no change needed
                        }
                    }
                }
                // If field is not a string (or is null), skip silently
            }
            // If field doesn't exist, skip silently
        }
        Ok(input)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_format_simple_yaml() {
        let formatter = YamlFormatter::new(vec!["yaml"]);
        let input = json!({
            "id": "wf-123",
            "name": "test-workflow",
            "yaml": "version: 1.0\nsteps:\n  - name: step1"
        });

        let output = formatter.transform(input).unwrap();
        let yaml_str = output["yaml"].as_str().unwrap();

        // Should be multi-line formatted
        assert!(yaml_str.contains("version: "));
        assert!(yaml_str.contains("1.0"));
        assert!(yaml_str.contains("\n"));
        assert!(yaml_str.contains("steps"));
    }

    #[test]
    fn test_format_nested_field() {
        let formatter = YamlFormatter::new(vec!["spec.definition"]);
        let input = json!({
            "id": "obj-123",
            "spec": {
                "definition": "key: value\nlist:\n  - item1\n  - item2"
            }
        });

        let output = formatter.transform(input).unwrap();
        assert!(output["spec"]["definition"].is_string());
        let yaml_str = output["spec"]["definition"].as_str().unwrap();
        assert!(yaml_str.contains("key:"));
        assert!(yaml_str.contains("list:"));
    }

    #[test]
    fn test_invalid_yaml_keeps_original() {
        let formatter = YamlFormatter::new(vec!["yaml"]);
        let invalid_yaml = "this is not valid yaml: [[[{";
        let input = json!({
            "yaml": invalid_yaml
        });

        let output = formatter.transform(input.clone()).unwrap();
        // Should keep original value
        assert_eq!(output["yaml"], invalid_yaml);
    }

    #[test]
    fn test_missing_field_silent() {
        let formatter = YamlFormatter::new(vec!["yaml"]);
        let input = json!({
            "id": "wf-123",
            "name": "test"
        });

        let output = formatter.transform(input.clone()).unwrap();
        // Should be unchanged
        assert_eq!(output, input);
    }

    #[test]
    fn test_null_field_silent() {
        let formatter = YamlFormatter::new(vec!["yaml"]);
        let input = json!({
            "yaml": null
        });

        let output = formatter.transform(input.clone()).unwrap();
        // Should be unchanged
        assert_eq!(output, input);
    }

    #[test]
    fn test_empty_string_skipped() {
        let formatter = YamlFormatter::new(vec!["yaml"]);
        let input = json!({
            "yaml": ""
        });

        let output = formatter.transform(input.clone()).unwrap();
        // Should be unchanged (empty strings are skipped)
        assert_eq!(output, input);
    }

    #[test]
    fn test_whitespace_only_skipped() {
        let formatter = YamlFormatter::new(vec!["yaml"]);
        let input = json!({
            "yaml": "   \n  \t  "
        });

        let output = formatter.transform(input.clone()).unwrap();
        // Should be unchanged (whitespace-only strings are skipped)
        assert_eq!(output, input);
    }

    #[test]
    fn test_multiple_fields() {
        let formatter = YamlFormatter::new(vec!["yaml", "definition"]);
        let input = json!({
            "yaml": "key1: value1",
            "definition": "key2: value2"
        });

        let output = formatter.transform(input).unwrap();
        assert!(output["yaml"].is_string());
        assert!(output["definition"].is_string());

        let yaml_str = output["yaml"].as_str().unwrap();
        assert!(yaml_str.contains("key1"));

        let def_str = output["definition"].as_str().unwrap();
        assert!(def_str.contains("key2"));
    }

    #[test]
    fn test_complex_yaml_structure() {
        let formatter = YamlFormatter::new(vec!["yaml"]);
        let complex_yaml = "version: 1.0\nsteps:\n  - name: step1\n    action: run\n    params:\n      timeout: 30\n      retry: 3\n  - name: step2\n    action: deploy\n    targets:\n      - prod\n      - staging";

        let input = json!({
            "yaml": complex_yaml
        });

        let output = formatter.transform(input).unwrap();
        let formatted = output["yaml"].as_str().unwrap();

        // Check key elements are preserved
        assert!(formatted.contains("version"));
        assert!(formatted.contains("steps"));
        assert!(formatted.contains("action"));
        assert!(formatted.contains("params"));
        assert!(formatted.contains("timeout"));
        assert!(formatted.contains("targets"));
    }

    #[test]
    fn test_yaml_with_special_characters() {
        let formatter = YamlFormatter::new(vec!["yaml"]);
        let input = json!({
            "yaml": "message: \"Hello: world\"\npath: '/usr/local/bin'"
        });

        let output = formatter.transform(input).unwrap();
        assert!(output["yaml"].is_string());
        let yaml_str = output["yaml"].as_str().unwrap();
        // Should preserve quoted strings and special characters
        assert!(yaml_str.contains("message"));
        assert!(yaml_str.contains("path"));
    }

    #[test]
    fn test_yaml_with_arrays() {
        let formatter = YamlFormatter::new(vec!["yaml"]);
        let input = json!({
            "yaml": "items:\n  - first\n  - second\n  - third"
        });

        let output = formatter.transform(input).unwrap();
        let yaml_str = output["yaml"].as_str().unwrap();
        assert!(yaml_str.contains("items"));
        assert!(yaml_str.contains("first"));
        assert!(yaml_str.contains("second"));
        assert!(yaml_str.contains("third"));
    }

    #[test]
    fn test_yaml_with_nested_objects() {
        let formatter = YamlFormatter::new(vec!["yaml"]);
        let input = json!({
            "yaml": "server:\n  host: localhost\n  port: 8080\n  ssl:\n    enabled: true\n    cert: /path/to/cert"
        });

        let output = formatter.transform(input).unwrap();
        let yaml_str = output["yaml"].as_str().unwrap();
        assert!(yaml_str.contains("server"));
        assert!(yaml_str.contains("host"));
        assert!(yaml_str.contains("ssl"));
        assert!(yaml_str.contains("enabled"));
    }

    #[test]
    fn test_for_workflows_constructor() {
        let formatter = YamlFormatter::for_workflows();
        let input = json!({
            "yaml": "version: 1.0\nname: test"
        });

        let output = formatter.transform(input).unwrap();
        assert!(output["yaml"].is_string());
        let yaml_str = output["yaml"].as_str().unwrap();
        assert!(yaml_str.contains("version"));
        assert!(yaml_str.contains("name"));
    }

    #[test]
    fn test_for_agents_constructor() {
        let formatter = YamlFormatter::for_agents();
        let input = json!({
            "definition": "type: agent\nconfig:\n  model: gpt-4"
        });

        let output = formatter.transform(input).unwrap();
        assert!(output["definition"].is_string());
        let def_str = output["definition"].as_str().unwrap();
        assert!(def_str.contains("type"));
        assert!(def_str.contains("config"));
    }

    #[test]
    fn test_non_string_field_skipped() {
        let formatter = YamlFormatter::new(vec!["yaml"]);
        let input = json!({
            "yaml": {"key": "value"}  // Already an object, not a string
        });

        let output = formatter.transform(input.clone()).unwrap();
        // Should be unchanged
        assert_eq!(output, input);
    }

    #[test]
    fn test_number_field_skipped() {
        let formatter = YamlFormatter::new(vec!["yaml"]);
        let input = json!({
            "yaml": 12345
        });

        let output = formatter.transform(input.clone()).unwrap();
        // Should be unchanged
        assert_eq!(output, input);
    }

    #[test]
    fn test_deeply_nested_field() {
        let formatter = YamlFormatter::new(vec!["a.b.c.yaml"]);
        let input = json!({
            "a": {
                "b": {
                    "c": {
                        "yaml": "key: value"
                    }
                }
            }
        });

        let output = formatter.transform(input).unwrap();
        assert!(output["a"]["b"]["c"]["yaml"].is_string());
        let yaml_str = output["a"]["b"]["c"]["yaml"].as_str().unwrap();
        assert!(yaml_str.contains("key"));
    }

    #[test]
    fn test_multiple_nested_fields() {
        let formatter = YamlFormatter::new(vec!["spec.yaml", "config.definition"]);
        let input = json!({
            "spec": {
                "yaml": "version: 1.0"
            },
            "config": {
                "definition": "type: custom"
            }
        });

        let output = formatter.transform(input).unwrap();
        assert!(output["spec"]["yaml"].is_string());
        assert!(output["config"]["definition"].is_string());
    }

    #[test]
    fn test_yaml_already_formatted() {
        let formatter = YamlFormatter::new(vec!["yaml"]);
        let formatted_yaml = "version: 1.0\nsteps:\n  - name: step1\n    action: run\n";
        let input = json!({
            "yaml": formatted_yaml
        });

        let output = formatter.transform(input).unwrap();
        // Should still be valid and formatted
        assert!(output["yaml"].is_string());
        let yaml_str = output["yaml"].as_str().unwrap();
        assert!(yaml_str.contains("version"));
        assert!(yaml_str.contains("steps"));
    }

    #[test]
    fn test_roundtrip_consistency() {
        let formatter = YamlFormatter::new(vec!["yaml"]);
        let original = json!({
            "yaml": "key1: value1\nkey2: value2"
        });

        // First format
        let first = formatter.transform(original.clone()).unwrap();
        let first_yaml = first["yaml"].as_str().unwrap();

        // Second format (should be idempotent)
        let second = formatter.transform(first.clone()).unwrap();
        let second_yaml = second["yaml"].as_str().unwrap();

        // Parse both to verify they're semantically equivalent
        let first_parsed: serde_yaml::Value = serde_yaml::from_str(first_yaml).unwrap();
        let second_parsed: serde_yaml::Value = serde_yaml::from_str(second_yaml).unwrap();

        assert_eq!(first_parsed, second_parsed);
    }
}
#[test]
fn debug_yaml_output() {
    use crate::etl::Transformer;
    use crate::transform::YamlFormatter;
    use serde_json::json;

    let input = json!({
        "yaml": "name: Test\nversion: 1.0\nsteps:\n  - step1"
    });

    println!("\n=== INPUT ===");
    println!("{}", serde_json::to_string_pretty(&input).unwrap());

    let formatter = YamlFormatter::for_workflows();
    let output = formatter.transform(input).unwrap();

    println!("\n=== OUTPUT ===");
    println!("{}", serde_json::to_string_pretty(&output).unwrap());

    println!("\n=== YAML FIELD ===");
    println!("{}", output["yaml"].as_str().unwrap());
}
