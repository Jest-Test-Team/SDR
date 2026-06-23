#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
PORT="${1:-/dev/ttyUSB1}"
BAUD="${2:-921600}"

export PATH="${HOME}/.cargo/bin:${PATH}"
# shellcheck source=/dev/null
source "${HOME}/export-esp.sh"
export RUST_MIN_STACK=16777216

cd "${ROOT}"

echo "Building esp32s3-gateway..."
cargo +esp build --release -p esp32s3-gateway \
  --config 'build.target="xtensa-esp32s3-espidf"' \
  --config 'unstable.build-std=["std","panic_abort"]' \
  --config 'target."cfg(target_os = \"espidf\")".linker="ldproxy"' \
  --config 'target."cfg(target_os = \"espidf\")".rustflags=["--cfg","espidf_time64"]' \
  --config 'env.ESP_IDF_TOOLS_INSTALL_DIR="workspace"' \
  --config 'env.MCU="esp32s3"' \
  --config 'env.ESP_IDF_SYS_ROOT_CRATE="esp32s3-gateway"'

BIN="${ROOT}/target/xtensa-esp32s3-espidf/release/esp32s3-gateway"
echo "Flashing to ${PORT} at ${BAUD} baud..."
espflash flash --port "${PORT}" --baud "${BAUD}" "${BIN}"

if [[ "${3:-}" == "--monitor" ]]; then
    echo "Starting monitor..."
    espflash monitor --port "${PORT}" --baud "${BAUD}"
fi
