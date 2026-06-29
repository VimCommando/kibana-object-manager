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
