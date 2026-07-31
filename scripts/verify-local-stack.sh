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

require_loopback_bindings() {
  local name="$1"
  local bindings
  bindings=$(docker port "$name" 2>/dev/null || true)
  [[ -n "$bindings" ]] || {
    echo "$name has no published development ports" >&2
    return 1
  }
  while IFS= read -r binding; do
    [[ "$binding" == *"-> 127.0.0.1:"* ]] || {
      echo "$name has a non-loopback binding: $binding" >&2
      return 1
    }
  done <<<"$bindings"
}

require_container buzz-postgres healthy
require_container buzz-redis healthy
require_container buzz-keycloak healthy
require_container buzz-minio healthy
require_container buzz-prometheus running

require_loopback_bindings buzz-postgres
require_loopback_bindings buzz-redis
require_loopback_bindings buzz-keycloak
require_loopback_bindings buzz-minio
require_loopback_bindings buzz-prometheus

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

  relay_info=$(
    curl --fail --silent --show-error \
      -H 'Accept: application/nostr+json' \
      http://localhost:3000/
  )
  pairing_relay_url=$(jq -r '.pairing_relay_url // empty' <<<"$relay_info")
  [[ "$pairing_relay_url" =~ ^wss://[^/]+(/.*)?$ ]] || {
    echo "relay does not advertise a phone-reachable WSS pairing relay" >&2
    exit 1
  }

  relay_headers=""
  relay_curl_status=0
  relay_headers=$(
    curl --http1.1 --max-time 2 --silent \
      --dump-header - --output /dev/null \
      -H 'Connection: Upgrade' \
      -H 'Upgrade: websocket' \
      -H 'Sec-WebSocket-Version: 13' \
      -H 'Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==' \
      http://127.0.0.1:3000/
  ) || relay_curl_status=$?
  [[ "$relay_curl_status" == "0" || "$relay_curl_status" == "28" ]] || {
    echo "relay WebSocket handshake failed with curl status $relay_curl_status" >&2
    exit 1
  }
  grep -q '^HTTP/1.1 101 Switching Protocols' <<<"$relay_headers" || {
    echo "relay did not accept a WebSocket upgrade" >&2
    exit 1
  }
fi

printf '{"containers":"healthy","migrationState":"%s","localCommunities":%s,"relayChecked":%s,"pairingRelayConfigured":%s}\n' \
  "$migration_state" "$community_count" \
  "$([[ "$with_relay" == "1" ]] && echo true || echo false)" \
  "$([[ "$with_relay" == "1" ]] && echo true || echo false)"
