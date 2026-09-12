## ADDED Requirements

### Requirement: Diagnostic output contract
The CLI SHALL write diagnostics to stderr and keep stdout available for help, version, or documented data output.

#### Scenario: Redirected diagnostics
- **WHEN** a command fails with stderr redirected and color has not been explicitly forced
- **THEN** stdout contains no diagnostics
- **AND** stderr contains no ANSI color codes, including codes embedded in log fields

#### Scenario: Explicit no-color preference
- **WHEN** NO_COLOR and a force-color variable are both set
- **THEN** diagnostics contain no ANSI color codes
