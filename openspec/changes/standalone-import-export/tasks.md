## 1. Standalone Resource Artifact Model

- [ ] 1.1 Add typed standalone Skill source discovery for a direct `SKILL.md` directory or an immediate-child collection without manifest access.
- [ ] 1.2 Add typed Tools, Agents, and Workflows source discovery for one JSON/JSON5 file or an immediate-file directory without manifest access.
- [ ] 1.3 Add discovered and validated import plan states that retain resource family, source paths, projected values, deterministic order, and authoritative IDs.
- [ ] 1.4 Validate complete import batches for invalid layouts, empty discovery, invalid projections, unsafe paths, and duplicate IDs before client creation.
- [ ] 1.5 Add unit tests for every family’s single and collection layouts, invalid and empty layouts, duplicate IDs, ignored manifests, deterministic order, and Skills path safety.

## 2. Structured Resource Batch Loading

- [ ] 2.1 Add shared per-resource create, update, skipped, and failed outcome types plus aggregate attempted, applied, skipped, and failed counts.
- [ ] 2.2 Add bounded-concurrency batch reporting to Skills, Tools, Agents, and Workflows loaders while preserving validated input order and response details.
- [ ] 2.3 Treat server-side readonly resources as unapplied failed standalone outcomes without sending create or update requests.
- [ ] 2.4 Preserve each existing count-oriented `Loader` interface for project push while exposing structured outcomes to standalone orchestration.
- [ ] 2.5 Add mocked family API tests for create, update, readonly collision, partial failure, deterministic reporting order, required headers, and space-prefixed paths.

## 3. Standalone Client and Capability Preflight

- [ ] 3.1 Refactor environment-based Kibana client construction so standalone commands can register one requested space without reading `spaces.yml`.
- [ ] 3.2 Add selected-family capability preflight using server version detection without reading or persisting project version metadata.
- [ ] 3.3 Add tests for default space, explicit non-default space, each family’s minimum Kibana version, and the existing version-bypass behavior.

## 4. Resource Import Commands

- [ ] 4.1 Add nested Clap import variants for `skills`, `tools`, `agents`, and `workflows` with shared source, space, and version options.
- [ ] 4.2 Implement generic import orchestration from validated artifact plans through family capability preflight and structured family batch loading.
- [ ] 4.3 Apply existing Tools, Agents, and Workflows JSON5 parsing and multiline transforms and the existing Skills directory projection.
- [ ] 4.4 Add deterministic terminal output for each resource and aggregate attempted, applied, skipped, and failed counts.
- [ ] 4.5 Return a non-zero command result after reporting when any selected resource fails, without pruning resources or expanding dependencies.
- [ ] 4.6 Add CLI tests for every family covering syntax, help, unsupported resource rejection, manifest isolation, successful upsert, readonly failure where applicable, partial failure, and unchanged `push` parsing.

## 5. Resource Export Commands

- [ ] 5.1 Add nested Clap export variants for `skills`, `tools`, `agents`, and `workflows` with a required mutually exclusive `--id` or `--all` selector group and `--overwrite`.
- [ ] 5.2 Add selected and ready-to-write export plan states that retain resource family, selection order, fetched definitions, output paths, and collision results.
- [ ] 5.3 Implement family-specific explicit-ID fetching and `--all` discovery using existing list/search endpoints, readonly filtering where applicable, deterministic ID sorting, and full-definition fetches.
- [ ] 5.4 Validate all fetched resources and family-specific output collisions before writing any selected resource.
- [ ] 5.5 Write manifest-free Skills directories and Tools, Agents, and Workflows JSON files using their existing project transforms, filename rules, and selected-output overwrite behavior.
- [ ] 5.6 Add mocked API and filesystem tests for every family covering explicit IDs, `--all`, missing/conflicting selectors, readonly rejection where applicable, required Workflow headers, fetch failure with no writes, filename collisions, overwrite, round-trip importability, and unchanged `pull` parsing.

## 6. License-Aware Live Validation

- [ ] 6.1 Extend the ignored live-test harness so it can target an operator-configured Kibana node without provisioning or changing its license.
- [ ] 6.2 Add family preflight that records detected version and recognizes explicit license-unavailable responses without treating unexpected authentication or API failures as skips.
- [ ] 6.3 Add live export, artifact verification, import, remote verification, and best-effort cleanup cases with unique temporary IDs for each supported family.
- [ ] 6.4 Report each live family case as passed, skipped with its version/license reason, or failed with preserved response details.
- [ ] 6.5 Document how to run license-independent tests and how to opt into full live validation with a compatible licensed test node.

## 7. Documentation and Verification

- [ ] 7.1 Document standalone `import`/`export`, all four family layouts, upsert behavior, explicit selection, manifest isolation, readonly policy, and examples in the README and command reference.
- [ ] 7.2 Add an Unreleased changelog entry for standalone Skills, Tools, Agents, and Workflows import/export.
- [ ] 7.3 Run `cargo fmt --all --check`.
- [ ] 7.4 Run `cargo clippy --workspace --all-targets --all-features -- -D warnings`.
- [ ] 7.5 Run `cargo test --workspace --all-features`.
- [ ] 7.6 Run OpenSpec validation for `standalone-import-export` and resolve all findings.
