# Manifest Migration Guide

## Overview

Starting from version 0.1.0 (Rust rewrite), Kibana Object Manager uses a **self-contained space directory structure**:
1. Each space is completely self-contained in `{space_id}/` directories
2. Space definitions are co-located at `{space_id}/space.json`
3. Per-space manifests are at `{space_id}/manifest/*.json`
4. Global space list is at root `spaces.yml`

This provides better organization when managing multiple spaces and resource types.

## New Structure

### Legacy (Bash version)
```
project/
├── manifest.json                           # Saved objects manifest
└── objects/                                # Flat object files
    ├── allocation-overview.dashboard.json  # Format: name.type.json
    ├── data-summary.dashboard.json
    └── test-viz.visualization.json
```

### Intermediate (v0.1.0)
```
project/
├── manifest/
│   ├── saved_objects.json    # Saved objects manifest
│   └── spaces.yml            # Spaces manifest
└── objects/                  # Hierarchical object files
    ├── dashboard/            # Organized by type
    │   ├── allocation-overview.json
    │   └── data-summary.json
    └── visualization/
        └── test-viz.json
```

### Current (v0.1.0+, Self-Contained Spaces)
```
project/
├── spaces.yml                # Global spaces list
├── default/                  # Self-contained space
│   ├── space.json           # Space definition
│   ├── manifest/            # Per-space manifests
│   │   ├── saved_objects.json
│   │   ├── workflows.yml
│   │   ├── agents.yml
│   │   └── tools.yml
│   ├── objects/             # Space-specific objects
│   │   ├── dashboard/
│   │   └── visualization/
│   ├── workflows/
│   ├── agents/
│   └── tools/
└── bundle/
    ├── default/
    │   ├── saved_objects.ndjson
    │   └── workflows.ndjson
    └── spaces.ndjson
```

## Benefits

1. **Self-contained spaces**: Each space directory contains everything related to that space
2. **Hierarchical organization**: Objects grouped by type in subdirectories
3. **Simpler filenames**: No need for type suffix in filename
4. **Extensibility**: Easily add new resource types (spaces, workflows, agents, tools)
5. **Clarity**: Each manifest file has a descriptive name and location
6. **Git-friendly**: Better diffs when objects of different types change
7. **Co-located definitions**: Space definition lives with its resources

## Migration

### Automatic Migration

Use the `kibob migrate` command to automatically convert your project:

```bash
# Migrate with backup (recommended)
kibob migrate /path/to/project

# Migrate without backup
kibob migrate /path/to/project --no-backup

# Migrate to a custom space (uses KIBANA_SPACE env var)
KIBANA_SPACE=production kibob migrate /path/to/project
```

The migration process:
1. Reads your existing `manifest.json` or `manifest/saved_objects.json`
2. Creates a space directory (default: `default/`, or from `KIBANA_SPACE` env var)
3. Creates `{space}/manifest/saved_objects.json`
4. Migrates object files to `{space}/objects/type/name.json`
5. If spaces existed at `manifest/spaces.yml`, moves to root `spaces.yml`
6. Cleans up empty `manifest/` directory
7. Backs up or removes old files

**Migration Paths:**

**From Legacy (manifest.json):**
```
manifest.json          → {space}/manifest/saved_objects.json
objects/name.type.json → {space}/objects/type/name.json
```

**From v0.1.0 (manifest/ directory):**
```
manifest/saved_objects.json → {space}/manifest/saved_objects.json
manifest/spaces.yml         → spaces.yml (root)
objects/type/name.json      → {space}/objects/type/name.json
```

**Example:**
```bash
# Before migration (legacy)
objects/
├── sales-dashboard.dashboard.json
├── revenue-chart.visualization.json
└── logs-*.index-pattern.json

# After migration (default space)
default/
├── manifest/
│   └── saved_objects.json
└── objects/
    ├── dashboard/
    │   └── sales-dashboard.json
    ├── visualization/
    │   └── revenue-chart.json
    └── index-pattern/
        └── logs-*.json
```

### Manual Migration

If you prefer to migrate manually:

```bash
cd /path/to/project

# Step 1: Create default space directory structure
mkdir -p default/manifest
mkdir -p default/objects

# Step 2: Move manifest
if [ -f manifest.json ]; then
  mv manifest.json default/manifest/saved_objects.json
elif [ -f manifest/saved_objects.json ]; then
  mv manifest/saved_objects.json default/manifest/saved_objects.json
fi

# Step 3: Move spaces manifest to root if it exists
if [ -f manifest/spaces.yml ]; then
  mv manifest/spaces.yml spaces.yml
fi

# Step 4: Reorganize objects into space directory
if [ -d objects ]; then
  # If objects are already hierarchical
  mv objects/* default/objects/
  rmdir objects
else
  # If objects are flat (legacy format)
  cd objects
  for file in *.json; do
    if [[ $file =~ ^(.+)\.([^.]+)\.json$ ]]; then
      name="${BASH_REMATCH[1]}"
      type="${BASH_REMATCH[2]}"
      
      mkdir -p "../default/objects/$type"
      mv "$file" "../default/objects/$type/$name.json"
      echo "Moved $file to default/objects/$type/$name.json"
    fi
  done
  cd ..
  rmdir objects
fi

# Step 5: Clean up empty manifest directory
if [ -d manifest ] && [ -z "$(ls -A manifest)" ]; then
  rmdir manifest
fi
```

## Backward Compatibility

The migration functions provide automatic backward compatibility:

```rust
use kibana_object_manager::migration::{needs_migration_unified, migrate_to_multispace_unified};

// Automatically detects legacy formats and migrates to self-contained space structure
if needs_migration_unified(".", "default") {
    migrate_to_multispace_unified(".", true)?;
}
```

This allows tools to work with legacy, intermediate, and current project structures.

## Checking Migration Status

To check if a project needs migration:

```bash
kibob migrate /path/to/project
```

The command will report:
- "No migration needed" - Project is already using self-contained space structure
- "Migration completed" - Successfully migrated from legacy or v0.1.0 format

## File Formats

### saved_objects.json

JSON format that doubles as the Kibana export API payload (now per-space):

```json
{
  "objects": [
    {
      "type": "dashboard",
      "id": "my-dashboard-id"
    },
    {
      "type": "visualization",
      "id": "my-viz-id"
    }
  ],
  "excludeExportDetails": true,
  "includeReferencesDeep": true
}
```

Location: `{space_id}/manifest/saved_objects.json`

### spaces.yml

YAML format listing spaces to manage:

```yaml
spaces:
  - id: default
    name: Default
  - id: marketing
    name: Marketing Team
  - id: engineering
    name: Engineering
```

Location: `spaces.yml` (root level)

## Examples

### Example 1: New Project

When creating a new project, use the self-contained space structure:

```bash
mkdir my-kibana-project
cd my-kibana-project

# Create default space directory
mkdir -p default/manifest
mkdir -p default/objects

# Create saved objects manifest
cat > default/manifest/saved_objects.json <<'EOF'
{
  "objects": [],
  "excludeExportDetails": true,
  "includeReferencesDeep": true
}
EOF

# Create spaces manifest
cat > spaces.yml <<'EOF'
spaces:
  - id: default
    name: Default
EOF

# Create space definition
cat > default/space.json <<'EOF'
{
  "id": "default",
  "name": "Default",
  "description": "Default space"
}
EOF
```

### Example 2: Migrating Existing Project

```bash
cd existing-project

# Check current structure
ls -la
# manifest.json ✓ (legacy)
# objects/ ✓

# Run migration
kibob migrate .

# Verify new structure
ls -la
# default/ ✓ (self-contained space)
# manifest.json.backup ✓ (if using --backup)

ls default/
# space.json ✓
# manifest/ ✓
# objects/ ✓

ls default/manifest/
# saved_objects.json ✓
```

### Example 3: Adding Multiple Spaces

After initial setup, you can add more spaces:

```bash
cd project

# Update spaces manifest
cat > spaces.yml <<'EOF'
spaces:
  - id: default
    name: Default
  - id: production
    name: Production
  - id: staging
    name: Staging
EOF

# Create production space
mkdir -p production/manifest
mkdir -p production/objects

cat > production/space.json <<'EOF'
{
  "id": "production",
  "name": "Production",
  "description": "Production environment - monitored 24/7"
}
EOF

cat > production/manifest/saved_objects.json <<'EOF'
{
  "objects": [],
  "excludeExportDetails": true,
  "includeReferencesDeep": true
}
EOF

# Create staging space
mkdir -p staging/manifest
mkdir -p staging/objects

cat > staging/space.json <<'EOF'
{
  "id": "staging",
  "name": "Staging",
  "description": "Staging environment for testing"
}
EOF

cat > staging/manifest/saved_objects.json <<'EOF'
{
  "objects": [],
  "excludeExportDetails": true,
  "includeReferencesDeep": true
}
EOF
```

## API Usage

### Loading Manifests

```rust
use kibana_object_manager::migration::load_saved_objects_manifest;
use kibana_object_manager::space_context::SpaceContext;
use std::path::Path;

// Load space context (reads spaces.yml)
let ctx = SpaceContext::load(Path::new("."), None)?;

// Load saved objects for a specific space
let space_manifest = load_saved_objects_manifest_for_space(Path::new("."), "default")?;
```

### Programmatic Migration

```rust
use kibana_object_manager::migration::{migrate_to_multispace_unified, needs_migration_unified};

// Check if migration needed
if needs_migration_unified("/path/to/project", "default") {
    // Perform migration
    migrate_to_multispace_unified("/path/to/project", true)?;
    println!("Migration completed");
}
```

## Troubleshooting

### "No saved objects manifest found"

Neither per-space manifest nor legacy manifest exists. Create one:

```bash
mkdir -p default/manifest
cat > default/manifest/saved_objects.json <<'EOF'
{
  "objects": [],
  "excludeExportDetails": true,
  "includeReferencesDeep": true
}
EOF
```

### "Already migrated"

Project is already using self-contained space structure. No action needed.

### "KIBANA_SPACE is deprecated"

If you see this warning during migration:
```
Warning: KIBANA_SPACE environment variable is deprecated for migration. 
The 'production' space has been created, but future operations should use --space flag.
```

This means you used `KIBANA_SPACE=production kibob migrate .` to migrate to a non-default space. While this works, it's recommended to:
1. Use the default migration: `kibob migrate .`
2. Then use `--space` flag for operations: `kibob pull . --space production`

### Migration Failed Midway

If migration fails partway through:

1. Check that `{space}/manifest/saved_objects.json` is valid JSON
2. Verify the space directory structure is correct
3. Look for backup files (`.backup` suffix) if migration used `--backup`
4. If needed, restore from backup and try again
5. Check logs for specific error messages

## See Also

- [Saved Objects API Documentation](../kibana/saved_objects/)
- [Spaces API Documentation](../kibana/spaces/)
- [CLI Reference](../cli/)
