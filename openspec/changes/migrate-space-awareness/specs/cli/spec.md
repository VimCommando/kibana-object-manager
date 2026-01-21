# cli Specification

## MODIFIED Requirements

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
