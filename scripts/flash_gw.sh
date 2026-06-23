#!/usr/bin/env bash
set -euo pipefail

PORT="${1:-/dev/ttyUSB1}"
BAUD="${2:-921600}"

echo "Building esp32s3-gateway..."
cargo +esp build --release -p esp32s3-gateway \
  --config 'build.target="xtensa-esp32s3-espidf"' \
  --config 'env.ESP_IDF_TOOLS_INSTALL_DIR="workspace"' \
  --config 'env.MCU="esp32s3"' \
  --config 'env.ESP_IDF_SYS_ROOT_CRATE="esp32s3-gateway"'

BIN="$(dirname "$0")/../target/xtensa-esp32s3-espidf/release/esp32s3-gateway"
echo "Flashing to $PORT at $BAUD baud..."
espflash flash --port "$PORT" --baud "$BAUD" "$BIN"

if [[ "${3:-}" == "--monitor" ]]; then
    echo "Starting monitor..."
    espflash monitor --port "$PORT" --baud "$BAUD"
fi