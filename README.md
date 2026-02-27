# Kibana Object Manager

A Git-inspired CLI tool for managing Kibana saved objects in version control. Built with Rust for reliability, speed, and modern DevOps workflows.

## What is kibob?

`kibob` (Kibana Object Manager) helps you manage Kibana dashboards, visualizations, and other saved objects using a familiar Git-like workflow. Version control your Kibana artifacts alongside your application code, deploy them across environments, and collaborate with your team using standard Git practices.

### Key Features

- **Git-like workflow** - `pull`, `push`, and version control your Kibana objects
- **Spaces management** - Version control and deploy Kibana spaces alongside objects
- **Workflows management** - Version control and deploy Kibana workflows
- **Environment management** - Easy deployment across dev, staging, and production
- **Manifest-based tracking** - Explicitly define which objects, spaces, and workflows to manage
- **Managed vs. unmanaged** - Control whether objects can be edited in Kibana UI
- **Modern architecture** - Built with async Rust, no external dependencies
- **Fast and reliable** - ETL pipeline design with proper error handling

## Installation

Published on:
- crates.io: https://crates.io/crates/kibana-object-manager
- Homebrew: https://formulae.brew.sh/formula/kibob

From crates.io:
```bash
cargo install kibana-object-manager
```

### From Homebrew

```bash
brew install kibob
```

### From Cargo

```bash
cargo install kibana-object-manager
```

### From Source

```bash
git clone https://github.com/VimCommando/kibana-object-manager.git
cd kibana-object-manager
cargo build --release
# Binary will be at target/release/kibob
```

## Quick Start

### 1. Set up environment variables

```bash
export KIBANA_URL=http://localhost:5601
export KIBANA_USERNAME=elastic
export KIBANA_PASSWORD=changeme
# OR use API key authentication:
# export KIBANA_APIKEY=your_api_key_here
```

### 2. Test your connection

```bash
kibob auth
```

### 3. Initialize a project from an export

First, export your dashboards from Kibana UI (Stack Management → Saved Objects → Export).

```bash
kibob init export.ndjson ./my-dashboards
cd my-dashboards
```

This creates:
- `default/manifest/saved_objects.json` - Tracks which objects to manage
- `default/objects/` - Directory with your objects organized by type

**Optional: Add spaces management**

Create a `spaces.yml` to also manage Kibana spaces:
```yaml
spaces:
  - id: default
    name: Default
  - id: marketing
    name: Marketing
  - id: engineering
    name: Engineering
```

Now `pull` and `push` will also manage spaces! Each space's definition will be stored at `{space_id}/space.json`. See [Spaces Guide](docs/SPACES.md) for details.

**Optional: Add workflows management**

Create per-space workflow manifests like `default/manifest/workflows.yml`:
```yaml
workflows:
  - id: workflow-123
    name: my-workflow
  - id: workflow-456
    name: alert-workflow
  - id: workflow-789
    name: data-pipeline
```

Now `pull` and `push` will also manage workflows!

### 4. Version control with Git

```bash
git init
git add .
git commit -m "Initial dashboard import"
```

### 5. Pull changes from Kibana

```bash
kibob pull .
git diff  # Review changes
git add . && git commit -m "Update from Kibana"
```

### 6. Push to another environment

```bash
# Set production credentials
export KIBANA_URL=https://prod-kibana.example.com
export KIBANA_APIKEY=prod_api_key

# Deploy as managed objects (read-only in Kibana UI)
kibob push . --managed true
```

## CLI Commands

### `kibob auth`
Test connection to Kibana with current credentials.

### `kibob init <export.ndjson> <output_dir>`
Initialize a new project from a Kibana export file.

### `kibob pull <dir> [--space <name>]`
Fetch saved objects from Kibana (specified in manifest) and save to local files. Also pulls spaces if `spaces.yml` exists and workflows if per-space `manifest/workflows.yml` files exist.
- `--space <name>`: Override the Kibana space (overrides KIBANA_SPACE env var)

### `kibob push <dir> [--managed true|false] [--space <name>]`
Upload local objects to Kibana. Also pushes spaces if `spaces.yml` exists and workflows if per-space `manifest/workflows.yml` files exist.
- `--managed true` (default): Objects are read-only in Kibana UI
- `--managed false`: Objects can be edited in Kibana UI
- `--space <name>`: Override the Kibana space (overrides KIBANA_SPACE env var)

### `kibob add <api> <dir> [options]`
Add items to an existing manifest. Supports: `objects`, `workflows`, `spaces`

**For Workflows:**
- `kibob add workflows .` - Search and add all workflows
- `kibob add workflows . --query "alert"` - Search workflows matching "alert"
- `kibob add workflows . --include "^prod"` - Include workflows matching regex "^prod"
- `kibob add workflows . --exclude "test"` - Exclude workflows matching regex "test"
- `kibob add workflows . --include "(?i)prod"` - Case-insensitive include
- `kibob add workflows . --file export.json` - Add from API response file
- `kibob add workflows . --file bundle.ndjson` - Add from bundle file

**For Spaces:**
- `kibob add spaces .` - Fetch and add all spaces
- `kibob add spaces . --include "prod|staging"` - Include spaces matching pattern
- `kibob add spaces . --exclude "(?i)test"` - Exclude test spaces (case-insensitive)
- `kibob add spaces . --file spaces.json` - Add from API response file
- `kibob add spaces . --file bundle.ndjson` - Add from bundle file

**For Objects (legacy):**
- `kibob add objects . --objects "dashboard=abc123,visualization=xyz789"`
- `kibob add objects . --file export.ndjson`

**Regex Filtering:**
- `--include` and `--exclude` accept standard Rust regex patterns
- Include filter is applied first, then exclude filter
- Use `(?i)` prefix for case-insensitive matching
- Examples: `^prod`, `test$`, `(?i)staging`, `dev|test`

### `kibob togo <dir>`
Bundle objects into a distributable `bundle/` directory containing NDJSON files:
- `bundle/{space_id}/saved_objects.ndjson` - Per-space saved objects
- `bundle/spaces.ndjson` - Spaces (if spaces.yml exists)
- `bundle/{space_id}/workflows.ndjson` - Per-space workflows (if manifest/workflows.yml exists)

The bundle directory can be easily zipped for distribution.

### `kibob migrate <dir>`
Migrate legacy `manifest.json` to new `manifest/saved_objects.json` format.

## Use Cases

### For Kibana Admins
Back up and version control your dashboards. Easily restore or roll back changes.

### For Developers
Store dashboards in your application's Git repository. Deploy observability alongside code.

### For DevOps Engineers
Automate dashboard deployments in CI/CD pipelines. Consistent environments from dev to production.

## Documentation

- [User Guide](docs/USER_GUIDE.md) - Comprehensive command reference and workflows
- [Architecture](docs/ARCHITECTURE.md) - Technical deep-dive for contributors
- [Examples](docs/EXAMPLES.md) - Real-world usage scenarios
- [Migration Guide](docs/MIGRATION.md) - Migrating from legacy format
- [Quick Reference](docs/QUICK_REFERENCE.md) - Command cheat sheet
- [Contributing](CONTRIBUTING.md) - Development guidelines

## Agent Skill

This repository includes a Codex skill for `kibob` workflows:
- `skills/kibob/SKILL.md`
- `skills/kibob/references/kibob-commands.md`

The skill is designed to help with:
- Selecting the right `kibob` command and flags
- Environment promotion workflows (`pull` -> `git commit` -> `push`)
- Managed mode policy by environment:
  - Production: `--managed true`
  - Dev/test: `--managed false`

Example promotion flow:

```bash
# Pull from dev
kibob pull . --env .env.dev --space dev --api saved_objects,workflows,agents,tools
git add .
git commit -m "Sync from dev"

# Push to stage (dev/test posture)
kibob push . --env .env.stage --space stage --api saved_objects,workflows,agents,tools --managed false

# Promote to production (production posture)
kibob push . --env .env.prod --space prod --api saved_objects,workflows,agents,tools --managed true
```

## Authentication

kibob supports multiple authentication methods:

### Basic Authentication
```bash
export KIBANA_USERNAME=elastic
export KIBANA_PASSWORD=changeme
```

### API Key
```bash
export KIBANA_APIKEY=your_base64_encoded_key
```

## Architecture

kibob uses a modern ETL (Extract-Transform-Load) pipeline architecture:

```
Pull: Kibana → Extract → Transform → Store Files
Push: Read Files → Transform → Load → Kibana
```

Built with:
- **Rust** - Memory-safe, fast, reliable
- **Tokio** - Async runtime for efficient I/O
- **reqwest** - HTTP client with connection pooling
- **Clap** - Modern CLI framework
- **serde** - Robust JSON serialization

## Project Structure

```
my-dashboards/
├── spaces.yml                # Global: managed spaces list
├── default/                  # Default space (self-contained)
│   ├── space.json           # Space definition
│   ├── manifest/            # Per-space manifests
│   │   ├── saved_objects.json
│   │   ├── workflows.yml
│   │   └── agents.yml
│   ├── objects/             # Saved objects organized by type
│   │   ├── dashboard/
│   │   │   ├── abc-123.json
│   │   │   └── xyz-789.json
│   │   ├── visualization/
│   │   │   └── def-456.json
│   │   └── index-pattern/
│   │       └── logs-*.json
│   ├── workflows/           # Workflow configurations
│   │   ├── my-workflow.json
│   │   └── alert-workflow.json
│   └── agents/              # Agent configurations
│       └── my-agent.json
├── marketing/               # Marketing space (self-contained)
│   ├── space.json
│   ├── manifest/
│   │   └── workflows.yml
│   └── workflows/
│       └── campaign-workflow.json
└── bundle/                  # (Generated by 'togo' command)
    ├── default/
    │   ├── saved_objects.ndjson
    │   ├── workflows.ndjson
    │   └── agents.ndjson
    ├── marketing/
    │   └── workflows.ndjson
    └── spaces.ndjson        # All space definitions
```

## Managing Kibana Spaces

`kibob` can also manage Kibana Spaces alongside saved objects. Create a `spaces.yml`:

```yaml
spaces:
  - id: default
    name: Default
  - id: marketing
    name: Marketing
  - id: engineering
    name: Engineering
```

Then use the same workflow:
```bash
kibob pull .    # Pulls space definitions to {space_id}/space.json
kibob push .    # Creates/updates spaces in Kibana
kibob togo .    # Bundles to bundle/spaces.ndjson
```

Each space's definition is stored in its own directory as `{space_id}/space.json`. For example:
- `default/space.json`
- `marketing/space.json`
- `engineering/space.json`

See the [Spaces Guide](docs/SPACES.md) for complete documentation.

## Migrating from Bash Version

If you have an existing project using the old Bash script:

```bash
# The new Rust version uses a different manifest format
kibob migrate ./my-project

# Review the migrated manifest
cat manifest/saved_objects.json

# Test by pulling from Kibana
kibob pull ./my-project
```

See [Migration Guide](docs/MIGRATION.md) for details.

## Environment Variables Reference

| Variable | Description | Default |
|----------|-------------|---------|
| `KIBANA_URL` | Kibana base URL | Required |
| `KIBANA_USERNAME` | Basic auth username | Optional |
| `KIBANA_PASSWORD` | Basic auth password | Optional |
| `KIBANA_APIKEY` | API key (conflicts with user/pass) | Optional |

## Support

- **Issues**: https://github.com/VimCommando/kibana-object-manager/issues
- **Discussions**: https://github.com/VimCommando/kibana-object-manager/discussions

## Contributing

Contributions are welcome! See [CONTRIBUTING.md](CONTRIBUTING.md) for development setup and guidelines.

## License

Licensed under the Apache License, Version 2.0. See [LICENSE](LICENSE) for details.
