#!/usr/bin/env bash
#
# up.sh — ONE command to build (only if needed) and bring up the whole stack
# with the dev boards in the loop.
#
# Brings up, in order:
#   1. control-plane           (telemetry sink / secure ingest)        :8092 health
#   2. hil-simulator backend   (gateway + provisioning, HARDWARE mode) :8090
#         └─ drives the ESP32-S3 over USB ─ESP-NOW─ ESP32 gateway  ← the boards' pipeline
#   3. hil-dashboard frontend  (Next.js, REAL HARDWARE badge)          :3001
#
# If no S3 board is found, the backend falls back to SIMULATION automatically,
# so this always comes up. Ctrl+C tears everything down.
#
# Usage:
#   ./scripts/up.sh                 # build-if-needed, hardware auto-detect, up
#   ./scripts/up.sh --sim           # force simulation (no boards)
#   ./scripts/up.sh --flash         # flash both boards first, then up
#   ./scripts/up.sh --rebuild       # force a clean FE/BE rebuild
#   ./scripts/up.sh --no-cp         # skip control-plane (gateway/dashboard only)
#   ./scripts/up.sh --docker        # build+run FE/BE as Docker images (board-less)
#
# Board flashing / hardware mode needs the ESP toolchain (source ~/export-esp.sh)
# and nothing else holding the S3 usbmodem port (close espflash monitors first).
#
set -uo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

HIL_PORT="${HIL_PORT:-8090}"
DASH_PORT="${DASHBOARD_PORT:-3001}"
CP_HEALTH="${CP_HEALTH_PORT:-8092}"
GW_SERIAL="auto"
DO_FLASH=0
REBUILD=0
DOCKER=0
WANT_CP=1

while [[ $# -gt 0 ]]; do
  case "$1" in
    --sim)     GW_SERIAL=""; shift ;;
    --flash)   DO_FLASH=1; shift ;;
    --rebuild) REBUILD=1; shift ;;
    --docker)  DOCKER=1; shift ;;
    --no-cp)   WANT_CP=0; shift ;;
    -h|--help) sed -n '2,26p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) echo "unknown arg: $1" >&2; exit 2 ;;
  esac
done

note() { printf '\n\033[1m>> %s\033[0m\n' "$*"; }

# ---------------------------------------------------------------------------
# Docker path: rebuild FE/BE images (layer cache => "only if needed") and run.
# No board access (Docker Desktop can't see /dev/cu.usbmodem*), so this is the
# board-less / simulation deployment.
# ---------------------------------------------------------------------------
if [[ "$DOCKER" -eq 1 ]]; then
  note "Docker: building images (FE/BE)${REBUILD:+ --no-cache}"
  BUILD_ARGS=(); [[ "$REBUILD" -eq 1 ]] && BUILD_ARGS=(--no-cache)
  docker compose build ${BUILD_ARGS[@]+"${BUILD_ARGS[@]}"} control-plane hil-simulator hil-dashboard
  note "Docker: starting stack (Ctrl+C to stop). Dashboard: http://localhost:3001"
  exec docker compose up control-plane hil-simulator hil-dashboard
fi

# ---------------------------------------------------------------------------
# Native path (default): boards in the loop.
# ---------------------------------------------------------------------------
PIDS=()
cleanup() {
  note "shutting down"
  for p in ${PIDS[@]+"${PIDS[@]}"}; do kill "$p" 2>/dev/null || true; done
}
trap cleanup EXIT INT TERM

# 0. Optional: flash both boards ------------------------------------------
if [[ "$DO_FLASH" -eq 1 ]]; then
  note "flashing ESP32 gateway (esp32-tx-node)"
  ./scripts/flash_tx.sh "" 460800
  note "flashing ESP32-S3 sim node (esp32s3-gateway)"
  ./scripts/flash_gw.sh
fi

# 1. Build Rust (cargo is incremental: only rebuilds what changed) --------
note "building backend binaries (release)"
BUILD_PKGS=(-p hil-simulator)
[[ "$WANT_CP" -eq 1 ]] && BUILD_PKGS+=(-p control-plane)
cargo build --release "${BUILD_PKGS[@]}"

# 2. Prepare the frontend --------------------------------------------------
note "preparing dashboard frontend"
( cd web/hil-dashboard
  [[ -d node_modules ]] || npm install
  if [[ "$REBUILD" -eq 1 ]]; then rm -rf .next; npm run build; fi
)

# 3. Start control-plane ---------------------------------------------------
if [[ "$WANT_CP" -eq 1 ]]; then
  note "starting control-plane (health :$CP_HEALTH)"
  RUST_LOG="${RUST_LOG:-info}" HEALTH_PORT="$CP_HEALTH" \
    ./target/release/control-plane >/tmp/sdr-control-plane.log 2>&1 &
  PIDS+=($!)
fi

# 4. Start hil-simulator backend (boards' pipeline) -----------------------
note "starting hil-simulator backend on :$HIL_PORT (gw_serial='${GW_SERIAL:-<sim>}')"
RUST_LOG="${RUST_LOG:-info}" HIL_PORT="$HIL_PORT" HIL_GW_SERIAL="$GW_SERIAL" \
  ./target/release/hil-simulator >/tmp/sdr-hil-simulator.log 2>&1 &
PIDS+=($!)

# wait for the API, then report which gateway mode is live
printf '   waiting for HIL API'
for _ in $(seq 1 90); do
  curl -sf "http://127.0.0.1:${HIL_PORT}/api/v1/gateway" >/dev/null 2>&1 && { printf ' — ready\n'; break; }
  printf '.'; sleep 1
done
MODE="$(curl -s "http://127.0.0.1:${HIL_PORT}/api/v1/gateway/status" | python3 -c 'import json,sys; print(json.load(sys.stdin).get("mode","?"))' 2>/dev/null || echo '?')"
note "gateway backend mode: ${MODE}"
[[ "$MODE" == "simulation" && "$GW_SERIAL" == "auto" ]] && \
  echo "   (no S3 board detected — running board-less; flash with --flash or check the USB cable)"

# 5. Start the dashboard in the foreground (Ctrl+C stops the whole stack) --
note "starting dashboard on :$DASH_PORT  →  http://localhost:${DASH_PORT}/gateway"
cd web/hil-dashboard
export NEXT_PUBLIC_HIL_WS_URL="ws://127.0.0.1:${HIL_PORT}/ws/live"
export HIL_API_URL="http://127.0.0.1:${HIL_PORT}"
export PORT="$DASH_PORT"
if [[ "$REBUILD" -eq 1 && -d .next ]]; then
  npm run start -- -p "$DASH_PORT"   # production server (next start)
else
  npm run dev                        # dev server; dev.mjs honors $PORT
fi
