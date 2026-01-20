## Context

Kibana spaces are a fundamental concept - every API call is scoped to a space. The current design passes `space_id` as a separate parameter everywhere, requiring duplicate `*_with_space` method variants. This refactor integrates space awareness directly into the client.

## Goals / Non-Goals

**Goals:**
- Single source of truth for space validation (the client)
- Cleaner API: `client.space("id")?.get("/api/...")` instead of `client.get_with_space("id", "/api/...")`
- Reduce boilerplate in extractors/loaders (remove stored `space_id` field)
- Eliminate `SpaceContext` module redundancy

**Non-Goals:**
- Changing the Kibana API interaction patterns
- Modifying the ETL trait signatures
- Adding new Kibana API capabilities

## Decisions

### Decision 1: Two-struct design (`KibanaClient` + `SpaceClient`)

**What:** Root client holds shared resources; space-bound client is a lightweight view.

```rust
pub struct KibanaClient {
    client: Client,                    // reqwest::Client (internally Arc'd)
    url: Url,
    spaces: HashMap<String, String>,   // id -> name from spaces.yml
}

pub struct SpaceClient {
    client: Client,                    // Clone of reqwest::Client (cheap)
    url: Url,
    space: Option<String>,             // None = default space
}
```

**Why:** Enables the builder-lite pattern while keeping each space client lightweight. `reqwest::Client` is designed to be cloned cheaply (internally `Arc`).

**Alternatives considered:**
- Single struct with `space: Option<String>`: Would require passing the root client everywhere for space validation
- `Arc<KibanaClient>` with `&self` methods: More complex lifetime management

### Decision 2: Space validation at `.space()` call time

**What:** `kibana.space("unknown")` returns `Err` if space not in loaded manifest.

**Why:** Fail-fast prevents runtime errors deep in extractors/loaders. CLI layer can catch and report clearly.

### Decision 3: Load spaces from disk (`spaces.yml`) at client creation

**What:** `KibanaClient::try_new()` takes `project_dir` and loads `SpacesManifest`.

```rust
impl KibanaClient {
    pub fn try_new(url: Url, auth: Auth, project_dir: impl AsRef<Path>) -> Result<Self>;
}
```

**Why:** Matches current `SpaceContext` behavior. Keeps the client as the single authority on available spaces.

**Alternatives considered:**
- Fetch from Kibana API: Would require async constructor and live connectivity
- Deferred loading: Would push validation to call sites

### Decision 4: Remove all `*_with_space` method variants

**What:** `SpaceClient` has only non-suffixed methods (`get`, `post_json`, etc.)

**Why:** Space is already baked into the client - suffix is redundant and noisy.

## Data Flow (After Refactor)

```
CLI
 |
 +- Load KibanaClient::try_new(url, auth, project_dir)
 |   +- Loads spaces.yml into HashMap<id, name>
 |
 +- For each target space:
 |   +- let space_client = kibana.space("shanks")?
 |   |   +- Validates space exists, returns SpaceClient
 |   |
 |   +- WorkflowsExtractor::new(space_client.clone(), manifest)
 |   |   +- No more space_id field
 |   |
 |   +- space_client.get_internal("/api/workflows/...")
 |       +- Automatically prefixes /s/shanks/ if needed
```

## Risks / Trade-offs

| Risk | Mitigation |
|------|------------|
| Large refactor touches many files | Incremental: client first, then extractors, then loaders, then CLI |
| Breaking API for extractors/loaders | All internal code, no external consumers |
| `spaces.yml` must exist or default used | Same behavior as current `SpaceContext` |

## Migration Plan

1. Refactor `src/client/kibana.rs` to new structure
2. Update `src/client/mod.rs` exports
3. Update each extractor (5 files)
4. Update each loader (5 files)
5. Update CLI orchestration
6. Delete `src/space_context.rs`
7. Update `src/lib.rs` exports
8. Run full test suite

## Open Questions

None - all design decisions confirmed with user.
