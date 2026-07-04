## ADDED Requirements

### Requirement: Skills API Capability Gate
The `kibana-sync` crate SHALL expose Skills as a version-gated API capability.

#### Scenario: Capability matrix includes skills
- **WHEN** a consumer checks supported capabilities for a detected Kibana version
- **THEN** `skills` is evaluated independently from `agents`, `tools`, and `workflows`
- **AND** `skills` requires Kibana `9.4.0` or newer
- **AND** `skills` is labeled experimental as of Kibana `9.4`
- **AND** unsupported Skills requests produce the same skip, warning, or force behavior as other version-gated API families

#### Scenario: Sync planning includes skills
- **WHEN** a consumer plans pull or push sync for Skills
- **THEN** the returned capability plan includes Skills in either supported or unsupported capabilities
- **AND** the unsupported message names the `skills` API and Kibana `9.4.0` as the minimum required version

### Requirement: Storage-Neutral Skills Sync Support
The `kibana-sync` crate SHALL support Skills in storage-neutral sync bundles.

#### Scenario: Pull sync returns skills
- **WHEN** a consumer requests pull sync with Skills enabled
- **THEN** the returned space bundle includes the fetched Skill definitions for each selected space
- **AND** it can write those Skill definitions as skill directories through filesystem sync

#### Scenario: Push sync applies skills
- **WHEN** a consumer requests push sync with Skills in a space bundle
- **THEN** the library projects Skill directories or bundle records to Kibana JSON
- **AND** applies the projected Skill definitions through the Skills loader
- **AND** the returned summary includes attempted and applied Skill counts

#### Scenario: Dependency expansion can fetch skills
- **WHEN** dependency expansion discovers a missing Skill reference and Skills are enabled
- **THEN** the library fetches the Skill through `GET /api/agent_builder/skills/{skillId}`
- **AND** inserts it into the space bundle Skills collection

## MODIFIED Requirements

### Requirement: Explicit Filesystem Manifest and Bundle Sync

The `kibana-sync` crate SHALL support reusable filesystem-backed sync for Kibana manifests and file-backed assets using caller-provided paths.

#### Scenario: Read filesystem bundle from explicit path
- **WHEN** a consumer asks the library to read a Kibana asset bundle from a provided path
- **THEN** the library reads supported manifest files and file-backed saved objects, workflows, agents, tools, and skills from that path
- **AND** returns a sync bundle or equivalent resource collection that can be pushed to Kibana
- **AND** it does not infer the path from environment variables, process working directory, or CLI command state

#### Scenario: Write filesystem bundle to explicit path
- **WHEN** a consumer asks the library to write a pulled sync bundle to a provided path
- **THEN** the library writes supported manifests and file-backed resources in a stable bundle layout
- **AND** the written bundle can be read back by the library and pushed to another Kibana instance

#### Scenario: CLI project policy remains outside filesystem sync
- **WHEN** `kibob` uses the library filesystem sync APIs
- **THEN** the CLI crate chooses default paths, command behavior, terminal output, gitignore behavior, and migration policy
- **AND** the library only receives explicit paths, sync options, and resource data
