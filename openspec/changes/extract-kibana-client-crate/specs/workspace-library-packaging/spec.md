## ADDED Requirements

### Requirement: Multi-Crate Workspace Structure
The repository SHALL be organized as a Cargo workspace with a reusable `kibana-client` library crate and a `kibana-object-manager` CLI crate.

#### Scenario: Workspace members are declared
- **WHEN** Cargo metadata is evaluated at the repository root
- **THEN** the workspace includes a `kibana-client` crate
- **AND** the workspace includes a `kibana-object-manager` crate that builds the `kibob` binary

#### Scenario: Library crate builds independently
- **WHEN** `cargo check -p kibana-client` is run
- **THEN** the reusable library crate compiles without depending on the CLI crate

#### Scenario: CLI crate builds the existing binary
- **WHEN** `cargo check -p kibana-object-manager` is run
- **THEN** the `kibob` binary target is available
- **AND** the CLI crate depends on the `kibana-client` crate for Kibana API operations

### Requirement: Library Boundary Excludes CLI Concerns
The `kibana-client` crate SHALL NOT depend on CLI-only concerns including command-line parsing, dotenv loading, terminal coloring, repository file layout, gitignore management, or migration code.

#### Scenario: CLI dependencies remain outside the library
- **WHEN** the `kibana-client` crate manifest is inspected
- **THEN** it does not declare dependencies on `clap`, `dotenvy`, `env_logger`, or `owo-colors`

#### Scenario: Project storage remains outside the library
- **WHEN** the `kibana-client` crate source is inspected
- **THEN** it does not read or write `spaces.yml`
- **AND** it does not know about `manifest/`, `objects/`, `bundle/`, or per-space project directories

### Requirement: CLI Behavior Preservation
The `kibana-object-manager` CLI crate SHALL preserve existing `kibob` user-facing commands and project file layout while consuming `kibana-client` internally.

#### Scenario: Existing commands remain available
- **WHEN** `kibob --help` is generated
- **THEN** the `init`, `auth`, `pull`, `push`, `add`, `togo`, and `migrate` commands remain available

#### Scenario: Existing project layout remains supported
- **WHEN** `kibob pull` or `kibob push` operates on a project directory
- **THEN** it continues to use the existing `spaces.yml`, `{space}/manifest`, `{space}/objects`, `{space}/workflows`, `{space}/agents`, and `{space}/tools` layout
- **AND** it adapts that layout to `kibana-client` inputs and outputs

### Requirement: External Consumer Dependency
External Rust crates SHALL be able to depend on `kibana-client` without building or invoking the `kibob` CLI.

#### Scenario: External dependency compiles
- **WHEN** another crate declares a dependency on `kibana-client`
- **THEN** it can construct a Kibana client and use the public API modules without referencing `kibana-object-manager`

#### Scenario: ESDiag-compatible auth adaptation
- **WHEN** a consumer has an existing URL and auth model equivalent to none, basic auth, or API key auth
- **THEN** it can convert those values into `kibana-client` configuration without environment variables or `kibob` project files

### Requirement: Independent Library Publication
The `kibana-client` crate SHALL be prepared for independent publication as a reusable Rust library.

#### Scenario: Publish-ready metadata exists
- **WHEN** the `kibana-client` crate manifest is inspected
- **THEN** it includes package metadata required for independent publication, including version, edition, rust-version, license, repository, description, keywords or categories, and documentation or README references

#### Scenario: Publish dry-run succeeds
- **WHEN** `cargo publish -p kibana-client --dry-run` is run before release
- **THEN** the crate packages successfully without requiring `kibana-object-manager` CLI-only files
