#!/usr/bin/env bash
# Native macOS QA evidence helper for the isolated Buzz development app.
#
# This script never launches the production app, resets state, or prints a
# keyring secret. It records hashes and metadata so native observations can be
# compared without treating browser-mock screenshots as Tauri evidence.

set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
evidence_root=${BUZZ_NATIVE_QA_EVIDENCE_DIR:-"$repo_root/build/native-qa"}
production_data="$HOME/Library/Application Support/xyz.block.buzz.app"
production_nest="$HOME/.buzz/AGENTS.md"
production_keyring_service="buzz-desktop"

usage() {
    echo "Usage: $0 preflight | snapshot LABEL | compare LABEL_A LABEL_B | status"
}

sha256_stream() {
    shasum -a 256 | awk '{print $1}'
}

instance_values() {
    (
        cd "$repo_root"
        # shellcheck source=instance-env.sh
        source "$repo_root/scripts/instance-env.sh"
        instance_id=$(node -e 'console.log(JSON.parse(process.env.BUZZ_TAURI_CONFIG).identifier)')
        keyring_service="buzz-desktop-dev.${BUZZ_INSTANCE_SLUG:-main}"
        printf '%s\n%s\n%s\n' "$instance_id" "$keyring_service" "$BUZZ_RELAY_URL"
    )
}

preflight() {
    values=$(instance_values)
    instance_id=$(printf '%s\n' "$values" | sed -n '1p')
    keyring_service=$(printf '%s\n' "$values" | sed -n '2p')
    relay_url=$(printf '%s\n' "$values" | sed -n '3p')

    if [[ "$instance_id" != xyz.block.buzz.app.dev* ]]; then
        echo "Refusing native QA: non-development bundle identifier '$instance_id'." >&2
        exit 1
    fi
    if [[ "$keyring_service" != buzz-desktop-dev.* ]]; then
        echo "Refusing native QA: non-development keyring service '$keyring_service'." >&2
        exit 1
    fi
    if [[ "$instance_id" == "xyz.block.buzz.app" || "$keyring_service" == "$production_keyring_service" ]]; then
        echo "Refusing native QA: production identity boundary collapsed." >&2
        exit 1
    fi

    python3 - "$instance_id" "$keyring_service" "$relay_url" <<'PY'
import json
import sys

print(json.dumps({
    "bundleIdentifier": sys.argv[1],
    "keyringService": sys.argv[2],
    "relayUrl": sys.argv[3],
    "productionIsolated": True,
}, sort_keys=True))
PY
}

directory_manifest() {
    target=$1
    if [[ ! -d "$target" ]]; then
        printf 'absent\n'
        return
    fi
    while IFS= read -r path; do
        relative=${path#"$target"/}
        if [[ -L "$path" ]]; then
            printf 'link\t%s\t%s\n' "$relative" "$(readlink "$path")"
        elif [[ -f "$path" ]]; then
            digest=$(shasum -a 256 "$path" | awk '{print $1}')
            printf 'file\t%s\t%s\n' "$relative" "$digest"
        elif [[ -d "$path" ]]; then
            printf 'dir\t%s\n' "$relative"
        fi
    done < <(find "$target" -mindepth 1 -print | LC_ALL=C sort)
}

keyring_metadata_digest() {
    service=$1
    security find-generic-password -s "$service" -a secrets 2>/dev/null |
        sed -E '/^password:/d' |
        sha256_stream
}

keyring_secret_digest() {
    service=$1
    if ! security find-generic-password -s "$service" -a secrets >/dev/null 2>&1; then
        printf 'absent\n'
        return
    fi
    security find-generic-password -s "$service" -a secrets -w 2>/dev/null |
        sha256_stream
}

snapshot() {
    label=${1:-}
    if [[ -z "$label" || ! "$label" =~ ^[a-zA-Z0-9._-]+$ ]]; then
        echo "Snapshot label must use only letters, digits, dot, underscore, or dash." >&2
        exit 1
    fi
    preflight >/dev/null
    values=$(instance_values)
    instance_id=$(printf '%s\n' "$values" | sed -n '1p')
    keyring_service=$(printf '%s\n' "$values" | sed -n '2p')
    destination="$evidence_root/$label"
    mkdir -p "$destination"

    directory_manifest "$production_data" >"$destination/production-data.manifest"
    keyring_metadata_digest "$production_keyring_service" >"$destination/production-keyring-metadata.sha256"
    keyring_secret_digest "$keyring_service" >"$destination/development-identity.sha256"
    if [[ -f "$production_nest" ]]; then
        shasum -a 256 "$production_nest" | awk '{print $1}' >"$destination/production-nest.sha256"
    else
        printf 'absent\n' >"$destination/production-nest.sha256"
    fi
    python3 - "$destination/summary.json" "$label" "$instance_id" "$keyring_service" <<'PY'
import datetime
import json
import pathlib
import sys

path = pathlib.Path(sys.argv[1])
path.write_text(json.dumps({
    "capturedAt": datetime.datetime.now(datetime.timezone.utc).isoformat(),
    "label": sys.argv[2],
    "bundleIdentifier": sys.argv[3],
    "keyringService": sys.argv[4],
    "containsSecrets": False,
}, indent=2, sort_keys=True) + "\n")
PY
    echo "$destination"
}

compare() {
    left=${1:-}
    right=${2:-}
    if [[ -z "$left" || -z "$right" ]]; then
        usage >&2
        exit 1
    fi
    left_dir="$evidence_root/$left"
    right_dir="$evidence_root/$right"
    for name in production-data.manifest production-keyring-metadata.sha256 production-nest.sha256; do
        cmp -s "$left_dir/$name" "$right_dir/$name" || {
            echo "Production isolation check failed: $name changed." >&2
            exit 1
        }
    done
    left_identity=$(<"$left_dir/development-identity.sha256")
    right_identity=$(<"$right_dir/development-identity.sha256")
    if [[ "$left_identity" == "absent" || "$right_identity" == "absent" ]]; then
        echo "Development identity persistence is not proven: keyring record absent." >&2
        exit 1
    fi
    if [[ "$left_identity" != "$right_identity" ]]; then
        echo "Development identity persistence failed: keyring digest changed." >&2
        exit 1
    fi
    echo '{"developmentIdentityPersistent":true,"productionStateUnchanged":true}'
}

status() {
    preflight
    if [[ ! -d "$evidence_root" ]]; then
        echo '{"nativeEvidence":"not-started"}'
        return
    fi
    find "$evidence_root" -mindepth 2 -maxdepth 2 -name summary.json -print | sort
}

case "${1:-}" in
    preflight)
        preflight
        ;;
    snapshot)
        snapshot "${2:-}"
        ;;
    compare)
        compare "${2:-}" "${3:-}"
        ;;
    status)
        status
        ;;
    *)
        usage >&2
        exit 1
        ;;
esac
