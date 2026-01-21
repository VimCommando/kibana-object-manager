<!-- OPENSPEC:START -->
# OpenSpec Instructions

These instructions are for AI assistants working in this project.

Always open `@/openspec/AGENTS.md` when the request:
- Mentions planning or proposals (words like proposal, spec, change, plan)
- Introduces new capabilities, breaking changes, architecture shifts, or big performance/security work
- Sounds ambiguous and you need the authoritative spec before coding

Use `@/openspec/AGENTS.md` to learn:
- How to create and apply change proposals
- Spec format and conventions
- Project structure and guidelines

Keep this managed block so 'openspec update' can refresh the instructions.

<!-- OPENSPEC:END -->

# Project Guidelines

## Design Philosophy

### CLI-First Experience
This is a CLI-first program designed to feel like a natural companion to familiar `git` commands:
- Commands mirror Git vocabulary: `init`, `pull`, `push`, `add`
- Output formatting follows Git conventions (colored, concise)
- Environment config via `.env` files (like `.git/config` but for credentials)
- `.gitignore` integration is automatic

### Type-State Pattern
Use type-state (state machine) patterns for compile-time safety. The ETL pipeline demonstrates this:
```rust
// Type constraints enforce valid pipelines at compile time
impl<E, T, L> Pipeline<E, T, L>
where
    E: Extractor,
    T: Transformer<Input = E::Item>,  // Input must match extractor output
    L: Loader<Item = T::Output>,       // Input must match transformer output
```
Prefer compile-time guarantees over runtime checks when possible.

## Architecture Patterns

### ETL Pipeline
All data flows follow Extract-Transform-Load:
```
Pull: Kibana API → Extractor → Transformer → Loader → Files
Push: Files → Extractor → Transformer → Loader → Kibana API
```

### Trait-Based Strategy Pattern
Core traits enable pluggable components:
- `Extractor` - Fetches data from sources (async)
- `Transformer` - Modifies data between steps (sync)
- `Loader` - Writes data to destinations (async)

When adding new functionality, implement these traits rather than creating one-off functions.

## Error Handling

Use `eyre` for all error handling:

| Pattern | Use Case |
|---------|----------|
| `.context("message")` | Add static context |
| `.with_context(\|\| format!(...))` | Add dynamic context |
| `eyre::bail!(...)` | Early return with error |
| `eyre::eyre!(...)` | Create error for `.ok_or_else()` |

For API errors, always include status and body:
```rust
if !response.status().is_success() {
    let status = response.status();
    let body = response.text().await.unwrap_or_default();
    eyre::bail!("Failed to fetch data ({}): {}", status, body);
}
```

## Logging Conventions

| Level | Use Case | Example |
|-------|----------|---------|
| `trace!` | HTTP details, internal debug | Request headers, response bodies |
| `debug!` | Operation progress, API paths | "Fetching workflow 'x' from space 'y'" |
| `info!` | User-facing progress/success | "✓ Pulled 5 object(s)" |
| `warn!` | Recoverable issues, skipped items | "Space 'x' not found, skipping" |
| `error!` | Fatal errors before return | "✗ Authorization failed" |

Symbols:
- Success: `✓` (checkmark)
- Failure: `✗` (cross)

## Output Formatting

Use `owo-colors` for consistent coloring:

| Color | Method | Use Case |
|-------|--------|----------|
| Cyan | `.cyan()` | Identifiers (space IDs, workflow names) |
| Green | `.green()` | URLs, HTTP methods, success values |
| Yellow | `.yellow()` | Warning status codes (404, etc.) |
| Bright black | `.bright_black()` | File paths, directories |

No emojis except `✓` and `✗`.

## Async Conventions

- All I/O operations MUST be async via Tokio
- Use `#[tokio::test]` for async tests
- Extractors and Loaders are async; Transformers are sync
- Prefer `.await` over blocking operations

## Testing Patterns

| Pattern | Use Case |
|---------|----------|
| `#[cfg(test)]` module at file bottom | Unit tests |
| `#[tokio::test]` | Async tests |
| `tempfile::TempDir` | File system tests |
| `serial_test` crate | Tests that can't run in parallel |
| `#[ignore]` | Tests requiring live Kibana |
| Mock extractors | Pipeline tests without live API |

## File Format Conventions

| Format | Extension | Use Case |
|--------|-----------|----------|
| JSON5 | `.json` | Local object storage (supports comments, trailing commas) |
| YAML | `.yml` | Manifest files (easier to edit) |
| NDJSON | `.ndjson` | Kibana import/export bundles |

## API Interaction Patterns

When adding new Kibana API integrations:

1. **Create extractor** in `src/kibana/{feature}/extractor.rs`
2. **Create loader** in `src/kibana/{feature}/loader.rs`
3. **Create manifest** in `src/kibana/{feature}/manifest.rs`
4. **Add to mod.rs** re-exports

For internal Kibana APIs, include the required header:
```rust
.header("X-Elastic-Internal-Origin", "Kibana")
```

Always handle pagination for list endpoints.

## Code Organization

**File structure:**
1. Public items at top
2. Private helpers below
3. Tests at bottom in `#[cfg(test)]`

**Doc comments must include:**
- Description
- `# Arguments` - Parameter descriptions
- `# Returns` - Return value description
- `# Errors` - Error conditions
- `# Example` - Usage example (when helpful)