#!/usr/bin/env python3
"""Inject TelemetryFrame COBS wire bytes into ZMQ (simulates edge-gateway)."""

from __future__ import annotations

import argparse
import struct
import sys
import time

try:
    import zmq
except ImportError:
    print("Install pyzmq: pip install pyzmq", file=sys.stderr)
    sys.exit(1)


def crc16_xmodem(data: bytes) -> int:
    crc = 0
    for byte in data:
        crc ^= byte << 8
        for _ in range(8):
            if crc & 0x8000:
                crc = ((crc << 1) ^ 0x1021) & 0xFFFF
            else:
                crc = (crc << 1) & 0xFFFF
    return crc


def cobs_encode(data: bytes) -> bytes:
    out = bytearray()
    code_ptr = 0
    out.append(0)
    code = 1
    for b in data:
        if b == 0:
            out[code_ptr] = code
            code_ptr = len(out)
            out.append(0)
            code = 1
        else:
            out.append(b)
            code += 1
            if code == 0xFF:
                out[code_ptr] = code
                code_ptr = len(out)
                out.append(0)
                code = 1
    out[code_ptr] = code
    return bytes(out)


def encode_bool_cmd(seq: int, node_id: int, value: bool, timestamp_ms: int) -> bytes:
    # Postcard: seq u32, timestamp u64, node_id u8, variant index + bool
    payload = struct.pack("<IQB", seq, timestamp_ms, node_id)
    variant = 0 if value else 1  # BoolCmd(true)=0, BoolCmd(false)=1 in enum order
    payload += bytes([variant])
    if not value:
        payload += bytes([0])  # false bool byte in postcard

    crc = crc16_xmodem(payload)
    wire = payload + struct.pack("<H", crc)
    return cobs_encode(wire)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--endpoint", default="tcp://127.0.0.1:5556")
    parser.add_argument("--node-id", type=int, default=1)
    parser.add_argument("--seq-start", type=int, default=1)
    parser.add_argument("--count", type=int, default=3)
    parser.add_argument("--value", choices=("true", "false"), default="true")
    parser.add_argument("--replay-last", action="store_true", help="Resend final seq twice")
    parser.add_argument("--sleep-ms", type=int, default=50)
    args = parser.parse_args()

    ctx = zmq.Context()
    sock = ctx.socket(zmq.PUB)
    sock.connect(args.endpoint)
    time.sleep(0.2)

    value = args.value == "true"
    seq = args.seq_start
    frames = []
    for _ in range(args.count):
        frame = encode_bool_cmd(seq, args.node_id, value, int(time.time() * 1000))
        frames.append(frame)
        seq += 1

    if args.replay_last and frames:
        frames.append(frames[-1])

    for frame in frames:
        sock.send(frame)
        time.sleep(args.sleep_ms / 1000.0)

    sock.close()
    ctx.term()
    print(f"Sent {len(frames)} frame(s) to {args.endpoint}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
