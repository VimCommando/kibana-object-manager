# Design: Space-Aware Migration and Env Transformation

## Space Awareness in Migration
The `migrate_to_multispace_unified` function in `src/migration.rs` will be enhanced:
1.  **Space ID Resolution**:
    *   Check `std::env::var("kibana_space")` (lowercase).
    *   Fallback to `std::env::var("KIBANA_SPACE")` (uppercase).
    *   Fallback to "default".
2.  **Space Fetching**:
    *   During migration, if a Kibana connection can be established, it will fetch the space definition for the detected space ID.
    *   It will write this to `{space_id}/space.json`.
3.  **Spaces Manifest**:
    *   It will ensure the space is registered in the root `spaces.yml`.

## Env File Transformation
A new utility module or function will be created to handle `.env` updates:
1.  **Parsing**: Read the `.env` file line by line.
2.  **UPPERCASING**: Any line matching `key=value` (case-insensitive) will have its `key` converted to UPPERCASE.
3.  **KIBANA_SPACE Handling**:
    *   Locate the line containing `KIBANA_SPACE` (after uppercasing).
    *   Insert `# Commented out by Kibana Migrate, now use spaces.yml and space directories` above it.
    *   Comment out the line itself: `# KIBANA_SPACE=...`.

## CLI Changes
The `Migrate` subcommand in `src/main.rs` needs to pass the `.env` file path to the migration logic.
The `migrate_to_multispace_unified` signature will be updated:
`pub async fn migrate_to_multispace_unified(project_dir: impl AsRef<Path>, backup_old: bool, env_path: Option<impl AsRef<Path>>) -> Result<MigrationResult>`
Note: It becomes `async` because it needs to fetch the space definition from Kibana.
