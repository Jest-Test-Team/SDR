#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
DASHBOARD_PORT="${DASHBOARD_PORT:-3001}"
LIVE_CP_URL="${LIVE_CP_URL:-http://127.0.0.1:8092}"
LIVE_EDGE_URL="${LIVE_EDGE_URL:-http://127.0.0.1:8081}"

wait_for_health() {
  local url="$1"
  local name="$2"
  local i
  for i in $(seq 1 40); do
    if curl -sf "$url" >/dev/null 2>&1; then
      echo "$name ready: $url"
      return 0
    fi
    sleep 0.5
  done
  echo "WARN: $name not reachable at $url (start ./scripts/run_local.sh first)" >&2
  return 1
}

if [[ "${1:-}" == "--with-pipeline" ]]; then
  if ! curl -sf "${LIVE_CP_URL}/health" >/dev/null 2>&1; then
    echo "Starting local pipeline in background..."
    "$ROOT/scripts/run_local.sh" &
    PIPELINE_PID=$!
    trap 'kill "$PIPELINE_PID" 2>/dev/null || true' EXIT INT TERM
    wait_for_health "${LIVE_EDGE_URL}/health" "edge-gateway" || true
    wait_for_health "${LIVE_CP_URL}/health" "control-plane" || true
  else
    echo "Pipeline already running"
  fi
else
  wait_for_health "${LIVE_EDGE_URL}/health" "edge-gateway" || true
  wait_for_health "${LIVE_CP_URL}/health" "control-plane" || true
fi

echo "Starting Live dashboard on :${DASHBOARD_PORT}"
echo "  Control plane: ${LIVE_CP_URL}"
echo "  Edge gateway:  ${LIVE_EDGE_URL}"
echo "Open http://127.0.0.1:${DASHBOARD_PORT} → Live hardware tab"

cd "$ROOT/web/hil-dashboard"
export LIVE_CP_URL LIVE_EDGE_URL
export PORT="$DASHBOARD_PORT"
if [[ ! -d node_modules ]]; then
  npm install
fi
npm run dev
