#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
export RUST_LOG="${RUST_LOG:-info}"
export HIL_PORT="${HIL_PORT:-8090}"
export ZMQ_ENDPOINT="${ZMQ_ENDPOINT:-tcp://127.0.0.1:5556}"

echo "Starting HIL simulator API on :$HIL_PORT"
cargo run -p hil-simulator --release &
HIL_PID=$!

cleanup() {
  kill $HIL_PID 2>/dev/null || true
}
trap cleanup EXIT INT TERM

if [[ "${1:-}" == "--api-only" ]]; then
  wait $HIL_PID
  exit 0
fi

echo "Starting Next.js dashboard on :3000"
cd "$ROOT/web/hil-dashboard"
export NEXT_PUBLIC_HIL_WS_URL="ws://127.0.0.1:${HIL_PORT}/ws/live"
npm install
npm run dev
