#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
ENDPOINT="${ZMQ_ENDPOINT:-tcp://127.0.0.1:5556}"

echo "GNU Radio sim container is optional. Running protocol injector..."
python3 "$ROOT/dsp-core/scripts/inject_zmq.py" --endpoint "$ENDPOINT" "$@"
