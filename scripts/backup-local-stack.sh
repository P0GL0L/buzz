#!/usr/bin/env bash
# Create a local-only logical database backup without stopping or resetting
# persistent Compose volumes.
set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
timestamp=$(date -u +%Y%m%dT%H%M%SZ)
backup_dir="${BUZZ_LOCAL_BACKUP_DIR:-$repo_root/build/local-stack-backups/$timestamp}"

running=$(docker inspect --format '{{.State.Running}}' buzz-postgres 2>/dev/null || true)
[[ "$running" == "true" ]] || {
  echo "buzz-postgres is not running" >&2
  exit 1
}

umask 077
mkdir -p "$backup_dir"
docker exec buzz-postgres pg_dump -U buzz -d buzz --format=custom >"$backup_dir/buzz.pgcustom"
docker exec buzz-postgres psql -U buzz -d buzz -Atc \
  "select version, description, success, installed_on from _sqlx_migrations order by version" \
  >"$backup_dir/migrations.tsv"
docker compose -f "$repo_root/docker-compose.yml" config --images \
  >"$backup_dir/images.txt"

shasum -a 256 "$backup_dir/buzz.pgcustom" "$backup_dir/migrations.tsv" \
  >"$backup_dir/SHA256SUMS"
printf '%s\n' "$backup_dir"
