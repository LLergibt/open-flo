#!/usr/bin/env bash
set -euo pipefail
umask 077

project_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
source "$project_root/scripts/tooling.sh"
cd "$project_root"

for tool in cargo npm curl; do
  command -v "$tool" >/dev/null 2>&1 || { echo "Required tool not found: $tool" >&2; exit 1; }
done

if [ ! -d apps/web/node_modules ]; then
  npm --prefix apps/web ci
fi

export BIND_ADDR="${BIND_ADDR:-127.0.0.1:8080}"
export DATABASE_URL="${DATABASE_URL:-sqlite://.data/flo.db}"
export API_PROXY_TARGET="${API_PROXY_TARGET:-http://127.0.0.1:8080}"
# Explicit local development mode. Public installations require HTTPS + Secure cookies.
export COOKIE_SECURE="${COOKIE_SECURE:-false}"
export TRUSTED_ORIGINS="${TRUSTED_ORIGINS:-http://127.0.0.1:5173,http://localhost:5173}"
export PUBLIC_APP_URL="${PUBLIC_APP_URL:-http://127.0.0.1:5173}"
mkdir -p .data

api_pid=""
web_pid=""
cleanup() {
  trap - EXIT INT TERM
  [ -z "$web_pid" ] || kill "$web_pid" 2>/dev/null || true
  [ -z "$api_pid" ] || kill "$api_pid" 2>/dev/null || true
  [ -z "$web_pid" ] || wait "$web_pid" 2>/dev/null || true
  [ -z "$api_pid" ] || wait "$api_pid" 2>/dev/null || true
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM

cargo run --locked -p flo-api &
api_pid=$!

echo "Waiting for API and SQLite: $API_PROXY_TARGET/health"
attempts=0
until curl --fail --silent --connect-timeout 1 --max-time 2 "${API_PROXY_TARGET%/}/health" >/dev/null 2>&1; do
  if ! kill -0 "$api_pid" 2>/dev/null; then
    wait "$api_pid"
    exit 1
  fi
  attempts=$((attempts + 1))
  if [ "$attempts" -ge 300 ]; then
    echo "API did not become ready. Check the build output and API_PROXY_TARGET." >&2
    exit 1
  fi
  sleep 1
done

# Run Vite directly so stopping this script also stops the actual dev server.
node apps/web/node_modules/vite/bin/vite.js apps/web --host 127.0.0.1 --port 5173 --strictPort &
web_pid=$!

echo "Web: http://127.0.0.1:5173"
echo "API: $BIND_ADDR (first Rust build may take a few minutes)"
echo "Stop both services: Ctrl+C"

while kill -0 "$api_pid" 2>/dev/null && kill -0 "$web_pid" 2>/dev/null; do
  sleep 1
done

# A service stopped; return its exit status and stop the other service.
if ! kill -0 "$api_pid" 2>/dev/null; then
  wait "$api_pid"
else
  wait "$web_pid"
fi
