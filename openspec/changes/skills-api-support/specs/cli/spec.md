## MODIFIED Requirements

### Requirement: API Filtering
The CLI SHALL support filtering operations by API type using the `--api` flag on `pull`, `push`, and `togo` commands.

#### Scenario: Default behavior (no filter)
- **WHEN** the user runs `kibob pull` without the `--api` flag
- **THEN** the system pulls all supported object types (Spaces, Saved Objects, Workflows, Agents, Tools, Skills)

#### Scenario: Filter by single API
- **WHEN** the user runs `kibob pull --api skills`
- **THEN** the system ONLY pulls Skills
- **AND** skips Spaces, Saved Objects, Workflows, Agents, and Tools

#### Scenario: Filter by multiple APIs
- **WHEN** the user runs `kibob push --api agents,skills,workflows`
- **THEN** the system pushes Agents, Skills, and Workflows
- **AND** skips Spaces, Saved Objects, and Tools

#### Scenario: API Aliases
- **WHEN** the user uses singular aliases (e.g., `tool`, `agent`, `skill`, `object`)
- **THEN** the system treats them as their plural counterparts (`tools`, `agents`, `skills`, `saved_objects`)

## ADDED Requirements

### Requirement: Skills Command Integration
The CLI SHALL expose Skills anywhere Agent Builder API families can be selected, summarized, or added to a project.

#### Scenario: Add skill to manifest
- **WHEN** the user runs `kibob add skill <skill-id>`
- **THEN** the system fetches `GET /api/agent_builder/skills/{skillId}`
- **AND** writes the Skill as `skills/{skill-directory}/SKILL.md`
- **AND** writes referenced content as markdown files under the Skill directory

#### Scenario: Pull summary includes skills
- **WHEN** a pull operation includes Skills
- **THEN** the command summary reports the number of Skills discovered and written per space

#### Scenario: Push summary includes skills
- **WHEN** a push operation includes Skills
- **THEN** the command summary reports the number of Skills attempted and applied per space

#### Scenario: Unsupported skills API is skipped
- **WHEN** the user requests Skills against a Kibana version that does not support the Skills API
- **THEN** the CLI skips Skills before issuing Skills API requests
- **AND** reports Kibana `9.4.0` as the required version and the detected Kibana version using the existing unsupported API warning behavior
- **AND** identifies Skills as experimental as of Kibana `9.4` in command help and documentation
