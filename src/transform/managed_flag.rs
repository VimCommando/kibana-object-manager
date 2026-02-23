//! Managed flag transformer
//!
//! Adds the "managed: true" flag to objects being imported to Kibana.

use crate::etl::Transformer;
use eyre::Result;
use serde_json::{json, Value};

/// Transformer that adds a "managed" flag to objects
///
/// This flag indicates that the object is managed by an external system
/// (version control) and should be handled differently by Kibana.
///
/// # Example
/// ```
/// use kibana_object_manager::transform::ManagedFlagAdder;
/// use kibana_object_manager::etl::Transformer;
/// use serde_json::json;
///
/// let adder = ManagedFlagAdder::new(true);
/// let input = json!({
///     "id": "test",
///     "type": "dashboard"
/// });
///
/// let output = adder.transform(input).unwrap();
/// assert_eq!(output["managed"], true);
/// ```
pub struct ManagedFlagAdder {
    managed: bool,
}

impl ManagedFlagAdder {
    /// Create a new managed flag adder
    pub fn new(managed: bool) -> Self {
        Self { managed }
    }
}

impl Default for ManagedFlagAdder {
    fn default() -> Self {
        Self::new(true)
    }
}

impl Transformer for ManagedFlagAdder {
    type Input = Value;
    type Output = Value;

    fn transform(&self, mut input: Self::Input) -> Result<Self::Output> {
        if let Some(obj) = input.as_object_mut() {
            if self.managed {
                obj.insert("managed".to_string(), json!(true));
            } else {
                obj.remove("managed");
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
    fn test_add_managed_flag() {
        let adder = ManagedFlagAdder::new(true);
        let input = json!({
            "id": "test",
            "type": "dashboard",
            "attributes": {"title": "Test"}
        });

        let output = adder.transform(input).unwrap();

        assert_eq!(output["managed"], true);
        assert_eq!(output["id"], "test");
        assert_eq!(output["type"], "dashboard");
    }

    #[test]
    fn test_add_unmanaged_flag() {
        let adder = ManagedFlagAdder::new(false);
        let input = json!({
            "id": "test",
            "type": "dashboard",
            "attributes": {"title": "Test"},
            "managed": true
        });

        let output = adder.transform(input).unwrap();

        assert_eq!(output.get("managed"), None);
        assert_eq!(output["id"], "test");
        assert_eq!(output["type"], "dashboard");
    }

    #[test]
    fn test_overwrites_existing_managed_flag() {
        let adder = ManagedFlagAdder::new(true);
        let input = json!({
            "id": "test",
            "managed": false
        });

        let output = adder.transform(input).unwrap();
        assert_eq!(output["managed"], true);
    }

    #[test]
    fn test_transform_many() {
        let adder = ManagedFlagAdder::new(true);
        let inputs = vec![json!({"id": "1"}), json!({"id": "2"}), json!({"id": "3"})];

        let outputs = adder.transform_many(inputs).unwrap();

        assert_eq!(outputs.len(), 3);
        for output in outputs {
            assert_eq!(output["managed"], true);
        }
    }
}
