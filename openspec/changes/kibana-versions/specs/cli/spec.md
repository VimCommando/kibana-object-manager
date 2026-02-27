## MODIFIED Requirements

### Requirement: API Filtering
The CLI SHALL support filtering operations by API type using the `--api` flag on `pull`, `push`, and `togo` commands, and SHALL apply Kibana version preflight checks before performing API operations.

#### Scenario: Default behavior (no filter)
- **WHEN** the user runs `kibob pull` without the `--api` flag
- **THEN** the system evaluates all supported object types (Spaces, Saved Objects, Workflows, Agents, Tools)
- **AND** the system gates each API by its minimum supported Kibana version

#### Scenario: Filter by single API
- **WHEN** the user runs `kibob pull --api tools`
- **THEN** the system ONLY evaluates Tools
- **AND** the system skips Tools with a version-gate message if Kibana version is lower than 9.2.0

#### Scenario: Filter by multiple APIs
- **WHEN** the user runs `kibob push --api agents,workflows`
- **THEN** the system evaluates Agents and Workflows
- **AND** the system runs each API only if its minimum Kibana version is met
- **AND** the system reports skipped APIs in command output

#### Scenario: API Aliases
- **WHEN** the user uses singular aliases (e.g., `tool`, `agent`, `object`)
- **THEN** the system treats them as their plural counterparts (`tools`, `agents`, `saved_objects`)
- **AND** version gating is applied to the normalized API names

#### Scenario: Warning exit status on unsupported requested APIs
- **WHEN** a command includes one or more APIs unsupported by the connected Kibana version
- **THEN** the system SHALL perform version checks before sending API requests
- **AND** the system SHALL return a warning exit status after execution

#### Scenario: Force flag bypasses API version preflight
- **WHEN** the user runs a command with `--force`
- **THEN** the system SHALL bypass API minimum-version preflight blocks
- **AND** the system SHALL print a warning that unsupported API calls may fail

### Requirement: Spaces Manifest Generation
The `add spaces` command SHALL automatically generate or update the `spaces.yml` manifest file in the project root directory, and pull flows SHALL maintain Kibana version provenance in this file.

#### Scenario: Generate spaces.yml
- **GIVEN** no `spaces.yml` exists
- **WHEN** the user runs `kibob add spaces .`
- **THEN** the system creates `spaces.yml` in the project root
- **AND** populates it with all spaces found in Kibana

#### Scenario: Record Kibana version on pull
- **WHEN** the user runs a successful `kibob pull`
- **THEN** the system updates root `spaces.yml` with `kibana.version: <full-semver>`
- **AND** the recorded version reflects the Kibana cluster used for that pull

## ADDED Requirements

### Requirement: Push Version Floor Enforcement
The CLI SHALL block push operations to clusters older than the version recorded in `spaces.yml`.

#### Scenario: Block push to older minor version
- **GIVEN** `spaces.yml` contains `kibana.version: 9.3.2`
- **WHEN** the user runs `kibob push` against Kibana `9.2.7`
- **THEN** the system SHALL abort push before object API calls
- **AND** the system SHALL return a warning exit status describing version incompatibility

#### Scenario: Allow push with patch drift on same minor
- **GIVEN** `spaces.yml` contains `kibana.version: 9.3.2`
- **WHEN** the user runs `kibob push` against Kibana `9.3.0`
- **THEN** the system SHALL allow push
- **AND** patch-level differences SHALL NOT block execution

#### Scenario: Force flag bypasses push version floor
- **GIVEN** `spaces.yml` contains `kibana.version: 9.3.2`
- **WHEN** the user runs `kibob push --force` against Kibana `9.2.7`
- **THEN** the system SHALL continue with push attempts
- **AND** the system SHALL print a warning that version-floor protection was bypassed

### Requirement: API Version Guidance in Help and Docs
The CLI help output and project documentation SHALL include per-API minimum Kibana versions and tech preview markers.

#### Scenario: API help lists minimum version
- **WHEN** the user reads CLI help for API selection
- **THEN** each API entry SHALL include its minimum supported Kibana version
- **AND** APIs in tech preview SHALL be labeled as tech preview
