---
type: Guide
title: "Live Kibana Tests"
description: Run isolated integration tests against a live Kibana instance.
generated: { by: codex/gpt-6, at: 2026-09-07T05:36:37Z }
---

# Live Kibana Tests

The live integration suite runs against a containerized Elasticsearch and
Kibana stack. It is ignored by default because it downloads container images,
starts services, and mutates a temporary Kibana space.

## Run

```bash
crates/kibana-object-manager/scripts/live-kibana-tests.sh test
```

The script:

1. Creates `target/live-kibana/.env` with deterministic local credentials.
2. Starts `tests/live/docker-compose.yml`.
3. Waits for Kibana to answer `/api/status`.
4. Runs `cargo test --test live_kibana_integration -- --ignored --nocapture`.

The ordinary workspace test suite remains license-independent:

```bash
cargo test --workspace --all-features
```

It covers standalone discovery, parsing, CLI behavior, filesystem round trips,
and mocked APIs for Skills, Tools, Agents, and Workflows without a live node.

## Stop and Clean Up

```bash
crates/kibana-object-manager/scripts/live-kibana-tests.sh down
```

This removes the live-test containers and their Docker volume.

## Defaults

The stack listens on:

- Elasticsearch: `http://localhost:19200`
- Kibana: `http://localhost:15601`

The generated environment can be edited at `target/live-kibana/.env`. Useful
settings:

- `ELASTIC_VERSION`: Elastic Stack image tag, default `9.3.3`
- `ELASTICSEARCH_HEAP_INIT` / `ELASTICSEARCH_HEAP_MAX`: default `2g`
- `KIBANA_TEST_KIBANA_PORT`: host port for Kibana, default `15601`
- `KIBANA_TEST_SPACE_PREFIX`: prefix for temporary test spaces

## Test Against an Existing Licensed Kibana

Use the harness without starting containers by configuring an
operator-controlled node:

```bash
KIBANA_TEST_URL=http://localhost:5601 \
KIBANA_TEST_USERNAME=elastic \
KIBANA_TEST_PASSWORD=changeme \
crates/kibana-object-manager/scripts/live-kibana-tests.sh test-existing
```

`KIBANA_TEST_APIKEY` can be used instead of username/password. The harness does
not provision, activate, downgrade, or otherwise change the node’s license.

Each standalone family case records the detected Kibana version, probes the
family API, creates a unique temporary resource, exports and verifies its local
artifact, re-imports it, verifies the remote definition, and performs
best-effort cleanup.

Results are reported per family:

- `passed` when the complete round trip succeeds.
- `skipped` only when the detected Kibana version is below the family minimum,
  or the API explicitly returns a license/subscription restriction.
- `failed` for authentication errors, generic `403`/`404` responses, malformed
  responses, and all other unexpected API failures; response details are
  retained for diagnosis.

Minimum versions are Agents and Tools 9.2.0, Workflows 9.3.0, and Skills 9.4.0.
