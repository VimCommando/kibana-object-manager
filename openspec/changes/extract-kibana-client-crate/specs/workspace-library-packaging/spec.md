## ADDED Requirements

### Requirement: Multi-Crate Workspace Structure
The repository SHALL be organized as a Cargo workspace with a reusable `kibana-sync` library crate and a `kibana-object-manager` CLI crate.

#### Scenario: Workspace members are declared
- **WHEN** Cargo metadata is evaluated at the repository root
- **THEN** the workspace includes a `kibana-sync` crate
- **AND** the workspace includes a `kibana-object-manager` crate that builds the `kibob` binary

#### Scenario: Library crate builds independently
- **WHEN** `cargo check -p kibana-sync` is run
- **THEN** the reusable library crate compiles without depending on the CLI crate

#### Scenario: CLI crate builds the existing binary
- **WHEN** `cargo check -p kibana-object-manager` is run
- **THEN** the `kibob` binary target is available
- **AND** the CLI crate depends on the `kibana-sync` crate for Kibana API operations

### Requirement: Library Boundary Excludes CLI Concerns
The `kibana-sync` crate SHALL NOT depend on CLI-only concerns including command-line parsing, dotenv loading, terminal coloring, implicit project-root discovery, gitignore management, migration code, or command exit policy.

#### Scenario: CLI dependencies remain outside the library
- **WHEN** the `kibana-sync` crate manifest is inspected
- **THEN** it does not declare dependencies on `clap`, `dotenvy`, `env_logger`, or `owo-colors`

#### Scenario: Filesystem APIs remain explicit
- **WHEN** the `kibana-sync` crate source is inspected
- **THEN** it does not read environment variables or the current working directory to discover a project root
- **AND** filesystem manifest or bundle APIs operate only on caller-provided paths
- **AND** it does not depend on gitignore helpers, migration helpers, terminal output, warning exit status, or `kibob` command semantics

#### Scenario: Reusable bundle formats live in the library
- **WHEN** an external consumer needs to version-control or bundle Kibana assets
- **THEN** `kibana-sync` may expose reusable manifest schemas, bundle schemas, and path-explicit filesystem readers or writers
- **AND** those APIs are usable without constructing or invoking the `kibob` CLI

### Requirement: CLI Behavior Preservation
The `kibana-object-manager` CLI crate SHALL preserve existing `kibob` user-facing commands and project file layout while consuming `kibana-sync` internally.

#### Scenario: Existing commands remain available
- **WHEN** `kibob --help` is generated
- **THEN** the `init`, `auth`, `pull`, `push`, `add`, `togo`, and `migrate` commands remain available

#### Scenario: Existing project layout remains supported
- **WHEN** `kibob pull` or `kibob push` operates on a project directory
- **THEN** it continues to use the existing `spaces.yml`, `{space}/manifest`, `{space}/objects`, `{space}/workflows`, `{space}/agents`, and `{space}/tools` layout
- **AND** it adapts that layout to `kibana-sync` inputs and outputs

### Requirement: External Consumer Dependency
External Rust crates SHALL be able to depend on `kibana-sync` without building or invoking the `kibob` CLI.

#### Scenario: External dependency compiles
- **WHEN** another crate declares a dependency on `kibana-sync`
- **THEN** it can construct a Kibana client and use the public API modules without referencing `kibana-object-manager`

#### Scenario: ESDiag-compatible auth adaptation
- **WHEN** a consumer has an existing URL and auth model equivalent to none, basic auth, or API key auth
- **THEN** it can convert those values into `kibana-sync` configuration without environment variables or `kibob` project files

#### Scenario: ESDiag-compatible filesystem bundle loading
- **WHEN** a consumer has a Kibana asset bundle containing manifests and file-backed resources
- **THEN** it can load that bundle through `kibana-sync` APIs using explicit paths
- **AND** it can push the resulting resources to a target Kibana instance without invoking `kibob`

### Requirement: Independent Library Publication
The `kibana-sync` crate SHALL be prepared for independent publication as a reusable Rust library.

#### Scenario: Publish-ready metadata exists
- **WHEN** the `kibana-sync` crate manifest is inspected
- **THEN** it includes package metadata required for independent publication, including version, edition, rust-version, license, repository, description, keywords or categories, and documentation or README references

#### Scenario: Publish dry-run succeeds
- **WHEN** `cargo publish -p kibana-sync --dry-run` is run before release
- **THEN** the crate packages successfully without requiring `kibana-object-manager` CLI-only files
