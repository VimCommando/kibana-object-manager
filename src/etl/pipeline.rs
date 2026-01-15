//! Pipeline orchestration for ETL operations

use super::{Extractor, Loader, Transformer};
use eyre::Result;

/// ETL Pipeline that orchestrates Extract, Transform, and Load operations
///
/// # Type Parameters
/// - `E`: Extractor type
/// - `T`: Transformer type (must transform from E::Item)
/// - `L`: Loader type (must load T::Output)
///
/// # Example
/// ```no_run
/// use kibana_object_manager::etl::Pipeline;
/// # use kibana_object_manager::etl::{Extractor, Transformer, Loader};
/// # use async_trait::async_trait;
/// # use eyre::Result;
/// # struct MyExtractor;
/// # #[async_trait]
/// # impl Extractor for MyExtractor {
/// #     type Item = i32;
/// #     async fn extract(&self) -> Result<Vec<Self::Item>> { Ok(vec![]) }
/// # }
/// # struct MyTransformer;
/// # impl Transformer for MyTransformer {
/// #     type Input = i32;
/// #     type Output = i32;
/// #     fn transform(&self, input: Self::Input) -> Result<Self::Output> { Ok(input) }
/// # }
/// # struct MyLoader;
/// # #[async_trait]
/// # impl Loader for MyLoader {
/// #     type Item = i32;
/// #     async fn load(&self, items: Vec<Self::Item>) -> Result<usize> { Ok(items.len()) }
/// # }
///
/// # async fn example() -> Result<()> {
/// let pipeline = Pipeline::new(
///     MyExtractor,
///     MyTransformer,
///     MyLoader,
/// );
///
/// let count = pipeline.run().await?;
/// println!("Processed {} items", count);
/// # Ok(())
/// # }
/// ```
pub struct Pipeline<E, T, L> {
    extractor: E,
    transformer: T,
    loader: L,
}

impl<E, T, L> Pipeline<E, T, L>
where
    E: Extractor,
    T: Transformer<Input = E::Item>,
    L: Loader<Item = T::Output>,
{
    /// Create a new pipeline
    pub fn new(extractor: E, transformer: T, loader: L) -> Self {
        Self {
            extractor,
            transformer,
            loader,
        }
    }

    /// Run the complete ETL pipeline
    ///
    /// Steps:
    /// 1. Extract items from source
    /// 2. Transform each item
    /// 3. Load items to destination
    ///
    /// Returns the number of items successfully loaded
    ///
    /// # Errors
    /// Returns an error if any stage fails
    pub async fn run(&self) -> Result<usize> {
        log::info!("Starting ETL pipeline");

        // Extract
        log::debug!("Extracting from source...");
        let items = self.extractor.extract().await?;
        log::info!("Extracted {} items", items.len());

        if items.is_empty() {
            log::warn!("No items extracted, pipeline complete");
            return Ok(0);
        }

        // Transform
        log::debug!("Transforming items...");
        let transformed = self.transformer.transform_many(items)?;
        log::info!("Transformed {} items", transformed.len());

        // Load
        log::debug!("Loading to destination...");
        let count = self.loader.load(transformed).await?;
        log::info!("Loaded {} items", count);

        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use eyre::Result;

    struct MockExtractor(Vec<i32>);

    #[async_trait]
    impl Extractor for MockExtractor {
        type Item = i32;
        async fn extract(&self) -> Result<Vec<Self::Item>> {
            Ok(self.0.clone())
        }
    }

    struct DoubleTransformer;

    impl Transformer for DoubleTransformer {
        type Input = i32;
        type Output = i32;
        fn transform(&self, input: Self::Input) -> Result<Self::Output> {
            Ok(input * 2)
        }
    }

    struct SumLoader(std::sync::Arc<std::sync::Mutex<i32>>);

    #[async_trait]
    impl Loader for SumLoader {
        type Item = i32;
        async fn load(&self, items: Vec<Self::Item>) -> Result<usize> {
            let sum: i32 = items.iter().sum();
            *self.0.lock().unwrap() = sum;
            Ok(items.len())
        }
    }

    #[tokio::test]
    async fn test_pipeline() {
        let result = std::sync::Arc::new(std::sync::Mutex::new(0));

        let pipeline = Pipeline::new(
            MockExtractor(vec![1, 2, 3]),
            DoubleTransformer,
            SumLoader(result.clone()),
        );

        let count = pipeline.run().await.unwrap();
        assert_eq!(count, 3);
        assert_eq!(*result.lock().unwrap(), 12); // (1+2+3)*2 = 12
    }

    #[tokio::test]
    async fn test_empty_pipeline() {
        let result = std::sync::Arc::new(std::sync::Mutex::new(0));

        let pipeline = Pipeline::new(
            MockExtractor(vec![]),
            DoubleTransformer,
            SumLoader(result.clone()),
        );

        let count = pipeline.run().await.unwrap();
        assert_eq!(count, 0);
    }
}
