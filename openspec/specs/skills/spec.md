# skills Specification

## Purpose
Defines storage, extraction, loading, bundling, deletion, and live validation behavior for Kibana Skills API support.

## Requirements
### Requirement: Skills Filesystem Storage
The system SHALL represent user-created Kibana Skills at rest as per-skill directories, not as API JSON blobs.

#### Scenario: Write skill directory to a filesystem bundle
- **WHEN** a pull or filesystem sync writes Skills for a space
- **THEN** the system writes `manifest/skills.yml` for the space
- **AND** the manifest lists each tracked Skill by `id` and `name`
- **AND** the system writes each Skill under `skills/{skill-directory}/`
- **AND** writes a main `SKILL.md` file in that Skill directory
- **AND** writes Skill `content` as the markdown body of `SKILL.md`
- **AND** writes `id`, `name`, `description`, `tool_ids`, and `experimental` in the YAML frontmatter of `SKILL.md`
- **AND** writes each `referenced_content` entry as a markdown file under the Skill directory
- **AND** uses the Skill `id` as the skill directory name
- **AND** returns an error when the Skill `id` does not satisfy Kibana's Skill ID format
- **AND** treats the `id` value in `SKILL.md` frontmatter as authoritative when it differs from the directory name

#### Scenario: Read skill directory from a filesystem bundle
- **WHEN** a push or filesystem sync reads a space bundle containing `skills/{skill-directory}/SKILL.md`
- **THEN** the system parses the `SKILL.md` YAML frontmatter for `id`, `name`, `description`, `tool_ids`, and `experimental`
- **AND** treats the markdown body of `SKILL.md` as the Skill `content`
- **AND** reads additional markdown files under the Skill directory as referenced content
- **AND** returns an error when `SKILL.md` is missing a string `id`
- **AND** preserves markdown body and referenced markdown file contents exactly, including newline content

#### Scenario: Read tracked Skills from the space manifest
- **WHEN** a push, to-go bundle, or filesystem sync reads a space bundle with `manifest/skills.yml`
- **THEN** the system treats `manifest/skills.yml` as the tracked Skill list for that space
- **AND** includes only Skills listed in the manifest
- **AND** reads listed Skills in manifest order
- **AND** returns an error when a Skill listed in the manifest is missing from the `skills/` directory

#### Scenario: Discover Skills when no manifest exists
- **WHEN** a push, to-go bundle, or filesystem sync reads a space bundle without `manifest/skills.yml`
- **THEN** the system discovers all `skills/{skill-directory}/SKILL.md` directories
- **AND** reads discovered Skills in deterministic directory order

#### Scenario: Build referenced content from directory structure
- **WHEN** the system projects a Skill directory to Kibana JSON
- **THEN** each markdown file under the Skill directory except `SKILL.md` becomes one `referenced_content` object
- **AND** each referenced content object contains `name` equal to the filename without `.md`
- **AND** each referenced content object contains `relativePath` equal to the markdown file parent path relative to the Skill directory using Kibana's `./subdirectory` form for non-root files
- **AND** markdown files directly under the Skill directory use an empty string `relativePath`
- **AND** each referenced content object contains `content` equal to the markdown file contents
- **AND** each referenced content object contains no fields other than `name`, `relativePath`, and `content`
- **AND** referenced content is generated in deterministic order sorted by relative path and filename

#### Scenario: Reject unsafe referenced content paths
- **WHEN** the system reads referenced markdown files under a Skill directory
- **THEN** it rejects files whose resolved paths escape the Skill directory
- **AND** it rejects symlink traversal that resolves outside the Skill directory

#### Scenario: Sanitize paths without changing JSON values
- **WHEN** the system writes Skill directories or referenced markdown files from Kibana JSON
- **THEN** it sanitizes filesystem path components for portability and safety
- **AND** preserves original Skill `id` and referenced content `name` values when projecting back to Kibana JSON
- **AND** normalizes nested referenced content `relativePath` values to the Kibana-accepted `./subdirectory` form
- **AND** uses sidecar metadata when sanitization is needed to preserve referenced content `name` and `relativePath` values

### Requirement: Skills Extraction
The system SHALL fetch Skills from Kibana using the documented Skills API endpoints.

#### Scenario: List skills
- **WHEN** the system discovers Skills for a space
- **THEN** it sends `GET /api/agent_builder/skills`
- **AND** it uses the space-prefixed path `/s/{space_id}/api/agent_builder/skills` for non-default spaces
- **AND** it parses the response into Skill JSON definitions
- **AND** it treats Skills with `readonly: true` as system Skills that are not pushable user-created Skills

#### Scenario: List skills including plugin skills
- **WHEN** Skill discovery is configured to include plugin-provided Skills
- **THEN** the system sends `GET /api/agent_builder/skills?include_plugins=true`
- **AND** it does not mark Skills with `readonly: true` as pushable user-created Skills

#### Scenario: Fetch skill by id
- **WHEN** the caller selects a specific Skill id
- **THEN** the system sends `GET /api/agent_builder/skills/{skillId}`
- **AND** it uses the response body as that Skill definition
- **AND** it preserves the `readonly` field when Kibana returns it

### Requirement: Skills Loading
The system SHALL create and update user-created Skills through the documented Skills API endpoints.

#### Scenario: Create skill
- **WHEN** a Skill directory has an `id` that does not exist in Kibana
- **THEN** the system sends `POST /api/agent_builder/skills`
- **AND** the request body is projected from `SKILL.md` frontmatter, `SKILL.md` markdown body, and referenced markdown files
- **AND** the request body includes `id`, `name`, `description`, `content`, `referenced_content`, and `tool_ids` when present
- **AND** `tool_ids` and `referenced_content` are serialized as stable arrays, including empty arrays when no values are present
- **AND** the request body DOES NOT include `readonly`
- **AND** the request body DOES NOT include local-only `experimental` frontmatter metadata
- **AND** the request includes the required `kbn-xsrf` header through the Kibana client

#### Scenario: Update skill
- **WHEN** a Skill directory has an `id` that already exists in Kibana
- **THEN** the system sends `PUT /api/agent_builder/skills/{skillId}`
- **AND** the request body is projected from `SKILL.md` frontmatter, `SKILL.md` markdown body, and referenced markdown files
- **AND** the request body includes mutable fields such as `name`, `description`, `content`, `referenced_content`, and `tool_ids`
- **AND** the request body DOES NOT include `id`
- **AND** the request body DOES NOT include `readonly`
- **AND** the request body DOES NOT include local-only `experimental` frontmatter metadata
- **AND** `tool_ids` and `referenced_content` are serialized as stable arrays, including empty arrays when no values are present
- **AND** the request includes the required `kbn-xsrf` header through the Kibana client

#### Scenario: Missing skill id
- **WHEN** a Skill directory `SKILL.md` has no string `id` in frontmatter
- **THEN** the system does not send a create or update request for that Skill
- **AND** it reports the missing resource identifier

#### Scenario: Skip non-user-created skills
- **WHEN** a Skill definition has `readonly: true`
- **THEN** the system does not create or update that Skill
- **AND** it reports the skip through diagnostic output

### Requirement: Skills Delete Helper
The system SHALL expose deletion behavior for user-created Skills without invoking it during ordinary push operations.

#### Scenario: Delete unreferenced skill
- **WHEN** a caller requests deletion for a user-created Skill id without force
- **THEN** the system sends `DELETE /api/agent_builder/skills/{skillId}`

#### Scenario: Delete referenced skill with force
- **WHEN** a caller requests forced deletion for a user-created Skill id
- **THEN** the system sends `DELETE /api/agent_builder/skills/{skillId}?force=true`
- **AND** Kibana may remove references from agents before deleting the Skill

#### Scenario: Delete referenced skill without force
- **WHEN** Kibana returns `409 Conflict` because agents still reference the Skill
- **THEN** the system preserves the conflict response details for the caller

### Requirement: Skills Bundle Format
The system SHALL include Skills in multi-object bundle exports and imports as their own API family.

#### Scenario: Bundle skills to NDJSON
- **WHEN** the system writes a bundle containing Skills
- **THEN** it projects Skill directories to Kibana JSON records in `bundle/{space_id}/skills.ndjson`
- **AND** each NDJSON record preserves the Skill `id`

#### Scenario: Read skills from NDJSON bundle
- **WHEN** the system reads a bundle containing `bundle/{space_id}/skills.ndjson`
- **THEN** it loads those JSON records into the space Skills collection
- **AND** they are pushed through the Skills loader when the Skills API is selected and supported

### Requirement: Skills Referenced Content Validation
The system SHALL validate referenced-content export/import using a renamed copy of the out-of-the-box `threat-hunting` Skill.

#### Scenario: Clone threat-hunting for referenced content validation
- **WHEN** live validation runs against a Kibana version that supports Skills
- **THEN** the system fetches `GET /api/agent_builder/skills/threat-hunting`
- **AND** writes the fetched Skill into the skill directory representation
- **AND** changes the Skill `id` and `name` to a new test copy
- **AND** projects the renamed Skill directory to Kibana JSON without `readonly` or other system metadata
- **AND** imports the renamed copy through `POST /api/agent_builder/skills`
- **AND** verifies that the imported copy preserves the original referenced content names, relative paths, and contents
- **AND** deletes the renamed test Skill at the end of validation
- **AND** attempts best-effort deletion of the renamed test Skill if validation fails after creation

#### Scenario: Override validation source when threat-hunting is unavailable
- **WHEN** the target Kibana deployment does not expose `threat-hunting`
- **AND** the operator sets `KIBANA_TEST_SOURCE_SKILL_ID` to another out-of-the-box Skill with `referenced_content`
- **THEN** live validation uses that Skill as the referenced-content source
- **AND** performs the same rename, import, verify, and cleanup flow
