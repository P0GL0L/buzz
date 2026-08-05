#!/bin/zsh
set -euo pipefail

repo_dir="${0:A:h:h}"
cd "$repo_dir"

set -a
source "$repo_dir/.env"
set +a

exec "$repo_dir/target/debug/buzz-relay"
