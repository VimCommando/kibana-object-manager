# Kibob Command Matrix

## Purpose

Use this file to choose the right `kibob` command quickly and avoid flag mistakes.

## Command Selection

| Intent | Command |
|---|---|
| Verify credentials and connectivity | `kibob auth` |
| Create project from export file | `kibob init export.ndjson ./project-dir` |
| Fetch objects from Kibana | `kibob pull ./project-dir` |
| Upload local objects to Kibana | `kibob push ./project-dir` |
| Add workflows/agents/tools/spaces/objects to manifest | `kibob add <api> ./project-dir` |
| Build distributable NDJSON bundle files | `kibob togo ./project-dir` |
| Convert old manifest format to multi-space format | `kibob migrate ./project-dir` |

## High-Impact Flags

| Flag | Applies To | Notes |
|---|---|---|
| `--env <file>` | all commands | Load alternate dotenv file (default `.env`) |
| `--space <ids>` | pull, push, add, togo | Use comma-separated space IDs |
| `--api <list>` | pull, push, togo | Limit to APIs like `saved_objects,workflows,agents,tools,spaces` |
| `--managed true|false` | push, togo | `true` means read-only in Kibana UI |
| `--debug` | all commands | Print verbose logs for troubleshooting |

## Add Command Patterns

Use API search:

```bash
kibob add workflows . --space marketing --query "alert"
```

Filter by regex:

```bash
kibob add agents . --include "^support" --exclude "test"
```

Ingest from file:

```bash
kibob add tools . --file tools.ndjson
```

Add specific object IDs:

```bash
kibob add objects . --objects "dashboard=abc123,visualization=def456"
```

## Safety Defaults

- Run `kibob auth` before pull or push.
- Pull before push for each target space.
- Use `--managed true` unless editable objects are explicitly required.
- Keep `.env` free of mixed auth modes; use API key or user/password, not both.

## Managed Policy by Environment

- Production: use `--managed true`.
- Dev/test: use `--managed false`.
- Keep environments explicit with `--space` and `--api` so policies are applied correctly.
- Reconcile dev/test UI edits back to Git with pull + commit.

## Promotion Workflow Examples

Dev to Stage:

```bash
kibob pull . --env .env.dev --space dev --api saved_objects,workflows,agents,tools
git checkout -b promote/dev-to-stage-YYYYMMDD
git add .
git commit -m "Promote Kibana objects from dev to stage"
kibob push . --env .env.stage --space stage --api saved_objects,workflows,agents,tools --managed false
```

Stage to Production:

```bash
kibob pull . --env .env.stage --space stage --api saved_objects,workflows,agents,tools
git checkout -b release/kibana-YYYYMMDD
git add .
git commit -m "Release Kibana objects from stage to production"
kibob push . --env .env.prod --space prod --api saved_objects,workflows,agents,tools --managed true
```

Dev/Test Reconciliation:

```bash
kibob pull . --env .env.dev --space dev --api saved_objects,workflows,agents,tools
git add .
git commit -m "Sync dev UI edits back to Git"
```
