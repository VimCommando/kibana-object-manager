# Kibana Object Manager

A Git-inspired CLI tool for managing Kibana saved objects in version control. Built with Rust for reliability, speed, and modern DevOps workflows.

## What is kibob?

`kibob` (Kibana Object Manager) helps you manage Kibana dashboards, visualizations, and other saved objects using a familiar Git-like workflow. Version control your Kibana artifacts alongside your application code, deploy them across environments, and collaborate with your team using standard Git practices.

### Key Features

- **Git-like workflow** - `pull`, `push`, and version control your Kibana objects
- **Spaces management** - Version control and deploy Kibana spaces alongside objects
- **Environment management** - Easy deployment across dev, staging, and production
- **Manifest-based tracking** - Explicitly define which objects and spaces to manage
- **Managed vs. unmanaged** - Control whether objects can be edited in Kibana UI
- **Modern architecture** - Built with async Rust, no external dependencies
- **Fast and reliable** - ETL pipeline design with proper error handling

## Installation

### From Cargo (Recommended)

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

### Future: Homebrew

```bash
brew install kibob  # Coming soon!
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
- `manifest/saved_objects.json` - Tracks which objects to manage
- `objects/` - Directory with your objects organized by type

**Optional: Add spaces management**

Create a `manifest/spaces.yml` to also manage Kibana spaces:
```yaml
spaces:
  - default
  - marketing
  - engineering
```

Now `pull` and `push` will also manage spaces! See [Spaces Guide](docs/SPACES.md) for details.

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
Fetch saved objects from Kibana (specified in manifest) and save to local files. Also pulls spaces if `manifest/spaces.yml` exists.
- `--space <name>`: Override the Kibana space (overrides KIBANA_SPACE env var)

### `kibob push <dir> [--managed true|false] [--space <name>]`
Upload local objects to Kibana. Also pushes spaces if `manifest/spaces.yml` exists.
- `--managed true` (default): Objects are read-only in Kibana UI
- `--managed false`: Objects can be edited in Kibana UI
- `--space <name>`: Override the Kibana space (overrides KIBANA_SPACE env var)

### `kibob add <dir> [--objects <specs> | --file <export.ndjson>]`
Add objects to an existing manifest.
- `--objects "dashboard=abc123,visualization=xyz789"`
- `--file export.ndjson`

### `kibob togo <dir>`
Bundle objects into a distributable `bundle/` directory containing NDJSON files:
- `bundle/saved_objects.ndjson` - Saved objects
- `bundle/spaces.ndjson` - Spaces (if manifest/spaces.yml exists)

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

### Custom Space
```bash
export KIBANA_SPACE=my-space  # Defaults to 'default'
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
├── manifest/
│   ├── saved_objects.json    # Tracks which objects to manage
│   └── spaces.yml            # (Optional) Tracks which spaces to manage
├── objects/                  # Saved objects organized by type
│   ├── dashboard/
│   │   ├── abc-123.json
│   │   └── xyz-789.json
│   ├── visualization/
│   │   └── def-456.json
│   └── index-pattern/
│       └── logs-*.json
├── spaces/                   # (Optional) Space configurations
│   ├── default.json
│   ├── marketing.json
│   └── engineering.json
└── bundle/                   # (Generated by 'togo' command)
    ├── saved_objects.ndjson  # Bundled objects
    └── spaces.ndjson         # Bundled spaces
```

## Managing Kibana Spaces

`kibob` can also manage Kibana Spaces alongside saved objects. Create a `manifest/spaces.yml`:

```yaml
spaces:
  - default
  - marketing
  - engineering
```

Then use the same workflow:
```bash
kibob pull .    # Pulls spaces to spaces/*.json
kibob push .    # Creates/updates spaces in Kibana
kibob togo .    # Bundles to bundle/spaces.ndjson
```

Each space is stored as a pretty-printed JSON file. See the [Spaces Guide](docs/SPACES.md) for complete documentation.

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
| `KIBANA_SPACE` | Kibana space ID | `default` |

## Contributing

Contributions are welcome! See [CONTRIBUTING.md](CONTRIBUTING.md) for development setup and guidelines.

## License

Licensed under the Apache License, Version 2.0. See [LICENSE](LICENSE) for details.

## Support

- **Issues**: https://github.com/VimCommando/kibana-object-manager/issues
- **Discussions**: https://github.com/VimCommando/kibana-object-manager/discussions

## Acknowledgments

This is a complete rewrite of the original Bash-based Kibana Object Manager, reimagined with modern development practices and a robust architecture.

Built by [Ryan Eno](https://github.com/VimCommando).
