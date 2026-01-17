use crate::etl::Transformer;
use eyre::{Context, Result};
use serde_json::{Map, Value};

/// Transformer that unescapes Vega specification fields from JSON strings to objects
#[derive(Debug, Clone)]
pub struct VegaSpecUnescaper {
    /// JSON paths to search for vega specs
    #[allow(dead_code)]
    search_paths: Vec<String>,
}

impl VegaSpecUnescaper {
    /// Create a new VegaSpecUnescaper with default search paths
    pub fn new() -> Self {
        Self {
            search_paths: vec![
                "*.params.spec".to_string(),
                "attributes.visState.params.spec".to_string(),
            ],
        }
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
            if let Some(attributes) = obj.get_mut("attributes") {
                if let Value::Object(attr_obj) = attributes {
                    if let Some(vis_state) = attr_obj.get_mut("visState") {
                        if let Value::Object(vis_state_obj) = vis_state {
                            if is_vega_visualization(vis_state_obj) {
                                if let Some(spec_value) = vis_state_obj
                                    .get_mut("params")
                                    .and_then(|p| p.as_object_mut())
                                    .and_then(|params| params.get_mut("spec"))
                                {
                                    unescape_spec_field(spec_value, "Kibana saved object")?;
                                }
                            }
                        }
                    }
                }
            }

            // Check if this object is a Vega visualization (direct format)
            if is_vega_visualization(obj) {
                if let Some(spec_value) = obj
                    .get_mut("params")
                    .and_then(|p| p.as_object_mut())
                    .and_then(|params| params.get_mut("spec"))
                {
                    unescape_spec_field(spec_value, "direct format")?;
                }
            }

            // Check for embedded saved visualizations (dashboard panels)
            if let Some(saved_vis) = obj.get_mut("savedVis") {
                if let Value::Object(saved_vis_obj) = saved_vis {
                    if is_vega_visualization(saved_vis_obj) {
                        if let Some(spec_value) = saved_vis_obj
                            .get_mut("params")
                            .and_then(|p| p.as_object_mut())
                            .and_then(|params| params.get_mut("spec"))
                        {
                            unescape_spec_field(spec_value, "embedded")?;
                        }
                    }
                }
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
fn unescape_spec_field(spec_value: &mut Value, context: &str) -> Result<()> {
    if let Some(spec_str) = spec_value.as_str() {
        log::debug!(
            "VegaSpecUnescaper: Found {} Vega spec string to unescape (length: {})",
            context,
            spec_str.len()
        );

        // Check if this is already a plain JSON string (after FieldUnescaper)
        // or an escaped JSON string that needs parsing
        let spec_to_parse = spec_str.trim();

        if spec_to_parse.starts_with('{') || spec_to_parse.starts_with('[') {
            // This looks like a plain JSON string (already unescaped by FieldUnescaper)
            // Use HJSON parser which handles Kibana's Vega spec format:
            // - Optional quotes
            // - Single or double quotes
            // - Optional commas (including trailing commas)
            // - Comments (// and /* */)
            // - Multiline strings
            if let Ok(parsed_spec) = serde_hjson::from_str::<Value>(spec_str) {
                *spec_value = parsed_spec;
                log::debug!(
                    "VegaSpecUnescaper: Successfully parsed {} Vega spec",
                    context
                );
            } else {
                // Vega specs can contain syntax that even HJSON can't parse
                // (e.g., Vega expressions). Leave as-is.
                log::debug!(
                    "VegaSpecUnescaper: {} Vega spec contains non-standard syntax, leaving as string",
                    context
                );
            }
        } else {
            // This appears to be an escaped JSON string - try to parse with HJSON
            // If it fails, it's likely not valid, so leave as-is
            if let Ok(parsed_spec) = serde_hjson::from_str::<Value>(spec_str) {
                *spec_value = parsed_spec;
                log::debug!(
                    "VegaSpecUnescaper: Successfully unescaped {} Vega spec",
                    context
                );
            } else {
                log::debug!(
                    "VegaSpecUnescaper: {} Vega spec is not valid HJSON, leaving as string",
                    context
                );
            }
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
pub struct VegaSpecEscaper {
    /// JSON paths to search for vega specs (should match VegaSpecUnescaper)
    #[allow(dead_code)]
    search_paths: Vec<String>,
}

impl VegaSpecEscaper {
    /// Create a new VegaSpecEscaper with default search paths
    pub fn new() -> Self {
        Self {
            search_paths: vec![
                "*.params.spec".to_string(),
                "attributes.visState.params.spec".to_string(),
            ],
        }
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
            if let Some(attributes) = obj.get_mut("attributes") {
                if let Value::Object(attr_obj) = attributes {
                    if let Some(vis_state) = attr_obj.get_mut("visState") {
                        if let Value::Object(vis_state_obj) = vis_state {
                            if is_vega_visualization(vis_state_obj) {
                                if let Some(spec_value) = vis_state_obj
                                    .get_mut("params")
                                    .and_then(|p| p.as_object_mut())
                                    .and_then(|params| params.get_mut("spec"))
                                {
                                    escape_spec_field(spec_value, "Kibana saved object")?;
                                }
                            }
                        }
                    }
                }
            }

            // Check if this object is a Vega visualization (direct format)
            if is_vega_visualization(obj) {
                if let Some(spec_value) = obj
                    .get_mut("params")
                    .and_then(|p| p.as_object_mut())
                    .and_then(|params| params.get_mut("spec"))
                {
                    escape_spec_field(spec_value, "direct format")?;
                }
            }

            // Check for embedded saved visualizations (dashboard panels)
            if let Some(saved_vis) = obj.get_mut("savedVis") {
                if let Value::Object(saved_vis_obj) = saved_vis {
                    if is_vega_visualization(saved_vis_obj) {
                        if let Some(spec_value) = saved_vis_obj
                            .get_mut("params")
                            .and_then(|p| p.as_object_mut())
                            .and_then(|params| params.get_mut("spec"))
                        {
                            escape_spec_field(spec_value, "embedded")?;
                        }
                    }
                }
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
                        "spec": "{\"$schema\":\"https://vega.github.io/schema/vega/v5.json\",\"width\":400}"
                    }
                }
            }
        });

        let unescaper = VegaSpecUnescaper::new();
        let result = unescaper.transform(input).unwrap();

        let spec = &result["attributes"]["visState"]["params"]["spec"];
        assert!(spec.is_object());
        assert_eq!(
            spec["$schema"],
            "https://vega.github.io/schema/vega/v5.json"
        );
        assert_eq!(spec["width"], 400);
    }

    #[test]
    fn test_unescape_vega_lite_visualization() {
        let input = json!({
            "attributes": {
                "visState": {
                    "type": "vega-lite",
                    "params": {
                        "spec": "{\"$schema\":\"https://vega.github.io/schema/vega-lite/v5.json\",\"mark\":\"bar\"}"
                    }
                }
            }
        });

        let unescaper = VegaSpecUnescaper::new();
        let result = unescaper.transform(input).unwrap();

        let spec = &result["attributes"]["visState"]["params"]["spec"];
        assert!(spec.is_object());
        assert_eq!(
            spec["$schema"],
            "https://vega.github.io/schema/vega-lite/v5.json"
        );
        assert_eq!(spec["mark"], "bar");
    }

    #[test]
    fn test_unescape_dashboard_panel_vega() {
        let input = json!({
            "embeddableConfig": {
                "savedVis": {
                    "type": "vega",
                    "params": {
                        "spec": "{\"$schema\":\"https://vega.github.io/schema/vega/v5.json\",\"data\":[]}"
                    }
                }
            }
        });

        let unescaper = VegaSpecUnescaper::new();
        let result = unescaper.transform(input).unwrap();

        let spec = &result["embeddableConfig"]["savedVis"]["params"]["spec"];
        assert!(spec.is_object());
        assert_eq!(
            spec["$schema"],
            "https://vega.github.io/schema/vega/v5.json"
        );
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

        // Should remain unchanged
        assert_eq!(result, input);
    }

    #[test]
    fn test_handle_invalid_json_gracefully() {
        let input = json!({
            "attributes": {
                "visState": {
                    "type": "vega",
                    "params": {
                        "spec": "invalid json here"
                    }
                }
            }
        });

        let unescaper = VegaSpecUnescaper::new();
        let result = unescaper.transform(input.clone()).unwrap();

        // Should remain unchanged since JSON is invalid
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
                                "spec": "{\"width\":300}"
                            }
                        }
                    }
                }
            ]
        });

        let unescaper = VegaSpecUnescaper::new();
        let result = unescaper.transform(input).unwrap();

        let spec = &result["panels"][0]["embeddableConfig"]["savedVis"]["params"]["spec"];
        assert!(spec.is_object());
        assert_eq!(spec["width"], 300);
    }

    #[test]
    fn test_escape_vega_visualization() {
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
    fn test_round_trip_compatibility() {
        let original = json!({
            "attributes": {
                "visState": {
                    "type": "vega-lite",
                    "params": {
                        "spec": {
                            "$schema": "https://vega.github.io/schema/vega-lite/v5.json",
                            "mark": "bar",
                            "encoding": {
                                "x": {"field": "category", "type": "nominal"},
                                "y": {"field": "value", "type": "quantitative"}
                            }
                        }
                    }
                }
            }
        });

        // Escape to string format
        let escaper = VegaSpecEscaper::new();
        let escaped = escaper.transform(original.clone()).unwrap();

        // Verify it's escaped
        assert!(escaped["attributes"]["visState"]["params"]["spec"].is_string());

        // Unescape back to object format
        let unescaper = VegaSpecUnescaper::new();
        let unescaped = unescaper.transform(escaped).unwrap();

        // Should match the original
        assert_eq!(unescaped, original);
    }

    #[test]
    fn test_real_kibana_export_transformation() {
        // Test with real Kibana export format (visState is escaped, spec is double-escaped)
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
        assert!(spec.is_object(), "spec should be converted to object");
        assert_eq!(
            spec["$schema"],
            "https://vega.github.io/schema/vega-lite/v5.json"
        );
        assert!(spec["data"]["url"]["%context%"].as_bool().unwrap());
    }
}
