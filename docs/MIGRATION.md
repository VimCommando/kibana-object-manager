# Manifest Migration Guide

## Overview

Starting from version 2.0, Kibana Object Manager uses a directory-based manifest structure instead of a single `manifest.json` file. This allows better organization when managing multiple types of Kibana resources (saved objects, spaces, etc.).

## New Structure

### Legacy (v1.x)
```
project/
├── manifest.json        # Saved objects manifest
└── objects/             # Saved object files
```

### New (v2.0+)
```
project/
├── manifest/
│   ├── saved_objects.json    # Saved objects manifest (required)
│   └── spaces.yml            # Spaces manifest (optional)
├── objects/                  # Saved object files
└── spaces/                   # Space definition files (optional)
```

## Benefits

1. **Extensibility**: Easily add new resource types (workflows, etc.) without cluttering the root directory
2. **Clarity**: Each manifest file has a descriptive name
3. **Organization**: Related configuration lives in one place
4. **Consistency**: Aligns with multi-resource management patterns

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
4. Backs up or removes the old `manifest.json`

### Manual Migration

If you prefer to migrate manually:

```bash
cd /path/to/project

# Create manifest directory
mkdir -p manifest

# Move and rename the manifest
mv manifest.json manifest/saved_objects.json

# (Optional) Add spaces manifest
cat > manifest/spaces.yml <<EOF
spaces:
  - default
EOF
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
