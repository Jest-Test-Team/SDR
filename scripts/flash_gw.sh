#!/usr/bin/env bash
set -euo pipefail

PORT="${1:-/dev/ttyUSB1}"
BAUD="${2:-921600}"

echo "Building esp32s3-gateway..."
cargo build --release -p esp32s3-gateway

echo "Flashing to $PORT at $BAUD baud..."
espflash flash --port "$PORT" --baud "$BAUD" target/xtensa-esp32s3-none-elf/release/esp32s3-gateway

if [[ "${3:-}" == "--monitor" ]]; then
    echo "Starting monitor..."
    espflash monitor --port "$PORT" --baud "$BAUD"
fi