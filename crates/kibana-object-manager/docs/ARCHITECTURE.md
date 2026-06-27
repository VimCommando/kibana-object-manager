# Kibana Object Manager - Architecture

Technical deep-dive into the design and implementation of `kibob`.

## Table of Contents

- [Design Philosophy](#design-philosophy)
- [Architecture Overview](#architecture-overview)
- [Core Modules](#core-modules)
- [Data Flow](#data-flow)
- [Extension Points](#extension-points)
- [Testing Strategy](#testing-strategy)
- [Performance Considerations](#performance-considerations)

---

## Design Philosophy

### Principles

1. **Modularity** - Loosely coupled components with clear interfaces
2. **Extensibility** - Easy to add new object types, storage backends, transformations
3. **Type Safety** - Leverage Rust's type system to prevent bugs at compile time
4. **Async First** - Non-blocking I/O for efficient network and file operations
5. **Explicit Over Implicit** - Clear data flow, no hidden magic
6. **Testability** - Every component can be tested in isolation

### Why ETL Pattern?

The Extract-Transform-Load pattern provides:
- **Separation of concerns** - Network, business logic, and storage are independent
- **Pipeline composition** - Chain operations declaratively
- **Reusability** - Extractors and loaders work with any transformers
- **Observability** - Each stage can be instrumented independently

### Why Rust?

- **Performance** - Near C-level speed without garbage collection pauses
- **Safety** - No null pointer exceptions, data races, or memory leaks
- **Concurrency** - Fearless concurrency with Tokio async runtime
- **Ecosystem** - Excellent HTTP, JSON, and CLI libraries
- **Single binary** - No runtime dependencies, easy distribution

---

## Architecture Overview

### High-Level Components

```
┌─────────────────────────────────────────────────────────────┐
│                         CLI Layer                            │
│                    (src/main.rs, src/cli.rs)                │
└───────────────────────────────┬─────────────────────────────┘
                                │
                                ▼
┌─────────────────────────────────────────────────────────────┐
│                     Pipeline Orchestration                   │
│                      (src/etl/pipeline.rs)                   │
└───────────────────────────────┬─────────────────────────────┘
                                │
                ┌───────────────┼───────────────┐
                ▼               ▼               ▼
        ┌───────────┐   ┌────────────┐   ┌──────────┐
        │ Extractor │   │ Transformer│   │  Loader  │
        │  (Pull)   │   │  (Process) │   │  (Push)  │
        └─────┬─────┘   └──────┬─────┘   └────┬─────┘
              │                │               │
              ▼                ▼               ▼
     ┌──────────────┐  ┌─────────────┐  ┌──────────────┐
     │ Kibana Client│  │ Transform   │  │   Storage    │
     │ (HTTP API)   │  │ Logic       │  │ (Files/NDJSON)│
     └──────────────┘  └─────────────┘  └──────────────┘
```

### Module Dependency Graph

```
main.rs
  └─> cli.rs
       ├─> etl/pipeline.rs
       │    ├─> etl/extract.rs (trait)
       │    ├─> etl/transform.rs (trait)
       │    └─> etl/load.rs (trait)
       ├─> kibana/saved_objects/
       │    ├─> extractor.rs (implements Extractor)
       │    ├─> loader.rs (implements Loader)
       │    └─> manifest.rs (data structures)
       ├─> storage/
       │    ├─> directory.rs (implements Loader/Extractor)
       │    ├─> ndjson.rs (implements Loader/Extractor)
       │    └─> gitignore.rs (utility)
       ├─> transform/
       │    ├─> field_dropper.rs (implements Transformer)
       │    ├─> field_escaper.rs (implements Transformer)
       │    └─> managed_flag.rs (implements Transformer)
       └─> client/
            ├─> kibana.rs (HTTP client)
            └─> auth.rs (authentication)
```

---

## Core Modules

### 1. ETL Framework (`src/etl/`)

The heart of kibob's architecture. Defines three core traits:

#### **Extractor Trait**

```rust
#[async_trait]
pub trait Extractor {
    async fn extract(&self) -> Result<Vec<Value>>;
}
```

**Purpose:** Fetch data from a source (Kibana API, files, etc.)

**Implementations:**
- `SavedObjectsExtractor` - Fetches from Kibana API
- `DirectoryReader` - Reads from filesystem
- `NdjsonReader` - Parses NDJSON files

#### **Transformer Trait**

```rust
#[async_trait]
pub trait Transformer {
    async fn transform(&self, data: Vec<Value>) -> Result<Vec<Value>>;
}
```

**Purpose:** Modify data between extraction and loading

**Implementations:**
- `FieldDropper` - Removes metadata fields (managed, updated_at, etc.)
- `FieldEscaper` - Escapes JSON strings for Kibana
- `FieldUnescaper` - Unescapes JSON strings for readability
- `ManagedFlagAdder` - Adds managed flag

**Chaining Example:**
```rust
// Pull pipeline: Kibana → Clean → Unescape → Files
let pipeline = Pipeline::new()
    .with_extractor(SavedObjectsExtractor::new(client, manifest))
    .with_transformer(FieldDropper::new(vec!["managed", "updated_at"]))
    .with_transformer(FieldUnescaper::new(vec!["attributes.kibanaSavedObjectMeta"]))
    .with_loader(DirectoryWriter::new("objects/"));
```

#### **Loader Trait**

```rust
#[async_trait]
pub trait Loader {
    async fn load(&self, data: Vec<Value>) -> Result<usize>;
}
```

**Purpose:** Write data to destination (Kibana API, files, etc.)

**Implementations:**
- `SavedObjectsLoader` - Uploads to Kibana API
- `DirectoryWriter` - Writes to filesystem
- `NdjsonWriter` - Creates NDJSON files

#### **Pipeline Composition**

```rust
pub struct Pipeline {
    extractor: Option<Box<dyn Extractor>>,
    transformers: Vec<Box<dyn Transformer>>,
    loader: Option<Box<dyn Loader>>,
}

impl Pipeline {
    pub async fn execute(&self) -> Result<usize> {
        // 1. Extract
        let mut data = self.extractor.extract().await?;
        
        // 2. Transform (chain)
        for transformer in &self.transformers {
            data = transformer.transform(data).await?;
        }
        
        // 3. Load
        let count = self.loader.load(data).await?;
        
        Ok(count)
    }
}
```

---

### 2. Kibana Module (`src/kibana/`)

Kibana-specific implementations of ETL traits.

#### **Saved Objects Manifest**

```rust
pub struct SavedObjectsManifest {
    pub version: String,
    pub objects: Vec<ObjectReference>,
}

pub struct ObjectReference {
    pub type_: String,
    pub id: String,
    pub attributes: Option<Value>,
}
```

**Purpose:** Tracks which objects to manage

**Format:** JSON file at `manifest/saved_objects.json`

#### **Saved Objects Extractor**

**Responsibilities:**
1. Read manifest to get object list
2. Call Kibana export API with object IDs
3. Parse NDJSON response
4. Return as Vec<Value>

**Key Code:**
```rust
impl Extractor for SavedObjectsExtractor {
    async fn extract(&self) -> Result<Vec<Value>> {
        let response = self.client
            .export_objects(&self.manifest.objects)
            .await?;
        
        let objects = parse_ndjson(&response)?;
        Ok(objects)
    }
}
```

#### **Saved Objects Loader**

**Responsibilities:**
1. Receive objects as Vec<Value>
2. Convert to NDJSON format
3. Call Kibana import API
4. Handle import results

**Key Features:**
- Overwrites existing objects (idempotent)
- Supports managed flag
- Error handling for conflicts

---

### 3. Storage Module (`src/storage/`)

File and directory operations.

#### **Directory Reader/Writer**

**Structure:**
```
objects/
├── dashboard/
│   ├── abc-123.json
│   └── xyz-789.json
├── visualization/
│   └── def-456.json
└── index-pattern/
    └── logs-*.json
```

**DirectoryReader:**
- Scans directory tree
- Groups by object type
- Loads JSON files
- Returns Vec<Value>

**DirectoryWriter:**
- Receives Vec<Value>
- Organizes by type into subdirectories
- Pretty-prints JSON (2-space indent)
- Handles special characters in filenames

#### **NDJSON Reader/Writer**

**Format:** Newline-delimited JSON
```json
{"type":"dashboard","id":"abc","attributes":{...}}
{"type":"visualization","id":"xyz","attributes":{...}}
```

**NdjsonReader:**
- Reads file line-by-line
- Parses each line as JSON
- Skips empty lines
- Returns Vec<Value>

**NdjsonWriter:**
- Receives Vec<Value>
- Serializes each as single-line JSON
- Appends newline
- Writes to file

#### **GitIgnore Integration**

```rust
pub struct GitIgnore {
    patterns: Vec<String>,
}

impl GitIgnore {
    pub fn should_ignore(&self, path: &Path) -> bool {
        // Pattern matching logic
    }
    
    pub fn ensure_patterns(&mut self, path: &Path) {
        // Add patterns to .gitignore if missing
    }
}
```

**Patterns added:**
- `.env*` - Never commit credentials
- `*.ndjson` - Temporary export files
- `manifest.json.bak` - Backup files

---

### 4. Transform Module (`src/transform/`)

Data transformation implementations.

#### **Field Dropper**

**Purpose:** Remove unwanted metadata fields

**Example:**
```rust
let dropper = FieldDropper::new(vec![
    "managed",
    "updated_at",
    "version",
]);

// Before:
{"type": "dashboard", "id": "abc", "managed": true, "version": "8.0"}

// After:
{"type": "dashboard", "id": "abc"}
```

#### **Field Escaper/Unescaper**

**Purpose:** Handle Kibana's JSON string escaping

**Why needed:** Kibana stores JSON objects as escaped strings:
```json
{
  "attributes": {
    "kibanaSavedObjectMeta": "{\"searchSourceJSON\": \"{\\\"query\\\":{}}\"}"
  }
}
```

**FieldUnescaper (Pull):** Converts strings to objects for readability
**FieldEscaper (Push):** Converts objects back to strings for Kibana

#### **Managed Flag Adder**

**Purpose:** Add `managed: true/false` to objects

**Usage:**
```rust
let adder = ManagedFlagAdder::new(true);  // managed: true
```

**Effect:** Controls whether objects are editable in Kibana UI

---

### 5. Client Module (`src/client/`)

HTTP client for Kibana API.

#### **KibanaClient**

```rust
pub struct KibanaClient {
    base_url: String,
    client: reqwest::Client,
    auth: Auth,
    space: String,
}

impl KibanaClient {
    pub async fn export_objects(&self, refs: &[ObjectReference]) -> Result<String> {
        let url = format!("{}/api/saved_objects/_export", self.base_url);
        let body = create_export_body(refs);
        
        let response = self.client
            .post(&url)
            .header("kbn-xsrf", "true")
            .json(&body)
            .send()
            .await?;
        
        Ok(response.text().await?)
    }
    
    pub async fn import_objects(&self, ndjson: &str, overwrite: bool) -> Result<()> {
        let url = format!("{}/api/saved_objects/_import", self.base_url);
        
        let form = multipart::Form::new()
            .text("file", ndjson.to_string())
            .text("overwrite", overwrite.to_string());
        
        self.client
            .post(&url)
            .header("kbn-xsrf", "true")
            .multipart(form)
            .send()
            .await?;
        
        Ok(())
    }
}
```

#### **Authentication**

```rust
pub enum Auth {
    None,
    Basic { username: String, password: String },
    ApiKey { key: String },
}

impl Auth {
    pub fn apply(&self, request: RequestBuilder) -> RequestBuilder {
        match self {
            Auth::None => request,
            Auth::Basic { username, password } => {
                request.basic_auth(username, Some(password))
            }
            Auth::ApiKey { key } => {
                request.header("Authorization", format!("ApiKey {}", key))
            }
        }
    }
}
```

---

### 6. CLI Module (`src/cli.rs`)

Helper functions that compose pipelines for commands.

#### **Pull Pipeline**

```rust
pub async fn pull_saved_objects(output_dir: &str) -> Result<usize> {
    // Load manifest
    let manifest = load_saved_objects_manifest(output_dir)?;
    
    // Create client
    let client = load_kibana_client()?;
    
    // Build pipeline
    let pipeline = Pipeline::new()
        .with_extractor(SavedObjectsExtractor::new(client, manifest))
        .with_transformer(FieldDropper::new(vec!["managed", "updated_at"]))
        .with_transformer(FieldUnescaper::new(vec!["attributes"]))
        .with_loader(DirectoryWriter::new(format!("{}/objects", output_dir)));
    
    // Execute
    pipeline.execute().await
}
```

#### **Push Pipeline**

```rust
pub async fn push_saved_objects(input_dir: &str, managed: bool) -> Result<usize> {
    let client = load_kibana_client()?;
    
    let pipeline = Pipeline::new()
        .with_extractor(DirectoryReader::new(format!("{}/objects", input_dir)))
        .with_transformer(FieldEscaper::new(vec!["attributes"]))
        .with_transformer(ManagedFlagAdder::new(managed))
        .with_loader(SavedObjectsLoader::new(client));
    
    pipeline.execute().await
}
```

---

## Data Flow

### Pull Operation (Kibana → Files)

```
┌─────────────────────────────────────────────────────────────┐
│ 1. Load Manifest                                            │
│    manifest/saved_objects.json → ObjectReference[]         │
└──────────────────────────┬──────────────────────────────────┘
                           │
┌──────────────────────────▼──────────────────────────────────┐
│ 2. SavedObjectsExtractor                                    │
│    POST /api/saved_objects/_export                          │
│    Returns: NDJSON string                                   │
└──────────────────────────┬──────────────────────────────────┘
                           │
┌──────────────────────────▼──────────────────────────────────┐
│ 3. Parse NDJSON → Vec<Value>                               │
│    Parse each line as JSON object                           │
└──────────────────────────┬──────────────────────────────────┘
                           │
┌──────────────────────────▼──────────────────────────────────┐
│ 4. FieldDropper                                             │
│    Remove: managed, updated_at, version, references         │
└──────────────────────────┬──────────────────────────────────┘
                           │
┌──────────────────────────▼──────────────────────────────────┐
│ 5. FieldUnescaper                                           │
│    Convert escaped JSON strings to objects                  │
│    "attributes.kibanaSavedObjectMeta.searchSourceJSON"      │
└──────────────────────────┬──────────────────────────────────┘
                           │
┌──────────────────────────▼──────────────────────────────────┐
│ 6. DirectoryWriter                                          │
│    Write to: objects/{type}/{id}.json                       │
│    Pretty print with 2-space indent                         │
└─────────────────────────────────────────────────────────────┘
```

### Push Operation (Files → Kibana)

```
┌─────────────────────────────────────────────────────────────┐
│ 1. DirectoryReader                                          │
│    Scan: objects/{type}/*.json                              │
│    Returns: Vec<Value>                                      │
└──────────────────────────┬──────────────────────────────────┘
                           │
┌──────────────────────────▼──────────────────────────────────┐
│ 2. FieldEscaper                                             │
│    Convert objects to escaped JSON strings                  │
│    For Kibana compatibility                                 │
└──────────────────────────┬──────────────────────────────────┘
                           │
┌──────────────────────────▼──────────────────────────────────┐
│ 3. ManagedFlagAdder                                         │
│    Add: "managed": true/false                               │
└──────────────────────────┬──────────────────────────────────┘
                           │
┌──────────────────────────▼──────────────────────────────────┐
│ 4. Convert to NDJSON                                        │
│    Serialize each object as single-line JSON                │
│    Join with newlines                                       │
└──────────────────────────┬──────────────────────────────────┘
                           │
┌──────────────────────────▼──────────────────────────────────┐
│ 5. SavedObjectsLoader                                       │
│    POST /api/saved_objects/_import                          │
│    multipart/form-data with NDJSON                          │
│    overwrite=true                                           │
└─────────────────────────────────────────────────────────────┘
```

---

## Extension Points

### Adding New Object Types

1. **Update manifest format** (if needed)
2. **No code changes required!** ETL is object-type agnostic

Example: Add Canvas workpads
```json
{
  "objects": [
    {
      "type": "canvas-workpad",
      "id": "my-workpad-id",
      "attributes": {"title": "My Workpad"}
    }
  ]
}
```

### Adding New Storage Backends

Implement the `Loader` and `Extractor` traits:

```rust
pub struct S3Storage {
    bucket: String,
    prefix: String,
}

#[async_trait]
impl Loader for S3Storage {
    async fn load(&self, data: Vec<Value>) -> Result<usize> {
        // Upload to S3
    }
}

#[async_trait]
impl Extractor for S3Storage {
    async fn extract(&self) -> Result<Vec<Value>> {
        // Download from S3
    }
}
```

### Adding New Transformations

Implement the `Transformer` trait:

```rust
pub struct TitlePrefixer {
    prefix: String,
}

#[async_trait]
impl Transformer for TitlePrefixer {
    async fn transform(&self, mut data: Vec<Value>) -> Result<Vec<Value>> {
        for obj in &mut data {
            if let Some(title) = obj.pointer_mut("/attributes/title") {
                let new_title = format!("{}{}", self.prefix, title);
                *title = Value::String(new_title);
            }
        }
        Ok(data)
    }
}

// Usage:
pipeline
    .with_transformer(TitlePrefixer::new("[PROD] "))
    .execute().await?;
```

### Adding New Commands

1. Add variant to `Commands` enum in `src/main.rs`
2. Create helper function in `src/cli.rs`
3. Wire up in `match` statement

Example: Add `validate` command
```rust
// In src/main.rs
Commands::Validate { dir } => {
    validate_project(&dir).await?;
}

// In src/cli.rs
pub async fn validate_project(dir: &str) -> Result<()> {
    // Load manifest
    let manifest = load_saved_objects_manifest(dir)?;
    
    // Check all referenced files exist
    for obj in &manifest.objects {
        let path = format!("{}/objects/{}/{}.json", dir, obj.type_, obj.id);
        if !Path::new(&path).exists() {
            return Err(eyre!("Missing object file: {}", path));
        }
    }
    
    Ok(())
}
```

---

## Testing Strategy

### Unit Tests

Each module has comprehensive unit tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_field_dropper() {
        let dropper = FieldDropper::new(vec!["managed"]);
        let input = json!({"id": "abc", "managed": true});
        let output = dropper.drop_fields(input);
        assert_eq!(output, json!({"id": "abc"}));
    }
}
```

### Integration Tests

Located in `tests/` directory:

```rust
// tests/etl_integration.rs
#[tokio::test]
async fn test_pull_push_roundtrip() {
    // Create test data
    let temp_dir = TempDir::new()?;
    
    // Pull from Kibana
    pull_saved_objects(temp_dir.path()).await?;
    
    // Verify files exist
    assert!(temp_dir.path().join("objects/dashboard").exists());
    
    // Push back to Kibana
    push_saved_objects(temp_dir.path(), true).await?;
}
```

### Mocking

Use traits for dependency injection:

```rust
pub struct MockExtractor {
    pub data: Vec<Value>,
}

#[async_trait]
impl Extractor for MockExtractor {
    async fn extract(&self) -> Result<Vec<Value>> {
        Ok(self.data.clone())
    }
}

// Test pipeline without real Kibana
let pipeline = Pipeline::new()
    .with_extractor(MockExtractor { data: test_data })
    .with_transformer(FieldDropper::new(vec!["managed"]))
    .with_loader(MockLoader::new());
```

### Test Coverage

Run tests with coverage:
```bash
cargo test --all
cargo tarpaulin --out Html
```

Current coverage: **~85%** (targeting 90%+)

---

## Performance Considerations

### Async I/O

All network and file operations use Tokio for non-blocking I/O:

```rust
// Multiple requests in parallel
let futures = objects.iter()
    .map(|obj| client.fetch_object(obj))
    .collect::<Vec<_>>();

let results = futures::future::join_all(futures).await;
```

### Memory Management

- **Streaming NDJSON parsing** - Don't load entire export into memory
- **Incremental processing** - Transform objects one at a time
- **String interning** - Reuse common strings (type names, field names)

### Connection Pooling

reqwest reuses HTTP connections:
```rust
let client = reqwest::Client::builder()
    .pool_max_idle_per_host(10)
    .build()?;
```

### Benchmarks

```bash
# 100 dashboards, local Kibana
kibob pull ./test-project
# Time: ~2.3s

# 100 dashboards, push
kibob push ./test-project
# Time: ~3.1s

# Memory usage: ~15MB peak
```

---

## Future Architecture Improvements

### Planned Enhancements

1. **Caching Layer**
   - Cache manifests in memory
   - Skip unchanged objects during sync

2. **Incremental Sync**
   - Compare checksums
   - Only transfer changed objects

3. **Parallel Processing**
   - Process multiple objects concurrently
   - Batch API requests

4. **Plugin System**
   - Dynamic transformer loading
   - Custom extractors/loaders as plugins

5. **Observability**
   - Structured logging with tracing
   - Metrics collection
   - OpenTelemetry integration

### Experimental Features

- **Watch mode** - Auto-sync on file changes
- **Bidirectional sync** - Merge changes from both sides
- **Conflict resolution** - Handle concurrent edits
- **Delta encoding** - Transfer only diffs

---

## Contributing

Want to extend kibob? See [CONTRIBUTING.md](../CONTRIBUTING.md) for:
- Development setup
- Code style guidelines
- How to add new features
- Pull request process

---

## Resources

- **Kibana API Docs**: https://www.elastic.co/guide/en/kibana/current/api.html
- **Tokio Docs**: https://tokio.rs/
- **Async Trait**: https://docs.rs/async-trait/
- **reqwest Docs**: https://docs.rs/reqwest/

---

**Questions?** Open an issue: https://github.com/VimCommando/kibana-object-manager/issues
