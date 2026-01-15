//! Field dropper transformer
//!
//! Removes specified fields from JSON objects, typically metadata fields
//! that should not be version controlled.

use crate::etl::Transformer;
use eyre::Result;
use serde_json::Value;

/// Transformer that drops specified fields from objects
///
/// This is used during export to remove Kibana metadata fields like:
/// - created_at, updated_at
/// - created_by, updated_by
/// - version, count
/// - managed
///
/// # Example
/// ```
/// use kibana_object_manager::transform::FieldDropper;
/// use kibana_object_manager::etl::Transformer;
/// use serde_json::json;
///
/// let dropper = FieldDropper::new(vec!["created_at", "version"]);
/// let input = json!({
///     "id": "test",
///     "created_at": "2024-01-01",
///     "version": "1.0",
///     "title": "My Object"
/// });
///
/// let output = dropper.transform(input).unwrap();
/// assert!(!output.as_object().unwrap().contains_key("created_at"));
/// assert!(!output.as_object().unwrap().contains_key("version"));
/// assert_eq!(output["title"], "My Object");
/// ```
pub struct FieldDropper {
    fields: Vec<String>,
}

impl FieldDropper {
    /// Create a new field dropper with the specified fields to remove
    pub fn new(fields: Vec<&str>) -> Self {
        Self {
            fields: fields.iter().map(|s| s.to_string()).collect(),
        }
    }

    /// Create a field dropper with default Kibana metadata fields
    ///
    /// Drops: created_at, created_by, updated_at, updated_by, version, count, managed
    pub fn default_kibana_fields() -> Self {
        Self::new(vec![
            "created_at",
            "created_by",
            "updated_at",
            "updated_by",
            "version",
            "count",
            "managed",
        ])
    }
}

impl Transformer for FieldDropper {
    type Input = Value;
    type Output = Value;

    fn transform(&self, mut input: Self::Input) -> Result<Self::Output> {
        if let Some(obj) = input.as_object_mut() {
            for field in &self.fields {
                obj.remove(field);
            }
        }
        Ok(input)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_drop_fields() {
        let dropper = FieldDropper::new(vec!["created_at", "version"]);
        let input = json!({
            "id": "test",
            "created_at": "2024-01-01",
            "version": "1.0",
            "title": "My Object"
        });

        let output = dropper.transform(input).unwrap();
        let obj = output.as_object().unwrap();

        assert!(!obj.contains_key("created_at"));
        assert!(!obj.contains_key("version"));
        assert_eq!(output["id"], "test");
        assert_eq!(output["title"], "My Object");
    }

    #[test]
    fn test_default_kibana_fields() {
        let dropper = FieldDropper::default_kibana_fields();
        let input = json!({
            "id": "test",
            "created_at": "2024-01-01",
            "created_by": "user",
            "updated_at": "2024-01-02",
            "updated_by": "admin",
            "version": "1.0",
            "count": 5,
            "managed": true,
            "title": "My Object"
        });

        let output = dropper.transform(input).unwrap();
        let obj = output.as_object().unwrap();

        assert!(!obj.contains_key("created_at"));
        assert!(!obj.contains_key("created_by"));
        assert!(!obj.contains_key("updated_at"));
        assert!(!obj.contains_key("updated_by"));
        assert!(!obj.contains_key("version"));
        assert!(!obj.contains_key("count"));
        assert!(!obj.contains_key("managed"));
        assert_eq!(output["id"], "test");
        assert_eq!(output["title"], "My Object");
    }

    #[test]
    fn test_transform_many() {
        let dropper = FieldDropper::new(vec!["temp"]);
        let inputs = vec![
            json!({"id": "1", "temp": "remove"}),
            json!({"id": "2", "temp": "remove"}),
        ];

        let outputs = dropper.transform_many(inputs).unwrap();

        assert_eq!(outputs.len(), 2);
        assert!(!outputs[0].as_object().unwrap().contains_key("temp"));
        assert!(!outputs[1].as_object().unwrap().contains_key("temp"));
    }
}
