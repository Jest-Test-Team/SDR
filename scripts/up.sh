#!/usr/bin/env bash
#
# up.sh — ONE command to build (only if needed) and bring up the stack.
#
# There are TWO board pipelines and they CANNOT run at once, because both need
# the single ESP32-S3 USB port. Pick one:
#
#   (default) GATEWAY / provisioning pipeline   →  page:  http://localhost:3001/gateway
#       hil-simulator (HARDWARE mode) owns the S3 ─ESP-NOW─ ESP32 gateway.
#       Drives Secure-Gateway commands + device provisioning. The main "/" live
#       telemetry page will read as offline in this mode — that's expected.
#
#   --telemetry  LIVE TELEMETRY pipeline         →  page:  http://localhost:3001
#       edge-gateway reads the S3 over USB → ZMQ → control-plane → main page.
#       hil-simulator runs in SIMULATION (so /gateway still loads, badged SIM).
#       NOTE: needs the S3 flashed as the telemetry bridge + a TX node sending
#       frames — a DIFFERENT firmware build than the Secure-Gateway topology.
#
# Common flags:
#   --control    run the pipelinectl supervisor so the dashboard ‘⇄’ button can
#                restart the backend into the other pipeline live (no terminal)
#   --sim        force hil-simulator simulation (gateway mode, no boards)
#   --flash      flash both boards (Secure-Gateway topology) before up
#   --rebuild    force a clean FE/BE rebuild
#   --docker     build+run FE/BE as Docker images (board-less)
#
# Ctrl+C tears everything down. Hardware needs the ESP toolchain
# (source ~/export-esp.sh) and a free S3 usbmodem port (close espflash monitors).
#
set -uo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

HIL_PORT="${HIL_PORT:-8090}"
DASH_PORT="${DASHBOARD_PORT:-3001}"
PIPELINE="gateway"
GW_SERIAL="auto"
DO_FLASH=0
REBUILD=0
DOCKER=0
CONTROL=0

while [[ $# -gt 0 ]]; do
  case "$1" in
    --telemetry) PIPELINE="telemetry"; shift ;;
    --gateway)   PIPELINE="gateway"; shift ;;
    --sim)       GW_SERIAL=""; shift ;;
    --flash)     DO_FLASH=1; shift ;;
    --rebuild)   REBUILD=1; shift ;;
    --docker)    DOCKER=1; shift ;;
    --control)   CONTROL=1; shift ;;
    -h|--help)   sed -n '2,36p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) echo "unknown arg: $1" >&2; exit 2 ;;
  esac
done

note()  { printf '\n\033[1m>> %s\033[0m\n' "$*"; }
banner(){ printf '\n\033[1;36m%s\033[0m\n' "$*"; }

# --- Docker path (board-less) ----------------------------------------------
if [[ "$DOCKER" -eq 1 ]]; then
  note "Docker: building FE/BE images${REBUILD:+ (no-cache)}"
  BUILD_ARGS=(); [[ "$REBUILD" -eq 1 ]] && BUILD_ARGS=(--no-cache)
  docker compose build ${BUILD_ARGS[@]+"${BUILD_ARGS[@]}"} control-plane hil-simulator hil-dashboard
  note "Docker: up (Ctrl+C to stop). Dashboard: http://localhost:3001"
  exec docker compose up control-plane hil-simulator hil-dashboard
fi

PIDS=()
cleanup() {
  note "shutting down"
  for p in ${PIDS[@]+"${PIDS[@]}"}; do kill "$p" 2>/dev/null || true; done
  # run_local.sh / pipelinectl start their own children; make sure they go too
  pkill -f 'scripts/pipelinectl.py' 2>/dev/null || true
  pkill -f 'target/release/edge-gateway' 2>/dev/null || true
  pkill -f 'target/release/control-plane' 2>/dev/null || true
  pkill -f 'target/release/hil-simulator' 2>/dev/null || true
}
trap cleanup EXIT INT TERM

# --- Optional flashing (Secure-Gateway topology) ---------------------------
if [[ "$DO_FLASH" -eq 1 ]]; then
  if [[ "$PIPELINE" == "telemetry" ]]; then
    echo "WARN: --flash flashes the Secure-Gateway topology, not the telemetry" >&2
    echo "      bridge firmware the --telemetry pipeline expects. Skipping flash." >&2
  else
    note "flashing ESP32 gateway (esp32-tx-node)"; ./scripts/flash_tx.sh "" 460800
    note "flashing ESP32-S3 sim node (esp32s3-gateway)"; ./scripts/flash_gw.sh
  fi
fi

# --- Frontend prep (shared) ------------------------------------------------
note "preparing dashboard frontend"
( cd web/hil-dashboard
  [[ -d node_modules ]] || npm install
  if [[ "$REBUILD" -eq 1 ]]; then rm -rf .next; npm run build; fi
)

wait_api() {
  printf '   waiting for HIL API'
  for _ in $(seq 1 90); do
    curl -sf "http://127.0.0.1:${HIL_PORT}/api/v1/gateway" >/dev/null 2>&1 && { printf ' — ready\n'; return 0; }
    printf '.'; sleep 1
  done
  printf ' — timeout\n'
}

wait_pipelinectl() {
  local pid="$1"
  local ctl_port="${PIPELINECTL_PORT:-8099}"
  local token_file="${PIPELINECTL_TOKEN_FILE:-/tmp/sdr-pipelinectl.token}"
  printf '   waiting for pipelinectl'
  for _ in $(seq 1 30); do
    if ! kill -0 "$pid" 2>/dev/null; then
      printf ' — failed\n'
      sed -n '1,80p' /tmp/sdr-pipelinectl.log >&2 2>/dev/null || true
      return 1
    fi
    if [[ -s "$token_file" ]]; then
      local token
      token="$(cat "$token_file" 2>/dev/null || true)"
      if curl -sf -H "Authorization: Bearer ${token}" \
        "http://127.0.0.1:${ctl_port}/status" >/dev/null 2>&1; then
        printf ' — ready\n'
        return 0
      fi
    fi
    printf '.'; sleep 1
  done
  printf ' — timeout\n'
  sed -n '1,80p' /tmp/sdr-pipelinectl.log >&2 2>/dev/null || true
  return 1
}

start_dashboard() {
  cd "$ROOT/web/hil-dashboard"
  export NEXT_PUBLIC_HIL_WS_URL="ws://127.0.0.1:${HIL_PORT}/ws/live"
  export HIL_API_URL="http://127.0.0.1:${HIL_PORT}"
  export LIVE_CP_URL="${LIVE_CP_URL:-http://127.0.0.1:8092}"
  export LIVE_EDGE_URL="${LIVE_EDGE_URL:-http://127.0.0.1:8081}"
  export PORT="$DASH_PORT"
  if [[ "$REBUILD" -eq 1 && -d .next ]]; then npm run start -- -p "$DASH_PORT"; else npm run dev; fi
}

if [[ "$CONTROL" -eq 1 ]]; then
  # ---- CONTROLLED: pipelinectl supervises the backend; button switches live --
  INIT_PIPE="$PIPELINE"; [[ -n "$GW_SERIAL" ]] || INIT_PIPE="sim"
  note "starting pipelinectl supervisor (initial pipeline: $INIT_PIPE)"
  ./scripts/pipelinectl.py --start "$INIT_PIPE" >/tmp/sdr-pipelinectl.log 2>&1 &
  ctl_pid=$!
  PIDS+=("$ctl_pid")
  wait_pipelinectl "$ctl_pid" || exit 1
  wait_api
  banner "CONTROLLED mode — the nav ‘⇄’ button now switches pipelines live"
  echo "   open http://localhost:${DASH_PORT}  (telemetry)  or  /gateway  (provisioning)"
  echo "   logs: /tmp/sdr-pipelinectl.log  /tmp/sdr-hil-simulator.log  /tmp/sdr-run-local.log"
  start_dashboard

elif [[ "$PIPELINE" == "telemetry" ]]; then
  # ---- LIVE TELEMETRY: edge-gateway + control-plane + hil-simulator(SIM) ----
  note "building binaries (release)"
  cargo build --release -p edge-gateway -p control-plane -p hil-simulator

  note "starting live pipeline (edge-gateway + control-plane) via run_local.sh"
  ./scripts/run_local.sh >/tmp/sdr-run-local.log 2>&1 &
  PIDS+=($!)

  note "starting hil-simulator in SIMULATION (S3 is owned by edge-gateway) on :$HIL_PORT"
  RUST_LOG="${RUST_LOG:-info}" HIL_PORT="$HIL_PORT" \
    ./target/release/hil-simulator >/tmp/sdr-hil-simulator.log 2>&1 &
  PIDS+=($!)
  wait_api

  banner "TELEMETRY mode — open  http://localhost:${DASH_PORT}  (the main live page)"
  echo "   TX ESP32: short-press BOOT (GPIO0) near the gateway → expect ACTION_TRIGGERED."
  echo "   /gateway will show SIMULATION MODE in this pipeline."
  echo "   logs: /tmp/sdr-run-local.log  /tmp/sdr-hil-simulator.log"
  start_dashboard

else
  # ---- GATEWAY / PROVISIONING: hil-simulator(HARDWARE) ----------------------
  note "building hil-simulator (release)"
  cargo build --release -p hil-simulator

  note "starting hil-simulator on :$HIL_PORT (gw_serial='${GW_SERIAL:-<sim>}')"
  RUST_LOG="${RUST_LOG:-info}" HIL_PORT="$HIL_PORT" HIL_GW_SERIAL="$GW_SERIAL" \
    ./target/release/hil-simulator >/tmp/sdr-hil-simulator.log 2>&1 &
  PIDS+=($!)
  wait_api

  MODE="$(curl -s "http://127.0.0.1:${HIL_PORT}/api/v1/gateway/status" | python3 -c 'import json,sys; print(json.load(sys.stdin).get("mode","?"))' 2>/dev/null || echo '?')"
  banner "GATEWAY mode (${MODE}) — open  http://localhost:${DASH_PORT}/gateway"
  [[ "$MODE" == "simulation" && "$GW_SERIAL" == "auto" ]] && \
    echo "   (no S3 board detected — board-less; --flash or check the USB cable)"
  echo "   The main '/' live-telemetry page is OFFLINE by design here."
  echo "   For that page instead, run:  ./scripts/up.sh --telemetry"
  echo "   logs: /tmp/sdr-hil-simulator.log"
  start_dashboard
fi
