//! Integration tests for ETL pipeline functionality
//!
//! These tests demonstrate end-to-end workflows using the ETL framework
//! with real file I/O operations.

use eyre::Result;
use kibana_object_manager::etl::{Extractor, Pipeline, Transformer};
use kibana_object_manager::storage::{
    DirectoryReader, DirectoryWriter, NdjsonReader, NdjsonWriter,
};
use serde_json::{Value, json};
use tempfile::TempDir;

/// Mock extractor that produces sample saved objects
struct MockSavedObjectsExtractor {
    objects: Vec<Value>,
}

impl MockSavedObjectsExtractor {
    fn new() -> Self {
        Self {
            objects: vec![
                json!({
                    "type": "dashboard",
                    "id": "dashboard-1",
                    "attributes": {
                        "title": "My Dashboard",
                        "description": "A test dashboard",
                        "created_at": "2024-01-01T00:00:00Z",
                        "updated_at": "2024-01-02T00:00:00Z"
                    }
                }),
                json!({
                    "type": "visualization",
                    "id": "viz-1",
                    "attributes": {
                        "title": "My Visualization",
                        "visState": "{}",
                        "created_at": "2024-01-01T00:00:00Z",
                        "updated_at": "2024-01-02T00:00:00Z"
                    }
                }),
                json!({
                    "type": "search",
                    "id": "search-1",
                    "attributes": {
                        "title": "My Search",
                        "columns": ["field1", "field2"],
                        "created_at": "2024-01-01T00:00:00Z",
                        "updated_at": "2024-01-02T00:00:00Z"
                    }
                }),
            ],
        }
    }
}

impl Extractor for MockSavedObjectsExtractor {
    type Item = Value;

    async fn extract(&self) -> Result<Vec<Self::Item>> {
        Ok(self.objects.clone())
    }
}

/// Transformer that drops specified fields from objects
struct FieldDropper {
    fields_to_drop: Vec<String>,
}

impl FieldDropper {
    fn new(fields: Vec<&str>) -> Self {
        Self {
            fields_to_drop: fields.iter().map(|s| s.to_string()).collect(),
        }
    }
}

impl Transformer for FieldDropper {
    type Input = Value;
    type Output = Value;

    fn transform(&self, mut input: Self::Input) -> Result<Self::Output> {
        // Drop fields from root object
        if let Some(obj) = input.as_object_mut() {
            for field in &self.fields_to_drop {
                obj.remove(field);
            }
        }

        // Also drop fields from attributes if they exist
        if let Some(attrs) = input.get_mut("attributes").and_then(|a| a.as_object_mut()) {
            for field in &self.fields_to_drop {
                attrs.remove(field);
            }
        }
        Ok(input)
    }
}

/// Transformer that adds a "managed" flag to objects
struct ManagedFlagAdder;

impl Transformer for ManagedFlagAdder {
    type Input = Value;
    type Output = Value;

    fn transform(&self, mut input: Self::Input) -> Result<Self::Output> {
        if let Some(obj) = input.as_object_mut() {
            obj.insert("managed".to_string(), json!(true));
        }
        Ok(input)
    }
}

#[tokio::test]
async fn test_extract_transform_load_to_ndjson() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let output_file = temp_dir.path().join("objects.ndjson");

    // Extract from mock source
    let extractor = MockSavedObjectsExtractor::new();

    // Transform by dropping timestamp fields
    let transformer = FieldDropper::new(vec!["created_at", "updated_at"]);

    // Load to NDJSON file
    let loader = NdjsonWriter::new(&output_file);

    // Run pipeline
    let pipeline = Pipeline::new(extractor, transformer, loader);
    let count = pipeline.run().await?;

    assert_eq!(count, 3, "Should have processed 3 objects");

    // Verify the output file
    let content = std::fs::read_to_string(&output_file)?;
    let lines: Vec<&str> = content.lines().collect();
    assert_eq!(lines.len(), 3, "Output file should have 3 lines");

    // Verify that timestamp fields were dropped
    for line in lines {
        let obj: Value = serde_json::from_str(line)?;
        let attrs = obj["attributes"].as_object().unwrap();
        assert!(
            !attrs.contains_key("created_at"),
            "created_at should be dropped"
        );
        assert!(
            !attrs.contains_key("updated_at"),
            "updated_at should be dropped"
        );
    }

    Ok(())
}

#[tokio::test]
async fn test_agent_multiline_instructions_formatting() -> Result<()> {
    use kibana_object_manager::transform::MultilineFieldFormatter;

    let temp_dir = TempDir::new()?;
    let output_dir = temp_dir.path().join("agents");
    std::fs::create_dir_all(&output_dir)?;

    // Create a mock agent with multiline instructions (similar to what Kibana returns)
    let agent = json!({
        "type": "agent",
        "id": "test-agent",
        "attributes": {
            "name": "Test Agent",
            "configuration": {
                "instructions": "Your Role: You are a helpful assistant.\n\nYour Tasks:\n1. Help users with their questions\n2. Provide clear explanations\n3. Write clean code\n\nRemember to always be polite and professional."
            }
        }
    });

    // Create a mock extractor
    struct SingleAgentExtractor {
        agent: Value,
    }

    impl Extractor for SingleAgentExtractor {
        type Item = Value;

        fn extract(&self) -> impl std::future::Future<Output = Result<Vec<Self::Item>>> + Send {
            async { Ok(vec![self.agent.clone()]) }
        }
    }

    let extractor = SingleAgentExtractor {
        agent: agent.clone(),
    };

    // Transform with MultilineFieldFormatter
    let transformer = MultilineFieldFormatter::for_agents();

    // Load to directory
    let loader = DirectoryWriter::new(&output_dir)?;

    // Run pipeline
    let pipeline = Pipeline::new(extractor, transformer, loader);
    let count = pipeline.run().await?;

    assert_eq!(count, 1, "Should have processed 1 agent");

    // Verify the output file uses triple-quote syntax
    let agent_file = output_dir.join("agent").join("Test Agent.json");
    assert!(agent_file.exists(), "Agent file should exist");

    let content = std::fs::read_to_string(&agent_file)?;

    // Print the formatted content for verification
    println!("\n=== Formatted Agent JSON ===");
    println!("{}", content);
    println!("=== End of Formatted JSON ===\n");

    // Check that triple-quotes are used for multiline instructions
    assert!(
        content.contains(r#""""#),
        "Should contain triple-quotes for multiline string: {}",
        content
    );

    // Verify the instructions are properly formatted with real newlines
    assert!(
        content.contains("Your Role:"),
        "Should contain instruction content"
    );
    assert!(
        content.contains("Your Tasks:"),
        "Should contain task section"
    );

    // Verify we can parse it back correctly
    let parsed: Value = kibana_object_manager::storage::from_json5_str(&content)?;
    let instructions = parsed["attributes"]["configuration"]["instructions"]
        .as_str()
        .expect("Instructions should be a string");

    assert!(
        instructions.contains('\n'),
        "Instructions should contain actual newlines after parsing"
    );
    assert!(
        instructions.contains("Your Role: You are a helpful assistant."),
        "Content should be preserved"
    );

    Ok(())
}

#[tokio::test]
async fn test_ndjson_to_directory_pipeline() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let input_file = temp_dir.path().join("input.ndjson");
    let output_dir = temp_dir.path().join("output");

    // Create input NDJSON file
    let input_objects = vec![
        json!({"type": "dashboard", "id": "dash-1", "attributes": {"title": "Dashboard 1"}}),
        json!({"type": "dashboard", "id": "dash-2", "attributes": {"title": "Dashboard 2"}}),
    ];

    let writer = NdjsonWriter::new(&input_file);
    writer.write(&input_objects)?;

    // Extract from NDJSON file
    let extractor = NdjsonReader::new(&input_file);

    // Transform by adding a tag
    let transformer = ManagedFlagAdder;

    // Load to directory
    let loader = DirectoryWriter::new(&output_dir)?;

    // Run pipeline
    let pipeline = Pipeline::new(extractor, transformer, loader);
    let count = pipeline.run().await?;

    assert_eq!(count, 2, "Should have processed 2 objects");

    // Verify output (hierarchical structure: dashboard/Dashboard 1.json)
    let dash1_file = output_dir.join("dashboard").join("Dashboard 1.json");
    assert!(dash1_file.exists());

    let dash1 = std::fs::read_to_string(&dash1_file)?;
    let obj1: Value = serde_json::from_str(&dash1)?;
    assert_eq!(obj1["managed"], json!(true));
    assert_eq!(obj1["attributes"]["title"], json!("Dashboard 1"));

    Ok(())
}

#[tokio::test]
async fn test_directory_to_ndjson_pipeline() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let input_dir = temp_dir.path().join("input");
    let output_file = temp_dir.path().join("output.ndjson");

    // Create input directory with objects
    std::fs::create_dir_all(&input_dir)?;

    let objects = vec![
        (
            "dash-1.json",
            json!({"type": "dashboard", "id": "dash-1", "attributes": {"title": "Dashboard 1"}}),
        ),
        (
            "dash-2.json",
            json!({"type": "dashboard", "id": "dash-2", "attributes": {"title": "Dashboard 2"}}),
        ),
        (
            "viz-1.json",
            json!({"type": "visualization", "id": "viz-1", "attributes": {"title": "Viz 1"}}),
        ),
    ];

    for (filename, obj) in objects {
        std::fs::write(
            input_dir.join(filename),
            serde_json::to_string_pretty(&obj)?,
        )?;
    }

    // Extract from directory
    let extractor = DirectoryReader::new(&input_dir);

    // Transform by dropping type field (it's redundant with directory structure)
    let transformer = FieldDropper::new(vec!["type"]);

    // Load to NDJSON
    let loader = NdjsonWriter::new(&output_file);

    // Run pipeline
    let pipeline = Pipeline::new(extractor, transformer, loader);
    let count = pipeline.run().await?;

    assert_eq!(count, 3, "Should have processed 3 objects");

    // Verify output file
    let content = std::fs::read_to_string(&output_file)?;
    let lines: Vec<&str> = content.lines().collect();
    assert_eq!(lines.len(), 3, "Should have 3 lines in NDJSON");

    for line in lines {
        let obj: Value = serde_json::from_str(line)?;
        assert!(
            !obj.as_object().unwrap().contains_key("type"),
            "type field should be dropped"
        );
    }

    Ok(())
}

#[tokio::test]
async fn test_roundtrip_directory_to_directory() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let source_dir = temp_dir.path().join("source");
    let dest_dir = temp_dir.path().join("dest");

    // Create source directory with objects
    std::fs::create_dir_all(&source_dir)?;
    let dashboard = json!({
        "type": "dashboard",
        "id": "my-dashboard",
        "attributes": {
            "title": "Test Dashboard",
            "created_at": "2024-01-01T00:00:00Z"
        }
    });
    std::fs::write(
        source_dir.join("my-dashboard.json"),
        serde_json::to_string_pretty(&dashboard)?,
    )?;

    // Extract from source directory
    let extractor = DirectoryReader::new(&source_dir);

    // Transform by dropping created_at
    let transformer = FieldDropper::new(vec!["created_at"]);

    // Load to destination directory
    let loader = DirectoryWriter::new(&dest_dir)?;

    // Run pipeline
    let pipeline = Pipeline::new(extractor, transformer, loader);
    let count = pipeline.run().await?;

    assert_eq!(count, 1, "Should have processed 1 object");

    // Verify destination (hierarchical structure: dashboard/Test Dashboard.json)
    let dest_file = dest_dir.join("dashboard").join("Test Dashboard.json");
    assert!(dest_file.exists());

    let content = std::fs::read_to_string(&dest_file)?;
    let obj: Value = serde_json::from_str(&content)?;

    assert_eq!(obj["type"], json!("dashboard"));
    assert_eq!(obj["id"], json!("my-dashboard"));
    assert_eq!(obj["attributes"]["title"], json!("Test Dashboard"));
    assert!(
        !obj["attributes"]
            .as_object()
            .unwrap()
            .contains_key("created_at")
    );

    Ok(())
}
