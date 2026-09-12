---
type: Policy
title: Repository maintenance
description: Local validation, documentation boundaries, compatibility, and release procedures.
generated: { by: codex/gpt-6, at: 2026-09-12T17:19:44Z }
---

# Repository maintenance

## Validation

Run from the repository root:

```sh
bash scripts/preflight.sh
bash scripts/preflight.sh msrv
```

Install the compiler in `rust-toolchain.toml`, Rust 1.89.0 for the minimum-version check, ShellCheck, ripgrep, tq-cli 0.3.0, OKF 0.2.7, and OpenSpec 1.11.0. Repository scripts use Bash 3.2 with composed utilities. Use tq for JSON queries, Cargo for manifest parsing, and standard text/archive utilities for the remaining work.

```sh
cargo install okf --version 0.2.7 --locked
cargo install tq-cli --version 0.3.0 --locked
npm install --global @fission-ai/openspec@1.11.0
```

The `rust` preflight mode runs formatting, Clippy, behavior tests, doctests, shell lint, and maintenance-tool tests. The `docs` mode validates the complete documentation bundle, local authored links, its index, and all current OpenSpec specs and active changes. `msrv` checks all workspace targets and features with the promised minimum compiler. CI calls the same entry point. Live Kibana tests remain a separate opt-in check described in [Live tests](live-tests.md).

The development/release compiler pin is separate from package `rust-version`. Application builds use the committed lockfile and `--locked`. Minimum-version checks cover the locked dependencies; release preparation must also test the library as a registry consumer with fresh dependency resolution.

Tests should cover behavior and failures, rather than a fixed coverage percentage. The useful cases include invalid input, API compatibility, collisions, partial writes, timeouts, and recovery.

## Documentation boundary

This directory is the complete OKF bundle. Its [index](index.md) lists every concept. The repository root, OpenSpec, agent skills, scripts, caches, and temporary reports are outside the bundle. Validation does not scan those paths as OKF concepts. Authored local links from the README and contributor guide are checked separately.

All filenames and directory names under this bundle use lower-kebab-case, including nested assets. Separate words with hyphens, not underscores. Use numbered lists in indexes and authored reference lists. The shared validator checks naming and exact link case; review list style when editing documentation.

Use native OKF validation and lint capabilities before adding custom checks. Supplemental scripts should cover only demonstrated repository-policy gaps, such as filename conventions and links originating outside the bundle. Documentation links in the root README must also work when Cargo packages that README at the crate root.

Every documentation PR must pass the complete bundle check. Metadata changes record the actual actor and UTC time in `generated`. Preserve existing bodies and attribution when importing references. Format validation does not establish that the guidance is factually current.

## Pull requests and OpenSpec

Use a Conventional Commit PR title and squash merge so the resulting main-branch commit follows the same convention. Preserve existing commit history. The PR template requires `OpenSpec-Changes: none` or comma-separated IDs, including changes associated with implementation whose artifacts do not appear in the diff.

The PR check compares committed head state against the target merge base. It selects added, edited, deleted, and renamed active change paths and archive paths, plus explicitly associated IDs. Associated changes must be archived with proposal and completed tasks. Requirement additions and modifications must match the main specs after whitespace normalization; removals and renames must appear in the final spec state. A change without deltas needs an archived `no-spec-deltas.md` explaining why, reviewed with the PR. Unrelated active changes do not block the gate.

Direct changes under `openspec/specs/` require an associated change, either selected from the artifact diff or declared in `OpenSpec-Changes`. A PR that only edits main specs and declares `none` fails the gate.

Contributors review the requirement correspondence, particularly when multiple changes modify the same requirement. The automated comparison is intentionally strict about wording and cannot judge semantic equivalence. Update the associated delta to the final reviewed contract when necessary.

OpenSpec's native `validate --archived` reports archived task completion. Accept its semantics rather than adding stricter task parsing. The historical findings in `2026-01-20-refactor-space-into-kibana-client` and `2026-01-21-add-spaces-list` were accepted as-is on 2026-09-07. Preserve their task records and report the CLI's result accurately. They do not block unrelated PRs.

Repository maintainers should require `checks` and `msrv` before merge and require PR review. The workflow alone does not configure GitHub branch protection. Local gate testing uses a GitHub pull request event fixture:

```sh
bash scripts/check_pr.sh --base origin/main --head HEAD --event /tmp/pr-event.json
```

## Changelog

New entries use Keep a Changelog categories in urgency order: Security, Removed, Changed, Deprecated, Fixed, Added. Keep Unreleased, dated versions, and comparison links. Omit empty categories and preserve existing release history. Describe user-visible outcomes. A reproduced behavior correction qualifies as Fixed without requiring a GitHub issue; link issues or PRs only when verified. Documentation-only work does not automatically require an entry.

This repository policy resolves the older local skill's category-order and issue-first defaults prospectively. It adopts these explicit rules without making a claim about an online Keep a Changelog 2.0.0 publication.

## Versions and releases

The library and CLI are independently versioned packages. Each package manifest owns its version. The CLI manifest records the compatible library requirement. A CLI release uses `v<cli-version>`; a library-only release may use `kibana-sync-v<library-version>`. Preserve existing tags and published bytes. For each 0.x package, incompatible changes require a minor version increase and upgrade instructions; compatible fixes use a patch increase. State minimum-compiler increases in release notes and prefer a minor release.

A CLI release may include a new library version. Publish the library first when needed, then the CLI. Do not republish an unchanged dependency. The [release checklist](release.md) specifies package selection, review, registry checks, source installation checks, and recovery from partial publication.

Distribution currently uses crates.io and a source-building Homebrew formula. No binary archive platform matrix is promised. Test installation on each host advertised by the tap before a release; record the host, compiler, versions, tag, and commit. A successful local macOS build does not certify Linux or Windows installation.

## CLI contract

Diagnostics go to stderr. Captured diagnostics omit ANSI escapes unless the caller explicitly forces color; `NO_COLOR` takes precedence. Help and version use stdout. Exit 0 means success, 1 means an operational error, and 2 means invalid CLI arguments or a capability warning. Commands write resource data to explicit files; human log wording is not a machine protocol. Export supports overwrite controls and batch failure reports.

HTTP requests default to a 300-second deadline including body reads and a 10-second connection timeout. Configure positive integer seconds through `KIBANA_REQUEST_TIMEOUT` and `KIBANA_CONNECT_TIMEOUT`, or use the library builder's duration setters. Time waiting for a concurrency permit is separate. A timeout can happen after a remote write succeeded: inspect remote state before retrying. There is no automatic rollback of previously applied resources.

General structured stdout reporting and a remote-mutation preview mode remain interface proposals. They require a command-specific design and compatibility review; this repository does not claim to provide those modes. Existing local batch validation is not a dry run. Parser memory limits and whole-command cancellation guarantees are also not promised by the current API.
