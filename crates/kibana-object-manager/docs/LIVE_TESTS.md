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

## Manual Test Against Existing Kibana

```bash
KIBOB_LIVE_KIBANA_TESTS=1 \
KIBANA_TEST_URL=http://localhost:5601 \
KIBANA_TEST_USERNAME=elastic \
KIBANA_TEST_PASSWORD=changeme \
cargo test --test live_kibana_integration -- --ignored --nocapture
```
