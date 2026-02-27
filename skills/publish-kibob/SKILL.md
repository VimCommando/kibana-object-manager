---
name: publish-kibob
description: Publish a new kibob release to crates.io and the VimCommando/homebrew-kibob tap. Use when preparing or executing a versioned release, including version bumps, changelog updates, cargo publish, Git tag/release creation, and Homebrew formula url/sha256 updates.
---

# Publish Kibob

## Overview

Use this skill to run the full release pipeline for `kibana-object-manager`/`kibob`: crate publish, GitHub release/tag, and Homebrew tap update.

Read [references/release-checklist.md](references/release-checklist.md) at the start of the run, then execute each stage in order.

## Workflow

1. Validate release inputs.
2. Prepare and verify the repository release commit.
3. Publish to crates.io.
4. Push git commit/tag.
5. Update Homebrew formula in `VimCommando/homebrew-kibob`.
6. Run post-release verification checks.

## Stage 1: Validate Inputs

- Confirm target version (semver) and expected tag (`v<version>`).
- Ensure working tree is clean before release edits.
- Confirm required credentials are available:
  - crates.io token for `cargo publish`
  - GitHub auth for pushing tags and opening release/tap PRs

## Stage 2: Prepare Release Commit

- Update version in `Cargo.toml`.
- Update `Cargo.lock` package version entry if needed by tooling.
- Update `CHANGELOG.md` with a dated section for the new version.
- Run verification before committing:
  - `cargo clippy --all-targets --all-features -- -D warnings`
  - `cargo test --all-targets`
- Commit release prep changes.

## Stage 3: Publish Crate

- Run dry run first:
  - `cargo publish --dry-run`
- Publish:
  - `cargo publish`
- If publish fails due to duplicate version, stop and choose a new version.

## Stage 4: Push Git Updates

- Create and push release tag:
  - `git tag v<version>`
  - `git push origin main`
  - `git push origin v<version>`
- If push is rejected, rebase `main` on `origin/main` and retry.
- Homebrew source URL used by formula should be:
  - `https://github.com/VimCommando/kibana-object-manager/archive/refs/tags/v<version>.tar.gz`
- Optional: create a GitHub release using changelog notes.

## Stage 5: Update Homebrew Tap

- In `VimCommando/homebrew-kibob`, edit `Formula/kibob.rb`:
  - Update `url` to new tag tarball URL.
  - Update `sha256` to tarball hash.
- If tap repo does not exist yet:
  - `brew tap-new VimCommando/homebrew-kibob`
  - `gh repo create VimCommando/homebrew-kibob --public --source="$(brew --repository VimCommando/homebrew-kibob)" --push`
- Preferred automation:
  - `python skills/publish-kibob/scripts/update_homebrew_formula.py --version <version> --formula /path/to/homebrew-kibob/Formula/kibob.rb`
- Manual fallback hash command:
  - `curl -L "https://github.com/VimCommando/kibana-object-manager/archive/refs/tags/v<version>.tar.gz" | shasum -a 256`
- Validate formula locally if possible:
  - `HOMEBREW_NO_INSTALL_FROM_API=1 brew audit --strict --tap VimCommando/homebrew-kibob kibob`
  - `HOMEBREW_NO_INSTALL_FROM_API=1 brew install --build-from-source VimCommando/homebrew-kibob/kibob`
  - `brew test VimCommando/homebrew-kibob/kibob`
- Commit and push tap update (or open PR per maintainer workflow).

## Stage 6: Verify

- Confirm crate version appears on crates.io.
- Confirm git tag exists on origin for `v<version>`.
- Optional: confirm GitHub release exists for `v<version>`.
- Confirm Homebrew tap has updated formula and checksum.
- Optionally test install path:
  - `brew tap VimCommando/homebrew-kibob`
  - `brew install VimCommando/homebrew-kibob/kibob`
  - `kibob --version`
- Note: a tap formula does not automatically appear on `https://formulae.brew.sh/formula/<name>`. That page is for formulas indexed in Homebrew's main catalogs (for example `homebrew/core`).

## Guardrails

- Do not publish until clippy/tests pass.
- Do not reuse an already-published crate version.
- Keep crate version, git tag, and Homebrew formula URL on the same version.
- Update `sha256` only from the final tag tarball.
