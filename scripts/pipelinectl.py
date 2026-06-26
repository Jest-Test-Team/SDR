#!/usr/bin/env python3
"""pipelinectl — a tiny *local, privileged* control service for the board pipelines.

It lets the dashboard's pipeline-switch button restart the backend into the other
pipeline (GATEWAY/provisioning vs LIVE TELEMETRY), which both contend for the one
ESP32-S3 USB port and so cannot run at once.

Security model (intentionally small + locked down):
  * binds 127.0.0.1 only (never exposed off-host);
  * every request needs `Authorization: Bearer <token>`; the token is generated at
    startup and written to a 0600 file the dashboard reads SERVER-SIDE;
  * the body's `pipeline` is matched against a fixed allowlist — no shell strings,
    no arbitrary commands, no user input reaches a shell.

Endpoints:
  GET  /status                      -> {"pipeline": "...", "hil_port": "..."}
  POST /switch  {"pipeline": "..."} -> stop current backend, start the requested one

Pipelines:
  gateway    hil-simulator HARDWARE mode (owns S3, ESP-NOW to ESP32 gateway)  -> /gateway
  telemetry  run_local.sh (edge-gateway + control-plane) + hil-simulator SIM  -> /
  sim        hil-simulator SIMULATION only (no boards)
  stop       stop everything

Usage:
  ./scripts/pipelinectl.py                 # start service, no pipeline running
  ./scripts/pipelinectl.py --start gateway # start service + bring up a pipeline
"""
import argparse
import json
import os
import secrets
import signal
import subprocess
import sys
import threading
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
HOST = "127.0.0.1"
PORT = int(os.environ.get("PIPELINECTL_PORT", "8099"))
HIL_PORT = os.environ.get("HIL_PORT", "8090")
TOKEN_FILE = Path(os.environ.get("PIPELINECTL_TOKEN_FILE", "/tmp/sdr-pipelinectl.token"))
TOKEN = secrets.token_hex(16)

PIPELINES = ("gateway", "telemetry", "sim", "stop")
RELEASE_BINS = (
    "target/release/hil-simulator",
    "target/release/edge-gateway",
    "target/release/control-plane",
)

_lock = threading.Lock()
_procs: list[subprocess.Popen] = []
_state = {"pipeline": "stopped"}


def log(*a):
    print("[pipelinectl]", *a, file=sys.stderr, flush=True)


def _pkill(pattern: str):
    subprocess.run(["pkill", "-f", pattern], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)


def _logfile(name: str):
    return open(f"/tmp/sdr-{name}.log", "ab")


def build():
    log("cargo build --release (incremental)…")
    subprocess.run(
        ["cargo", "build", "--release", "-p", "hil-simulator", "-p", "edge-gateway", "-p", "control-plane"],
        cwd=ROOT, check=False,
    )


def stop_all():
    with _lock:
        for p in _procs:
            try:
                p.terminate()
            except Exception:
                pass
        _procs.clear()
    # belt-and-braces: kill anything the launchers spawned
    _pkill("scripts/run_local.sh")
    for b in RELEASE_BINS:
        _pkill(b)
    _state["pipeline"] = "stopped"


def _spawn(name: str, cmd: list[str], env: dict | None = None):
    e = os.environ.copy()
    if env:
        e.update(env)
    p = subprocess.Popen(cmd, cwd=str(ROOT), env=e, stdout=_logfile(name), stderr=subprocess.STDOUT)
    _procs.append(p)
    log(f"started {name} (pid {p.pid})")


def start(pipeline: str):
    if pipeline not in PIPELINES:
        raise ValueError("unknown pipeline")
    stop_all()
    time.sleep(1)
    if pipeline == "stop":
        log("pipeline stopped")
        return
    build()
    hil = str(ROOT / "target/release/hil-simulator")
    if pipeline == "telemetry":
        _spawn("run-local", [str(ROOT / "scripts/run_local.sh")])
        _spawn("hil-simulator", [hil], env={"HIL_PORT": HIL_PORT})  # SIM (no HIL_GW_SERIAL)
    elif pipeline == "gateway":
        _spawn("hil-simulator", [hil], env={"HIL_PORT": HIL_PORT, "HIL_GW_SERIAL": "auto"})
    elif pipeline == "sim":
        _spawn("hil-simulator", [hil], env={"HIL_PORT": HIL_PORT})
    _state["pipeline"] = pipeline
    log(f"pipeline -> {pipeline}")


class Handler(BaseHTTPRequestHandler):
    def log_message(self, *a):  # quiet default logging
        pass

    def _authed(self) -> bool:
        return self.headers.get("Authorization", "") == f"Bearer {TOKEN}"

    def _json(self, code: int, body: dict):
        data = json.dumps(body).encode()
        self.send_response(code)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)

    def do_GET(self):
        if not self._authed():
            return self._json(401, {"error": "unauthorized"})
        if self.path == "/status":
            return self._json(200, {"pipeline": _state["pipeline"], "hil_port": HIL_PORT})
        return self._json(404, {"error": "not found"})

    def do_POST(self):
        if not self._authed():
            return self._json(401, {"error": "unauthorized"})
        if self.path != "/switch":
            return self._json(404, {"error": "not found"})
        length = int(self.headers.get("Content-Length", "0") or "0")
        try:
            body = json.loads(self.rfile.read(length) or b"{}")
        except Exception:
            return self._json(400, {"error": "bad json"})
        pipeline = body.get("pipeline")
        if pipeline not in PIPELINES:
            return self._json(400, {"error": f"pipeline must be one of {PIPELINES}"})
        # Run the switch in a thread so the request returns promptly.
        threading.Thread(target=start, args=(pipeline,), daemon=True).start()
        return self._json(202, {"accepted": True, "pipeline": pipeline})


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--start", choices=PIPELINES, help="bring up a pipeline on launch")
    ap.add_argument("--port", type=int, default=PORT)
    args = ap.parse_args()

    TOKEN_FILE.write_text(TOKEN)
    os.chmod(TOKEN_FILE, 0o600)
    log(f"token written to {TOKEN_FILE} (0600)")

    def shutdown(*_):
        log("shutting down; stopping pipelines")
        stop_all()
        try:
            TOKEN_FILE.unlink()
        except Exception:
            pass
        os._exit(0)

    signal.signal(signal.SIGINT, shutdown)
    signal.signal(signal.SIGTERM, shutdown)

    if args.start:
        threading.Thread(target=start, args=(args.start,), daemon=True).start()

    httpd = ThreadingHTTPServer((HOST, args.port), Handler)
    log(f"listening on http://{HOST}:{args.port}  (pipelines: {', '.join(PIPELINES)})")
    httpd.serve_forever()


if __name__ == "__main__":
    main()
