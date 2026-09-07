# Repository validation and operational fixes

## Why

The audit found diagnostics on stdout with ANSI escapes, unbounded HTTP requests, stale workspace release guidance, broken documentation links, and missing local/CI validation.

## What Changes

- Keep diagnostics on stderr and strip embedded color codes when color is disabled.
- Add configurable request and connection deadlines.
- Establish a documented preflight, pinned tools, a documentation bundle, and PR-scoped OpenSpec checks.
- Correct release instructions and verify source archives before formula updates.

## Impact

HTTP requests now have finite defaults. Long operations can raise the configured deadlines. An operation can time out after the server applies a write, so callers must inspect remote state before retrying. Existing published versions and tags remain intact.
