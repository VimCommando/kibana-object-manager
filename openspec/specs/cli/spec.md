# cli Specification

## Purpose
Defines the command-line interface behavior, arguments, and user interaction patterns for the `kibob` tool.
## Requirements
### Requirement: API Filtering
The CLI SHALL support filtering operations by API type using the `--api` flag on `pull`, `push`, and `togo` commands.

#### Scenario: Default behavior (no filter)
- **WHEN** the user runs `kibob pull` without the `--api` flag
- **THEN** the system pulls all supported object types (Spaces, Saved Objects, Workflows, Agents, Tools)

#### Scenario: Filter by single API
- **WHEN** the user runs `kibob pull --api tools`
- **THEN** the system ONLY pulls Tools
- **AND** skips Spaces, Saved Objects, Workflows, and Agents

#### Scenario: Filter by multiple APIs
- **WHEN** the user runs `kibob push --api agents,workflows`
- **THEN** the system pushes Agents and Workflows
- **AND** skips Spaces, Saved Objects, and Tools

#### Scenario: API Aliases
- **WHEN** the user uses singular aliases (e.g., `tool`, `agent`, `object`)
- **THEN** the system treats them as their plural counterparts (`tools`, `agents`, `saved_objects`)

### Requirement: Space Filtering
The CLI SHALL support filtering operations by space ID using the `--space` flag on `pull`, `push`, and `togo` commands. Multiple space IDs MAY be specified as a comma-separated list.

#### Scenario: Pull multiple spaces
- **WHEN** the user runs `kibob pull --space default,marketing`
- **THEN** the system pulls objects from BOTH the `default` and `marketing` spaces
- **AND** skips other spaces defined in `spaces.yml`

#### Scenario: Push to specific spaces
- **WHEN** the user runs `kibob push --space engineering`
- **THEN** the system pushes objects ONLY to the `engineering` space

### Requirement: Space Management Filtering
The `add spaces` command SHALL support filtering spaces to add by ID using the `--space` flag.

#### Scenario: Add specific spaces to manifest
- **WHEN** the user runs `kibob add spaces . --space prod,staging`
- **THEN** the system fetches all spaces from Kibana
- **AND** filters the list to ONLY include those with IDs `prod` or `staging`
- **AND** adds these spaces to `spaces.yml` in the project root

### Requirement: Spaces Manifest Generation
The `add spaces` command SHALL automatically generate or update the `spaces.yml` manifest file in the project root directory.

#### Scenario: Generate spaces.yml
- **GIVEN** no `spaces.yml` exists
- **WHEN** the user runs `kibob add spaces .`
- **THEN** the system creates `spaces.yml` in the project root
- **AND** populates it with all spaces found in Kibana

