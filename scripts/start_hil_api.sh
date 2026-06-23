#!/usr/bin/env bash
# Start only the HIL simulator API (port 8090). Keep this terminal open.
set -euo pipefail
cd "$(dirname "$0")/.."
export RUST_LOG="${RUST_LOG:-info}"
export HIL_PORT="${HIL_PORT:-8090}"
exec cargo run -p hil-simulator --release
