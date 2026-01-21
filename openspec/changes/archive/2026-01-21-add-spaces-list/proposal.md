# Proposal: Support Multiple Spaces in CLI and Spaces Manifest Generation

## Problem
Currently, the `kibob pull`, `push`, and `togo` commands only support a single space ID in the `--space` flag. Additionally, the `kibob add spaces` command lacks a direct `--space` ID filter and does not automatically generate or update the `spaces.yml` manifest file in the root directory, making it harder to manage multiple spaces.

## Proposed Changes
1.  **Multiple Space Filtering**: Update the `--space` flag in `pull`, `push`, and `togo` commands to support comma-separated values, consistent with the `--api` flag.
2.  **Add Command Space Filter**: Enable the `--space` filter for the `add spaces` command to selectively add spaces by ID.
3.  **Automatic Manifest Generation**: Ensure `add spaces` generates or updates `spaces.yml` in the project root.
4.  **Consistency**: Use `spaces.yml` in the project root as the primary manifest for managed spaces.

## Impact
- **Users**: Can now easily sync multiple specific spaces using `kibob pull --space id1,id2`.
- **Workflow**: `kibob add spaces` becomes the standard way to initialize and update the list of managed spaces in a project.
- **Maintainability**: Standardizes the location and usage of `spaces.yml`.
