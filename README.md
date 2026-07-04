# Kibana Object Manager

A Git-inspired CLI tool for managing Kibana saved objects in version control. Built with Rust for reliability, speed, and modern DevOps workflows.

## What is kibob?

`kibob` (Kibana Object Manager) helps you manage Kibana dashboards, visualizations, workflows, agents, tools, spaces, and other Kibana assets using a familiar Git-like workflow. Version control your Kibana artifacts alongside your application code, deploy them across environments, and collaborate with your team using standard Git practices.

### Key Features

- **Git-like workflow** - `pull`, `push`, and version control your Kibana assets
- **Spaces management** - Version control and deploy Kibana spaces alongside assets
- **Workflows, agents, tools, and skills** - Manage newer Kibana APIs alongside saved objects
- **Environment management** - Easy deployment across dev, staging, and production
- **Manifest-based tracking** - Explicitly define which objects, spaces, workflows, agents, tools, and skills to manage
- **Managed vs. unmanaged** - Control whether saved objects can be edited in the Kibana UI
- **Flexible filtering** - Target specific spaces and APIs with `--space` and `--api`
- **Modern architecture** - Built with Rust and a composable ETL pipeline
- **Fast and reliable** - Concurrent requests, proper error handling, and deterministic file layouts

## Installation

Published on:
- Homebrew tap: https://github.com/VimCommando/homebrew-tools
- crates.io: https://crates.io/crates/kibana-object-manager

### From Homebrew

```sh
brew tap VimCommando/tools && brew install kibob
```

### From Cargo

```sh
cargo install kibana-object-manager
```

### From Source

```sh
git clone https://github.com/VimCommando/kibana-object-manager.git
cd kibana-object-manager
cargo build --release
# Binary will be at target/release/kibob
```

## Library Crate

This repository also publishes `kibana-sync` as a standalone library for Rust
applications that need Kibana API behavior without the `kibob` project layout or
CLI policy.

```toml
[dependencies]
kibana-sync = "0.1"
```

```rust,no_run
use kibana_sync::{Auth, KibanaClient};
use url::Url;

# async fn run() -> kibana_sync::Result<()> {
let client = KibanaClient::builder(Url::parse("http://localhost:5601")?)
    .auth(Auth::basic("elastic", "changeme"))
    .max_concurrency(8)
    .spaces([
        ("default".to_string(), "Default".to_string()),
        ("esdiag".to_string(), "ESDiag".to_string()),
    ])
    .build()?;

let esdiag = client.space("esdiag")?;
let version = esdiag.server_version().await?;
# Ok(())
# }
```

`kibana-sync` exposes saved objects, spaces, agents, tools, skills, workflows,
capability gates, dependency discovery, tracing instrumentation, and
storage-neutral sync models. It does not read `spaces.yml`; `kibob` reads that
file in the CLI crate and passes the resulting registry into the library.

## Quick Start

### 1. Set up environment variables

You can either export variables in your shell or store them in a dotenv file and use `--env`.

```sh
export KIBANA_URL=http://localhost:5601
export KIBANA_USERNAME=elastic
export KIBANA_PASSWORD=changeme
# OR use API key authentication:
# export KIBANA_APIKEY=your_api_key_here
```

Example `.env` file:

```sh
KIBANA_URL=http://localhost:5601
KIBANA_USERNAME=elastic
KIBANA_PASSWORD=changeme
KIBANA_MAX_REQUESTS=8
```

### 2. Test your connection

```sh
kibob auth
kibob --env local auth
```

### 3. Initialize a project from an export

First, export your dashboards from Kibana UI (`Stack Management → Saved Objects → Export`).

```sh
kibob init export.ndjson ./my-dashboards
cd my-dashboards
```

This creates:
- `manifest/saved_objects.json` - tracks which saved objects to manage
- `objects/` - directory with your exported objects organized by type

**Optional: Add spaces management**

Create a top-level `spaces.yml` to manage Kibana spaces:

```yml
spaces:
  - id: default
    name: Default
  - id: marketing
    name: Marketing
  - id: engineering
    name: Engineering
```

Now `pull`, `push`, and `togo` can also manage spaces. Each space definition is stored at `{space_id}/space.json`.

**Optional: Add workflows, agents, tools, and skills**

Create per-space manifests like these:

```yml
workflows:
  - id: workflow-123
    name: my-workflow
  - id: workflow-456
    name: alert-workflow
  - id: workflow-789
    name: data-pipeline
```

```yml
agents:
  - id: agent-123
    name: support-agent
```

```yml
tools:
  - id: search-tool
    name: search-tool
```

```yml
skills:
  - id: threat-hunting-copy
    name: threat-hunting-copy
```

Now `pull`, `push`, and `togo` will also manage those APIs for each configured space.

Skills are stored as directories instead of JSON files:

```text
default/
  manifest/
    skills.yml
  skills/
    my-skill--0123456789abcdef/
      SKILL.md
      examples/
        query.md
```

`manifest/skills.yml` lists the tracked Skills for the space by `id` and `name`. Skill directory names are generated as `{sanitized_id}--{stable_hash}` to avoid filesystem collisions; the frontmatter `id` remains authoritative. When the manifest exists, `push` and `togo` include only the listed Skills in manifest order; when it is absent, all `skills/*/SKILL.md` directories are discovered.

`SKILL.md` contains YAML frontmatter with `id`, `name`, `description`, `tool_ids`, and `experimental`; the markdown body is the API `content` field. Additional `.md` files under the skill directory become `referenced_content` entries when bundling or pushing: the filename without `.md` becomes `name`, the parent directory becomes `relativePath` (`examples/query.md` is projected as `./examples`), and the file contents become `content`. The `experimental` field is preserved locally but omitted from create/update API requests because Kibana 9.4 rejects it in request bodies.

### 4. Version control with Git

```sh
git init
git add .
git commit -m "Initial dashboard import"
```

### 5. Pull changes from Kibana

```sh
kibob pull
kibob pull --space default,marketing --api saved_objects,workflows,agents,tools,skills
git diff
git add . && git commit -m "Update from Kibana"
```

### 6. Push to another environment

```sh
export KIBANA_URL=https://prod-kibana.example.com
export KIBANA_APIKEY=prod_api_key

# Deploy as managed objects (read-only in Kibana UI)
kibob push --managed true
```

Or with dotenv files:

```sh
kibob --env stage push --managed false
kibob --env prod push --managed true
```

## CLI Commands

## Global Options

These options work with every command:

- `--env <FILE>` - load environment variables from a dotenv file. Default: `.env`. Shorthand values like `dev`, `stage`, or `prod` resolve to `.env.dev`, `.env.stage`, and `.env.prod`
- `--debug` - enable debug-level logging

## `kibob auth`

Test connection and authentication to Kibana with the current credentials.

Examples:

```sh
kibob auth
kibob --env prod auth
```

## `kibob init [export] [output_dir]`

Initialize a new project from a Kibana NDJSON export file.

Defaults:
- `export` defaults to `export.ndjson`
- `output_dir` defaults to `manifest.json` in the current CLI definition

Examples:

```sh
kibob init export.ndjson ./my-dashboards
kibob init ./exports ./my-dashboards
kibob init
```

Notes:
- If the first argument is a directory, `kibob` looks for `export.ndjson` inside it.
- `init` writes:
  - `manifest/saved_objects.json`
  - `objects/...`

## `kibob pull [dir] [--space <space1,space2,...>] [--api <api1,api2,...>]`

Fetch managed content from Kibana into local files.

Defaults:
- `dir` defaults to `.`
- if `--space` is omitted, `pull` operates on all managed spaces known to the client
- if `--api` is omitted, `pull` processes all supported APIs

Supported API filters:
- `saved_objects`
- `objects`
- `workflows`
- `agents`
- `tools`
- `skills`
- `spaces`

Examples:

```sh
kibob pull
kibob pull --space default,marketing
kibob pull --api saved_objects,workflows,agents,tools,skills
kibob --env dev pull --space default --api spaces
```

Notes:
- `--space` accepts a comma-separated list.
- `spaces` are pulled from top-level `spaces.yml` if it exists.
- Per-space workflows, agents, and tools are pulled when their manifests exist:
  - `{space_id}/manifest/workflows.yml`
  - `{space_id}/manifest/agents.yml`
  - `{space_id}/manifest/tools.yml`
- Per-space skills are pulled from the Skills API and written under `{space_id}/skills/`.
- Skills require Kibana 9.4.0 or newer and are experimental as of Kibana 9.4.

## `kibob push [dir] [--managed true|false] [--space <space1,space2,...>] [--api <api1,api2,...>]`

Upload local content to Kibana.

Defaults:
- `dir` defaults to `.`
- `--managed true`
- if `--space` is omitted, `push` operates on all managed spaces known to the client
- if `--api` is omitted, `push` processes all supported APIs

Supported API filters:
- `saved_objects`
- `objects`
- `workflows`
- `agents`
- `tools`
- `skills`
- `spaces`

Examples:

```sh
kibob push --managed true
kibob push --managed false --space default,marketing
kibob push --api tools,agents,skills
kibob --env prod push --space production --api saved_objects,workflows --managed true
```

Options:
- `--managed true` - saved objects are marked managed/read-only in Kibana UI
- `--managed false` - saved objects remain editable in Kibana UI
- `--space <...>` - comma-separated list of target space IDs
- `--api <...>` - comma-separated list of APIs to push
- Skills are tracked in `{space_id}/manifest/skills.yml` and projected from `{space_id}/skills/*/SKILL.md` directories to Kibana JSON only when pushing.

## `kibob add <api> [dir] [options]`

Add items to an existing manifest.

Supported APIs:
- `objects`
- `workflows`
- `spaces`
- `agents`
- `tools`
- `skills`

Common options:
- `--query <TEXT>` - search query for API-backed discovery
- `--include <REGEX>` - include items whose name matches the regex
- `--exclude <REGEX>` - exclude items whose name matches the regex after include filtering
- `--file <FILE>` - load items from `.json` or `.ndjson`
- `--space <space1,space2,...>` - space selection/filtering
- `--exclude-dependencies` - do not automatically add discovered dependencies for workflows, agents, tools, or skills

Regex notes:
- `--include` and `--exclude` use Rust regex syntax
- include is applied first, then exclude
- use `(?i)` for case-insensitive matching

### Add workflows

```sh
kibob add workflows
kibob add workflows --space marketing
kibob add workflows --query "alert"
kibob add workflows --include "^prod"
kibob add workflows --exclude "test"
kibob add workflows --file export.json
kibob add workflows --exclude-dependencies
```

### Add agents

```sh
kibob add agents
kibob add agents --space default
kibob add agents --include "^support"
kibob add agents --file agents.ndjson
kibob add agents --exclude-dependencies
```

### Add tools

```sh
kibob add tools
kibob add tools --space default
kibob add tools --include "^search"
kibob add tools --file tools.ndjson
kibob add tools --exclude-dependencies
```

### Add skills

```sh
kibob add skill threat-hunting
kibob add skills --space default
kibob add skills --query my-skill-id
kibob add skills --include "^triage"
kibob add skills --file skills.ndjson
kibob add skills --exclude-dependencies
```

The singular shortcut `kibob add skill <skill-id>` fetches that exact Skill ID, tracks it in `{space_id}/manifest/skills.yml`, and writes it as `{space_id}/skills/{skill-directory}/SKILL.md` with referenced markdown files. Skills referenced by agents are written as skill directories, and a skill's `tool_ids` are added as tool dependencies unless `--exclude-dependencies` is used.

### Add spaces

```sh
kibob add spaces
kibob add spaces --include "prod|staging"
kibob add spaces --exclude "(?i)test"
kibob add spaces --space default,marketing
kibob add spaces --file spaces.json
```

### Add objects (legacy saved objects manifest support)

```sh
kibob add objects --objects "dashboard=abc123,visualization=xyz789"
kibob add objects --file export.ndjson
```

Important notes:
- For `objects`, `--objects` is required unless you use `--file`.
- For `spaces`, `--query` is accepted by the CLI, but space discovery currently fetches all spaces and applies filtering afterward.
- For non-`spaces` APIs, the CLI currently uses the first value from `--space` if multiple are supplied.

## `kibob togo [dir] [--managed true|false] [--space <space1,space2,...>] [--api <api1,api2,...>]`

Bundle local content into distributable NDJSON files under `bundle/`.

Defaults:
- `dir` defaults to `.`
- `--managed true`

Supported API filters:
- `saved_objects`
- `objects`
- `workflows`
- `agents`
- `tools`
- `skills`
- `spaces`

Generated outputs can include:
- `bundle/{space_id}/saved_objects.ndjson`
- `bundle/{space_id}/workflows.ndjson`
- `bundle/{space_id}/agents.ndjson`
- `bundle/{space_id}/tools.ndjson`
- `bundle/{space_id}/skills.ndjson`
- `bundle/spaces.ndjson`

Examples:

```sh
kibob togo
kibob togo --space default,marketing
kibob togo --api saved_objects,workflows,agents,tools,skills
zip -r dashboards.zip bundle/
```

Notes:
- `bundle/spaces.ndjson` is generated when top-level `spaces.yml` exists.
- `--api` lets you create partial bundles for specific APIs only.
- `skills.ndjson` is generated from skill directories; JSON is not the at-rest representation.

## `kibob migrate [dir] [--backup true|false]`

Migrate legacy project structure into the multi-space layout.

Defaults:
- `dir` defaults to `.`
- `--backup true`

Examples:

```sh
kibob migrate ./old-project
kibob migrate ./old-project --backup false
kibob --env local migrate
```

Migration notes:
- Legacy content is moved into the target space layout:
  - `{space_id}/manifest/saved_objects.json`
- At runtime the target space is resolved from `KIBANA_SPACE`, falling back to `default`.

## Use Cases

### For Kibana Admins

Back up and version control your dashboards. Easily restore or roll back changes.

### For Developers

Store dashboards and related Kibana assets in your application's Git repository. Deploy observability alongside code.

### For DevOps Engineers

Automate dashboard and asset deployments in CI/CD pipelines. Keep environments consistent from dev to production.

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

```sh
# Pull from dev
kibob --env dev pull --space dev --api saved_objects,workflows,agents,tools
git add .
git commit -m "Sync from dev"

# Push to stage (dev/test posture)
kibob --env stage push --space stage --api saved_objects,workflows,agents,tools --managed false

# Promote to production (production posture)
kibob --env prod push --space prod --api saved_objects,workflows,agents,tools --managed true
```

## Authentication

kibob supports multiple authentication methods.

### Basic Authentication

```sh
export KIBANA_USERNAME=elastic
export KIBANA_PASSWORD=changeme
```

### API Key

```sh
export KIBANA_APIKEY=your_base64_encoded_key
```

## Architecture

kibob uses a modern ETL (Extract-Transform-Load) pipeline architecture:

```text
Pull: Kibana → Extract → Transform → Store Files
Push: Read Files → Transform → Load → Kibana
```

Built with:
- **Rust** - memory-safe, fast, reliable
- **Tokio** - async runtime for efficient I/O
- **reqwest** - HTTP client with connection pooling
- **Clap** - CLI framework
- **serde** - JSON serialization
- **dotenvy** - dotenv loading
- **env_logger** - CLI logging
- **owo-colors** - readable terminal output

## Project Structure

A multi-space project typically looks like this:

```text
my-dashboards/
├── spaces.yml
├── default/
│   ├── space.json
│   ├── manifest/
│   │   ├── saved_objects.json
│   │   ├── workflows.yml
│   │   ├── agents.yml
│   │   └── tools.yml
│   ├── objects/
│   │   ├── dashboard/
│   │   │   ├── abc-123.json
│   │   │   └── xyz-789.json
│   │   ├── visualization/
│   │   │   └── def-456.json
│   │   └── index-pattern/
│   │       └── logs-*.json
│   ├── workflows/
│   │   ├── my-workflow.json
│   │   └── alert-workflow.json
│   ├── agents/
│   │   └── my-agent.json
│   └── tools/
│       └── search-tool.json
├── marketing/
│   ├── space.json
│   ├── manifest/
│   │   ├── workflows.yml
│   │   └── tools.yml
│   ├── workflows/
│   │   └── campaign-workflow.json
│   └── tools/
│       └── campaign-tool.json
└── bundle/
    ├── default/
    │   ├── saved_objects.ndjson
    │   ├── workflows.ndjson
    │   ├── agents.ndjson
    │   └── tools.ndjson
    ├── marketing/
    │   ├── workflows.ndjson
    │   └── tools.ndjson
    └── spaces.ndjson
```

A freshly initialized single-space project from `kibob init` starts simpler:

```text
my-dashboards/
├── manifest/
│   └── saved_objects.json
└── objects/
```

## Managing Kibana Spaces

`kibob` can manage Kibana Spaces alongside saved objects. Create a top-level `spaces.yml`:

```yml
spaces:
  - id: default
    name: Default
  - id: marketing
    name: Marketing
  - id: engineering
    name: Engineering
```

Then use the same workflow:

```sh
kibob pull    # Pulls space definitions to {space_id}/space.json
kibob push    # Creates/updates spaces in Kibana
kibob togo    # Bundles to bundle/spaces.ndjson
```

Each space definition is stored in its own directory as `{space_id}/space.json`. For example:
- `default/space.json`
- `marketing/space.json`
- `engineering/space.json`

See the [Spaces Guide](docs/SPACES.md) for complete documentation.

## Migrating from Bash Version

If you have an existing project using the old Bash script:

```sh
# Migrate the legacy structure
kibob migrate ./my-project

# Review the migrated manifest
cat default/manifest/saved_objects.json

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
| `KIBANA_APIKEY` | API key authentication | Optional |
| `KIBANA_SPACE` | Default target space used by some workflows | `default` |
| `KIBANA_MAX_REQUESTS` | Maximum number of concurrent requests | `8` |

## Support

- **Issues**: https://github.com/VimCommando/kibana-object-manager/issues
- **Discussions**: https://github.com/VimCommando/kibana-object-manager/discussions

## Contributing

Contributions are welcome. See [CONTRIBUTING.md](CONTRIBUTING.md) for development setup and guidelines.

## License

Licensed under the Apache License, Version 2.0. See [LICENSE](LICENSE) for details.
