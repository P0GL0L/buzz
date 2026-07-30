#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)

read_identity() {
    working_directory=$1
    (
        cd "$working_directory"
        # shellcheck source=instance-env.sh
        source "$repo_root/scripts/instance-env.sh" >/dev/null
        node -e 'const value=JSON.parse(process.env.BUZZ_TAURI_CONFIG); console.log(`${value.identifier}|${process.env.BUZZ_INSTANCE_SLUG ?? "main"}`)'
    )
}

root_identity=$(read_identity "$repo_root")
desktop_identity=$(read_identity "$repo_root/desktop")

if [[ "$root_identity" != "$desktop_identity" ]]; then
    echo "instance-env changes identity by source directory: $root_identity != $desktop_identity" >&2
    exit 1
fi

echo "instance-env source-directory identity is stable: $root_identity"
