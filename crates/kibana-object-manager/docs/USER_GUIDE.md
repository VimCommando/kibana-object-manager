# Kibana Object Manager - User Guide

Complete guide to using `kibob` for managing Kibana saved objects with version control.

## Table of Contents

- [Getting Started](#getting-started)
- [Environment Configuration](#environment-configuration)
- [Command Reference](#command-reference)
- [Common Workflows](#common-workflows)
- [Manifest Format](#manifest-format)
- [Troubleshooting](#troubleshooting)

## Getting Started

### Prerequisites

- Rust (for installation from source)
- Access to a Kibana instance
- Credentials (username/password or API key)

### Installation

**From Cargo:**
```bash
cargo install kibana-object-manager
```

**From Source:**
```bash
git clone https://github.com/VimCommando/kibana-object-manager.git
cd kibana-object-manager
cargo build --release
# Binary is at target/release/kibob
```

### Your First Project

1. **Export dashboards from Kibana UI**
   - Navigate to Stack Management → Saved Objects
   - Select your dashboards and visualizations
   - Click "Export" and save as `export.ndjson`

2. **Set up environment**
   ```bash
   export KIBANA_URL=http://localhost:5601
   export KIBANA_USERNAME=elastic
   export KIBANA_PASSWORD=changeme
   ```

3. **Initialize project**
   ```bash
   kibob init export.ndjson ./my-dashboards
   cd my-dashboards
   ```

4. **Initialize Git repository**
   ```bash
   git init
   git add .
   git commit -m "Initial dashboard import"
   ```

You're now tracking your Kibana objects in Git!

## Environment Configuration

### Required Variables

| Variable | Description | Example |
|----------|-------------|---------|
| `KIBANA_URL` | Base URL of your Kibana instance | `http://localhost:5601` |

### Authentication (Choose One)

**Basic Authentication:**
```bash
export KIBANA_USERNAME=elastic
export KIBANA_PASSWORD=changeme
```

**API Key Authentication:**
```bash
export KIBANA_APIKEY=your_base64_encoded_api_key
```

To create an API key in Kibana:
1. Go to Stack Management → API Keys
2. Click "Create API key"
3. Copy the encoded key and set it in `KIBANA_APIKEY`

### Using .env Files

Create a `.env` file in your project:

```bash
# .env - Development environment
KIBANA_URL=http://localhost:5601
KIBANA_USERNAME=elastic
KIBANA_PASSWORD=dev_password
```

Then use the `--env` flag to load it:
```bash
kibob --env .env pull .
```

For multiple environments, create separate files:
```bash
.env.dev      # Development
.env.staging  # Staging
.env.prod     # Production
```

Load them explicitly:
```bash
kibob --env .env.staging push .
```

## Command Reference

### `kibob auth`

Test connection and authentication to Kibana.

**Usage:**
```bash
kibob auth
```

**Example:**
```bash
export KIBANA_URL=http://localhost:5601
export KIBANA_USERNAME=elastic
export KIBANA_PASSWORD=changeme
kibob auth
# Output: ✓ Authorization successful
```

**Common Issues:**
- "Connection refused" → Check if Kibana is running
- "401 Unauthorized" → Verify credentials
- "404 Not Found" → Check KIBANA_URL is correct

---

### `kibob init`

Initialize a new project from a Kibana export file.

**Usage:**
```bash
kibob init [export_file] [output_dir]
```

**Arguments:**
- `export_file` - NDJSON export file (default: `export.ndjson`)
- `output_dir` - Directory to create (default: `manifest.json` in current dir)

**Examples:**

Initialize in a new directory:
```bash
kibob init export.ndjson ./my-dashboards
```

Initialize in current directory:
```bash
kibob init export.ndjson .
```

Use default export file name:
```bash
kibob init
```

**What it creates:**
```
my-dashboards/
├── manifest/
│   └── saved_objects.json
└── objects/
    ├── dashboard/
    │   ├── abc-123.json
    │   └── xyz-789.json
    └── visualization/
        └── def-456.json
```

---

### `kibob pull`

Fetch saved objects from Kibana and update local files.

**Usage:**
```bash
kibob pull [directory]
```

**Arguments:**
- `directory` - Project directory with manifest (default: `.`)

**Examples:**

Pull to current directory:
```bash
kibob pull
```

Pull to specific directory:
```bash
kibob pull ./my-dashboards
```

**Workflow:**
```bash
# Make changes in Kibana UI
# Pull changes to local files
kibob pull .

# Review changes
git diff

# Commit if satisfied
git add .
git commit -m "Update dashboard from Kibana"
```

**What happens:**
1. Reads `manifest/saved_objects.json`
2. Fetches each object from Kibana
3. Updates files in `objects/` directory
4. Removes metadata fields (managed, updated_at, etc.)
5. Unescapes JSON strings for readability

---

### `kibob push`

Upload local saved objects to Kibana.

**Usage:**
```bash
kibob push [directory] [--managed <true|false>]
```

**Arguments:**
- `directory` - Project directory (default: `.`)
- `--managed` - Make objects read-only in Kibana (default: `true`)

**Examples:**

Push as managed (read-only in Kibana):
```bash
kibob push . --managed true
```

Push as unmanaged (editable in Kibana):
```bash
kibob push . --managed false
```

Push to production:
```bash
# Set production environment
export KIBANA_URL=https://prod-kibana.example.com
export KIBANA_APIKEY=prod_api_key

# Deploy as managed objects
kibob push . --managed true
```

**Managed vs. Unmanaged:**

| Managed (`true`) | Unmanaged (`false`) |
|------------------|---------------------|
| Read-only in Kibana UI | Editable in Kibana UI |
| Recommended for production | Useful for development |
| Prevents drift from source | Allows quick iterations |
| Must update via `kibob push` | Can pull changes back |

**What happens:**
1. Reads objects from `objects/` directory
2. Escapes JSON strings for Kibana compatibility
3. Adds `managed` flag to each object
4. Uploads to Kibana via import API
5. Overwrites existing objects with same IDs

---

### `kibob add`

Add objects to an existing manifest.

**Usage:**
```bash
kibob add [directory] [--objects <specs> | --file <export.ndjson>]
```

**Arguments:**
- `directory` - Project directory (default: `.`)
- `--objects` - Comma-separated "type=id" specs
- `--file` - Export file to merge

**Examples:**

Add specific objects by ID:
```bash
kibob add . --objects "dashboard=abc-123,visualization=xyz-789"
```

Merge from export file:
```bash
kibob add . --file new-dashboards.ndjson
```

Add single object:
```bash
kibob add . --objects "index-pattern=logs-*"
```

**Use cases:**
- Add new dashboard created in Kibana
- Merge objects from another team
- Add index patterns or other dependencies

**What happens:**
1. Fetches specified objects from Kibana (if using `--objects`)
2. Merges with existing manifest
3. Saves objects to `objects/` directory
4. Updates `manifest/saved_objects.json`

---

### `kibob togo`

Bundle objects into a distributable NDJSON file.

**Usage:**
```bash
kibob togo [directory] [--managed <true|false>]
```

**Arguments:**
- `directory` - Project directory (default: `.`)
- `--managed` - Set managed flag in bundle (default: `true`)

**Examples:**

Create distributable bundle:
```bash
kibob togo ./my-dashboards
# Creates: my-dashboards/export.ndjson
```

Create unmanaged bundle:
```bash
kibob togo . --managed false
```

**Use cases:**
- Share dashboards with others
- Create release artifacts
- Import into different Kibana instances
- Distribute via package manager

**Output:**
Creates `export.ndjson` in the project directory, which can be:
- Imported via Kibana UI (Stack Management → Saved Objects → Import)
- Used with `kibob init` to create new projects
- Distributed to other teams or customers

---

### `kibob migrate`

Migrate legacy `manifest.json` to new format.

**Usage:**
```bash
kibob migrate [directory] [--backup <true|false>]
```

**Arguments:**
- `directory` - Project directory (default: `.`)
- `--backup` - Create backup of old manifest (default: `true`)

**Examples:**

Migrate with backup:
```bash
kibob migrate ./old-project
# Creates: manifest/saved_objects.json
# Backup: manifest.json.bak
```

Migrate without backup:
```bash
kibob migrate . --no-backup
```

**When to use:**
- Upgrading from Bash-based Kibana Object Manager
- Converting old projects to new format
- After pulling legacy repositories

**What changes:**
- Old: `manifest.json` (flat file)
- New: `manifest/saved_objects.json` (directory structure)

See [MIGRATION.md](MIGRATION.md) for detailed migration guide.

## Common Workflows

### Workflow 1: Single Environment Development

**Scenario:** One developer maintaining dashboards for a single Kibana instance.

```bash
# Initial setup
export KIBANA_URL=http://localhost:5601
export KIBANA_USERNAME=elastic
export KIBANA_PASSWORD=changeme

# Export from Kibana UI and initialize
kibob init export.ndjson ./dashboards
cd dashboards
git init && git add . && git commit -m "Initial commit"

# Daily workflow
# 1. Make changes in Kibana UI
# 2. Pull changes
kibob pull .
git diff  # Review
git add . && git commit -m "Update dashboard"

# 3. Or make changes in files
vim objects/dashboard/my-dash.json
kibob push .
```

### Workflow 2: Multi-Environment Deployment

**Scenario:** Deploy dashboards from dev → staging → production.

```bash
# Set up environment files
cat > .env.dev <<EOF
KIBANA_URL=http://dev-kibana:5601
KIBANA_USERNAME=elastic
KIBANA_PASSWORD=dev_pass
EOF

cat > .env.prod <<EOF
KIBANA_URL=https://prod-kibana.example.com
KIBANA_APIKEY=prod_api_key_here
EOF

# Develop in dev environment
kibob --env .env.dev pull .
# Make changes...
git commit -m "Add new dashboard"

# Deploy to production (managed)
kibob --env .env.prod push . --managed true

# Verify
kibob --env .env.prod auth
```

### Workflow 3: Team Collaboration

**Scenario:** Multiple developers working on dashboards together.

```bash
# Developer A: Create new dashboard
kibob pull .
# Create dashboard in Kibana UI
kibob add . --objects "dashboard=new-dash-id"
git add . && git commit -m "Add sales dashboard"
git push origin main

# Developer B: Pull changes
git pull origin main
kibob push .  # Deploy to their Kibana

# Review in their environment
kibob auth
```

### Workflow 4: Disaster Recovery

**Scenario:** Restore dashboards after accidental deletion.

```bash
# Oh no! Deleted production dashboards
# But we have them in Git!

# Check out last known good version
git log --oneline
git checkout abc123  # Last good commit

# Restore to Kibana
export KIBANA_URL=https://prod-kibana.example.com
export KIBANA_APIKEY=prod_key
kibob push . --managed true

# Verify restoration
kibob pull . --output-dir ./verify
diff -r objects/ verify/objects/
```

### Workflow 5: CI/CD Pipeline

**Scenario:** Automated dashboard deployment in CI/CD.

```yaml
# .github/workflows/deploy-dashboards.yml
name: Deploy Dashboards

on:
  push:
    branches: [main]

jobs:
  deploy:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      
      - name: Install kibob
        run: cargo install kibana-object-manager
      
      - name: Deploy to Production
        env:
          KIBANA_URL: ${{ secrets.KIBANA_URL }}
          KIBANA_APIKEY: ${{ secrets.KIBANA_APIKEY }}
        run: |
          kibob auth
          kibob push ./dashboards --managed true
```

## Manifest Format

### Structure

```json
{
  "version": "1.0",
  "objects": [
    {
      "type": "dashboard",
      "id": "abc-123",
      "attributes": {
        "title": "My Dashboard"
      }
    },
    {
      "type": "visualization", 
      "id": "xyz-789",
      "attributes": {
        "title": "My Visualization"
      }
    }
  ]
}
```

### Fields

- `version` - Manifest format version (currently "1.0")
- `objects` - Array of saved object references
  - `type` - Object type (dashboard, visualization, index-pattern, etc.)
  - `id` - Unique identifier
  - `attributes` - Minimal attributes (usually just title)

### Location

- **New format:** `manifest/saved_objects.json`
- **Legacy format:** `manifest.json` (requires migration)

### Editing Manually

You can manually edit the manifest to:
- Remove objects you no longer want to track
- Add placeholder entries (then use `kibob add --objects`)
- Change tracking scope

After editing, run:
```bash
kibob pull .  # Fetch any new objects
```

## Troubleshooting

### Connection Issues

**Error: "Connection refused"**
```bash
# Check if Kibana is running
curl $KIBANA_URL/api/status
# Check URL is correct
echo $KIBANA_URL
```

**Error: "401 Unauthorized"**
```bash
# Verify credentials
kibob auth
# Check environment variables
env | grep KIBANA
```

**Error: "SSL certificate verify failed"**
```bash
# For development only, accept self-signed certs
# Set in your environment or .env:
KIBANA_VERIFY_SSL=false
```

### Authentication Issues

**Error: "No authentication provided"**
```bash
# Must set either username/password OR apikey
export KIBANA_USERNAME=elastic
export KIBANA_PASSWORD=changeme
# OR
export KIBANA_APIKEY=your_key
```

**Error: "API key conflicts with basic auth"**
```bash
# Can't use both at once, unset one:
unset KIBANA_USERNAME KIBANA_PASSWORD
# Keep only KIBANA_APIKEY
```

### Manifest Issues

**Error: "Manifest not found"**
```bash
# Check you're in the right directory
ls -la manifest/
# Or specify directory explicitly
kibob pull /path/to/project
```

**Error: "Legacy manifest detected"**
```bash
# Migrate to new format
kibob migrate .
```

### Object Issues

**Error: "Object not found: dashboard=abc-123"**
```bash
# Object doesn't exist in Kibana
# Remove from manifest or fetch from another environment
vim manifest/saved_objects.json
# Or pull from source environment first
```

**Error: "Conflict: object already exists"**
```bash
# This is expected - kibob overwrites by default
# No action needed, object will be updated
```

### Permission Issues

**Error: "Insufficient privileges"**
```bash
# Your user/API key needs these Kibana privileges:
# - Read access to Saved Objects
# - Write access to Saved Objects (for push)
# Contact your Kibana administrator
```

### Debug Mode

Enable verbose logging:
```bash
kibob --debug pull .
# Shows detailed HTTP requests and responses
```

### Getting Help

- **GitHub Issues:** https://github.com/VimCommando/kibana-object-manager/issues
- **Discussions:** https://github.com/VimCommando/kibana-object-manager/discussions
- **Documentation:** https://github.com/VimCommando/kibana-object-manager/tree/main/docs

## Next Steps

- [Examples](EXAMPLES.md) - Real-world scenarios and recipes
- [Architecture](ARCHITECTURE.md) - Technical deep-dive
- [Contributing](../CONTRIBUTING.md) - Help improve kibob
- [Quick Reference](QUICK_REFERENCE.md) - Command cheat sheet
