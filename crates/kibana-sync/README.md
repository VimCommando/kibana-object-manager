# kibana-sync

Reusable Kibana sync library for Rust applications.

The crate provides explicit client configuration, authentication helpers, space-aware request routing, endpoint modules for saved objects, spaces, agents, tools, and workflows, capability gates, dependency discovery, and storage-neutral sync models.

```rust,no_run
use kibana_sync::{Auth, KibanaClient};
use url::Url;

# async fn run() -> kibana_sync::Result<()> {
let client = KibanaClient::builder(Url::parse("http://localhost:5601")?)
    .auth(Auth::basic("elastic", "changeme"))
    .max_concurrency(8)
    .build()?;

let default_space = client.space("default")?;
let version = default_space.server_version().await?;
# Ok(())
# }
```

## Bundle sources

Use `KibanaBundle<Filesystem>` for the stable on-disk bundle layout:

```rust,no_run
use kibana_sync::{Filesystem, KibanaBundle};

# fn run() -> kibana_sync::Result<()> {
let bundle: KibanaBundle<Filesystem> = KibanaBundle::open("./kibana-bundle")?;
let resources = bundle.read_all()?;
# let _ = resources;
# Ok(())
# }
```

Embedded or in-memory consumers can provide root-relative paths and any uniform
content type implementing `AsRef<[u8]>`:

```rust,no_run
use kibana_sync::{Entries, KibanaBundle};

# fn run() -> kibana_sync::Result<()> {
let bundle: KibanaBundle<Entries<&'static [u8]>> = KibanaBundle::from_entries([
    (
        "spaces.yml",
        b"spaces:\n  - id: default\n    name: Default\n".as_slice(),
    ),
])?;
let resources = bundle.read_all()?;
# let _ = resources;
# Ok(())
# }
```

The same constructor accepts dynamically collected `(path, Vec<u8>)` entries
without changing bundle parsing or materializing temporary files.
