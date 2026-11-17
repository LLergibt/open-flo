#!/usr/bin/env bash
set -euo pipefail
umask 077
project_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
source "$project_root/scripts/tooling.sh"
cd "$project_root"
if [ "$#" -gt 2 ]; then
  echo "Usage: ./scripts/create-admin.sh [username] [display-name]" >&2
  exit 1
fi
username="${1:-root}"
display_name="${2:-root}"
export DATABASE_URL="${DATABASE_URL:-sqlite://.data/flo.db}"
exec cargo run --locked -p flo-api -- create-admin --username "$username" --display-name "$display_name"
