## ADDED Requirements

### Requirement: Resource-Explicit Standalone Commands
The CLI SHALL expose standalone transfer commands using the grammar `kibob import <resource> <source>` and `kibob export <resource> <destination>`. Each invocation SHALL operate on exactly one Kibana API family. The canonical resource names SHALL be `skills`, `tools`, `agents`, and `workflows`.

#### Scenario: Import resource command
- **GIVEN** a local source for a supported resource family
- **WHEN** the user runs `kibob import <resource> <source>`
- **THEN** the CLI interprets the source only as the named resource family
- **AND** it does not require `--api`, `--as`, or another resource-disambiguation flag

#### Scenario: Export resource command
- **GIVEN** a configured Kibana instance
- **WHEN** the user runs `kibob export <resource> <destination>` with a valid selection
- **THEN** the CLI exports only the named resource family
- **AND** it does not require `--api`, `--as`, or another resource-disambiguation flag

#### Scenario: Unsupported resource family
- **WHEN** the user supplies a resource family not implemented by the selected command
- **THEN** argument parsing fails before connecting to Kibana
- **AND** command help lists the supported resource families

### Requirement: Standalone Space and Version Selection
Standalone transfer commands SHALL target one configured Kibana space and SHALL apply the existing capability gate for the selected resource family.

#### Scenario: Explicit non-default space
- **GIVEN** the user supplies `--space security`
- **WHEN** a standalone command calls its selected resource API
- **THEN** the request path is prefixed with `/s/security`

#### Scenario: Default space
- **GIVEN** no `--space` value or configured non-default space
- **WHEN** a standalone command calls its selected resource API
- **THEN** the request path has no `/s/default` prefix

#### Scenario: Resource capability versions
- **GIVEN** the detected Kibana version does not support the selected resource family
- **WHEN** the user imports or exports without the existing version-bypass option
- **THEN** the command sends no request to that resource API
- **AND** reports the existing minimum version for the selected family: Agents and Tools require `9.2.0`, Workflows require `9.3.0`, and Skills require `9.4.0`

### Requirement: Manifest-Free Skills Import
`kibob import skills` SHALL read Skills directly from the supplied source and SHALL NOT read, create, update, or require a project manifest.

#### Scenario: Import one Skill directory
- **GIVEN** `<source>/SKILL.md` exists
- **AND** no immediate child directory of `<source>` contains `SKILL.md`
- **WHEN** the user runs `kibob import skills <source>`
- **THEN** the command projects `<source>` as exactly one Skill
- **AND** treats the `id` in `SKILL.md` frontmatter as authoritative

#### Scenario: Import a collection of Skills
- **GIVEN** `<source>` does not contain `SKILL.md`
- **AND** one or more immediate child directories contain `SKILL.md`
- **WHEN** the user runs `kibob import skills <source>`
- **THEN** the command projects each immediate child Skill directory
- **AND** processes the Skills in deterministic path order
- **AND** does not recursively discover Skill directories below those immediate children

#### Scenario: Reject ambiguous source layout
- **GIVEN** `<source>/SKILL.md` exists
- **AND** at least one immediate child directory of `<source>` also contains `SKILL.md`
- **WHEN** the user runs `kibob import skills <source>`
- **THEN** the command fails before connecting to Kibana
- **AND** identifies both supported source layouts

#### Scenario: Reject empty source layout
- **GIVEN** `<source>` contains neither `SKILL.md` nor an immediate child directory containing `SKILL.md`
- **WHEN** the user runs `kibob import skills <source>`
- **THEN** the command fails before connecting to Kibana
- **AND** reports that no Skills were found

#### Scenario: Ignore project manifests
- **GIVEN** a manifest file exists in or adjacent to the supplied source
- **WHEN** the user runs `kibob import skills <source>`
- **THEN** the command neither reads nor modifies that manifest
- **AND** imports exactly the Skills selected by the source layout

### Requirement: Manifest-Free JSON Resource Import
`kibob import tools`, `kibob import agents`, and `kibob import workflows` SHALL read their existing project JSON/JSON5 representation directly from the supplied source and SHALL NOT read, create, update, or require a project manifest.

#### Scenario: Import one JSON resource
- **GIVEN** `<source>` is a `.json` file containing one resource of the selected family
- **WHEN** the user imports Tools, Agents, or Workflows
- **THEN** the command parses the file with the existing JSON5 reader
- **AND** projects exactly that resource

#### Scenario: Import a directory of JSON resources
- **GIVEN** `<source>` is a directory containing one or more immediate `.json` files
- **WHEN** the user imports Tools, Agents, or Workflows
- **THEN** the command parses each immediate `.json` file with the existing JSON5 reader
- **AND** processes files in deterministic path order
- **AND** does not recursively discover JSON files in child directories

#### Scenario: Reject unsupported JSON source
- **GIVEN** `<source>` is neither a `.json` file nor a directory containing an immediate `.json` file
- **WHEN** the user imports Tools, Agents, or Workflows
- **THEN** the command fails before connecting to Kibana
- **AND** reports the accepted source layouts

#### Scenario: Ignore JSON-family project manifests
- **GIVEN** `tools.yml`, `agents.yml`, or `workflows.yml` exists in or adjacent to the source
- **WHEN** the corresponding standalone import runs
- **THEN** the command neither reads nor modifies that manifest
- **AND** imports exactly the resources represented by the selected file or directory

### Requirement: Complete Standalone Import Validation
The command SHALL parse and validate every selected resource before sending any mutating Kibana request.

#### Scenario: Invalid resource prevents all mutation
- **GIVEN** a collection containing at least one valid resource and at least one invalid or unreadable resource
- **WHEN** the user runs a standalone import
- **THEN** the command sends no resource create request
- **AND** sends no resource update request
- **AND** reports the invalid local path and validation failure

#### Scenario: Duplicate resource IDs
- **GIVEN** two selected artifacts project to the same resource `id`
- **WHEN** the user runs a standalone import
- **THEN** the command fails before sending a mutating request
- **AND** identifies both local paths

#### Scenario: Referenced content projection
- **GIVEN** a valid Skill directory contains referenced content
- **WHEN** the command validates the import source
- **THEN** it uses the existing Skills filesystem projection and path-safety rules
- **AND** preserves deterministic referenced-content ordering

### Requirement: Idempotent Skills Import
`kibob import skills` SHALL apply each validated Skill using the existing create-or-update semantics and SHALL never delete or prune a remote Skill.

#### Scenario: Create missing Skill
- **GIVEN** `GET /api/agent_builder/skills/{skillId}` returns `404`
- **WHEN** the command imports the Skill
- **THEN** it sends `POST /api/agent_builder/skills`
- **AND** includes the required `kbn-xsrf: true` header
- **AND** sends the projected Skill payload including its `id`

#### Scenario: Update existing user-created Skill
- **GIVEN** `GET /api/agent_builder/skills/{skillId}` returns an existing Skill with `readonly: false`
- **WHEN** the command imports the Skill
- **THEN** it sends `PUT /api/agent_builder/skills/{skillId}`
- **AND** includes the required `kbn-xsrf: true` header
- **AND** omits `id`, `readonly`, and local-only `experimental` metadata from the update body

#### Scenario: Existing readonly Skill
- **GIVEN** `GET /api/agent_builder/skills/{skillId}` returns a Skill with `readonly: true`
- **WHEN** the command attempts to import the same ID
- **THEN** it sends no `POST` or `PUT` request for that Skill
- **AND** records that Skill as a failed import rather than a successful application

#### Scenario: No implicit dependencies or pruning
- **GIVEN** an imported Skill references tool IDs
- **WHEN** the command completes
- **THEN** it does not import those Tools implicitly
- **AND** it does not delete any remote Skill absent from the source

### Requirement: Idempotent JSON Resource Import
Standalone Tools, Agents, and Workflows imports SHALL use their existing create-or-update loaders and SHALL never delete, prune, or expand dependencies.

#### Scenario: Create or update Tool
- **GIVEN** a validated Tool with an authoritative `id`
- **WHEN** the command imports the Tool
- **THEN** it checks `HEAD /api/agent_builder/tools/{id}`
- **AND** sends `POST /api/agent_builder/tools` when missing or `PUT /api/agent_builder/tools/{id}` when present
- **AND** mutating requests include `kbn-xsrf: true`

#### Scenario: Create or update Agent
- **GIVEN** a validated Agent with an authoritative `id`
- **WHEN** the command imports the Agent
- **THEN** it checks `HEAD /api/agent_builder/agents/{id}`
- **AND** sends `POST /api/agent_builder/agents` when missing or `PUT /api/agent_builder/agents/{id}` when present
- **AND** omits response-only audit fields such as `created_by` from the mutating request
- **AND** mutating requests include `kbn-xsrf: true`

#### Scenario: Create or update Workflow
- **GIVEN** a validated Workflow with an authoritative `id`
- **WHEN** the command imports the Workflow
- **THEN** on Kibana 9.3 it checks `HEAD /api/workflows/{id}`
- **AND** sends `POST /api/workflows` when missing or `PUT /api/workflows/{id}` when present
- **THEN** on Kibana 9.4 or later it checks `HEAD /api/workflows/workflow/{id}`
- **AND** sends `POST /api/workflows/workflow` when missing or `PUT /api/workflows/workflow/{id}` when present
- **AND** every Workflow request includes `X-Elastic-Internal-Origin: Kibana`
- **AND** mutating requests include `kbn-xsrf: true`

#### Scenario: JSON resource dependencies remain external
- **GIVEN** an imported Agent, Tool, or Workflow references another resource
- **WHEN** the command completes
- **THEN** it does not import the referenced resource implicitly
- **AND** it does not delete remote resources absent from the source

### Requirement: Explicit Resource Export Selection
Every standalone export SHALL require either one or more `--id <resource-id>` selectors or the `--all` selector, and the selector forms SHALL be mutually exclusive.

#### Scenario: Export selected resource IDs
- **GIVEN** the user supplies one or more `--id` values
- **WHEN** the user runs `kibob export <resource> <destination>`
- **THEN** the command fetches each selected ID through the existing family-specific item endpoint
- **AND** preserves the command-line ID order in its export plan and summary

#### Scenario: Export all user-created resources
- **GIVEN** the user supplies `--all`
- **WHEN** the user runs `kibob export <resource> <destination>`
- **THEN** the command uses the existing family-specific list or search endpoint
- **AND** excludes entries with `readonly: true` when that family exposes readonly resources
- **AND** fetches the complete definition of each selected resource through its item endpoint
- **AND** orders the selected resources deterministically by ID

#### Scenario: Missing export selection
- **WHEN** the user runs `kibob export <resource> <destination>` without `--id` or `--all`
- **THEN** argument parsing fails before connecting to Kibana
- **AND** explains that an explicit selection is required

#### Scenario: Conflicting export selection
- **WHEN** the user supplies both `--id` and `--all`
- **THEN** argument parsing fails before connecting to Kibana

#### Scenario: Explicit readonly resource
- **GIVEN** an explicitly selected resource has `readonly: true`
- **WHEN** the command fetches that resource
- **THEN** the command fails without writing export files
- **AND** reports that readonly resources are not standalone-importable user-created resources

#### Scenario: Family-specific export endpoints
- **WHEN** the command exports Tools
- **THEN** it lists with `GET /api/agent_builder/tools` and fetches with `GET /api/agent_builder/tools/{id}`
- **WHEN** the command exports Agents
- **THEN** it lists with `GET /api/agent_builder/agents` and fetches with `GET /api/agent_builder/agents/{id}`
- **WHEN** the command exports Skills
- **THEN** it lists with `GET /api/agent_builder/skills` and fetches with `GET /api/agent_builder/skills/{id}`
- **WHEN** the command exports Workflows
- **THEN** on Kibana 9.3 it searches with `POST /api/workflows/search` and fetches with `GET /api/workflows/{id}`
- **THEN** on Kibana 9.4 or later it lists with `GET /api/workflows` and fetches with `GET /api/workflows/workflow/{id}`
- **AND** Workflow requests include `X-Elastic-Internal-Origin: Kibana`

### Requirement: Manifest-Free Skills Export Layout
`kibob export skills` SHALL write each exported Skill beneath the destination as `{destination}/{skill-id}/SKILL.md` with referenced content, and SHALL NOT write a Skills manifest.

#### Scenario: Export one Skill
- **GIVEN** one selected user-created Skill is fetched and validated
- **WHEN** the command writes the export
- **THEN** it writes the Skill to `{destination}/{skill-id}/SKILL.md`
- **AND** writes referenced content using the existing Skills filesystem projection
- **AND** does not create `skills.yml` or another manifest

#### Scenario: Export multiple Skills
- **GIVEN** multiple selected user-created Skills are fetched and validated
- **WHEN** the command writes the export
- **THEN** it writes one immediate child directory per Skill ID
- **AND** the resulting destination can be passed directly to `kibob import skills <destination>`

#### Scenario: Existing Skill output without overwrite
- **GIVEN** `{destination}/{skill-id}` already exists
- **WHEN** the user exports that Skill without `--overwrite`
- **THEN** the command fails before writing any selected Skill
- **AND** reports every conflicting output path

#### Scenario: Existing Skill output with overwrite
- **GIVEN** `{destination}/{skill-id}` already exists
- **WHEN** the user exports that Skill with `--overwrite`
- **THEN** the command replaces that Skill directory using the existing safe Skills filesystem writer
- **AND** leaves unrelated destination entries unchanged

### Requirement: Manifest-Free JSON Resource Export Layout
Standalone Tools, Agents, and Workflows exports SHALL write one `.json` file per resource directly beneath the destination using the existing family-specific project serialization and filename rules, and SHALL NOT write a manifest.

#### Scenario: Export JSON resources
- **GIVEN** one or more selected Tools, Agents, or Workflows are fetched and validated
- **WHEN** the command writes the export
- **THEN** it applies the existing family-specific multiline formatting
- **AND** writes one immediate `.json` file per resource beneath the destination
- **AND** the resulting destination can be passed directly to the matching standalone import command
- **AND** it does not create `tools.yml`, `agents.yml`, `workflows.yml`, or another manifest

#### Scenario: JSON output filename collision
- **GIVEN** two selected resources map to the same existing project-format filename
- **WHEN** the command plans the export
- **THEN** it fails before writing any selected resource
- **AND** identifies the colliding resource IDs and output path

#### Scenario: Existing JSON output without overwrite
- **GIVEN** a selected JSON output path already exists
- **WHEN** the user exports without `--overwrite`
- **THEN** the command fails before writing any selected resource
- **AND** reports every conflicting output path

#### Scenario: Existing JSON output with overwrite
- **GIVEN** a selected JSON output path already exists
- **WHEN** the user exports with `--overwrite`
- **THEN** the command replaces only the selected resource file
- **AND** leaves unrelated destination entries unchanged

### Requirement: Standalone Batch Outcomes
Standalone import and export commands SHALL report attempted, applied or written, skipped, and failed counts and SHALL return a non-zero exit status when any selected resource fails.

#### Scenario: Successful batch
- **GIVEN** every selected resource is applied or written successfully
- **WHEN** the standalone command completes
- **THEN** it reports each successful resource ID
- **AND** exits successfully

#### Scenario: Partial API failure
- **GIVEN** at least one selected resource succeeds and at least one selected resource API request fails
- **WHEN** the import command completes its bounded batch
- **THEN** it reports the outcome for every selected resource
- **AND** reports aggregate attempted, applied, skipped, and failed counts
- **AND** exits non-zero

#### Scenario: Export fetch failure
- **GIVEN** at least one selected resource cannot be fetched or validated
- **WHEN** the export command completes its fetch phase
- **THEN** it writes none of the selected resources
- **AND** reports each fetch or validation failure
- **AND** exits non-zero

### Requirement: License-Aware Live Validation
The test strategy SHALL keep unit, filesystem, CLI, and mocked API coverage independent of a live license and SHALL make live import/export validation explicitly dependent on a compatible test node.

#### Scenario: Licensed compatible test node
- **GIVEN** the live-test runner is configured with a Kibana node whose version and active license permit a selected resource API
- **WHEN** live standalone transfer validation runs for that family
- **THEN** it exercises export, local artifact round-trip, import, verification, and cleanup against that node

#### Scenario: Resource API unavailable because of version or license
- **GIVEN** the configured live-test node explicitly reports that a selected resource API is unavailable because of its Kibana version or active license
- **WHEN** live standalone transfer validation reaches that family
- **THEN** the test reports that family as skipped with the detected reason
- **AND** does not report the condition as an import/export implementation failure

#### Scenario: Unexpected authentication or API failure
- **GIVEN** the live-test node returns an authentication failure or an API failure not explicitly identified as a version or license restriction
- **WHEN** live standalone transfer validation runs
- **THEN** the test fails
- **AND** preserves the response details for diagnosis

#### Scenario: License-independent verification
- **WHEN** the ordinary workspace test suite runs without a live licensed node
- **THEN** unit, filesystem, parser, command, and mocked HTTP tests still verify every supported resource family
- **AND** live tests remain opt-in

### Requirement: Existing Project Workflow Compatibility
The standalone commands SHALL NOT change the syntax or behavior of existing project-oriented commands.

#### Scenario: Existing push remains project-aware
- **WHEN** the user runs `kibob push <project>`
- **THEN** the command continues to interpret the path as a kibob project
- **AND** continues to apply existing manifest and API-filter behavior

#### Scenario: Existing pull remains project-aware
- **WHEN** the user runs `kibob pull <project>`
- **THEN** the command continues to write the existing tracked project layout
- **AND** continues to maintain project manifests
