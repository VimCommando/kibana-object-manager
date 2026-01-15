# Manifest Migration Guide

## Overview

Starting from version 0.1.0 (Rust rewrite), Kibana Object Manager uses:
1. A directory-based manifest structure (`manifest/saved_objects.json` instead of `manifest.json`)
2. Hierarchical object storage (`objects/type/id.json` instead of `objects/id.type.json`)

This provides better organization when managing multiple types of Kibana resources.

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

### New (Rust v0.1.0+)
```
project/
├── manifest/
│   └── saved_objects.json    # Saved objects manifest
└── objects/                  # Hierarchical object files
    ├── dashboard/            # Organized by type
    │   ├── allocation-overview.json
    │   └── data-summary.json
    └── visualization/
        └── test-viz.json
```

## Benefits

1. **Hierarchical organization**: Objects grouped by type in subdirectories
2. **Simpler filenames**: No need for type suffix in filename
3. **Extensibility**: Easily add new resource types (spaces, workflows, etc.)
4. **Clarity**: Each manifest file has a descriptive name
5. **Git-friendly**: Better diffs when objects of different types change

## Migration

### Automatic Migration

Use the `kibob migrate` command to automatically convert your project:

```bash
# Migrate with backup (recommended)
kibob migrate /path/to/project

# Migrate without backup
kibob migrate /path/to/project --no-backup
```

The migration process:
1. Reads your existing `manifest.json`
2. Creates a `manifest/` directory
3. Writes `manifest/saved_objects.json` with the same content
4. Migrates object files from flat to hierarchical structure:
   - `objects/name.type.json` → `objects/type/name.json`
5. Backs up or removes the old `manifest.json`

**Example:**
```bash
# Before migration
objects/
├── sales-dashboard.dashboard.json
├── revenue-chart.visualization.json
└── logs-*.index-pattern.json

# After migration
objects/
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

# Step 1: Create manifest directory and move manifest
mkdir -p manifest
mv manifest.json manifest/saved_objects.json

# Step 2: Reorganize objects into subdirectories
cd objects
for file in *.json; do
  # Extract name and type from filename (name.type.json)
  if [[ $file =~ ^(.+)\.([^.]+)\.json$ ]]; then
    name="${BASH_REMATCH[1]}"
    type="${BASH_REMATCH[2]}"
    
    # Create type directory and move file
    mkdir -p "$type"
    mv "$file" "$type/$name.json"
    echo "Moved $file to $type/$name.json"
  fi
done
```

## Backward Compatibility

The `load_saved_objects_manifest()` function provides automatic backward compatibility:

```rust
use kibana_object_manager::migration::load_saved_objects_manifest;

// Automatically checks both locations:
// 1. manifest/saved_objects.json (new)
// 2. manifest.json (legacy)
let manifest = load_saved_objects_manifest(".")?;
```

This allows tools to work with both old and new project structures without modification.

## Checking Migration Status

To check if a project needs migration:

```bash
kibob migrate /path/to/project
```

The command will report:
- "No legacy manifest.json found" - Nothing to migrate
- "Already migrated" - Project is up to date
- "Migration completed" - Successfully migrated

## File Formats

### saved_objects.json

JSON format that doubles as the Kibana export API payload:

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

### spaces.yml

YAML format listing space IDs to manage:

```yaml
spaces:
  - default
  - marketing
  - engineering
```

## Examples

### Example 1: New Project

When creating a new project, use the new structure:

```bash
mkdir my-kibana-project
cd my-kibana-project

# Create manifest directory
mkdir -p manifest

# Create saved objects manifest
cat > manifest/saved_objects.json <<'EOF'
{
  "objects": [],
  "excludeExportDetails": true,
  "includeReferencesDeep": true
}
EOF

# Create objects directory
mkdir objects
```

### Example 2: Migrating Existing Project

```bash
cd existing-project

# Check current structure
ls -la
# manifest.json ✓
# objects/ ✓

# Run migration
kibob migrate .

# Verify new structure
ls -la
# manifest/ ✓
# manifest.json.backup ✓ (if using --backup)
# objects/ ✓

ls manifest/
# saved_objects.json ✓
```

### Example 3: Adding Spaces Management

After migration, you can add spaces management:

```bash
cd project

# Create spaces manifest
cat > manifest/spaces.yml <<'EOF'
spaces:
  - default
  - team-a
  - team-b
EOF

# Create spaces directory
mkdir -p spaces
```

## API Usage

### Loading Manifests

```rust
use kibana_object_manager::migration::load_saved_objects_manifest;
use kibana_object_manager::kibana::spaces::SpacesManifest;

// Load saved objects (backward compatible)
let saved_objects = load_saved_objects_manifest(".")?;

// Load spaces (new location only)
let spaces = SpacesManifest::read("manifest/spaces.yml")?;
```

### Programmatic Migration

```rust
use kibana_object_manager::migration::{migrate_manifest, needs_migration};

// Check if migration needed
if needs_migration("/path/to/project") {
    // Perform migration
    let result = migrate_manifest("/path/to/project", true)?;
    println!("{}", result);
}
```

## Troubleshooting

### "No saved objects manifest found"

Neither `manifest/saved_objects.json` nor `manifest.json` exists. Create one:

```bash
mkdir -p manifest
cat > manifest/saved_objects.json <<'EOF'
{
  "objects": [],
  "excludeExportDetails": true,
  "includeReferencesDeep": true
}
EOF
```

### "Already migrated"

Both `manifest.json` and `manifest/saved_objects.json` exist. The tool won't migrate to avoid data loss. Either:

1. Remove the old `manifest.json` manually after verifying the migration
2. Delete `manifest/saved_objects.json` and re-run migration

### Migration Failed Midway

If migration fails after creating `manifest/saved_objects.json` but before removing the old file:

1. Check that `manifest/saved_objects.json` is valid
2. Manually remove `manifest.json` or create a backup
3. Or delete `manifest/` and try again

## See Also

- [Saved Objects API Documentation](../kibana/saved_objects/)
- [Spaces API Documentation](../kibana/spaces/)
- [CLI Reference](../cli/)
