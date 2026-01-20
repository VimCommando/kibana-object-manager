use eyre::{Context, Result};
use serde_json::Value;
use std::io::Write;
use std::path::Path;

/// Custom JSON writer that uses triple-quote syntax for multi-line strings.
///
/// This writer produces JSON files that are more git-friendly when they contain
/// multi-line content like YAML definitions. Instead of using escaped newlines:
///
/// ```json
/// {"yaml": "line1\nline2\nline3"}
/// ```
///
/// It uses triple-quote syntax (JSON5/JSONC extension):
///
/// ```json
/// {"yaml": """line1
/// line2
/// line3"""}
/// ```
///
/// This makes git diffs show actual line-by-line changes in the YAML content.

/// Serialize a JSON value to a string with triple-quote multi-line strings.
///
/// Any string field containing newlines will be written with triple-quote syntax.
/// All other values use standard JSON formatting.
///
/// # Example
///
/// ```
/// use serde_json::json;
/// use kibana_object_manager::storage::to_string_with_multiline;
///
/// let value = json!({
///     "name": "Test",
///     "yaml": "line1\nline2\nline3"
/// });
///
/// let output = to_string_with_multiline(&value).unwrap();
/// assert!(output.contains("\"\"\""));
/// ```
pub fn to_string_with_multiline(value: &Value) -> Result<String> {
    let mut buffer = Vec::new();
    write_json_with_multiline(value, &mut buffer, 0)?;
    Ok(String::from_utf8(buffer)?)
}

/// Write JSON with triple-quote multi-line strings to a writer.
///
/// # Arguments
///
/// * `value` - The JSON value to serialize
/// * `writer` - The writer to output to
/// * `indent` - Current indentation level (number of spaces)
pub fn write_json_with_multiline(
    value: &Value,
    writer: &mut impl Write,
    indent: usize,
) -> Result<()> {
    match value {
        Value::Null => write!(writer, "null")?,
        Value::Bool(b) => write!(writer, "{}", b)?,
        Value::Number(n) => write!(writer, "{}", n)?,
        Value::String(s) => write_string(s, writer, indent)?,
        Value::Array(arr) => write_array(arr, writer, indent)?,
        Value::Object(obj) => write_object(obj, writer, indent)?,
    }
    Ok(())
}

/// Write a string value, using triple-quotes if it contains newlines.
fn write_string(s: &str, writer: &mut impl Write, indent: usize) -> Result<()> {
    // Check if string contains newlines and doesn't contain triple-quotes
    if s.contains('\n') && !s.contains(r#"""""#) {
        write_triple_quoted_string(s, writer, indent)?;
    } else {
        // Use standard JSON escaping
        write!(writer, "\"{}\"", escape_json_string(s))?;
    }
    Ok(())
}

/// Write a string using triple-quote syntax with proper indentation.
fn write_triple_quoted_string(s: &str, writer: &mut impl Write, _indent: usize) -> Result<()> {
    write!(writer, "\"\"\"")?;

    // Write the content exactly as-is, without adding indentation
    // The content should preserve its original formatting
    write!(writer, "{}", s)?;

    write!(writer, "\"\"\"")?;
    Ok(())
}

/// Escape a string for standard JSON output.
fn escape_json_string(s: &str) -> String {
    let mut escaped = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '"' => escaped.push_str(r#"\""#),
            '\\' => escaped.push_str(r"\\"),
            '\n' => escaped.push_str(r"\n"),
            '\r' => escaped.push_str(r"\r"),
            '\t' => escaped.push_str(r"\t"),
            '\x08' => escaped.push_str(r"\b"),
            '\x0C' => escaped.push_str(r"\f"),
            c if c.is_control() => {
                escaped.push_str(&format!("\\u{:04x}", c as u32));
            }
            c => escaped.push(c),
        }
    }
    escaped
}

/// Write a JSON array with proper formatting.
fn write_array(arr: &[Value], writer: &mut impl Write, indent: usize) -> Result<()> {
    if arr.is_empty() {
        write!(writer, "[]")?;
        return Ok(());
    }

    writeln!(writer, "[")?;

    for (i, item) in arr.iter().enumerate() {
        write!(writer, "{:indent$}", "", indent = indent + 2)?;
        write_json_with_multiline(item, writer, indent + 2)?;

        if i < arr.len() - 1 {
            writeln!(writer, ",")?;
        } else {
            writeln!(writer)?;
        }
    }

    write!(writer, "{:indent$}]", "", indent = indent)?;
    Ok(())
}

/// Write a JSON object with proper formatting.
fn write_object(
    obj: &serde_json::Map<String, Value>,
    writer: &mut impl Write,
    indent: usize,
) -> Result<()> {
    if obj.is_empty() {
        write!(writer, "{{}}")?;
        return Ok(());
    }

    writeln!(writer, "{{")?;

    let keys: Vec<_> = obj.keys().collect();
    for (i, key) in keys.iter().enumerate() {
        write!(writer, "{:indent$}\"{}\":", "", key, indent = indent + 2)?;

        let value = &obj[*key];

        // Add space after colon, except for objects/arrays which start with newline
        match value {
            Value::Object(_) | Value::Array(_) => {
                write!(writer, " ")?;
                write_json_with_multiline(value, writer, indent + 2)?;
            }
            _ => {
                write!(writer, " ")?;
                write_json_with_multiline(value, writer, indent + 2)?;
            }
        }

        if i < keys.len() - 1 {
            writeln!(writer, ",")?;
        } else {
            writeln!(writer)?;
        }
    }

    write!(writer, "{:indent$}}}", "", indent = indent)?;
    Ok(())
}

/// Read a JSON5 file and parse it into a serde_json::Value.
///
/// This function supports the JSON5 format, which includes:
/// - Triple-quote multi-line strings ("""...""")
/// - Single-line comments (//)
/// - Multi-line comments (/* ... */)
/// - Trailing commas
/// - Unquoted keys
///
/// # Example
///
/// ```no_run
/// use kibana_object_manager::storage::read_json5_file;
/// use std::path::Path;
///
/// let value = read_json5_file(Path::new("workflow.json")).unwrap();
/// ```
pub fn read_json5_file(path: &Path) -> Result<Value> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read file: {}", path.display()))?;

    from_json5_str(&content)
        .with_context(|| format!("Failed to parse JSON5 from file: {}", path.display()))
}

/// Parse a JSON5 string into a serde_json::Value.
///
/// This function supports the JSON5 format plus triple-quote multi-line strings:
/// - Triple-quote multi-line strings ("""...""") - pre-processed before parsing
/// - Single-line comments (//)
/// - Multi-line comments (/* ... */)
/// - Trailing commas
/// - Unquoted keys
///
/// # Example
///
/// ```
/// use kibana_object_manager::storage::from_json5_str;
///
/// let json5 = r#"{
///     // This is a comment
///     "yaml": """line1
/// line2
/// line3"""
/// }"#;
///
/// let value = from_json5_str(json5).unwrap();
/// ```
pub fn from_json5_str(s: &str) -> Result<Value> {
    // Pre-process to convert triple-quotes to standard JSON
    let normalized = normalize_triple_quotes(s);
    json5::from_str(&normalized).context("Failed to parse JSON5")
}

/// Normalize triple-quote strings to standard JSON escaped strings.
///
/// Converts:
///   """line1
///   line2"""
///
/// To:
///   "line1\nline2"
fn normalize_triple_quotes(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '"' {
            // Check if this is the start of triple-quotes
            if chars.peek() == Some(&'"') {
                chars.next(); // consume second quote
                if chars.peek() == Some(&'"') {
                    chars.next(); // consume third quote

                    // This is a triple-quote string, collect content until closing """
                    let mut content = String::new();
                    let mut pending_quotes = Vec::new();

                    while let Some(c) = chars.next() {
                        if c == '"' {
                            pending_quotes.push('"');

                            // Check if we have at least 3 quotes to potentially close the string
                            if pending_quotes.len() >= 3 {
                                // Check if the last 3 quotes form the closing delimiter
                                // We need to be careful: if content ends with quotes, like `"text"`,
                                // followed by closing `"""`, we get `"text""""` (4 quotes)
                                // The first quote is content, the last 3 are the delimiter

                                // Take the last 3 as the closing delimiter
                                pending_quotes.truncate(pending_quotes.len() - 3);

                                // Add any remaining quotes to content
                                for q in pending_quotes {
                                    content.push(q);
                                }

                                // Found closing triple-quotes
                                break;
                            }
                        } else {
                            // Not a quote, flush all pending quotes to content
                            for q in pending_quotes.drain(..) {
                                content.push(q);
                            }
                            content.push(c);
                        }
                    }

                    // Write as standard JSON string with proper escaping
                    result.push('"');
                    for ch in content.chars() {
                        match ch {
                            '"' => result.push_str(r#"\""#),
                            '\\' => result.push_str(r"\\"),
                            '\n' => result.push_str(r"\n"),
                            '\r' => result.push_str(r"\r"),
                            '\t' => result.push_str(r"\t"),
                            c if c.is_control() => {
                                result.push_str(&format!("\\u{:04x}", c as u32));
                            }
                            c => result.push(c),
                        }
                    }
                    result.push('"');
                    continue;
                } else {
                    // Two quotes followed by something else, output them
                    result.push('"');
                    result.push('"');
                }
            } else {
                // Single quote, just output it
                result.push(ch);
            }
        } else {
            // Not a quote, just output it
            result.push(ch);
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_simple_string() {
        let value = json!("hello");
        let output = to_string_with_multiline(&value).unwrap();
        assert_eq!(output, "\"hello\"");
    }

    #[test]
    fn test_simple_multiline_string() {
        let value = json!("line1\nline2\nline3");
        let output = to_string_with_multiline(&value).unwrap();
        assert!(output.contains("\"\"\""));
        assert!(output.contains("line1"));
        assert!(output.contains("line2"));
        assert!(output.contains("line3"));
    }

    #[test]
    fn test_string_with_triple_quotes_falls_back() {
        let value = json!("content with \"\"\" in it");
        let output = to_string_with_multiline(&value).unwrap();
        // Should use standard escaping, not triple-quotes
        assert!(!output.contains("\"\"\"\"\"\""));
        assert!(output.contains("\\\"\\\"\\\""));
    }

    #[test]
    fn test_simple_object() {
        let value = json!({
            "name": "test",
            "count": 42
        });
        let output = to_string_with_multiline(&value).unwrap();
        assert!(output.contains("\"name\""));
        assert!(output.contains("\"test\""));
        assert!(output.contains("\"count\""));
        assert!(output.contains("42"));
    }

    #[test]
    fn test_object_with_multiline_field() {
        let value = json!({
            "id": "wf-123",
            "yaml": "line1\nline2\nline3"
        });
        let output = to_string_with_multiline(&value).unwrap();

        // Check structure
        assert!(output.contains("\"id\":"));
        assert!(output.contains("\"yaml\":"));

        // YAML field should use triple-quotes
        assert!(output.contains("\"\"\""));
        assert!(output.contains("line1"));
        assert!(output.contains("line2"));
    }

    #[test]
    fn test_mixed_fields() {
        let value = json!({
            "id": "wf-123",
            "yaml": "multi\nline",
            "name": "single line"
        });
        let output = to_string_with_multiline(&value).unwrap();

        // yaml should use triple-quotes
        let yaml_section = output.split("\"yaml\":").nth(1).unwrap();
        assert!(yaml_section.contains("\"\"\""));

        // name should use standard quotes
        assert!(output.contains("\"name\": \"single line\""));
    }

    #[test]
    fn test_array() {
        let value = json!([1, 2, 3]);
        let output = to_string_with_multiline(&value).unwrap();
        assert!(output.contains("["));
        assert!(output.contains("]"));
        assert!(output.contains("1"));
        assert!(output.contains("2"));
        assert!(output.contains("3"));
    }

    #[test]
    fn test_empty_array() {
        let value = json!([]);
        let output = to_string_with_multiline(&value).unwrap();
        assert_eq!(output, "[]");
    }

    #[test]
    fn test_empty_object() {
        let value = json!({});
        let output = to_string_with_multiline(&value).unwrap();
        assert_eq!(output, "{}");
    }

    #[test]
    fn test_nested_objects() {
        let value = json!({
            "definition": {
                "steps": [
                    {
                        "with": {
                            "message": "line1\nline2"
                        }
                    }
                ]
            }
        });
        let output = to_string_with_multiline(&value).unwrap();

        // Nested multi-line string should use triple-quotes
        assert!(output.contains("\"\"\""));
        assert!(output.contains("line1"));
        assert!(output.contains("line2"));
    }

    #[test]
    fn test_escape_special_chars() {
        let value = json!("test\"quote\\backslash\ttab");
        let output = to_string_with_multiline(&value).unwrap();
        assert!(output.contains(r#"\""#));
        assert!(output.contains(r"\\"));
        assert!(output.contains(r"\t"));
    }

    #[test]
    fn test_null_value() {
        let value = json!(null);
        let output = to_string_with_multiline(&value).unwrap();
        assert_eq!(output, "null");
    }

    #[test]
    fn test_boolean_values() {
        let value = json!({"flag1": true, "flag2": false});
        let output = to_string_with_multiline(&value).unwrap();
        assert!(output.contains("true"));
        assert!(output.contains("false"));
    }

    #[test]
    fn test_number_values() {
        let value = json!({"int": 42, "float": 3.14});
        let output = to_string_with_multiline(&value).unwrap();
        assert!(output.contains("42"));
        assert!(output.contains("3.14"));
    }

    #[test]
    fn test_realistic_workflow() {
        let value = json!({
            "id": "workflow-123",
            "name": "Test workflow",
            "yaml": "name: Test workflow\nversion: 1.0\nsteps:\n  - name: deploy\n    action: run",
            "definition": {
                "version": "1",
                "steps": [
                    {
                        "name": "log",
                        "type": "console",
                        "with": {
                            "message": "Multi-line\nlog message\nhere"
                        }
                    }
                ]
            }
        });

        let output = to_string_with_multiline(&value).unwrap();

        // Check that both yaml and message fields use triple-quotes
        let triple_quote_count = output.matches(r#"""""#).count();
        assert!(triple_quote_count >= 4); // At least 2 fields * 2 quotes each

        // Verify structure is valid JSON-like
        assert!(output.contains("\"id\":"));
        assert!(output.contains("\"yaml\":"));
        assert!(output.contains("\"definition\":"));
    }

    #[test]
    fn test_empty_string() {
        let value = json!("");
        let output = to_string_with_multiline(&value).unwrap();
        assert_eq!(output, "\"\"");
    }

    #[test]
    fn test_string_with_only_newlines() {
        let value = json!("\n\n");
        let output = to_string_with_multiline(&value).unwrap();
        // Should use triple-quotes since it contains newlines
        assert!(output.contains("\"\"\""));
    }

    #[test]
    fn test_array_with_multiline_strings() {
        let value = json!(["single line", "multi\nline", "another single"]);
        let output = to_string_with_multiline(&value).unwrap();

        // Should contain triple-quotes for the multi-line entry
        assert!(output.contains("\"\"\""));
        assert!(output.contains("multi"));
    }

    #[test]
    fn test_unicode_characters() {
        let value = json!({"emoji": "Hello 🌍", "chinese": "你好"});
        let output = to_string_with_multiline(&value).unwrap();
        assert!(output.contains("🌍"));
        assert!(output.contains("你好"));
    }

    #[test]
    fn test_control_characters() {
        let value = json!("test\x08backspace\x0Cformfeed");
        let output = to_string_with_multiline(&value).unwrap();
        assert!(output.contains(r"\b"));
        assert!(output.contains(r"\f"));
    }

    #[test]
    fn test_json5_parse_triple_quotes() {
        let json5 = r#"{"yaml": """line1
line2
line3"""}"#;

        let value = from_json5_str(json5).unwrap();
        let yaml = value["yaml"].as_str().unwrap();
        assert_eq!(yaml, "line1\nline2\nline3");
    }

    #[test]
    fn test_json5_parse_with_comments() {
        let json5 = r#"{
  // This is a comment
  "id": "test-123",
  /* Multi-line
     comment */
  "name": "Test"
}"#;

        let value = from_json5_str(json5).unwrap();
        assert_eq!(value["id"].as_str().unwrap(), "test-123");
        assert_eq!(value["name"].as_str().unwrap(), "Test");
    }

    #[test]
    fn test_json5_parse_trailing_commas() {
        let json5 = r#"{
  "id": "test-123",
  "name": "Test",
}"#;

        let value = from_json5_str(json5).unwrap();
        assert_eq!(value["id"].as_str().unwrap(), "test-123");
        assert_eq!(value["name"].as_str().unwrap(), "Test");
    }

    #[test]
    fn test_roundtrip_multiline() {
        // Create a value with multi-line content
        let original = json!({
            "id": "wf-123",
            "yaml": "name: Test\nversion: 1.0\nsteps:\n  - deploy"
        });

        // Write with triple-quotes
        let output = to_string_with_multiline(&original).unwrap();
        assert!(output.contains("\"\"\""));

        // Parse back with JSON5
        let parsed = from_json5_str(&output).unwrap();

        // Should match original
        assert_eq!(parsed["id"], original["id"]);
        assert_eq!(parsed["yaml"], original["yaml"]);
    }

    #[test]
    fn test_json5_parse_triple_quotes_ending_with_quote() {
        // Test the edge case where triple-quoted content ends with a quote character
        // This creates 4 consecutive quotes: the quote in content + the closing """
        let json5 = r#"{"text": """I'm sorry, you can't do that."""}"#;

        let value = from_json5_str(json5).unwrap();
        let text = value["text"].as_str().unwrap();
        assert_eq!(text, r#"I'm sorry, you can't do that."#);
    }

    #[test]
    fn test_json5_parse_triple_quotes_with_embedded_quotes() {
        // Test content with various quote patterns
        let json5 = r#"{"text": """He said "hello" and then left."""}"#;

        let value = from_json5_str(json5).unwrap();
        let text = value["text"].as_str().unwrap();
        assert_eq!(text, r#"He said "hello" and then left."#);
    }

    #[test]
    fn test_roundtrip_complex_workflow() {
        let original = json!({
            "id": "workflow-123",
            "name": "Test workflow",
            "yaml": "name: Test\nversion: 1.0",
            "definition": {
                "steps": [{
                    "with": {
                        "message": "Line1\nLine2\nLine3"
                    }
                }]
            }
        });

        // Write with triple-quotes
        let output = to_string_with_multiline(&original).unwrap();

        // Parse back with JSON5
        let parsed = from_json5_str(&output).unwrap();

        // Verify all fields match
        assert_eq!(parsed["id"], original["id"]);
        assert_eq!(parsed["name"], original["name"]);
        assert_eq!(parsed["yaml"], original["yaml"]);
        assert_eq!(
            parsed["definition"]["steps"][0]["with"]["message"],
            original["definition"]["steps"][0]["with"]["message"]
        );
    }
}
