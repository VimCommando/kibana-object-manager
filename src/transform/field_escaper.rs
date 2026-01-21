//! Field escaper/unescaper transformers
//!
//! Handles conversion between JSON strings and objects for specific fields
//! in Kibana saved objects that store nested JSON as strings.

use crate::etl::Transformer;
use eyre::{Context, Result};
use serde_json::Value;

/// Transformer that escapes specific fields (converts objects to JSON strings)
///
/// Used during import (Files → Kibana) to convert nested JSON objects
/// back into JSON strings that Kibana expects.
///
/// # Example
/// ```
/// use kibana_object_manager::transform::FieldEscaper;
/// use kibana_object_manager::etl::Transformer;
/// use serde_json::json;
///
/// let escaper = FieldEscaper::new(vec!["attributes.visState"]);
/// let input = json!({
///     "attributes": {
///         "visState": {"type": "pie", "params": {}}
///     }
/// });
///
/// let output = escaper.transform(input).unwrap();
/// assert!(output["attributes"]["visState"].is_string());
/// ```
pub struct FieldEscaper {
    fields: Vec<String>,
}

impl FieldEscaper {
    /// Create a new field escaper with specified fields to escape
    pub fn new(fields: Vec<&str>) -> Self {
        Self {
            fields: fields.iter().map(|s| s.to_string()).collect(),
        }
    }

    /// Create a field escaper with default Kibana JSON string fields
    pub fn default_kibana_fields() -> Self {
        Self::new(vec![
            "attributes.panelsJSON",
            "attributes.fieldFormatMap",
            "attributes.controlGroupInput.ignoreParentSettingsJSON",
            "attributes.controlGroupInput.panelsJSON",
            "attributes.kibanaSavedObjectMeta.searchSourceJSON",
            "attributes.optionsJSON",
            "attributes.visState",
            "attributes.fieldAttrs",
        ])
    }

    fn get_nested_mut<'a>(obj: &'a mut Value, path: &str) -> Option<&'a mut Value> {
        let parts: Vec<&str> = path.split('.').collect();
        let mut current = obj;

        for part in parts {
            current = current.get_mut(part)?;
        }

        Some(current)
    }
}

impl Transformer for FieldEscaper {
    type Input = Value;
    type Output = Value;

    fn transform(&self, mut input: Self::Input) -> Result<Self::Output> {
        for field_path in &self.fields {
            if let Some(field) = Self::get_nested_mut(&mut input, field_path) {
                // If it's an object or array, convert to JSON string
                if field.is_object() || field.is_array() {
                    let json_string = serde_json::to_string(field)
                        .with_context(|| format!("Failed to escape field: {}", field_path))?;
                    *field = Value::String(json_string);
                }
            }
        }
        Ok(input)
    }
}

/// Transformer that unescapes specific fields (converts JSON strings to objects)
///
/// Used during export (Kibana → Files) to convert JSON strings into proper
/// nested objects for better readability and version control.
///
/// # Example
/// ```
/// use kibana_object_manager::transform::FieldUnescaper;
/// use kibana_object_manager::etl::Transformer;
/// use serde_json::json;
///
/// let unescaper = FieldUnescaper::new(vec!["attributes.visState"]);
/// let input = json!({
///     "attributes": {
///         "visState": r#"{"type":"pie","params":{}}"#
///     }
/// });
///
/// let output = unescaper.transform(input).unwrap();
/// assert!(output["attributes"]["visState"].is_object());
/// assert_eq!(output["attributes"]["visState"]["type"], "pie");
/// ```
pub struct FieldUnescaper {
    fields: Vec<String>,
}

impl FieldUnescaper {
    /// Create a new field unescaper with specified fields to unescape
    pub fn new(fields: Vec<&str>) -> Self {
        Self {
            fields: fields.iter().map(|s| s.to_string()).collect(),
        }
    }

    /// Create a field unescaper with default Kibana JSON string fields
    pub fn default_kibana_fields() -> Self {
        Self::new(vec![
            "attributes.panelsJSON",
            "attributes.fieldFormatMap",
            "attributes.controlGroupInput.ignoreParentSettingsJSON",
            "attributes.controlGroupInput.panelsJSON",
            "attributes.kibanaSavedObjectMeta.searchSourceJSON",
            "attributes.optionsJSON",
            "attributes.visState",
            "attributes.fieldAttrs",
        ])
    }

    fn get_nested_mut<'a>(obj: &'a mut Value, path: &str) -> Option<&'a mut Value> {
        let parts: Vec<&str> = path.split('.').collect();
        let mut current = obj;

        for part in parts {
            current = current.get_mut(part)?;
        }

        Some(current)
    }
}

impl Transformer for FieldUnescaper {
    type Input = Value;
    type Output = Value;

    fn transform(&self, mut input: Self::Input) -> Result<Self::Output> {
        for field_path in &self.fields {
            if let Some(field) = Self::get_nested_mut(&mut input, field_path) {
                // If it's a string, try to parse as JSON
                if let Some(json_str) = field.as_str() {
                    // Only parse if it looks like JSON (starts with { or [)
                    let trimmed = json_str.trim();
                    if trimmed.starts_with('{') || trimmed.starts_with('[') {
                        match serde_json::from_str(json_str) {
                            Ok(parsed) => *field = parsed,
                            Err(_) => {
                                // If parsing fails, leave as string
                                log::debug!(
                                    "Failed to unescape field {}, leaving as string",
                                    field_path
                                );
                            }
                        }
                    }
                }
            }
        }
        Ok(input)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_escape_field() {
        let escaper = FieldEscaper::new(vec!["attributes.visState"]);
        let input = json!({
            "attributes": {
                "visState": {"type": "pie", "params": {"size": 10}},
                "title": "My Viz"
            }
        });

        let output = escaper.transform(input).unwrap();

        assert!(output["attributes"]["visState"].is_string());
        let vis_state_str = output["attributes"]["visState"].as_str().unwrap();
        let parsed: Value = serde_json::from_str(vis_state_str).unwrap();
        assert_eq!(parsed["type"], "pie");
        assert_eq!(parsed["params"]["size"], 10);
    }

    #[test]
    fn test_unescape_field() {
        let unescaper = FieldUnescaper::new(vec!["attributes.visState"]);
        let input = json!({
            "attributes": {
                "visState": r#"{"type":"pie","params":{"size":10}}"#,
                "title": "My Viz"
            }
        });

        let output = unescaper.transform(input).unwrap();

        assert!(output["attributes"]["visState"].is_object());
        assert_eq!(output["attributes"]["visState"]["type"], "pie");
        assert_eq!(output["attributes"]["visState"]["params"]["size"], 10);
    }

    #[test]
    fn test_roundtrip() {
        let unescaper = FieldUnescaper::new(vec!["attributes.visState"]);
        let escaper = FieldEscaper::new(vec!["attributes.visState"]);

        let original = json!({
            "attributes": {
                "visState": r#"{"type":"pie"}"#
            }
        });

        // Unescape (string -> object)
        let unescaped = unescaper.transform(original.clone()).unwrap();
        assert!(unescaped["attributes"]["visState"].is_object());

        // Escape (object -> string)
        let escaped = escaper.transform(unescaped).unwrap();
        assert!(escaped["attributes"]["visState"].is_string());

        // Should be equivalent to original
        let escaped_str = escaped["attributes"]["visState"].as_str().unwrap();
        let original_str = original["attributes"]["visState"].as_str().unwrap();
        let escaped_parsed: Value = serde_json::from_str(escaped_str).unwrap();
        let original_parsed: Value = serde_json::from_str(original_str).unwrap();
        assert_eq!(escaped_parsed, original_parsed);
    }

    #[test]
    fn test_nested_path() {
        let escaper = FieldEscaper::new(vec!["attributes.controlGroupInput.panelsJSON"]);
        let input = json!({
            "attributes": {
                "controlGroupInput": {
                    "panelsJSON": {"panel1": {"id": "test"}}
                }
            }
        });

        let output = escaper.transform(input).unwrap();
        assert!(output["attributes"]["controlGroupInput"]["panelsJSON"].is_string());
    }

    #[test]
    fn test_missing_field_ignored() {
        let escaper = FieldEscaper::new(vec!["attributes.nonexistent"]);
        let input = json!({"attributes": {"title": "Test"}});

        let output = escaper.transform(input.clone()).unwrap();
        assert_eq!(output, input);
    }

    #[test]
    fn test_already_string_unchanged() {
        let escaper = FieldEscaper::new(vec!["attributes.title"]);
        let input = json!({"attributes": {"title": "Already a string"}});

        let output = escaper.transform(input.clone()).unwrap();
        assert_eq!(output, input);
    }

    #[test]
    fn test_invalid_json_string_unchanged() {
        let unescaper = FieldUnescaper::new(vec!["attributes.visState"]);
        let input = json!({
            "attributes": {
                "visState": "not valid json"
            }
        });

        let output = unescaper.transform(input.clone()).unwrap();
        // Should remain as string since it's not valid JSON
        assert_eq!(output, input);
    }
}
