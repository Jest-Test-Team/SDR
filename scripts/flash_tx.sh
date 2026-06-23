#!/usr/bin/env bash
set -euo pipefail

PORT="${1:-/dev/ttyUSB0}"
BAUD="${2:-460800}"

echo "Building esp32-tx-node..."
cargo +esp build --release -p esp32-tx-node \
  --config 'build.target="xtensa-esp32-espidf"' \
  --config 'env.ESP_IDF_TOOLS_INSTALL_DIR="workspace"' \
  --config 'env.MCU="esp32"' \
  --config 'env.ESP_IDF_SYS_ROOT_CRATE="esp32-tx-node"'

BIN="$(dirname "$0")/../target/xtensa-esp32-espidf/release/esp32-tx-node"
echo "Flashing to $PORT at $BAUD baud..."
espflash flash --port "$PORT" --baud "$BAUD" "$BIN"

if [[ "${3:-}" == "--monitor" ]]; then
    echo "Starting monitor..."
    espflash monitor --port "$PORT" --baud "$BAUD"
fi