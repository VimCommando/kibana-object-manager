#[cfg(test)]
mod integration_test {
    use crate::etl::Transformer;
    use crate::transform::{FieldUnescaper, VegaSpecUnescaper};
    use serde_json::Value;
    use std::fs;

    #[test]
    fn test_full_vega_pipeline_with_real_data() {
        // Read the actual raw Kibana export
        let test_file = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/ironhide/export.ndjson");

        let raw_data = fs::read_to_string(test_file).expect("Failed to read export file");
        let first_line = raw_data.lines().next().expect("Export file is empty");

        let mut value: Value = serde_json::from_str(first_line).expect("Failed to parse JSON");

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
        // Read the actual raw Kibana export - line 3 has a vega vis with comments
        let test_file = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/ironhide/export.ndjson");

        let raw_data = fs::read_to_string(test_file).expect("Failed to read export file");

        // Get line 3 (index 2) which has the commented visualization
        let third_line = raw_data
            .lines()
            .nth(2)
            .expect("Export file should have at least 3 lines");

        let mut value: Value = serde_json::from_str(third_line).expect("Failed to parse JSON");

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
