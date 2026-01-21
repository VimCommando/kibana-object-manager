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

### Requirement: Unified Migration
The `migrate` command SHALL perform a unified migration from legacy structures to the multi-space structure, incorporating space awareness and environment configuration cleanup.

#### Scenario: Migrate with lowercase kibana_space
- **GIVEN** a legacy project structure
- **AND** environment variable `kibana_space=marketing` is set
- **WHEN** the user runs `kibob migrate`
- **THEN** the system migrates objects to the `marketing/` directory
- **AND** fetches the `marketing` space definition into `marketing/space.json`
- **AND** adds `marketing` to the root `spaces.yml`

#### Scenario: Update .env file during migration
- **GIVEN** a `.env` file with `kibana_url=...` and `KIBANA_SPACE=default`
- **WHEN** the user runs `kibob migrate --env .env`
- **THEN** the system updates `.env` to have `KIBANA_URL=...` (UPPERCASE)
- **AND** comments out `KIBANA_SPACE` with the migration note:
  ```text
  # Commented out by Kibana Migrate, now use spaces.yml and space directories
  # KIBANA_SPACE=default
  ```

