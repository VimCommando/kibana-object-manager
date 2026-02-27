---
name: kibob
description: Execute and troubleshoot Kibana object management with the kibob CLI. Use when Codex needs to initialize a kibob project from NDJSON exports, authenticate to Kibana, pull or push saved objects, add items to manifests, bundle distributable NDJSON artifacts, migrate legacy manifests, or choose correct flags for spaces, APIs, and managed mode.
---

# Kibob

## Overview

Use `kibob` to manage Kibana saved objects with a Git-like workflow: initialize from exports, track objects in files, pull updates from Kibana, and push changes back safely.
Use this skill to choose commands, apply flags correctly, and troubleshoot command failures quickly.

## Runbook

1. Verify environment and credentials.
2. Initialize or open a kibob project.
3. Pull current state before editing.
4. Add or adjust manifest items if scope changes.
5. Push with the correct `--managed` setting and target space/API.
6. Bundle artifacts with `togo` when preparing handoff packages.

## Configure Authentication

Create or update `.env` in the project directory:

```env
KIBANA_URL=https://your-kibana.example.com
KIBANA_SPACE=default
# Option A: user/pass
KIBANA_USERNAME=elastic
KIBANA_PASSWORD=secret
# Option B: API key (do not combine with user/pass)
# KIBANA_APIKEY=base64-or-token
```

Run connection test first:

```bash
kibob auth
```

Use `--env` to load a non-default dotenv file:

```bash
kibob auth --env .env.prod
```

## Initialize Project

Bootstrap from Kibana export:

```bash
kibob init export.ndjson ./dashboards
```

Create project from a directory that contains `export.ndjson`:

```bash
kibob init ./kibana-export ./dashboards
```

## Pull and Push Workflow

Pull full project scope from default space:

```bash
kibob pull .
```

Pull only selected APIs and spaces:

```bash
kibob pull . --space default,marketing --api saved_objects,workflows,tools
```

Push as managed objects (default and recommended for production):

```bash
kibob push . --managed true
```

Push editable objects in a dev environment:

```bash
kibob push . --managed false --space dev
```

## Managed Mode Policy by Environment

Enforce this default policy:
- Production: use `--managed true`.
- Dev and test: use `--managed false`.

Reasoning:
- `managed=true` keeps production objects read-only in Kibana UI and prevents drift from Git.
- `managed=false` lets dev/test teams iterate quickly in the UI and then sync back to Git.

Guardrails:
- Always pass explicit `--space` and `--api` in promotions.
- Pull from each environment before pushing to it.
- Commit reconciliation pulls from dev/test after UI-driven edits.

## Environment Promotion Workflows

### Workflow A: Dev to Stage Promotion

```bash
# 1) Verify connectivity
kibob auth --env .env.dev
kibob auth --env .env.stage

# 2) Pull from source (dev)
kibob pull . --env .env.dev --space dev --api saved_objects,workflows,agents,tools

# 3) Commit to Git
git checkout -b promote/dev-to-stage-YYYYMMDD
git add .
git commit -m "Promote Kibana objects from dev to stage"

# 4) Push to target (stage) as managed
kibob push . --env .env.stage --space stage --api saved_objects,workflows,agents,tools --managed false

# 5) Verify target state and commit any normalization diffs
kibob pull . --env .env.stage --space stage --api saved_objects,workflows,agents,tools
git add .
git commit -m "Post-push sync from stage"
```

### Workflow B: Stage to Production Promotion

```bash
# 1) Pull approved stage state
kibob pull . --env .env.stage --space stage --api saved_objects,workflows,agents,tools

# 2) Tag and commit approved release payload
git checkout -b release/kibana-YYYYMMDD
git add .
git commit -m "Release Kibana objects from stage to production"
git tag kibana-release-YYYYMMDD

# 3) Push to production (recommended managed=true)
kibob push . --env .env.prod --space prod --api saved_objects,workflows,agents,tools --managed true

# 4) Verification pull from production
kibob pull . --env .env.prod --space prod --api saved_objects,workflows,agents,tools
git add .
git commit -m "Production verification sync"
```

## Expand Manifest Scope

Add items by API search:

```bash
kibob add workflows . --space marketing --query "alert"
kibob add agents . --include "^support"
kibob add tools . --exclude "test"
```

Add from input files:

```bash
kibob add workflows . --file workflows.json
kibob add workflows . --file bundle.ndjson
```

Add explicit saved objects by type and id:

```bash
kibob add objects . --objects "dashboard=abc123,visualization=def456"
```

## Create Distributable Bundles

Build NDJSON package files:

```bash
kibob togo . --space default,marketing --api saved_objects,workflows,agents,tools,spaces
```

Zip bundle output:

```bash
zip -r kibob-bundle.zip bundle/
```

## Migrate Legacy Layout

Convert old manifest structure to multi-space format:

```bash
kibob migrate .
```

Disable backup only when explicitly requested:

```bash
kibob migrate . --backup false
```

## Troubleshoot Fast

- Run every command with `--debug` when behavior is unclear.
- Run `kibob auth` first for any network or auth failure.
- Prefer explicit `--space` and `--api` flags when results are unexpectedly broad or empty.
- Check `.env` for `KIBANA_APIKEY` versus `KIBANA_USERNAME`/`KIBANA_PASSWORD` conflicts.
- Pull before push to reduce overwrite risk.

## Reference

Use [references/kibob-commands.md](references/kibob-commands.md) for a command-selection matrix and flag reminders.
