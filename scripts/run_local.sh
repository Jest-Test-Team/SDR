#!/usr/bin/env bash
set -euo pipefail

GW_PORT="${GW_PORT:-/dev/ttyUSB1}"
GW_BAUD="${GW_BAUD:-921600}"
ZMQ_ENDPOINT="${ZMQ_ENDPOINT:-tcp://127.0.0.1:5556}"
EDGE_HEALTH_PORT="${EDGE_HEALTH_PORT:-8081}"
CP_HEALTH_PORT="${CP_HEALTH_PORT:-8080}"

export RUST_LOG="${RUST_LOG:-info}"
export GW_PORT GW_BAUD ZMQ_ENDPOINT HEALTH_PORT="$EDGE_HEALTH_PORT"

echo "Starting Edge Gateway (UART -> ZMQ)..."
echo "  UART: $GW_PORT @ $GW_BAUD"
echo "  ZMQ:  $ZMQ_ENDPOINT"
echo "  Health: :$EDGE_HEALTH_PORT"

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

echo "Both services running. Press Ctrl+C to stop."
wait