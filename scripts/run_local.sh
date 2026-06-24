#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
# shellcheck source=flash_helpers.sh
source "${ROOT}/scripts/flash_helpers.sh"

# Gateway (ESP32-S3) = /dev/cu.usbmodem* — NOT the TX usbserial port
if [[ "$(uname -s)" == "Darwin" && "${GW_PORT:-}" == "" ]]; then
  GW_PORT="$(detect_gateway_port || true)"
fi
GW_PORT="${GW_PORT:-/dev/ttyUSB1}"
GW_BAUD="${GW_BAUD:-921600}"
# macOS USB CDC (/dev/cu.usbmodem*) is more stable at 115200 on the host side
if [[ "$(uname -s)" == "Darwin" && "${GW_PORT}" == *usbmodem* && "${GW_BAUD_SET:-}" == "" ]]; then
  GW_BAUD=115200
fi
ZMQ_ENDPOINT="${ZMQ_ENDPOINT:-tcp://127.0.0.1:5556}"
EDGE_HEALTH_PORT="${EDGE_HEALTH_PORT:-8081}"
# Docker Desktop often binds :8080 on macOS
CP_HEALTH_PORT="${CP_HEALTH_PORT:-8092}"

if pgrep -f 'target/release/edge-gateway' >/dev/null 2>&1 \
  || pgrep -f 'target/release/control-plane' >/dev/null 2>&1; then
  echo "ERROR: edge-gateway or control-plane already running." >&2
  echo "Stop them first: pkill -f 'target/release/edge-gateway|target/release/control-plane'" >&2
  exit 1
fi

preflight_pipeline_port "${GW_PORT}" || exit 1

export RUST_LOG="${RUST_LOG:-info}"
export GW_PORT GW_BAUD ZMQ_ENDPOINT HEALTH_PORT="$EDGE_HEALTH_PORT"

echo "Starting Edge Gateway (UART -> ZMQ)..."
echo "  UART: $GW_PORT @ $GW_BAUD  (Gateway ESP32-S3)"
echo "  ZMQ:  $ZMQ_ENDPOINT"
echo "  Health: :$EDGE_HEALTH_PORT"
if [[ "${GW_PORT}" == *usbmodem* ]]; then
  echo "  (waiting 3s for USB serial to settle...)"
  sleep 3
fi

cargo run -p edge-gateway --release &
EDGE_PID=$!

sleep 2

export HEALTH_PORT="$CP_HEALTH_PORT"
echo "Starting Control Plane (ZMQ -> Processing)..."
echo "  ZMQ:  $ZMQ_ENDPOINT"
echo "  Health: :$CP_HEALTH_PORT"
cargo run -p control-plane --release &
CP_PID=$!

cleanup() {
    echo "Shutting down..."
    kill $EDGE_PID $CP_PID 2>/dev/null || true
    wait $EDGE_PID $CP_PID 2>/dev/null || true
}
trap cleanup EXIT INT TERM

echo ""
echo "Both services running. Press Ctrl+C to stop."
echo "TX node: short-press BOOT (GPIO0) while near Gateway — expect ACTION_TRIGGERED in logs."
wait
