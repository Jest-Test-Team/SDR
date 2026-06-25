#!/usr/bin/env python3
"""Drive the ESP32-S3 software-sim node over USB and print the gateway replies.

Topology:
    Mac --USB(this script)-- ESP32-S3 [sim node] --ESP-NOW-- ESP32 [gateway]

The S3 firmware accepts comma-separated command lines and prints `GWRESP ...`
or `SIMRECV ...` lines in response. This is a stdlib-only client (no pyserial):
it configures the tty with `stty` and does raw read/write.

Examples:
    ./scripts/sim_node.py GW,HEALTH
    ./scripts/sim_node.py GW,TOGGLE
    ./scripts/sim_node.py GW,SNMP_SET,1.3.6.1.4.1.custom.relay,on
    ./scripts/sim_node.py GW,SNMP_GET,1.3.6.1.4.1.custom.relay
    ./scripts/sim_node.py SIM,SEND,1

Note: stop the live pipeline first if it holds the port
      (pkill -f 'target/release/edge-gateway|target/release/control-plane').
"""
import argparse
import glob
import os
import select
import subprocess
import sys
import time


def detect_port():
    ports = sorted(glob.glob("/dev/cu.usbmodem*"))
    if not ports:
        raise SystemExit(
            "no /dev/cu.usbmodem* found — plug in the ESP32-S3 (data cable)."
        )
    return ports[0]


def configure_tty(port, baud):
    # macOS uses `stty -f`, Linux uses `stty -F`.
    flag = "-f" if sys.platform == "darwin" else "-F"
    subprocess.run(
        ["stty", flag, port, str(baud), "raw", "-echo", "cs8", "-ixon"],
        check=True,
    )


def main():
    parser = argparse.ArgumentParser(description=__doc__,
                                     formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("command", help="command line to send, e.g. GW,HEALTH")
    parser.add_argument("--port", default=None, help="serial port (default: auto-detect)")
    parser.add_argument("--baud", type=int, default=115200)
    parser.add_argument("--timeout", type=float, default=3.0,
                        help="seconds to wait for replies")
    args = parser.parse_args()

    port = args.port or detect_port()
    configure_tty(port, args.baud)

    fd = os.open(port, os.O_RDWR | os.O_NOCTTY | os.O_NONBLOCK)
    try:
        os.write(fd, (args.command + "\n").encode("ascii"))
        deadline = time.time() + args.timeout
        buf = b""
        got_reply = False
        while time.time() < deadline:
            r, _, _ = select.select([fd], [], [], 0.2)
            if fd in r:
                try:
                    chunk = os.read(fd, 256)
                except OSError:
                    continue
                if not chunk:
                    continue
                buf += chunk
                while b"\n" in buf:
                    line, buf = buf.split(b"\n", 1)
                    text = line.decode("ascii", "replace").strip()
                    if not text:
                        continue
                    print(text)
                    if text.startswith(("GWRESP", "SIMRECV", "SIMSENT", "ERR")):
                        got_reply = True
                        if text.startswith(("GWRESP", "ERR")):
                            return 0 if text.startswith("GWRESP") else 2
        if not got_reply:
            print(f"(no reply within {args.timeout}s — is the gateway ESP32 powered?)",
                  file=sys.stderr)
            return 1
        return 0
    finally:
        os.close(fd)


if __name__ == "__main__":
    raise SystemExit(main())
