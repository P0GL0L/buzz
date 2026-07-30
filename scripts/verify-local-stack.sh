#!/usr/bin/env bash
# Read-only health and migration verification for the development-only stack.
set -euo pipefail

with_relay=0
if [[ "${1:-}" == "--with-relay" ]]; then
  with_relay=1
elif [[ $# -gt 0 ]]; then
  echo "usage: $0 [--with-relay]" >&2
  exit 2
fi

require_container() {
  local name="$1"
  local expected="${2:-running}"
  local running health
  running=$(docker inspect --format '{{.State.Running}}' "$name" 2>/dev/null || true)
  [[ "$running" == "true" ]] || {
    echo "$name is not running" >&2
    return 1
  }
  if [[ "$expected" == "healthy" ]]; then
    health=$(docker inspect --format '{{if .State.Health}}{{.State.Health.Status}}{{else}}missing{{end}}' "$name")
    [[ "$health" == "healthy" ]] || {
      echo "$name health is $health" >&2
      return 1
    }
  fi
}

require_container buzz-postgres healthy
require_container buzz-redis healthy
require_container buzz-keycloak healthy
require_container buzz-minio healthy
require_container buzz-prometheus running

curl --fail --silent --show-error http://localhost:9000/minio/health/live >/dev/null
curl --fail --silent --show-error http://localhost:9090/-/ready >/dev/null

migration_state=$(
  docker exec buzz-postgres psql -U buzz -d buzz -Atc \
    "select count(*) || ':' || coalesce(max(version), 0) from _sqlx_migrations where success"
)
community_count=$(
  docker exec buzz-postgres psql -U buzz -d buzz -Atc \
    "select count(*) from communities"
)

if [[ "$with_relay" == "1" ]]; then
  curl --fail --silent --show-error http://localhost:3000/health >/dev/null
  curl --fail --silent --show-error http://localhost:3000/_readiness >/dev/null
fi

printf '{"containers":"healthy","migrationState":"%s","localCommunities":%s,"relayChecked":%s}\n' \
  "$migration_state" "$community_count" "$([[ "$with_relay" == "1" ]] && echo true || echo false)"
