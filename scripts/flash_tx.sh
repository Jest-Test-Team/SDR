#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
# shellcheck source=flash_helpers.sh
source "${ROOT}/scripts/flash_helpers.sh"

if [[ "$(uname -s)" == "Darwin" && "${1:-}" == "" ]]; then
  PORT="$(detect_tx_port || true)"
else
  PORT="${1:-/dev/ttyUSB0}"
fi
BAUD="${2:-460800}"

preflight_flash_port "${PORT}" "TX node" || exit 1

export PATH="${HOME}/.cargo/bin:${PATH}"
# shellcheck source=/dev/null
source "${HOME}/export-esp.sh"
export RUST_MIN_STACK=16777216

cd "${ROOT}"

export GATEWAY_MAC="${GATEWAY_MAC:-FF:FF:FF:FF:FF:FF}"
export NODE_ID="${NODE_ID:-1}"
if [[ -n "${TX_POWER_DBM:-}" ]]; then
    export TX_POWER_DBM
fi

echo "Building esp32-tx-node..."
echo "  GATEWAY_MAC=${GATEWAY_MAC}  NODE_ID=${NODE_ID}  TX_POWER_DBM=${TX_POWER_DBM:-default}"
cargo +esp build --release -p esp32-tx-node \
  --config 'build.target="xtensa-esp32-espidf"' \
  --config 'unstable.build-std=["std","panic_abort"]' \
  --config 'target."cfg(target_os = \"espidf\")".linker="ldproxy"' \
  --config 'target."cfg(target_os = \"espidf\")".rustflags=["--cfg","espidf_time64"]' \
  --config 'env.ESP_IDF_TOOLS_INSTALL_DIR="workspace"' \
  --config 'env.MCU="esp32"' \
  --config 'env.ESP_IDF_SYS_ROOT_CRATE="esp32-tx-node"'

BIN="${ROOT}/target/xtensa-esp32-espidf/release/esp32-tx-node"
preflight_flash_port "${PORT}" "TX node" || exit 1
if [[ "${3:-}" == "--monitor" ]]; then
    espflash flash --port "${PORT}" --baud "${BAUD}" --monitor "${BIN}"
else
    espflash flash --port "${PORT}" --baud "${BAUD}" "${BIN}"
fi
