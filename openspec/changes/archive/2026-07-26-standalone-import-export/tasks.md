## 1. Standalone Resource Artifact Model

- [x] 1.1 Add typed standalone Skill source discovery for a direct `SKILL.md` directory or an immediate-child collection without manifest access.
- [x] 1.2 Add typed Tools, Agents, and Workflows source discovery for one JSON/JSON5 file or an immediate-file directory without manifest access.
- [x] 1.3 Add discovered and validated import plan states that retain resource family, source paths, projected values, deterministic order, and authoritative IDs.
- [x] 1.4 Validate complete import batches for invalid layouts, empty discovery, invalid projections, unsafe paths, and duplicate IDs before client creation.
- [x] 1.5 Add unit tests for every family’s single and collection layouts, invalid and empty layouts, duplicate IDs, ignored manifests, deterministic order, and Skills path safety.

## 2. Structured Resource Batch Loading

- [x] 2.1 Add shared per-resource create, update, skipped, and failed outcome types plus aggregate attempted, applied, skipped, and failed counts.
- [x] 2.2 Add bounded-concurrency batch reporting to Skills, Tools, Agents, and Workflows loaders while preserving validated input order and response details.
- [x] 2.3 Treat server-side readonly resources as unapplied failed standalone outcomes without sending create or update requests.
- [x] 2.4 Preserve each existing count-oriented `Loader` interface for project push while exposing structured outcomes to standalone orchestration.
- [x] 2.5 Add mocked family API tests for create, update, readonly collision, partial failure, deterministic reporting order, required headers, and space-prefixed paths.

## 3. Standalone Client and Capability Preflight

- [x] 3.1 Refactor environment-based Kibana client construction so standalone commands can register one requested space without reading `spaces.yml`.
- [x] 3.2 Add selected-family capability preflight using server version detection without reading or persisting project version metadata.
- [x] 3.3 Add tests for default space, explicit non-default space, each family’s minimum Kibana version, and the existing version-bypass behavior.

## 4. Resource Import Commands

- [x] 4.1 Add nested Clap import variants for `skills`, `tools`, `agents`, and `workflows` with shared source, space, and version options.
- [x] 4.2 Implement generic import orchestration from validated artifact plans through family capability preflight and structured family batch loading.
- [x] 4.3 Apply existing Tools, Agents, and Workflows JSON5 parsing and multiline transforms and the existing Skills directory projection.
- [x] 4.4 Add deterministic terminal output for each resource and aggregate attempted, applied, skipped, and failed counts.
- [x] 4.5 Return a non-zero command result after reporting when any selected resource fails, without pruning resources or expanding dependencies.
- [x] 4.6 Add CLI tests for every family covering syntax, help, unsupported resource rejection, manifest isolation, successful upsert, readonly failure where applicable, partial failure, and unchanged `push` parsing.

## 5. Resource Export Commands

- [x] 5.1 Add nested Clap export variants for `skills`, `tools`, `agents`, and `workflows` with a required mutually exclusive `--id` or `--all` selector group and `--overwrite`.
- [x] 5.2 Add selected and ready-to-write export plan states that retain resource family, selection order, fetched definitions, output paths, and collision results.
- [x] 5.3 Implement family-specific explicit-ID fetching and `--all` discovery using existing list/search endpoints, readonly filtering where applicable, deterministic ID sorting, and full-definition fetches.
- [x] 5.4 Validate all fetched resources and family-specific output collisions before writing any selected resource.
- [x] 5.5 Write manifest-free Skills directories and Tools, Agents, and Workflows JSON files using their existing project transforms, filename rules, and selected-output overwrite behavior.
- [x] 5.6 Add mocked API and filesystem tests for every family covering explicit IDs, `--all`, missing/conflicting selectors, readonly rejection where applicable, required Workflow headers, fetch failure with no writes, filename collisions, overwrite, round-trip importability, and unchanged `pull` parsing.

## 6. License-Aware Live Validation

- [x] 6.1 Extend the ignored live-test harness so it can target an operator-configured Kibana node without provisioning or changing its license.
- [x] 6.2 Add family preflight that records detected version and recognizes explicit license-unavailable responses without treating unexpected authentication or API failures as skips.
- [x] 6.3 Add live export, artifact verification, import, remote verification, and best-effort cleanup cases with unique temporary IDs for each supported family.
- [x] 6.4 Report each live family case as passed, skipped with its version/license reason, or failed with preserved response details.
- [x] 6.5 Document how to run license-independent tests and how to opt into full live validation with a compatible licensed test node.

## 7. Documentation and Verification

- [x] 7.1 Document standalone `import`/`export`, all four family layouts, upsert behavior, explicit selection, manifest isolation, readonly policy, and examples in the README and command reference.
- [x] 7.2 Add an Unreleased changelog entry for standalone Skills, Tools, Agents, and Workflows import/export.
- [x] 7.3 Run `cargo fmt --all --check`.
- [x] 7.4 Run `cargo clippy --workspace --all-targets --all-features -- -D warnings`.
- [x] 7.5 Run `cargo test --workspace --all-features`.
- [x] 7.6 Run OpenSpec validation for `standalone-import-export` and resolve all findings.
- [x] 7.7 Run the licensed live suite against Kibana 9.3 and 9.4, correct versioned Workflow routing and canonical live resource fixtures, and verify all four families on the compatible node.
