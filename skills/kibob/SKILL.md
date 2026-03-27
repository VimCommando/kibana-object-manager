---
name: kibob
description: Execute and troubleshoot Kibana object management with the kibob CLI. Use when Codex needs to initialize a kibob project from NDJSON exports, authenticate to Kibana, pull or push saved objects, add items to manifests, bundle distributable NDJSON artifacts, migrate legacy manifests, or choose correct flags for spaces, APIs, managed mode, and dotenv loading.
---

# Kibob

## Overview

Use `kibob` to manage Kibana saved objects, workflows, agents, tools, and spaces with a Git-like workflow: initialize from exports, track assets in files, pull updates from Kibana, and push changes back safely.

Use this skill to:
- choose the right `kibob` command
- apply `--space`, `--api`, `--managed`, and `--env` correctly
- expand manifest scope with `add`
- create distributable bundles with `togo`
- troubleshoot auth, scope, and promotion issues quickly

## Runbook

1. Verify environment and credentials.
2. Initialize or open a `kibob` project.
3. Pull current state before editing.
4. Add or adjust manifest items if scope changes.
5. Push with the correct `--managed` setting and target space/API.
6. Bundle artifacts with `togo` when preparing handoff packages.
7. Re-pull after environment promotion when you need to verify normalization or server-side changes.

## Global Flags

Use these global flags before the subcommand:

- `--env <FILE>` - load environment variables from a dotenv file
- `--debug` - enable debug logging

### `--env` shorthand behavior

`kibob` accepts shorthand environment names:

- `--env dev` → `.env.dev`
- `--env stage` → `.env.stage`
- `--env prod` → `.env.prod`
- `--env local` → `.env.local`

These still work unchanged:

- `--env .env`
- `--env .env.prod`
- `--env ./custom.env`
- `--env config/prod.env`

Prefer the shorthand form in examples unless you need an explicit custom path.

## Configure Authentication

Create or update a dotenv file in the project directory.

Example `.env`:

```env
KIBANA_URL=https://your-kibana.example.com
KIBANA_SPACE=default
KIBANA_MAX_REQUESTS=8

# Option A: user/pass
KIBANA_USERNAME=elastic
KIBANA_PASSWORD=secret

# Option B: API key (do not combine with user/pass)
# KIBANA_APIKEY=base64-or-token
```

Run a connection test first:

```sh
kibob auth
```

Use `--env` for non-default environments:

```sh
kibob --env prod auth
kibob --env stage auth
```

## Initialize Project

Bootstrap from a Kibana export:

```sh
kibob init export.ndjson ./dashboards
```

Create a project from a directory that already contains `export.ndjson`:

```sh
kibob init ./kibana-export ./dashboards
```

Notes:
- If the first argument is a directory, `kibob` looks for `export.ndjson` inside it.
- A fresh initialized project writes:
  - `manifest/saved_objects.json`
  - `objects/`

## Pull and Push Workflow

Pull full project scope from the current directory:

```sh
kibob pull
```

Pull only selected APIs and spaces:

```sh
kibob pull --space default,marketing --api saved_objects,workflows,agents,tools
```

Push as managed objects (`true` is the default and recommended for production):

```sh
kibob push --managed true
```

Push editable objects in a dev environment:

```sh
kibob push --managed false --space dev
```

## API and Space Filtering

`pull`, `push`, and `togo` support:

- `--space <space1,space2,...>` - comma-separated list of spaces
- `--api <api1,api2,...>` - comma-separated list of APIs

Supported API values:
- `saved_objects`
- `objects`
- `workflows`
- `agents`
- `tools`
- `spaces`

Use explicit `--space` and `--api` in promotions and automation to avoid overly broad operations.

## Managed Mode Policy by Environment

Enforce this default policy:

- Production: use `--managed true`
- Dev and test: use `--managed false`

Reasoning:
- `managed=true` keeps production saved objects read-only in Kibana UI and reduces drift from Git
- `managed=false` lets dev/test teams iterate in the UI and then sync changes back to Git

Guardrails:
- Always pass explicit `--space` and `--api` in promotions
- Pull from each environment before pushing to it
- Commit reconciliation pulls from dev/test after UI-driven edits

## Environment Promotion Workflows

### Workflow A: Dev to Stage Promotion

```sh
# 1) Verify connectivity
kibob --env dev auth
kibob --env stage auth

# 2) Pull from source (dev)
kibob --env dev pull --space dev --api saved_objects,workflows,agents,tools

# 3) Commit to Git
git checkout -b promote/dev-to-stage-YYYYMMDD
git add .
git commit -m "Promote Kibana objects from dev to stage"

# 4) Push to target (stage) with dev/test posture
kibob --env stage push --space stage --api saved_objects,workflows,agents,tools --managed false

# 5) Verify target state and commit any normalization diffs
kibob --env stage pull --space stage --api saved_objects,workflows,agents,tools
git add .
git commit -m "Post-push sync from stage"
```

### Workflow B: Stage to Production Promotion

```sh
# 1) Pull approved stage state
kibob --env stage pull --space stage --api saved_objects,workflows,agents,tools

# 2) Tag and commit approved release payload
git checkout -b release/kibana-YYYYMMDD
git add .
git commit -m "Release Kibana objects from stage to production"
git tag kibana-release-YYYYMMDD

# 3) Push to production (recommended managed=true)
kibob --env prod push --space prod --api saved_objects,workflows,agents,tools --managed true

# 4) Verification pull from production
kibob --env prod pull --space prod --api saved_objects,workflows,agents,tools
git add .
git commit -m "Production verification sync"
```

## Expand Manifest Scope

Use `add` to grow what the project manages.

Supported APIs:
- `objects`
- `workflows`
- `spaces`
- `agents`
- `tools`

### Add items by API search

```sh
kibob add workflows --space marketing --query "alert"
kibob add agents --include "^support"
kibob add tools --exclude "test"
kibob add spaces --include "prod|staging"
```

### Add from input files

```sh
kibob add workflows --file workflows.json
kibob add workflows --file bundle.ndjson
kibob add agents --file agents.ndjson
kibob add tools --file tools.ndjson
kibob add spaces --file spaces.json
```

### Add explicit saved objects by type and id

```sh
kibob add objects --objects "dashboard=abc123,visualization=def456"
```

### Dependency behavior

`add workflows`, `add agents`, and `add tools` can automatically add dependencies unless you opt out:

```sh
kibob add workflows --space marketing --query "alert" --exclude-dependencies
kibob add agents --include "^support" --exclude-dependencies
kibob add tools --include "^search" --exclude-dependencies
```

### Important nuance for `add --space`

- `add spaces` can use `--space` as a filter list
- for non-`spaces` APIs, if multiple spaces are supplied, the CLI currently uses the first one

## Create Distributable Bundles

Build NDJSON bundle files:

```sh
kibob togo --space default,marketing --api saved_objects,workflows,agents,tools,spaces
```

Generated bundle outputs can include:
- `bundle/{space_id}/saved_objects.ndjson`
- `bundle/{space_id}/workflows.ndjson`
- `bundle/{space_id}/agents.ndjson`
- `bundle/{space_id}/tools.ndjson`
- `bundle/spaces.ndjson`

Zip bundle output:

```sh
zip -r kibob-bundle.zip bundle/
```

## Migrate Legacy Layout

Convert old manifest structure to the multi-space format:

```sh
kibob migrate
```

Disable backup only when explicitly requested:

```sh
kibob migrate --backup false
```

Notes:
- Migration targets `{space_id}/manifest/saved_objects.json`
- The target space is resolved from `KIBANA_SPACE`, falling back to `default`
- Migration also benefits from `--env` shorthand

Example:

```sh
kibob --env prod migrate
```

## Troubleshoot Fast

- Run commands with `--debug` when behavior is unclear
- Run `kibob auth` first for any network or auth failure
- Prefer explicit `--space` and `--api` flags when results are unexpectedly broad or empty
- Check dotenv files for `KIBANA_APIKEY` versus `KIBANA_USERNAME`/`KIBANA_PASSWORD` conflicts
- Use `KIBANA_MAX_REQUESTS` to tune concurrency when requests are too slow or too aggressive
- Pull before push to reduce overwrite risk
- Re-pull after push when verifying normalized output or promotion results

## Quick Decision Guide

- Need to start from a Kibana export? Use `kibob init`
- Need to verify credentials? Use `kibob auth`
- Need to sync from Kibana to disk? Use `kibob pull`
- Need to deploy local changes? Use `kibob push`
- Need to expand managed scope? Use `kibob add`
- Need handoff artifacts? Use `kibob togo`
- Need to upgrade an old project layout? Use `kibob migrate`

## Reference

Use [references/kibob-commands.md](references/kibob-commands.md) for a command-selection matrix and flag reminders.