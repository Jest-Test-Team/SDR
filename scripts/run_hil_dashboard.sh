#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
export RUST_LOG="${RUST_LOG:-info}"
export HIL_PORT="${HIL_PORT:-8090}"
export ZMQ_ENDPOINT="${ZMQ_ENDPOINT:-tcp://127.0.0.1:5556}"
DASHBOARD_PORT="${DASHBOARD_PORT:-3001}"

wait_for_api() {
  local i
  for i in $(seq 1 60); do
    if curl -sf "http://127.0.0.1:${HIL_PORT}/api/v1/status" >/dev/null 2>&1; then
      echo "HIL API ready on :${HIL_PORT}"
      return 0
    fi
    sleep 0.5
  done
  echo "ERROR: HIL API did not start on :${HIL_PORT}" >&2
  return 1
}

echo "Starting HIL simulator API on :${HIL_PORT} ..."
if curl -sf "http://127.0.0.1:${HIL_PORT}/api/v1/status" >/dev/null 2>&1; then
  echo "HIL API already running on :${HIL_PORT}"
  HIL_PID=""
else
  cargo run -p hil-simulator --release &
  HIL_PID=$!
  wait_for_api
fi

cleanup() {
  if [[ -n "${HIL_PID:-}" ]]; then
    kill "$HIL_PID" 2>/dev/null || true
  fi
}
trap cleanup EXIT INT TERM

if [[ "${1:-}" == "--api-only" ]]; then
  if [[ -n "${HIL_PID:-}" ]]; then
    wait "$HIL_PID"
  else
    echo "API already running; press Ctrl+C to exit"
    while true; do sleep 3600; done
  fi
  exit 0
fi

echo "Starting Next.js dashboard on :${DASHBOARD_PORT}"
cd "$ROOT/web/hil-dashboard"
export NEXT_PUBLIC_HIL_WS_URL="ws://127.0.0.1:${HIL_PORT}/ws/live"
export HIL_API_URL="http://127.0.0.1:${HIL_PORT}"
if [[ ! -d node_modules ]]; then
  npm install
fi
npm run dev
