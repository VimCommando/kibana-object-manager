#[cfg(test)]
mod tests {
    use crate::etl::Transformer;
    use crate::transform::{FieldUnescaper, VegaSpecUnescaper};
    use serde_json::json;

    #[test]
    fn test_full_vega_pipeline_with_real_data() {
        // Create a mock Kibana visualization export with escaped visState
        let mut value = json!({
            "type": "visualization",
            "id": "test-vega-vis",
            "attributes": {
                "title": "Test Vega Visualization",
                "visState": "{\"type\":\"vega\",\"params\":{\"spec\":\"{\\\"$schema\\\":\\\"https://vega.github.io/schema/vega-lite/v5.json\\\",\\\"data\\\":{\\\"values\\\":[{\\\"x\\\":1,\\\"y\\\":2}]},\\\"mark\\\":\\\"point\\\",\\\"encoding\\\":{\\\"x\\\":{\\\"field\\\":\\\"x\\\",\\\"type\\\":\\\"quantitative\\\"},\\\"y\\\":{\\\"field\\\":\\\"y\\\",\\\"type\\\":\\\"quantitative\\\"}}}\"}}"
            }
        });

        // Verify original state
        let vis_state_original = &value["attributes"]["visState"];
        assert!(
            vis_state_original.is_string(),
            "Original visState should be a string"
        );

        // Apply FieldUnescaper first (simulates the extraction pipeline)
        let field_unescaper = FieldUnescaper::default_kibana_fields();
        value = field_unescaper
            .transform(value)
            .expect("FieldUnescaper failed");

        // Verify visState is now an object but spec is still a string
        let vis_state_after_field = &value["attributes"]["visState"];
        assert!(
            vis_state_after_field.is_object(),
            "visState should be an object after FieldUnescaper"
        );

        let vis_state_obj = vis_state_after_field.as_object().unwrap();
        assert_eq!(
            vis_state_obj["type"], "vega",
            "Should be a vega visualization"
        );

        let spec_after_field = &vis_state_obj["params"]["spec"];
        assert!(
            spec_after_field.is_string(),
            "spec should still be a string after FieldUnescaper"
        );

        // Now apply VegaSpecUnescaper
        let vega_unescaper = VegaSpecUnescaper::new();
        value = vega_unescaper
            .transform(value)
            .expect("VegaSpecUnescaper failed");

        let vis_state_final = &value["attributes"]["visState"];
        let vis_state_final_obj = vis_state_final.as_object().unwrap();
        let spec_final = &vis_state_final_obj["params"]["spec"];

        // Spec should remain a string
        assert!(
            spec_final.is_string(),
            "spec should remain a string after VegaSpecUnescaper"
        );

        let spec_str = spec_final.as_str().unwrap();

        // Verify the content is correct by checking key fields are present
        assert!(
            spec_str.contains("vega-lite/v5.json") || spec_str.contains("vega/v5.json"),
            "spec should contain vega schema reference"
        );
        assert!(spec_str.contains("data"), "spec should contain data field");
    }

    #[test]
    fn test_vega_pipeline_preserves_comments() {
        // Create a mock Kibana visualization with comments in the Vega spec
        // The spec is escaped as a string within visState (which is also escaped)
        let vega_spec_with_comments = r#"{
  "$schema": "https://vega.github.io/schema/vega-lite/v5.json",
  /* xray tango */
  // Node load percent by tier
  "data": {
    "values": [{"tier": "hot", "load": 75}, {"tier": "warm", "load": 45}]
  },
  "mark": "bar",
  "encoding": {
    "x": {"field": "tier", "type": "nominal"},
    "y": {"field": "load", "type": "quantitative"}
  }
}"#;

        // Escape the spec for inclusion in visState JSON string
        let escaped_spec = vega_spec_with_comments
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('\n', "\\n");

        // Create visState JSON as a string
        let vis_state_json = format!(
            r#"{{"type":"vega","params":{{"spec":"{}"}}}}"#,
            escaped_spec
        );

        // Create the full saved object with escaped visState
        let mut value = json!({
            "type": "visualization",
            "id": "test-vega-commented",
            "attributes": {
                "title": "Vega with Comments",
                "visState": vis_state_json
            }
        });

        // Apply FieldUnescaper first
        let field_unescaper = FieldUnescaper::default_kibana_fields();
        value = field_unescaper
            .transform(value)
            .expect("FieldUnescaper failed");

        // Apply VegaSpecUnescaper
        let vega_unescaper = VegaSpecUnescaper::new();
        value = vega_unescaper
            .transform(value)
            .expect("VegaSpecUnescaper failed");

        let vis_state = &value["attributes"]["visState"];
        let vis_state_obj = vis_state.as_object().unwrap();
        let spec = &vis_state_obj["params"]["spec"];
        let spec_str = spec.as_str().expect("spec should be a string");

        // THE CRITICAL TEST: Comments MUST be preserved!
        assert!(
            spec_str.contains("/* xray tango */"),
            "Block comment '/* xray tango */' was stripped! Got:\n{}",
            &spec_str[..500.min(spec_str.len())]
        );
        assert!(
            spec_str.contains("// Node load percent"),
            "Line comment was stripped! Got:\n{}",
            &spec_str[..500.min(spec_str.len())]
        );

        println!("✅ SUCCESS: Comments preserved in Vega spec!");
    }
}
