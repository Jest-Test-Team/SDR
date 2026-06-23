#!/usr/bin/env bash
set -euo pipefail

GW_PORT="${GW_PORT:-/dev/ttyUSB1}"
GW_BAUD="${GW_BAUD:-921600}"
ZMQ_ENDPOINT="${ZMQ_ENDPOINT:-tcp://127.0.0.1:5556}"
HEALTH_PORT="${HEALTH_PORT:-8080}"
METRICS_PORT="${METRICS_PORT:-9090}"

export RUST_LOG="${RUST_LOG:-info}"
export GW_PORT GW_BAUD ZMQ_ENDPOINT HEALTH_PORT METRICS_PORT

echo "Starting Edge Gateway (UART -> ZMQ)..."
echo "  UART: $GW_PORT @ $GW_BAUD"
echo "  ZMQ:  $ZMQ_ENDPOINT"
echo "  Health: :$HEALTH_PORT"
echo "  Metrics: :$METRICS_PORT"

cargo run -p edge-gateway --release &
EDGE_PID=$!

sleep 2

echo "Starting Control Plane (ZMQ -> Processing)..."
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