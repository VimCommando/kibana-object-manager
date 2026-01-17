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

        // Verify visState is now an object but spec is still escaped
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

        println!("DEBUG: spec after FieldUnescaper:");
        println!(
            "  type: {}",
            if spec_after_field.is_string() {
                "string"
            } else {
                "object"
            }
        );
        if let Some(s) = spec_after_field.as_str() {
            println!("  length: {}", s.len());
            println!("  starts_with: {}", &s[..50.min(s.len())]);
        }

        // Now apply VegaSpecUnescaper
        let vega_unescaper = VegaSpecUnescaper::new();
        value = vega_unescaper
            .transform(value)
            .expect("VegaSpecUnescaper failed");

        println!("DEBUG: after VegaSpecUnescaper:");
        let vis_state_final = &value["attributes"]["visState"];
        let vis_state_final_obj = vis_state_final.as_object().unwrap();
        let spec_final = &vis_state_final_obj["params"]["spec"];
        println!(
            "  spec type: {}",
            if spec_final.is_string() {
                "string"
            } else {
                "object"
            }
        );
        if let Some(s) = spec_final.as_str() {
            println!("  spec string length: {}", s.len());
        }
        if let Some(o) = spec_final.as_object() {
            println!("  spec object keys: {}", o.len());
        }

        assert!(
            spec_final.is_object(),
            "spec should now be an object after VegaSpecUnescaper"
        );

        // Now apply VegaSpecUnescaper
        let vega_unescaper = VegaSpecUnescaper::new();
        value = vega_unescaper
            .transform(value)
            .expect("VegaSpecUnescaper failed");

        // Verify spec is now an object
        let vis_state_final = &value["attributes"]["visState"];
        let vis_state_final_obj = vis_state_final.as_object().unwrap();
        let spec_final = &vis_state_final_obj["params"]["spec"];

        assert!(
            spec_final.is_object(),
            "spec should now be an object after VegaSpecUnescaper"
        );

        let spec_obj = spec_final.as_object().unwrap();
        assert!(spec_obj.contains_key("$schema"), "spec should have $schema");
        assert!(spec_obj.contains_key("data"), "spec should have data");
        assert_eq!(
            spec_obj["$schema"], "https://vega.github.io/schema/vega-lite/v5.json",
            "Should be vega-lite v5"
        );

        println!("✅ SUCCESS: VegaSpec pipeline transformation complete!");
        println!(
            "   - Raw visState (string) → FieldUnescaper → visState (object) with spec (string)"
        );
        println!(
            "   - spec (string) → VegaSpecUnescaper → spec (object with {} properties)",
            spec_obj.len()
        );
        println!("   - Spec $schema: {}", spec_obj["$schema"]);
    }
}
