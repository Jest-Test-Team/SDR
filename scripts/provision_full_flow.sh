#!/usr/bin/env bash
#
# Full provisioning flow: bring up the hil-simulator backend, run the
# enroll -> claim -> rotate -> revoke self-check through its HTTP API (the same
# path the web dashboard uses), then tear down.
#
# By default the backend auto-detects the ESP32-S3 node and runs in REAL
# HARDWARE mode (driving the boards over ESP-NOW). If no board is found it falls
# back to SIMULATION mode automatically, so this script always produces a result.
#
# Usage:
#   ./scripts/provision_full_flow.sh                # hardware auto, run demo, stop
#   ./scripts/provision_full_flow.sh --sim          # force simulation backend
#   ./scripts/provision_full_flow.sh --dashboard    # after the demo, launch the UI
#   ./scripts/provision_full_flow.sh --device dev-007
#
# Prereqs: boards flashed (./scripts/flash_tx.sh + ./scripts/flash_gw.sh) if you
# want hardware mode. Nothing else may hold the S3 USB port (close espflash
# monitors / sim_node.py first).
#
set -uo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
HIL_PORT="${HIL_PORT:-8090}"
GW_SERIAL="auto"
DEVICE_ARGS=()
WANT_DASHBOARD=0
RESTART=0

while [[ $# -gt 0 ]]; do
  case "$1" in
    --sim)       GW_SERIAL=""; shift ;;
    --restart)   RESTART=1; shift ;;
    --dashboard) WANT_DASHBOARD=1; shift ;;
    --device)    DEVICE_ARGS=(--device "$2"); shift 2 ;;
    --port)      HIL_PORT="$2"; shift 2 ;;
    -h|--help)   sed -n '2,21p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) echo "unknown arg: $1" >&2; exit 2 ;;
  esac
done

BASE="http://127.0.0.1:${HIL_PORT}"
LOG="$(mktemp -t hilsim.XXXXXX.log)"
HIL_PID=""

cleanup() {
  if [[ -n "$HIL_PID" ]] && kill -0 "$HIL_PID" 2>/dev/null; then
    echo ">> stopping backend (pid $HIL_PID)"
    kill "$HIL_PID" 2>/dev/null || true
  fi
}
trap cleanup EXIT INT TERM

# 1. Start the backend (unless one is already up on this port) -------------
if curl -sf "${BASE}/api/v1/status" >/dev/null 2>&1; then
  RUNNING_MODE="$(curl -s "${BASE}/api/v1/gateway/status" | python3 -c 'import json,sys; print(json.load(sys.stdin).get("mode","?"))' 2>/dev/null)"
  if [[ "$RESTART" -eq 1 ]]; then
    echo ">> --restart: stopping existing backend on :${HIL_PORT}"
    pkill -f "target/release/hil-simulator" 2>/dev/null || true
    sleep 1
  elif [[ "$GW_SERIAL" == "auto" && "$RUNNING_MODE" == "simulation" ]]; then
    echo ">> NOTE: a backend is already running in SIMULATION mode, so the flow will"
    echo "         run in simulation. To drive the boards, restart it in hardware mode:"
    echo "           ./scripts/provision_full_flow.sh --restart"
  fi
fi
if curl -sf "${BASE}/api/v1/status" >/dev/null 2>&1; then
  echo ">> reusing backend already running on :${HIL_PORT}"
else
  echo ">> starting hil-simulator backend on :${HIL_PORT} (gw_serial='${GW_SERIAL:-<sim>}')"
  ( cd "$ROOT" && RUST_LOG="${RUST_LOG:-info}" HIL_PORT="$HIL_PORT" \
      HIL_GW_SERIAL="$GW_SERIAL" cargo run -p hil-simulator --release ) >"$LOG" 2>&1 &
  HIL_PID=$!
fi

# 2. Wait until the API answers -------------------------------------------
echo -n ">> waiting for API"
for _ in $(seq 1 90); do
  if curl -sf "${BASE}/api/v1/gateway" >/dev/null 2>&1; then echo " — ready"; break; fi
  if [[ -n "$HIL_PID" ]] && ! kill -0 "$HIL_PID" 2>/dev/null; then
    echo " — backend exited early; log:"; tail -20 "$LOG"; exit 1
  fi
  echo -n "."; sleep 1
done
if ! curl -sf "${BASE}/api/v1/gateway" >/dev/null 2>&1; then
  echo " — API never came up; log:"; tail -20 "$LOG"; exit 1
fi

MODE="$(curl -s "${BASE}/api/v1/gateway/status" | python3 -c 'import json,sys; print(json.load(sys.stdin).get("mode","?"))')"
echo ">> backend gateway mode: ${MODE}"
[[ "$MODE" == "simulation" && "$GW_SERIAL" == "auto" ]] && \
  echo ">> (no S3 board detected — running the flow in simulation; flash the boards for hardware mode)"

# 3. Run the provisioning self-check through the API ----------------------
"$ROOT/scripts/provision_demo.sh" --base "$BASE" ${DEVICE_ARGS[@]+"${DEVICE_ARGS[@]}"}
RESULT=$?

# 4. Optionally hand off to the dashboard ---------------------------------
if [[ "$WANT_DASHBOARD" -eq 1 && "$RESULT" -eq 0 ]]; then
  echo ">> launching dashboard; open http://localhost:3001/gateway  (Ctrl+C to quit)"
  trap - EXIT INT TERM          # keep the backend alive under the dashboard
  cd "$ROOT/web/hil-dashboard"
  export NEXT_PUBLIC_HIL_WS_URL="ws://127.0.0.1:${HIL_PORT}/ws/live"
  export HIL_API_URL="${BASE}"
  [[ -d node_modules ]] || npm install
  exec npm run dev
fi

exit "$RESULT"
