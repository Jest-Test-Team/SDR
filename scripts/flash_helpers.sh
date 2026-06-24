#!/usr/bin/env bash
# Shared preflight for espflash (source from flash_*.sh).

list_usb_serial_ports() {
  ls /dev/cu.usbmodem* /dev/cu.usbserial* 2>/dev/null || true
}

# Gateway (ESP32-S3) = /dev/cu.usbmodem* ; TX node = /dev/cu.usbserial*
detect_gateway_port() {
  local modem
  modem="$(ls /dev/cu.usbmodem* 2>/dev/null | head -1 || true)"
  if [[ -n "${modem}" ]]; then
    echo "${modem}"
    return 0
  fi
  return 1
}

port_is_busy() {
  local port="$1"
  lsof "${port}" 2>/dev/null || true
}

preflight_pipeline_port() {
  local port="$1"

  if [[ ! -e "${port}" ]]; then
    echo "ERROR: Gateway port ${port} not found." >&2
    echo "Plug in the ESP32-S3 Gateway (shows as /dev/cu.usbmodem*)." >&2
    echo "Available ports:" >&2
    list_usb_serial_ports | sed 's/^/  /' >&2 || echo "  (none)" >&2
    return 1
  fi

  if [[ "${port}" == *usbserial* ]]; then
    echo "ERROR: ${port} looks like the TX node (usbserial), not the Gateway." >&2
    echo "edge-gateway must read from Gateway USB: /dev/cu.usbmodem*" >&2
    echo "  1. Close TX espflash monitor (Ctrl+C in that terminal)" >&2
    echo "  2. Plug Gateway into Mac; TX can use a charger" >&2
    echo "  3. GW_PORT=/dev/cu.usbmodem101 ./scripts/run_local.sh" >&2
    return 1
  fi

  local holders
  holders="$(port_is_busy "${port}")"
  if [[ -n "${holders}" ]]; then
    echo "ERROR: ${port} is busy (edge-gateway cannot open it):" >&2
    echo "${holders}" >&2
    if echo "${holders}" | grep -qE 'espflash|monitor'; then
      echo "Close espflash monitor first (Ctrl+C in flash terminal)." >&2
    fi
    return 1
  fi
  return 0
}

preflight_flash_port() {
  local port="$1"
  local label="${2:-device}"

  if [[ ! -e "${port}" ]]; then
    echo "ERROR: ${label} port ${port} not found." >&2
    echo "Available USB serial ports:" >&2
    list_usb_serial_ports | sed 's/^/  /' >&2 || echo "  (none — plug in USB data cable)" >&2
    echo "Run: ls /dev/cu.* | grep -E 'usb(modem|serial)'" >&2
    return 1
  fi

  local holders
  holders="$(lsof "${port}" 2>/dev/null || true)"
  if [[ -n "${holders}" ]]; then
    echo "ERROR: ${port} is busy (cannot flash):" >&2
    echo "${holders}" >&2
    if echo "${holders}" | grep -q 'edge-gateway'; then
      echo "Stop pipeline first:" >&2
      echo "  pkill -f 'target/release/edge-gateway|target/release/control-plane'" >&2
    fi
    return 1
  fi
  return 0
}

release_pipeline_for_flash() {
  if pgrep -f 'target/release/edge-gateway' >/dev/null 2>&1; then
    echo "Stopping edge-gateway/control-plane so serial port is free..."
    pkill -f 'target/release/edge-gateway|target/release/control-plane' 2>/dev/null || true
    sleep 2
  fi
}
