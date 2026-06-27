#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
compose_file="${repo_root}/tests/live/docker-compose.yml"
env_dir="${repo_root}/target/live-kibana"
env_file="${env_dir}/.env"

container_runtime="${CONTAINER_RUNTIME:-}"
if [[ -z "${container_runtime}" ]]; then
  if command -v docker >/dev/null 2>&1; then
    container_runtime="docker"
  elif command -v podman >/dev/null 2>&1; then
    container_runtime="podman"
  else
    echo "No container runtime found; install docker or podman" >&2
    exit 1
  fi
fi

create_env_file() {
  mkdir -p "${env_dir}"
  if [[ -f "${env_file}" ]]; then
    return
  fi

  cat > "${env_file}" <<'ENV'
ELASTIC_CONTAINER_REGISTRY=docker.elastic.co
ELASTIC_VERSION=9.3.3
ELASTIC_PASSWORD=kibob-live-elastic-password
KIBANA_SYSTEM_PASSWORD=kibob-live-kibana-system-password
KIBANA_ENCRYPTION_KEY=kibob_live_tests_encryption_key_32
ELASTICSEARCH_HEAP_INIT=2g
ELASTICSEARCH_HEAP_MAX=2g
KIBANA_TEST_ES_PORT=19200
KIBANA_TEST_KIBANA_PORT=15601
KIBANA_TEST_SPACE_PREFIX=kibob-live
ENV
}

source_env() {
  create_env_file
  set -a
  # shellcheck disable=SC1090
  source "${env_file}"
  set +a
}

compose() {
  "${container_runtime}" compose --env-file "${env_file}" --file "${compose_file}" "$@"
}

wait_for_kibana() {
  local url="http://localhost:${KIBANA_TEST_KIBANA_PORT:-15601}"
  local username="elastic"
  local password="${ELASTIC_PASSWORD:-kibob-live-elastic-password}"
  local deadline=$((SECONDS + 300))

  until curl -fsS -u "${username}:${password}" "${url}/api/status" >/dev/null 2>&1; do
    if (( SECONDS >= deadline )); then
      echo "Timed out waiting for Kibana at ${url}" >&2
      compose logs --tail 80 kibana >&2 || true
      exit 1
    fi
    sleep 5
  done
}

cmd="${1:-test}"
source_env

case "${cmd}" in
  up)
    compose up --detach
    wait_for_kibana
    echo "Kibana is ready at http://localhost:${KIBANA_TEST_KIBANA_PORT}"
    ;;
  down)
    compose down --volumes --remove-orphans
    ;;
  logs)
    compose logs "${@:2}"
    ;;
  test)
    compose up --detach
    wait_for_kibana
    (
      cd "${repo_root}"
      KIBOB_LIVE_KIBANA_TESTS=1 \
      KIBANA_TEST_URL="http://localhost:${KIBANA_TEST_KIBANA_PORT}" \
      KIBANA_TEST_USERNAME=elastic \
      KIBANA_TEST_PASSWORD="${ELASTIC_PASSWORD}" \
      KIBANA_TEST_SPACE_PREFIX="${KIBANA_TEST_SPACE_PREFIX}" \
      cargo test --test live_kibana_integration -- --ignored --nocapture
    )
    ;;
  *)
    echo "Usage: $0 [up|down|logs|test]" >&2
    exit 1
    ;;
esac
