#!/usr/bin/env bash
set -euo pipefail
umask 077
project_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$project_root"
mode=local
if [ "${1:-}" = --docker ]; then
  mode=docker
  shift
fi
if [ "$#" -gt 2 ]; then
  echo "Usage: ./scripts/create-admin.sh [--docker] [username] [display-name]" >&2
  exit 1
fi
username="${1:-root}"
display_name="${2:-root}"
if [ "$mode" = docker ]; then
  if [ ! -s .env.ip-tls ]; then
    echo "Run scripts/ip-tls.sh init first to configure production Compose." >&2
    exit 1
  fi
  if docker compose version >/dev/null 2>&1; then
    compose=(docker compose)
  elif command -v docker-compose >/dev/null; then
    compose=(docker-compose)
  else
    echo "Docker Compose is required for --docker." >&2
    exit 1
  fi
  unset PUBLIC_ORIGIN
  exec "${compose[@]}" -p open-source-flo --env-file .env.ip-tls -f compose.prod.yaml \
    exec api flo-api create-admin --username "$username" --display-name "$display_name"
fi

source "$project_root/scripts/tooling.sh"
if ! command -v cargo >/dev/null; then
  echo "Cargo is needed for local development. For a running production stack, use ./scripts/create-admin.sh --docker." >&2
  exit 127
fi
export DATABASE_URL="${DATABASE_URL:-sqlite://.data/flo.db}"
exec cargo run --locked -p flo-api -- create-admin --username "$username" --display-name "$display_name"
