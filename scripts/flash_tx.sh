#!/usr/bin/env bash
set -euo pipefail

PORT="${1:-/dev/ttyUSB0}"
BAUD="${2:-460800}"

echo "Building esp32-tx-node..."
cargo build --release -p esp32-tx-node

echo "Flashing to $PORT at $BAUD baud..."
espflash flash --port "$PORT" --baud "$BAUD" target/xtensa-esp32-none-elf/release/esp32-tx-node

if [[ "${3:-}" == "--monitor" ]]; then
    echo "Starting monitor..."
    espflash monitor --port "$PORT" --baud "$BAUD"
fi