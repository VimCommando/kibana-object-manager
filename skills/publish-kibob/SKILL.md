---
name: publish-kibob
description: Prepare or publish kibob CLI and kibana-sync library releases, including package versions, registry publication, tags, and the Homebrew source formula.
---

# Publish Kibob

Read the [release checklist](../../crates/kibana-object-manager/docs/release.md) before preparing or publishing a release. It owns package order, version policy, validation, and recovery.

1. Identify whether the request covers the CLI, library, or both. Package versions are independent. The root workspace has no package version to bump.
2. Prepare the selected manifests, CLI dependency requirement when needed, lockfile, and changelog. Run the checklist's local checks and present the release commit for review.
3. Within the user's publication authorization, publish a changed library before a dependent CLI. Verify each published package before continuing.
4. Use the exact reviewed revision for tags and the Homebrew source URL. Follow the checklist's recovery path after partial failure. Preserve published versions, tags, and archive bytes.
5. Report package versions, tag and commit, checks performed, and incomplete registry or Homebrew steps.

For CLI releases, the [formula updater](scripts/update_homebrew_formula.sh) validates the source archive before editing the supplied formula. Run installation checks and open a reviewable tap PR. Creating a tap or publishing to a new owner is a separate task.
