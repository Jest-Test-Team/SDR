#!/usr/bin/env bash
#
# End-to-end device-provisioning demo + self-check.
#
# Runs the full enroll -> claim -> rotate -> revoke lifecycle (plus two negative
# cases) and verifies each step's result.
#
# Two modes:
#   (default) API mode  -- drives the hil-simulator backend HTTP API, the SAME
#                          path the web dashboard uses. Works whether the backend
#                          is in REAL HARDWARE mode (drives the boards) or
#                          SIMULATION mode. Start the backend first, e.g.:
#                            HIL_GW_SERIAL=auto ./scripts/run_hil_dashboard.sh
#                          or just:  cargo run -p hil-simulator --release
#
#   --usb               -- drives the ESP32-S3 node directly over USB via
#                          scripts/sim_node.py (no backend/dashboard running).
#                          Only one process may own the S3 port at a time.
#
# Usage:
#   ./scripts/provision_demo.sh                 # API mode, default device id
#   ./scripts/provision_demo.sh --base http://localhost:8090
#   ./scripts/provision_demo.sh --device dev-007
#   ./scripts/provision_demo.sh --usb           # direct USB mode
#
set -uo pipefail

BASE="http://localhost:8090"
DEVICE="dev-$(printf '%03d' $((RANDOM % 1000)))"
MAC="AA:BB:CC:00:00:$(printf '%02X' $((RANDOM % 256)))"
MODE="api"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --base)   BASE="$2"; shift 2 ;;
    --device) DEVICE="$2"; shift 2 ;;
    --mac)    MAC="$2"; shift 2 ;;
    --usb)    MODE="usb"; shift ;;
    -h|--help) sed -n '2,30p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) echo "unknown arg: $1" >&2; exit 2 ;;
  esac
done

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PASS=0
FAIL=0

say()  { printf '\n\033[1m== %s ==\033[0m\n' "$*"; }
ok()   { printf '  \033[32mPASS\033[0m %s\n' "$*"; PASS=$((PASS+1)); }
bad()  { printf '  \033[31mFAIL\033[0m %s\n' "$*"; FAIL=$((FAIL+1)); }

# --- API mode helpers -------------------------------------------------------

# api_command '<json>'  -> echoes the raw GatewayResponse JSON on stdout
api_command() {
  curl -s -X POST "$BASE/api/v1/gateway/command" \
    -H 'Content-Type: application/json' -d "$1"
}

# api_device_field <device_id> <field>  -> reads field from /api/v1/gateway
api_device_field() {
  curl -s "$BASE/api/v1/gateway" | python3 -c '
import json,sys
dev_id, field = sys.argv[1], sys.argv[2]
snap = json.load(sys.stdin)
for d in snap.get("devices", []):
    if d.get("device_id") == dev_id:
        print(d.get(field, "")); break
else:
    print("<absent>")
' "$1" "$2"
}

# assert_state <label> <device_id> <expected_state> <expected_version>
assert_state() {
  local label="$1" dev="$2" want_state="$3" want_ver="$4"
  local state ver
  state="$(api_device_field "$dev" state)"
  ver="$(api_device_field "$dev" credential_version)"
  if [[ "$state" == "$want_state" && "$ver" == "$want_ver" ]]; then
    ok "$label: state=$state version=$ver"
  else
    bad "$label: got state=$state version=$ver, want state=$want_state version=$want_ver"
  fi
}

# --- USB mode helper --------------------------------------------------------

usb() {
  # echoes the GWRESP line; asserts on substrings
  "$SCRIPT_DIR/sim_node.py" "$1" 2>/dev/null | grep -m1 PROVISION || true
}

assert_usb() {
  local label="$1" line="$2" needle="$3"
  if [[ "$line" == *"$needle"* ]]; then
    ok "$label: $line"
  else
    bad "$label: '$line' did not contain '$needle'"
  fi
}

# ---------------------------------------------------------------------------

echo "Device: $DEVICE   MAC: $MAC   mode: $MODE   base: $BASE"

if [[ "$MODE" == "api" ]]; then
  if ! curl -s -o /dev/null "$BASE/api/v1/gateway"; then
    echo "ERROR: backend not reachable at $BASE — start hil-simulator first." >&2
    exit 1
  fi
  MODE_LABEL="$(curl -s "$BASE/api/v1/gateway/status" | python3 -c 'import json,sys; print(json.load(sys.stdin).get("mode","?"))')"
  echo "Backend gateway mode: $MODE_LABEL"

  say "enroll $DEVICE"
  api_command "{\"command\":\"enroll_device\",\"device_id\":\"$DEVICE\",\"mac\":\"$MAC\"}" >/dev/null
  assert_state "enroll" "$DEVICE" pending 1
  FP1="$(api_device_field "$DEVICE" credential_fingerprint)"

  say "claim $DEVICE"
  api_command "{\"command\":\"claim_device\",\"device_id\":\"$DEVICE\"}" >/dev/null
  assert_state "claim" "$DEVICE" active 1

  say "rotate $DEVICE"
  api_command "{\"command\":\"rotate_credential\",\"device_id\":\"$DEVICE\"}" >/dev/null
  assert_state "rotate" "$DEVICE" active 2
  FP2="$(api_device_field "$DEVICE" credential_fingerprint)"
  if [[ -n "$FP1" && "$FP1" != "$FP2" ]]; then ok "rotate changed fingerprint ($FP1 -> $FP2)"; else bad "fingerprint did not change ($FP1 -> $FP2)"; fi

  say "revoke $DEVICE"
  api_command "{\"command\":\"revoke_device\",\"device_id\":\"$DEVICE\"}" >/dev/null
  assert_state "revoke" "$DEVICE" revoked 2

  say "rotate after revoke (must be rejected)"
  api_command "{\"command\":\"rotate_credential\",\"device_id\":\"$DEVICE\"}" >/dev/null
  assert_state "rotate-after-revoke (unchanged)" "$DEVICE" revoked 2

  say "claim ghost (must be rejected / absent)"
  api_command "{\"command\":\"claim_device\",\"device_id\":\"ghost-$DEVICE\"}" >/dev/null
  GHOST="$(api_device_field "ghost-$DEVICE" state)"
  if [[ "$GHOST" == "<absent>" ]]; then ok "ghost not provisioned"; else bad "ghost unexpectedly present (state=$GHOST)"; fi

else
  echo "(USB mode: asserts on the boards' GWRESP PROVISION replies)"
  say "enroll $DEVICE";  assert_usb "enroll"  "$(usb "GW,ENROLL,$DEVICE,$MAC")" "state=pending version=1 ok=true"
  say "claim $DEVICE";   assert_usb "claim"   "$(usb "GW,CLAIM,$DEVICE")"       "state=active version=1 ok=true"
  say "rotate $DEVICE";  assert_usb "rotate"  "$(usb "GW,ROTATE,$DEVICE")"      "state=active version=2 ok=true"
  say "revoke $DEVICE";  assert_usb "revoke"  "$(usb "GW,REVOKE,$DEVICE")"      "state=revoked version=2 ok=true"
  say "rotate after revoke"; assert_usb "rotate-after-revoke" "$(usb "GW,ROTATE,$DEVICE")" "ok=false"
  say "claim ghost";     assert_usb "ghost"   "$(usb "GW,CLAIM,ghost-$DEVICE")" "state=unknown version=0 ok=false"
fi

say "Result"
printf 'PASS=%d  FAIL=%d\n' "$PASS" "$FAIL"
[[ "$FAIL" -eq 0 ]]
