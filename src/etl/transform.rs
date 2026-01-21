//! Transformer trait for data transformation

use eyre::Result;

/// Transformer trait for transforming data items
///
/// Implementors define how to transform items:
/// - Data cleaning (removing fields)
/// - Data enrichment (adding fields)
/// - Format conversion
/// - Validation
///
/// # Example
/// ```no_run
/// use kibana_object_manager::etl::Transformer;
/// use eyre::Result;
///
/// struct FieldDropper {
///     fields: Vec<String>,
/// }
///
/// impl Transformer for FieldDropper {
///     type Input = serde_json::Value;
///     type Output = serde_json::Value;
///     
///     fn transform(&self, mut input: Self::Input) -> Result<Self::Output> {
///         if let Some(obj) = input.as_object_mut() {
///             for field in &self.fields {
///                 obj.remove(field);
///             }
///         }
///         Ok(input)
///     }
/// }
/// ```
pub trait Transformer: Send + Sync {
    /// Input item type
    type Input: Send;

    /// Output item type after transformation
    type Output: Send;

    /// Transform a single item
    ///
    /// # Errors
    /// Returns an error if transformation fails (validation, conversion, etc.)
    fn transform(&self, input: Self::Input) -> Result<Self::Output>;

    /// Transform multiple items (default batch implementation)
    ///
    /// Override this for optimized batch processing
    fn transform_many(&self, inputs: Vec<Self::Input>) -> Result<Vec<Self::Output>> {
        inputs.into_iter().map(|i| self.transform(i)).collect()
    }
}

/// Identity transformer that passes items through unchanged
///
/// Use this when you need a transformer but don't want to modify the data.
/// The generic parameter T must be specified when creating the transformer.
pub struct IdentityTransformer<T> {
    _phantom: std::marker::PhantomData<T>,
}

impl<T> Default for IdentityTransformer<T> {
    fn default() -> Self {
        Self {
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<T> IdentityTransformer<T> {
    pub fn new() -> Self {
        Self::default()
    }
}

impl<T: Send + Sync> Transformer for IdentityTransformer<T> {
    type Input = T;
    type Output = T;

    fn transform(&self, input: Self::Input) -> Result<Self::Output> {
        Ok(input)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_identity_transformer() {
        let transformer = IdentityTransformer::<i32>::new();
        let input = vec![1, 2, 3];
        let output = transformer.transform_many(input.clone()).unwrap();
        assert_eq!(input, output);
    }
}
