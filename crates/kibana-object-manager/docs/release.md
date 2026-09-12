---
type: Guide
title: Release checklist
description: Prepare, verify, publish, and recover workspace releases.
generated: { by: codex/gpt-6, at: 2026-09-07T18:06:33Z }
---

# Release checklist

Run shell commands from the repository root unless a step explicitly selects another checkout.

## Prepare reviewed source

1. Record selected package versions, commit, release date, and tag. Follow [repository maintenance](maintenance.md) for independent versioning and compatibility rules.
2. Update `crates/kibana-sync/Cargo.toml` for a library release. Update `crates/kibana-object-manager/Cargo.toml` for a CLI release, including the library requirement when needed. The root manifest is a virtual workspace.
3. Regenerate and review `Cargo.lock`. Add a dated changelog section and upgrade instructions for incompatible changes. Preserve earlier tags and release records.
4. Open a reviewable release proposal. Run these checks on the exact reviewed commit with a clean working tree:

```sh
bash scripts/preflight.sh
bash scripts/preflight.sh msrv
rustup run 1.97.1 cargo build --workspace --release --locked
rustup run 1.97.1 cargo publish -p kibana-sync --dry-run --locked
```

Skip the library publication dry run if the unchanged version is already published. Record commit, tag, compiler, package versions, features, target, and OS. Distribution uses crates.io and a source-building Homebrew formula; no binary archive platform matrix is promised.

## Publish in dependency order

Execute publication only within the user's authorization. Create the selected tag on the reviewed revision and verify it matches the manifests and changelog.

1. For a changed library, run `rustup run 1.97.1 cargo publish -p kibana-sync --locked`. Wait for that exact registry version. Verify its metadata and build a temporary consumer outside the workspace with fresh dependency resolution on both Rust 1.89.0 and the pinned compiler.
2. For a CLI release, run `rustup run 1.97.1 cargo publish -p kibana-object-manager --dry-run --locked` after its required library version is available. Then run `rustup run 1.97.1 cargo publish -p kibana-object-manager --locked` and verify the published version.
3. Push the reviewed commit and exact tag to origin. CLI tags use `v<cli-version>`; library-only tags use `kibana-sync-v<library-version>`. Preserve existing historical tags. Do not rebase already-published source to work around a rejected push.
4. Create the GitHub release from the curated changelog. Include validation results and mark prereleases explicitly.
5. For CLI releases, update Homebrew through a PR after the tag is available.

## Verify Homebrew source installation

Update `Formula/kibob.rb` in `VimCommando/homebrew-tools`. The user-facing tap name is `VimCommando/tools`.

```sh
bash skills/publish-kibob/scripts/update_homebrew_formula.sh \
  --version <cli-version> --formula /path/to/homebrew-tools/Formula/kibob.rb
```

The repository license is `LICENCE.md`. Historical source archives may retain `LICENSE`; the updater accepts that legacy entry without rewriting old releases.

The updater verifies the final source archive's license and workspace versions before computing SHA-256 and editing the formula. A computed hash records downloaded bytes; it is not independent authentication. Review the tag's commit against the release record.

Run these checks in the tap checkout on each advertised supported host before merging its PR:

```sh
brew audit --strict --tap VimCommando/tools kibob
brew install --build-from-source VimCommando/tools/kibob
brew test VimCommando/tools/kibob
kibob --version
```

Use `brew reinstall --build-from-source` if already installed. Also initialize a temporary project from a small saved-object NDJSON fixture and inspect its object and manifest. The version command alone is insufficient. Record untested hosts; source fallback is verified only after installation succeeds.

Review the formula URL, hash, dependency/build path, and license. Open a tap PR with test results. Verify the final installed version after merge.

## Recover from partial failure

- If library publication succeeds and CLI publication fails, retain the library version. Correct the unpublished CLI package and rerun its dry run. Changes to published library code require a new library version.
- If a registry version already exists, compare it with the intended release. Skip only a verified identical publication. Investigate unexplained duplicates.
- If push or release creation fails after registry publication, preserve the original commit and tag target. Resolve divergence through a PR retaining published source, then retry the missing step.
- If the tap update fails, leave registry packages and tags intact. Repair its PR against the same source archive. Code corrections require a new product release.
- Never overwrite published artifacts or move released tags. Report completed and remaining stages.
