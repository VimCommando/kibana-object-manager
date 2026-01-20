# Project Context

## Purpose

**Kibana Object Manager (`kibob`)** is a Git-inspired CLI tool for managing Kibana saved objects in version control. Built with Rust for reliability, speed, and modern DevOps workflows.

### Goals
- Version control Kibana dashboards, visualizations, spaces, workflows, agents, and tools
- Provide Git-like workflow with `pull`, `push`, `init`, `add`, `togo`, and `migrate` commands
- Enable manifest-based tracking of which objects to manage
- Support environment deployment across dev, staging, and production
- Handle managed (read-only) vs unmanaged objects in Kibana UI

### Target Users
- **Kibana Admins** - Backup and version control dashboards
- **Developers** - Store dashboards alongside application code
- **DevOps Engineers** - Automate dashboard deployments in CI/CD pipelines

## Tech Stack

### Language
- **Rust** (Edition 2024, minimum version 1.89)
- Binary name: `kibob`

### Core Dependencies
| Dependency | Purpose |
|------------|---------|
| `tokio` | Async runtime with multi-threaded scheduler |
| `reqwest` | HTTP client for Kibana API |
| `clap` | CLI framework with derive macros |
| `serde` / `serde_json` / `serde_yaml` | Serialization |
| `eyre` | Error handling with context |
| `regex` | Pattern matching |
| `base64` | Authentication encoding |
| `dotenvy` | Environment variable loading |
| `owo-colors` | Terminal colors |
| `json5` | Extended JSON parsing (comments, trailing commas) |

### Dev Dependencies
| Dependency | Purpose |
|------------|---------|
| `tempfile` | Temporary directories for tests |
| `serial_test` | Sequential test execution |

## Project Conventions

### Code Style

**Formatting:** `cargo fmt` (standard rustfmt defaults)
**Linting:** `cargo clippy` with `-D warnings` (deny all warnings)

**Naming Conventions:**
| Element | Convention | Examples |
|---------|------------|----------|
| Structs/Traits | PascalCase | `Kibana`, `SavedObjectsExtractor` |
| Functions/Variables | snake_case | `extract()`, `output_dir` |
| Constants | SCREAMING_SNAKE_CASE | `STYLES` |
| Modules | snake_case | `saved_objects`, `field_dropper` |
| Enum variants | PascalCase | `Commands::Pull`, `Auth::Basic` |

**Documentation:**
- Public APIs use `///` doc comments
- Module docs use `//!` at file top
- Include: Description, `# Arguments`, `# Returns`, `# Errors`, `# Example`

**File Organization:**
- Public items at top of module
- Private helpers below
- Tests at bottom in `#[cfg(test)]` module

### Architecture Patterns

**ETL Pipeline Pattern:**
```
Pull: Kibana API → Extract → Transform → Store Files
Push: Read Files → Transform → Load → Kibana API
```

**Directory Structure:**
```
src/
├── main.rs              # CLI entry point
├── lib.rs               # Library re-exports
├── cli.rs               # CLI helpers (pipeline composition)
├── client/              # HTTP client (auth, Kibana client)
├── etl/                 # Core ETL traits (Extractor, Transformer, Loader)
├── kibana/              # Kibana-specific implementations
│   ├── saved_objects/   # Dashboards, visualizations, etc.
│   ├── spaces/          # Kibana spaces
│   ├── workflows/       # Kibana workflows
│   ├── agents/          # Kibana agents
│   └── tools/           # Kibana tools
├── storage/             # File system operations
└── transform/           # Data transformations
```

**Key Traits:**
- `Extractor` - Fetches data from sources (async)
- `Transformer` - Modifies data between extraction and loading
- `Loader` - Writes data to destinations (async)

**Design Principles:**
1. Modularity - Loosely coupled components with clear interfaces
2. Extensibility - Easy to add new object types, storage backends
3. Type Safety - Leverage Rust's type system
4. Async First - Non-blocking I/O with Tokio
5. Explicit Over Implicit - Clear data flow
6. Testability - Every component can be tested in isolation

### Testing Strategy

**Test Types:**
- **Unit Tests** - In-module with `#[cfg(test)]`, test individual functions
- **Async Tests** - Using `#[tokio::test]` for async operations
- **Integration Tests** - In `tests/` directory for end-to-end workflows

**Commands:**
```bash
cargo test --all                    # Run all tests
cargo test --all -- --nocapture     # Verbose output
cargo test --test '*'               # Integration tests only
cargo test -- --ignored             # Tests requiring live Kibana
```

**Coverage:** Target 85%+ with `cargo tarpaulin`

**Patterns:**
- Mock extractors for pipeline testing without live Kibana
- `tempfile::TempDir` for file system tests
- `serial_test` for tests that can't run in parallel
- `#[ignore]` for tests requiring real Kibana instance

### Git Workflow

**Branch Naming:**
- `feature/add-status-command` - New features
- `fix/handle-empty-manifest` - Bug fixes
- `docs/improve-quickstart` - Documentation
- `refactor/simplify-pipeline` - Refactoring
- `test/add-integration-tests` - Test improvements

**Commit Messages (Conventional Commits):**
```
<type>(<scope>): <subject>
```
Types: `feat`, `fix`, `docs`, `style`, `refactor`, `test`, `chore`

**Pre-Commit Checks:**
```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test --all
cargo build --release
```

**Versioning:** Semantic Versioning (SemVer), currently 0.1.0

## Domain Context

### Kibana Concepts
- **Saved Objects** - Dashboards, visualizations, index patterns stored in Kibana
- **Spaces** - Isolated workspaces in Kibana for organizing objects
- **Workflows** - Automation connectors (formerly called "actions")
- **Agents** - AI/LLM-powered assistants in Kibana
- **Tools** - Functions that agents can invoke
- **Managed Objects** - Read-only objects that can't be edited in Kibana UI

### File Formats
- **NDJSON** - Newline-delimited JSON for Kibana import/export
- **JSON5** - Extended JSON for local storage (comments, trailing commas)
- **YAML** - For manifest files (`workflows.yml`, `spaces.yml`)

### Project File Structure
```
my-dashboards/
├── spaces.yml                    # Global: managed spaces
├── {space_id}/                   # Per-space directory
│   ├── space.json               # Space definition
│   ├── manifest/                # Per-space manifests
│   ├── objects/                 # Saved objects by type
│   ├── workflows/               # Workflow JSON files
│   ├── agents/                  # Agent JSON files
│   └── tools/                   # Tool JSON files
└── bundle/                      # Generated by 'togo' command
```

## Important Constraints

1. **No External CLI Dependencies** - Must not require jsrmx, curl, jq, grep, or other tools
2. **Rust Edition 2024** - Minimum Rust version 1.89
3. **Async Runtime** - All I/O operations must be async via Tokio
4. **Backward Compatibility** - Support migration from legacy manifest formats
5. **Cross-Platform** - Must work on macOS, Linux, and Windows

## External Dependencies

### Kibana REST API
Primary integration point for all operations.

**Key Endpoints:**
- `GET /api/status` - Connection test
- `POST /api/saved_objects/_export` - Export saved objects
- `POST /api/saved_objects/_import` - Import saved objects
- `GET/POST/PUT /api/spaces/space` - Space management
- `GET/POST/PUT /api/actions/connector` - Workflow management
- Internal APIs for agents/tools (require `X-Elastic-Internal-Origin: Kibana` header)

### Environment Configuration

**Environment Variables:**
| Variable | Description | Required |
|----------|-------------|----------|
| `KIBANA_URL` | Kibana base URL | Yes |
| `KIBANA_USERNAME` | Basic auth username | No* |
| `KIBANA_PASSWORD` | Basic auth password | No* |
| `KIBANA_APIKEY` | API key authentication | No* |

*One authentication method required (Basic Auth OR API Key)

**Multi-Environment Setup with `.env` Files:**

The CLI loads environment variables from `.env` files using the `--env` flag (defaults to `.env`). This enables managing multiple Kibana environments:

```bash
# Use default .env file
kibob pull

# Target specific environments
kibob --env .env.dev pull
kibob --env .env.staging push
kibob --env .env.prod push
```

**Example `.env` file structure:**
```bash
# .env.dev
KIBANA_URL=https://dev-kibana.example.com
KIBANA_USERNAME=admin
KIBANA_PASSWORD=dev-password

# .env.prod (using API key instead)
KIBANA_URL=https://prod-kibana.example.com
KIBANA_APIKEY=your-base64-encoded-api-key
```

**Security:**
- `.env*` files are automatically added to `.gitignore` by `kibob init`
- Never commit credentials to version control
- Use API keys in production for better security and auditability
