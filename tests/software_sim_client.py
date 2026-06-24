#!/usr/bin/env python3
import argparse
import json
import sys
import time
import urllib.error
import urllib.request


def request_json(method, url, body=None):
    data = None if body is None else json.dumps(body).encode("utf-8")
    req = urllib.request.Request(
        url,
        data=data,
        method=method,
        headers={"Content-Type": "application/json"},
    )
    with urllib.request.urlopen(req, timeout=2) as res:
        raw = res.read().decode("utf-8")
        return json.loads(raw) if raw else None


def wait_url(url, timeout):
    deadline = time.time() + timeout
    last_error = None
    while time.time() < deadline:
        try:
            with urllib.request.urlopen(url, timeout=2) as res:
                if 200 <= res.status < 300:
                    return
        except (OSError, urllib.error.URLError) as exc:
            last_error = exc
        time.sleep(0.25)
    raise SystemExit(f"timed out waiting for {url}: {last_error}")


def wait_status(url, min_frames, timeout):
    deadline = time.time() + timeout
    last = None
    while time.time() < deadline:
        try:
            last = request_json("GET", url)
            if last.get("frames_decoded", 0) >= min_frames:
                print(json.dumps(last))
                return
        except (OSError, urllib.error.URLError, ValueError):
            pass
        time.sleep(0.25)
    raise SystemExit(f"status did not reach {min_frames} frames: {last}")


def wait_event(url, contains, timeout):
    deadline = time.time() + timeout
    last = None
    while time.time() < deadline:
        try:
            last = request_json("GET", url)
            text = json.dumps(last)
            if contains in text:
                print(text)
                return
        except (OSError, urllib.error.URLError, ValueError):
            pass
        time.sleep(0.25)
    raise SystemExit(f"event stream did not contain {contains!r}: {last}")


def main():
    parser = argparse.ArgumentParser()
    sub = parser.add_subparsers(dest="cmd", required=True)
    wait = sub.add_parser("wait-url")
    wait.add_argument("url")
    wait.add_argument("--timeout", type=float, default=15)
    getj = sub.add_parser("get-json")
    getj.add_argument("url")
    put = sub.add_parser("put-json")
    put.add_argument("url")
    put.add_argument("json_body")
    post = sub.add_parser("post-json")
    post.add_argument("url")
    post.add_argument("json_body")
    status = sub.add_parser("wait-status")
    status.add_argument("url")
    status.add_argument("--min-frames", type=int, default=1)
    status.add_argument("--timeout", type=float, default=15)
    event = sub.add_parser("wait-event")
    event.add_argument("url")
    event.add_argument("contains")
    event.add_argument("--timeout", type=float, default=15)
    args = parser.parse_args()

    if args.cmd == "wait-url":
        wait_url(args.url, args.timeout)
    elif args.cmd == "get-json":
        print(json.dumps(request_json("GET", args.url)))
    elif args.cmd == "put-json":
        print(json.dumps(request_json("PUT", args.url, json.loads(args.json_body))))
    elif args.cmd == "post-json":
        print(json.dumps(request_json("POST", args.url, json.loads(args.json_body))))
    elif args.cmd == "wait-status":
        wait_status(args.url, args.min_frames, args.timeout)
    elif args.cmd == "wait-event":
        wait_event(args.url, args.contains, args.timeout)


if __name__ == "__main__":
    try:
        main()
    except Exception as exc:
        print(exc, file=sys.stderr)
        raise
