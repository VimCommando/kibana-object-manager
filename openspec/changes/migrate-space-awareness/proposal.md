# Proposal: Enhance Migrate Command with Space Awareness and Env Updates

## Why
The current `kibob migrate` command performs a basic migration of saved objects but lacks support for space-specific configurations and environment variable management. As the project matures towards a multi-space architecture, migration needs to handle space definitions and clean up legacy environment variables to prevent confusion.

## What Changes
1.  **Space ID Detection**: The migration will check for a `kibana_space` (lowercase) environment variable to use as the target space name.
2.  **Space Definition Extraction**: Migration will attempt to pull the space definition from Kibana to create the `{space_id}/space.json` file.
3.  **Root Manifest Integration**: The target space will be added to the root `spaces.yml` manifest.
4.  **`.env` File Transformation**:
    *   Convert all environment variable keys to UPPERCASE.
    *   Comment out `KIBANA_SPACE` with a specific migration note.
    *   Support targeting either the default `.env` or a specific file provided via the `--env` flag.
