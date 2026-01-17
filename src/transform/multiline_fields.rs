//! Multiline field formatter for preserving newlines in string fields
//!
//! This transformer ensures that string fields with newlines are preserved as-is,
//! so the json_writer will use triple-quote (""") syntax for better Git diffs.
//!
//! No parsing or validation is performed - this is purely for whitespace preservation.

use crate::etl::Transformer;
use eyre::Result;
use serde_json::Value;

/// Transformer that marks multiline string fields for triple-quote formatting
///
/// This transformer does NOT parse or validate content. It simply ensures that
/// string fields containing newlines are preserved as-is, so the json_writer
/// uses triple-quote syntax for them.
///
/// Used for:
/// - Agent instructions (`configuration.instructions`)
/// - Any other markdown/text content with newlines
#[derive(Debug, Clone)]
pub struct MultilineFieldFormatter {
    /// JSON paths to search for multiline fields (e.g., "configuration.instructions")
    field_paths: Vec<String>,
}

impl MultilineFieldFormatter {
    /// Create a new formatter with specified field paths
    pub fn new(field_paths: Vec<&str>) -> Self {
        Self {
            field_paths: field_paths.iter().map(|s| s.to_string()).collect(),
        }
    }

    /// Create formatter configured for agent instructions
    pub fn for_agents() -> Self {
        Self::new(vec!["configuration.instructions"])
    }

    /// Create formatter configured for tool queries
    pub fn for_tools() -> Self {
        Self::new(vec!["configuration.query"])
    }

    /// Get a nested mutable field by dot-separated path
    fn get_nested_mut<'a>(obj: &'a mut Value, path: &str) -> Option<&'a mut Value> {
        let parts: Vec<&str> = path.split('.').collect();
        let mut current = obj;

        for part in parts {
            current = current.get_mut(part)?;
        }

        Some(current)
    }
}

impl Default for MultilineFieldFormatter {
    fn default() -> Self {
        Self::new(vec![])
    }
}

impl Transformer for MultilineFieldFormatter {
    type Input = Value;
    type Output = Value;

    fn transform(&self, mut input: Self::Input) -> Result<Self::Output> {
        process_multiline_fields(&mut input, &self.field_paths)?;
        Ok(input)
    }
}

/// Process multiline fields in a JSON value
fn process_multiline_fields(value: &mut Value, field_paths: &[String]) -> Result<()> {
    // For each configured field path, try to find and process it
    for path in field_paths {
        if let Some(field_value) = MultilineFieldFormatter::get_nested_mut(value, path) {
            // The field exists - if it's a string with newlines, it will
            // automatically be written with triple-quotes by json_writer.
            // We don't need to do anything here - just log that we found it.
            if let Some(s) = field_value.as_str() {
                if s.contains('\n') {
                    log::debug!(
                        "MultilineFieldFormatter: Found multiline field '{}' with {} chars, will use triple-quote syntax",
                        path,
                        s.len()
                    );
                }
            }
        }
    }

    // Also recursively process arrays in case we have multiple objects
    if let Value::Array(arr) = value {
        for item in arr {
            process_multiline_fields(item, field_paths)?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_agent_instructions_preserved() {
        let instructions = "Your Role: You are an agent.\n\nYour Primary Directive: Help users.\n\nSteps:\n1. First step\n2. Second step";

        let input = json!({
            "id": "agent-1",
            "name": "Test Agent",
            "configuration": {
                "instructions": instructions
            }
        });

        let formatter = MultilineFieldFormatter::for_agents();
        let result = formatter.transform(input.clone()).unwrap();

        // Instructions should be unchanged - we don't modify content
        assert_eq!(
            result["configuration"]["instructions"].as_str().unwrap(),
            instructions
        );

        // The string should still contain newlines
        assert!(
            result["configuration"]["instructions"]
                .as_str()
                .unwrap()
                .contains('\n')
        );
    }

    #[test]
    fn test_missing_field_ignored() {
        let input = json!({
            "id": "agent-1",
            "name": "Test Agent"
        });

        let formatter = MultilineFieldFormatter::for_agents();
        let result = formatter.transform(input.clone()).unwrap();

        // Should be unchanged
        assert_eq!(result, input);
    }

    #[test]
    fn test_custom_field_paths() {
        let input = json!({
            "data": {
                "content": "Line 1\nLine 2\nLine 3"
            }
        });

        let formatter = MultilineFieldFormatter::new(vec!["data.content"]);
        let result = formatter.transform(input.clone()).unwrap();

        // Content should be unchanged
        assert_eq!(
            result["data"]["content"].as_str().unwrap(),
            "Line 1\nLine 2\nLine 3"
        );
    }

    #[test]
    fn test_array_of_objects() {
        let input = json!([
            {
                "id": "agent-1",
                "configuration": {
                    "instructions": "First agent\nMultiple lines"
                }
            },
            {
                "id": "agent-2",
                "configuration": {
                    "instructions": "Second agent\nAlso multiline"
                }
            }
        ]);

        let formatter = MultilineFieldFormatter::for_agents();
        let result = formatter.transform(input).unwrap();

        // Both should be preserved
        assert!(
            result[0]["configuration"]["instructions"]
                .as_str()
                .unwrap()
                .contains('\n')
        );
        assert!(
            result[1]["configuration"]["instructions"]
                .as_str()
                .unwrap()
                .contains('\n')
        );
    }

    #[test]
    fn test_non_string_field_ignored() {
        let input = json!({
            "configuration": {
                "instructions": 123
            }
        });

        let formatter = MultilineFieldFormatter::for_agents();
        let result = formatter.transform(input.clone()).unwrap();

        // Should be unchanged
        assert_eq!(result, input);
    }

    #[test]
    fn test_tool_query_preserved() {
        let query = "FROM settings-ilm-esdiag\n/* Check for hot phases > 30d */\n| WHERE diagnostic.id == ?diagnostic_id\n| LIMIT 10";

        let input = json!({
            "id": "tool-1",
            "type": "esql",
            "configuration": {
                "query": query
            }
        });

        let formatter = MultilineFieldFormatter::for_tools();
        let result = formatter.transform(input.clone()).unwrap();

        // Query should be unchanged - we don't modify content
        assert_eq!(result["configuration"]["query"].as_str().unwrap(), query);

        // The string should still contain newlines
        assert!(
            result["configuration"]["query"]
                .as_str()
                .unwrap()
                .contains('\n')
        );
    }
}
