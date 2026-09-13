# Repository guidance

Read [Contributor guide](crates/kibana-object-manager/docs/contributing.md) before changing this repository.

For validation, documentation boundaries, changelogs, OpenSpec completion, or compatibility decisions, follow [repository maintenance](crates/kibana-object-manager/docs/maintenance.md). Its changelog policy overrides the shared changelog skill's defaults in this repository.

Run `bash scripts/preflight.sh` before completing code or tooling changes. Documentation-only changes may use `bash scripts/preflight.sh docs`. Use `bash scripts/preflight.sh msrv` for compiler or dependency changes.

For releases, use [publish-kibob](skills/publish-kibob/SKILL.md). Release preparation does not authorize registry publication or remote repository changes unless the user requests them.

Shared standards are selected by the parent workspace's `AGENTS.md` when present. This repository's recorded decisions are authoritative for its documented exceptions and release contract.
