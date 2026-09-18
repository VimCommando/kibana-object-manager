//! JSON5 parsing for project-managed JSON resources.
//!
//! In addition to standard JSON5 syntax, this accepts the triple-quoted
//! multiline strings emitted by the project's JSON writer.

use crate::{Error, Result};
use serde_json::Value;

/// Parse JSON5 with support for triple-quoted multiline strings.
pub fn from_json5_str(input: &str) -> Result<Value> {
    let normalized = normalize_triple_quotes(input);
    json5::from_str(&normalized)
        .map_err(|error| Error::message(format!("Failed to parse JSON5: {error}")))
}

/// Convert triple-quoted strings to regular JSON strings before JSON5 parsing.
fn normalize_triple_quotes(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch != '"' {
            result.push(ch);
            continue;
        }

        if chars.peek() != Some(&'"') {
            result.push(ch);
            continue;
        }
        chars.next();
        if chars.peek() != Some(&'"') {
            result.push_str("\"\"");
            continue;
        }
        chars.next();

        let mut content = String::new();
        let mut pending_quotes = Vec::new();
        while let Some(character) = chars.next() {
            if character == '"' {
                pending_quotes.push('"');
                if pending_quotes.len() >= 3 {
                    if chars.peek() == Some(&'"') {
                        continue;
                    }
                    pending_quotes.truncate(pending_quotes.len() - 3);
                    content.extend(pending_quotes);
                    break;
                }
            } else {
                content.extend(pending_quotes.drain(..));
                content.push(character);
            }
        }

        result.push('"');
        for character in content.chars() {
            match character {
                '"' => result.push_str(r#"\""#),
                '\\' => result.push_str(r"\\"),
                '\n' => result.push_str(r"\n"),
                '\r' => result.push_str(r"\r"),
                '\t' => result.push_str(r"\t"),
                character if character.is_control() => {
                    result.push_str(&format!("\\u{:04x}", character as u32));
                }
                character => result.push(character),
            }
        }
        result.push('"');
    }

    result
}

#[cfg(test)]
mod tests {
    use super::from_json5_str;

    #[test]
    fn parses_json5_and_triple_quoted_strings() {
        let value = from_json5_str(
            r#"{
                // A comment
                unquoted: "value",
                multiline: """first line
second line""",
            }"#,
        )
        .unwrap();

        assert_eq!(value["unquoted"], "value");
        assert_eq!(value["multiline"], "first line\nsecond line");
    }
}
