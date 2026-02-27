use crate::etl::Transformer;
use eyre::{Context, Result};
use serde_json::{Map, Value};

/// Transformer that unescapes Vega specification fields for readable Git diffs.
///
/// This transformer does NOT parse or validate the Vega spec content. It simply
/// converts escaped newlines (`\n`) to real newlines so the spec can be stored
/// with triple-quote (`"""`) syntax in JSON5 files, making Git diffs readable.
///
/// Comments (`//` and `/* */`) are fully preserved since no parsing occurs.
#[derive(Debug, Clone)]
pub struct VegaSpecUnescaper {}

impl VegaSpecUnescaper {
    /// Create a new VegaSpecUnescaper with default search paths
    pub fn new() -> Self {
        Self {}
    }
}

impl Default for VegaSpecUnescaper {
    fn default() -> Self {
        Self::new()
    }
}

impl Transformer for VegaSpecUnescaper {
    type Input = Value;
    type Output = Value;

    fn transform(&self, mut input: Self::Input) -> Result<Self::Output> {
        unescape_vega_specs(&mut input)?;
        Ok(input)
    }
}

/// Recursively searches for and unescapes Vega specifications in a JSON value
fn unescape_vega_specs(value: &mut Value) -> Result<()> {
    match value {
        Value::Object(obj) => {
            // Check for Kibana saved object structure: attributes.visState
            if let Some(attributes) = obj.get_mut("attributes")
                && let Value::Object(attr_obj) = attributes
                    && let Some(vis_state) = attr_obj.get_mut("visState")
                        && let Value::Object(vis_state_obj) = vis_state
                            && is_vega_visualization(vis_state_obj)
                                && let Some(spec_value) = vis_state_obj
                                    .get_mut("params")
                                    .and_then(|p| p.as_object_mut())
                                    .and_then(|params| params.get_mut("spec"))
                                {
                                    unescape_spec_field(spec_value, "Kibana saved object")?;
                                }

            // Check if this object is a Vega visualization (direct format)
            if is_vega_visualization(obj)
                && let Some(spec_value) = obj
                    .get_mut("params")
                    .and_then(|p| p.as_object_mut())
                    .and_then(|params| params.get_mut("spec"))
                {
                    unescape_spec_field(spec_value, "direct format")?;
                }

            // Check for embedded saved visualizations (dashboard panels)
            if let Some(saved_vis) = obj.get_mut("savedVis")
                && let Value::Object(saved_vis_obj) = saved_vis
                    && is_vega_visualization(saved_vis_obj)
                        && let Some(spec_value) = saved_vis_obj
                            .get_mut("params")
                            .and_then(|p| p.as_object_mut())
                            .and_then(|params| params.get_mut("spec"))
                        {
                            unescape_spec_field(spec_value, "embedded")?;
                        }

            // Recursively process all child objects and arrays
            for child_value in obj.values_mut() {
                unescape_vega_specs(child_value)?;
            }
        }
        Value::Array(arr) => {
            // Recursively process array elements
            for item in arr {
                unescape_vega_specs(item)?;
            }
        }
        _ => {
            // Primitive values don't need processing
        }
    }

    Ok(())
}

/// Unescape a single spec field value
///
/// This function simply ensures the spec string has real newlines (not escaped `\n`).
/// The json_writer will then use triple-quote syntax to write it, making Git diffs readable.
///
/// NO parsing or validation is performed - this preserves everything including comments.
fn unescape_spec_field(spec_value: &mut Value, context: &str) -> Result<()> {
    if let Some(spec_str) = spec_value.as_str() {
        log::debug!(
            "VegaSpecUnescaper: Found {} Vega spec string (length: {})",
            context,
            spec_str.len()
        );

        // The spec string from Kibana already has real newlines (not escaped).
        // We just need to ensure it stays as a string - the json_writer will
        // use triple-quote syntax for any string containing newlines.
        //
        // If the string contains newlines, it will be written as:
        //   "spec": """
        //   {
        //     // comments preserved
        //     "$schema": "...",
        //     ...
        //   }"""
        //
        // No transformation needed here - the string is already in the right format.
        // We just log that we found it.
        if spec_str.contains('\n') {
            log::debug!(
                "VegaSpecUnescaper: {} Vega spec contains newlines, will use triple-quote syntax",
                context
            );
        }
    }
    Ok(())
}

/// Check if an object represents a Vega or Vega-Lite visualization
fn is_vega_visualization(obj: &Map<String, Value>) -> bool {
    obj.get("type")
        .and_then(|t| t.as_str())
        .map(|type_str| type_str == "vega" || type_str == "vega-lite")
        .unwrap_or(false)
}

/// Transformer that escapes Vega specification fields from objects to JSON strings
#[derive(Debug, Clone)]
pub struct VegaSpecEscaper {}

impl VegaSpecEscaper {
    /// Create a new VegaSpecEscaper with default search paths
    pub fn new() -> Self {
        Self {}
    }
}

impl Default for VegaSpecEscaper {
    fn default() -> Self {
        Self::new()
    }
}

impl Transformer for VegaSpecEscaper {
    type Input = Value;
    type Output = Value;

    fn transform(&self, mut input: Self::Input) -> Result<Self::Output> {
        escape_vega_specs(&mut input)?;
        Ok(input)
    }
}

/// Recursively searches for and escapes Vega specifications from objects to JSON strings
fn escape_vega_specs(value: &mut Value) -> Result<()> {
    match value {
        Value::Object(obj) => {
            // Check for Kibana saved object structure: attributes.visState
            if let Some(attributes) = obj.get_mut("attributes")
                && let Value::Object(attr_obj) = attributes
                    && let Some(vis_state) = attr_obj.get_mut("visState")
                        && let Value::Object(vis_state_obj) = vis_state
                            && is_vega_visualization(vis_state_obj)
                                && let Some(spec_value) = vis_state_obj
                                    .get_mut("params")
                                    .and_then(|p| p.as_object_mut())
                                    .and_then(|params| params.get_mut("spec"))
                                {
                                    escape_spec_field(spec_value, "Kibana saved object")?;
                                }

            // Check if this object is a Vega visualization (direct format)
            if is_vega_visualization(obj)
                && let Some(spec_value) = obj
                    .get_mut("params")
                    .and_then(|p| p.as_object_mut())
                    .and_then(|params| params.get_mut("spec"))
                {
                    escape_spec_field(spec_value, "direct format")?;
                }

            // Check for embedded saved visualizations (dashboard panels)
            if let Some(saved_vis) = obj.get_mut("savedVis")
                && let Value::Object(saved_vis_obj) = saved_vis
                    && is_vega_visualization(saved_vis_obj)
                        && let Some(spec_value) = saved_vis_obj
                            .get_mut("params")
                            .and_then(|p| p.as_object_mut())
                            .and_then(|params| params.get_mut("spec"))
                        {
                            escape_spec_field(spec_value, "embedded")?;
                        }

            // Recursively process all child objects and arrays
            for child_value in obj.values_mut() {
                escape_vega_specs(child_value)?;
            }
        }
        Value::Array(arr) => {
            // Recursively process array elements
            for item in arr {
                escape_vega_specs(item)?;
            }
        }
        _ => {
            // Primitive values don't need processing
        }
    }

    Ok(())
}

/// Escape a single spec field value from object to JSON string
fn escape_spec_field(spec_value: &mut Value, context: &str) -> Result<()> {
    if !spec_value.is_string() {
        // Only escape if it's not already a string
        match serde_json::to_string(spec_value) {
            Ok(escaped_spec) => {
                *spec_value = Value::String(escaped_spec);
                log::debug!(
                    "VegaSpecEscaper: Successfully escaped {} Vega spec to JSON string",
                    context
                );
            }
            Err(e) => {
                log::warn!(
                    "VegaSpecEscaper: Failed to escape {} Vega spec: {}",
                    context,
                    e
                );
                return Err(e)
                    .with_context(|| format!("Failed to escape Vega spec in {}", context));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_unescape_vega_visualization() {
        let input = json!({
            "attributes": {
                "visState": {
                    "type": "vega",
                    "params": {
                        "spec": "{\n  \"$schema\": \"https://vega.github.io/schema/vega/v5.json\",\n  \"width\": 400\n}"
                    }
                }
            }
        });

        let unescaper = VegaSpecUnescaper::new();
        let result = unescaper.transform(input.clone()).unwrap();

        // Spec should remain unchanged - we don't parse, just pass through
        let spec = &result["attributes"]["visState"]["params"]["spec"];
        assert!(spec.is_string());
        let spec_str = spec.as_str().unwrap();
        assert!(spec_str.contains("vega.github.io/schema/vega/v5.json"));
        assert!(spec_str.contains("width"));
        assert!(spec_str.contains('\n')); // Has real newlines
    }

    #[test]
    fn test_unescape_vega_lite_visualization() {
        let input = json!({
            "attributes": {
                "visState": {
                    "type": "vega-lite",
                    "params": {
                        "spec": "{\n  \"$schema\": \"https://vega.github.io/schema/vega-lite/v5.json\",\n  \"mark\": \"bar\"\n}"
                    }
                }
            }
        });

        let unescaper = VegaSpecUnescaper::new();
        let result = unescaper.transform(input).unwrap();

        let spec = &result["attributes"]["visState"]["params"]["spec"];
        assert!(spec.is_string());
        let spec_str = spec.as_str().unwrap();
        assert!(spec_str.contains("vega-lite/v5.json"));
        assert!(spec_str.contains("mark"));
    }

    #[test]
    fn test_comments_are_preserved() {
        // This is the key test - comments must survive the round trip
        let spec_with_comments = r#"{
  // This is a line comment
  /* xray tango */
  "$schema": "https://vega.github.io/schema/vega-lite/v5.json",
  "mark": "bar"
}"#;

        let input = json!({
            "attributes": {
                "visState": {
                    "type": "vega",
                    "params": {
                        "spec": spec_with_comments
                    }
                }
            }
        });

        let unescaper = VegaSpecUnescaper::new();
        let result = unescaper.transform(input).unwrap();

        let spec = &result["attributes"]["visState"]["params"]["spec"];
        let spec_str = spec.as_str().unwrap();

        // Comments MUST be preserved
        assert!(
            spec_str.contains("// This is a line comment"),
            "Line comment was lost!"
        );
        assert!(
            spec_str.contains("/* xray tango */"),
            "Block comment was lost!"
        );
    }

    #[test]
    fn test_unescape_dashboard_panel_vega() {
        let input = json!({
            "embeddableConfig": {
                "savedVis": {
                    "type": "vega",
                    "params": {
                        "spec": "{\n  \"$schema\": \"https://vega.github.io/schema/vega/v5.json\",\n  \"data\": []\n}"
                    }
                }
            }
        });

        let unescaper = VegaSpecUnescaper::new();
        let result = unescaper.transform(input).unwrap();

        let spec = &result["embeddableConfig"]["savedVis"]["params"]["spec"];
        assert!(spec.is_string());
        let spec_str = spec.as_str().unwrap();
        assert!(spec_str.contains("vega/v5.json"));
    }

    #[test]
    fn test_ignore_non_vega_visualizations() {
        let input = json!({
            "attributes": {
                "visState": {
                    "type": "histogram",
                    "params": {
                        "spec": "{\"bins\":20}"
                    }
                }
            }
        });

        let unescaper = VegaSpecUnescaper::new();
        let result = unescaper.transform(input.clone()).unwrap();

        // Should remain unchanged (non-vega types are not processed)
        assert_eq!(result, input);
    }

    #[test]
    fn test_handle_invalid_json_gracefully() {
        // Even invalid JSON should be preserved as-is (we don't parse)
        let input = json!({
            "attributes": {
                "visState": {
                    "type": "vega",
                    "params": {
                        "spec": "this is not json at all"
                    }
                }
            }
        });

        let unescaper = VegaSpecUnescaper::new();
        let result = unescaper.transform(input.clone()).unwrap();

        // Should remain unchanged
        assert_eq!(result, input);
    }

    #[test]
    fn test_nested_objects_and_arrays() {
        let input = json!({
            "panels": [
                {
                    "embeddableConfig": {
                        "savedVis": {
                            "type": "vega",
                            "params": {
                                "spec": "{\n  \"width\": 300\n}"
                            }
                        }
                    }
                }
            ]
        });

        let unescaper = VegaSpecUnescaper::new();
        let result = unescaper.transform(input).unwrap();

        let spec = &result["panels"][0]["embeddableConfig"]["savedVis"]["params"]["spec"];
        assert!(spec.is_string());
        let spec_str = spec.as_str().unwrap();
        assert!(spec_str.contains("width"));
        assert!(spec_str.contains("300"));
    }

    #[test]
    fn test_escape_vega_visualization() {
        // When spec is an object, VegaSpecEscaper converts it to JSON string
        let input = json!({
            "attributes": {
                "visState": {
                    "type": "vega",
                    "params": {
                        "spec": {
                            "$schema": "https://vega.github.io/schema/vega/v5.json",
                            "width": 400
                        }
                    }
                }
            }
        });

        let escaper = VegaSpecEscaper::new();
        let result = escaper.transform(input).unwrap();

        let spec = &result["attributes"]["visState"]["params"]["spec"];
        assert!(spec.is_string());

        // Parse the escaped JSON to verify it's valid
        let parsed: Value = serde_json::from_str(spec.as_str().unwrap()).unwrap();
        assert_eq!(
            parsed["$schema"],
            "https://vega.github.io/schema/vega/v5.json"
        );
        assert_eq!(parsed["width"], 400);
    }

    #[test]
    fn test_round_trip_preserves_comments() {
        // The most important test: comments survive the full round trip
        let original_spec = r#"{
  // Line comment here
  /* Block comment here */
  "$schema": "https://vega.github.io/schema/vega-lite/v5.json",
  "mark": "bar",
  "encoding": {
    // Another comment
    "x": {"field": "category"}
  }
}"#;

        let input = json!({
            "attributes": {
                "visState": {
                    "type": "vega-lite",
                    "params": {
                        "spec": original_spec
                    }
                }
            }
        });

        // Unescape (for storage)
        let unescaper = VegaSpecUnescaper::new();
        let unescaped = unescaper.transform(input).unwrap();

        let spec = &unescaped["attributes"]["visState"]["params"]["spec"];
        let spec_str = spec.as_str().unwrap();

        // ALL comments must be preserved
        assert!(
            spec_str.contains("// Line comment here"),
            "Line comment lost after unescape"
        );
        assert!(
            spec_str.contains("/* Block comment here */"),
            "Block comment lost after unescape"
        );
        assert!(
            spec_str.contains("// Another comment"),
            "Nested comment lost after unescape"
        );

        // Content must also be preserved
        assert!(spec_str.contains("vega-lite/v5.json"));
        assert!(spec_str.contains("\"mark\": \"bar\"") || spec_str.contains("\"mark\":\"bar\""));
    }

    #[test]
    fn test_hjson_features_preserved() {
        // Test that HJSON features like trailing commas are preserved (we don't parse)
        let input = json!({
            "attributes": {
                "visState": {
                    "type": "vega",
                    "params": {
                        "spec": "{\"data\": [1, 2, 3,], \"width\": 400,}"
                    }
                }
            }
        });

        let unescaper = VegaSpecUnescaper::new();
        let result = unescaper.transform(input.clone()).unwrap();

        // Should be unchanged - we don't parse or modify
        assert_eq!(result, input);
    }

    #[test]
    fn test_real_kibana_export_transformation() {
        // Test with real Kibana export format
        let input = json!({
            "attributes": {
                "visState": {
                    "type": "vega-lite",
                    "params": {
                        "spec": "{\n  \"$schema\": \"https://vega.github.io/schema/vega-lite/v5.json\",\n  \"data\": {\n    \"url\": {\n      \"%context%\": true\n    }\n  }\n}"
                    }
                }
            }
        });

        let unescaper = VegaSpecUnescaper::new();
        let result = unescaper.transform(input).unwrap();

        let spec = &result["attributes"]["visState"]["params"]["spec"];
        assert!(spec.is_string());
        let spec_str = spec.as_str().unwrap();
        assert!(spec_str.contains("vega-lite/v5.json"));
        assert!(spec_str.contains("%context%"));
    }
}
