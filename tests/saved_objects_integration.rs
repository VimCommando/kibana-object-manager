//! Integration test for Saved Objects API
//!
//! Tests the SavedObjectsExtractor and SavedObjectsLoader with a real Kibana instance.
//!
//! Prerequisites:
//! - Kibana running on localhost:5601
//! - Test space "test" must exist
//! - Test index pattern created in test space

use eyre::Result;
use kibana_object_manager::{
    client::{Auth, KibanaClient},
    etl::{Extractor, Loader},
    kibana::saved_objects::{SavedObjectsExtractor, SavedObjectsLoader, SavedObjectsManifest},
};
use url::Url;

#[tokio::test]
#[ignore] // Only run with --ignored flag
async fn test_saved_objects_extract() -> Result<()> {
    // Load manifest
    let manifest = SavedObjectsManifest::read("/tmp/kibana-test/test_manifest.json")?;

    println!("Manifest loaded: {} objects", manifest.count());

    // Create client and get space client for "test" space
    let url = Url::parse("http://localhost:5601")?;
    let client = KibanaClient::try_new(url, Auth::None, ".")?;
    let space_client = client.space("test")?;

    // Create extractor for test space
    let extractor = SavedObjectsExtractor::new(space_client, manifest);

    // Extract objects
    let objects = extractor.extract().await?;

    println!("Extracted {} object(s)", objects.len());
    assert!(
        objects.len() > 0,
        "Should have extracted at least one object"
    );

    // Verify the object has expected fields
    let first = &objects[0];
    assert!(
        first.get("type").is_some(),
        "Object should have 'type' field"
    );
    assert!(first.get("id").is_some(), "Object should have 'id' field");
    assert!(
        first.get("attributes").is_some(),
        "Object should have 'attributes' field"
    );

    println!("First object type: {}", first["type"]);
    println!("First object id: {}", first["id"]);

    Ok(())
}

#[tokio::test]
#[ignore] // Only run with --ignored flag
async fn test_saved_objects_roundtrip() -> Result<()> {
    println!("\n=== Testing Saved Objects Roundtrip ===\n");

    // Step 1: Extract from test space
    let manifest = SavedObjectsManifest::read("/tmp/kibana-test/test_manifest.json")?;
    let url = Url::parse("http://localhost:5601")?;
    let client = KibanaClient::try_new(url.clone(), Auth::None, ".")?;
    let space_client = client.space("test")?;

    let extractor = SavedObjectsExtractor::new(space_client.clone(), manifest);
    let objects = extractor.extract().await?;

    println!(
        "Step 1: Extracted {} object(s) from test space",
        objects.len()
    );
    assert!(objects.len() > 0, "Should have extracted objects");

    // Step 2: Delete the object from test space
    let object_id = objects[0]["id"].as_str().unwrap();
    let object_type = objects[0]["type"].as_str().unwrap();

    println!(
        "Step 2: Deleting object {}/{} from test space",
        object_type, object_id
    );

    let delete_url = format!("/api/saved_objects/{}/{}", object_type, object_id);
    let response = space_client
        .request(
            reqwest::Method::DELETE,
            &std::collections::HashMap::new(),
            &delete_url,
            None,
        )
        .await?;

    assert!(response.status().is_success(), "Delete should succeed");
    println!("   Object deleted successfully");

    // Step 3: Re-import the object
    println!("Step 3: Re-importing object to test space");
    let loader = SavedObjectsLoader::new(space_client.clone()).with_overwrite(true);
    let count = loader.load(objects.clone()).await?;

    println!("   Imported {} object(s)", count);
    assert_eq!(count, 1, "Should have imported 1 object");

    // Step 4: Verify it's back
    println!("Step 4: Verifying object exists again");
    let verify_url = Url::parse("http://localhost:5601")?;
    let verify_client = KibanaClient::try_new(verify_url, Auth::None, ".")?;
    let verify_space_client = verify_client.space("test")?;

    let verify_manifest = SavedObjectsManifest::read("/tmp/kibana-test/test_manifest.json")?;
    let verify_extractor = SavedObjectsExtractor::new(verify_space_client, verify_manifest);
    let verify_objects = verify_extractor.extract().await?;

    assert_eq!(verify_objects.len(), 1, "Object should exist again");
    println!("   Object verified: {}/{}", object_type, object_id);

    println!("\n=== Roundtrip Test Complete ===\n");

    Ok(())
}
