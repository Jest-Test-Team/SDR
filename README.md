# RF Simulation Workspace - ESP32 Telemetry Pipeline

End-to-end boolean command telemetry using ESP32/ESP32-S3 with ESP-NOW (hardware track) and GNU Radio / ZMQ injection (simulation track).

## Architecture

```
Hardware Track:
  ESP32 TX (×2) --ESP-NOW--> ESP32-S3 Gateway --UART/COBS--> edge-gateway --ZMQ--> control-plane

Simulation Track:
  dsp-core/inject_zmq.py ------------------------------------ZMQ-------------> control-plane
```

Both tracks publish the same COBS-wrapped `TelemetryFrame` on ZMQ.

## Current Status

- Live hardware path has been verified end to end:
  `ESP32 TX -> ESP-NOW -> ESP32-S3 Gateway -> USB -> edge-gateway -> ZMQ -> control-plane`.
- Current macOS gateway port is `/dev/cu.usbmodem1101`; run the PC pipeline with
  `GW_PORT=/dev/cu.usbmodem1101 GW_BAUD=115200 ./scripts/run_local.sh`.
- `edge-gateway` listens on `:8081`, `control-plane` listens on `:8092`, and the live dashboard proxies those through Next.js.
- TX node heartbeats are decoded as `BoolCmd(false)` about every 2 seconds; BOOT press should produce `ACTION_TRIGGERED`.
- If `control-plane` fails with sled corruption, preserve the local DB with:
  `mv data/telemetry.db data/telemetry.db.corrupt-$(date +%Y%m%d-%H%M)` and restart the pipeline.
- `scripts/run_live_dashboard.sh` clears stale `.next`, binds Next.js to `127.0.0.1`, and uses the next free dashboard port if `3001` is busy.
- Firmware-real dashboard controls are ESP-NOW live telemetry, sequence numbers, BOOT action, heartbeat, runtime TX power, and runtime 8-bit BOOT payload.
- Simulator-only or future-hardware controls are SNR, noise level, filter bandwidth, OOK threshold, and non-ESP-NOW modes.

## Hardware

| Role | Device | Qty | Interface |
|------|--------|-----|-----------|
| TX Node | ESP32 (WROOM-32) | 2 | ESP-NOW + GPIO0 button |
| Gateway | ESP32-S3 (WROOM-1U) | 1 | ESP-NOW + **USB serial** to PC (COBS telemetry on `/dev/cu.usbmodem*`) |

Compile-time env (firmware):

- `GATEWAY_MAC` — gateway peer MAC (default `FF:FF:FF:FF:FF:FF`)
- `NODE_ID` — TX node id (default `1`)
- `TX_POWER_DBM` — optional ESP32 Wi-Fi TX power for the TX node, clamped by firmware to the supported hardware range

See `firmware/HARDWARE_CAPABILITIES.md` for which dashboard controls are real firmware controls versus simulator/SDR-path controls, including the runtime FE/BE firmware-control path.

## HIL 模擬器儀表板（ESP32 軟體模擬）

動態視覺化 ESP32-S3 → RF → SDR → ZMQ → 控制層管線（真實 SDR 版本尚未啟用）。

```bash
# 終端 1：後端 API + 訊號模擬
cargo run -p hil-simulator --release

# 終端 2：Next.js 儀表板
cd web/hil-dashboard && npm install && npm run dev
```

或一鍵啟動：`./scripts/run_hil_dashboard.sh`

- 後端：http://localhost:8090（REST + WebSocket `/ws/live`）
- 前端：http://localhost:3000
- 可調 SNR、雜訊、閾值、傳輸模式，即時顯示 OOK 波形與 BER/CRC 分析
- 「發送布林指令」成功時可選發布至 ZMQ（`ZMQ_ENDPOINT`）供 control-plane 接收

## Quick Start

### Prerequisites

- Rust 1.85+
- Firmware toolchain (one-time setup):
  ```bash
  cargo install espup espflash ldproxy --locked
  espup install -t esp32,esp32s3 -f ~/export-esp.sh
  brew install ninja zeromq pkgconf   # macOS
  ```
  Each new terminal: `source ~/export-esp.sh` and `export PATH="$HOME/.cargo/bin:$PATH"`.
- `libzmq` (`brew install zeromq pkgconf` on macOS, `apt install libzmq3-dev` on Linux)
- Python 3 + `pyzmq` for simulation injector (`pip install pyzmq`)

### 1. Flash Firmware

List serial ports (macOS use `/dev/cu.*`, not `ttyUSB*`):

```bash
ls /dev/cu.* | grep -v Bluetooth
```

**ESP32-S3 Gateway** (already flashed example MAC `14:C1:9F:CB:51:B4`):

```bash
./scripts/flash_gw.sh /dev/cu.usbmodem1101 115200 --monitor
```

**ESP32 TX Node** (×2, set unique `NODE_ID`; point `GATEWAY_MAC` at gateway):

```bash
# TX #1
GATEWAY_MAC="14:C1:9F:CB:51:B4" NODE_ID=1 \
  ./scripts/flash_tx.sh /dev/cu.usbserial-57860443631 115200 --monitor

# TX #2 (unplug gateway, plug second ESP32, use its port)
GATEWAY_MAC="14:C1:9F:CB:51:B4" NODE_ID=2 TX_POWER_DBM=10 \
  ./scripts/flash_tx.sh /dev/cu.usbserial-57860443631 115200 --monitor
```

Notes:

- Replace port names with the actual output of `ls /dev/cu.* | grep -E 'usb(modem|serial)'`. If only one USB cable is connected, the visible port is the board currently plugged in.
- On macOS, `./scripts/flash_gw.sh` auto-detects the first `/dev/cu.usbmodem*` when no port is passed, and `./scripts/flash_tx.sh` auto-detects the first `/dev/cu.usbserial*` when no port is passed.
- If a port disappears after `pkill`, the board using that port is no longer connected or has re-enumerated. Run the `ls /dev/cu.*` command again and use the new visible port.
- If flash fails at high baud, use `115200` for `espflash`; on macOS, `run_local.sh` uses `115200` for `/dev/cu.usbmodem*` automatically.
- The TX node sends `BoolCmd(true)` on BOOT press and `BoolCmd(false)` heartbeats about every 2 seconds.

### Dashboard controls vs. physical hardware

The two-board ESP-NOW path receives decoded packets, not raw RF samples. Some dashboard controls can therefore be applied to firmware, while others require SDR/RF hardware:

| Control | How to make it physically real |
| --- | --- |
| SNR | Add an RF attenuator, distance-controlled setup, programmable attenuator, or estimate channel quality from RSSI/packet loss. |
| Noise level | Add an RF noise source, SDR signal injection, or a controlled interference transmitter. |
| Filter bandwidth | Add an SDR receiver path such as RTL-SDR, USRP, or GNU Radio, then filter raw samples in DSP. |
| Decision threshold | Add an SDR/OOK demodulator path where firmware or backend code slices raw magnitude samples. |
| non-ESP-NOW mode | Implement BLE firmware mode, or add 433 MHz OOK TX/RX hardware. |
| replay guard | Make this a runtime backend/control-plane toggle; it is a packet-rule control, not an RF firmware setting. |

### 2. Run Pipeline (PC)

```bash
GW_PORT=/dev/cu.usbmodem1101 GW_BAUD=115200 ./scripts/run_local.sh
```

### 3. Simulate Without Hardware

```bash
cargo run -p control-plane --release &
python3 dsp-core/scripts/inject_zmq.py --replay-last
```

Expect `ACTION_TRIGGERED` in control-plane logs for unique `BoolCmd(true)` frames.

### 4. Verify

- Press GPIO0/BOOT on the TX node.
- Health: `http://localhost:8080/health` (control-plane), `http://localhost:8081/health` (edge-gateway)
- Metrics: `/metrics` on the same ports

## Repository Layout

```
protocol/           Shared TelemetryFrame, COBS/UART + ESP-NOW framing, ReplayGuard
firmware/           esp32-tx-node, esp32s3-gateway
edge-gateway/       UART -> ZMQ PUB
control-plane/      ZMQ SUB -> rules -> sled
dsp-core/           inject_zmq.py, optional GNU Radio Docker image
infrastructure/     Dockerfiles
```

## Protocol

| Layer | Format |
|-------|--------|
| ESP-NOW | `[vendor_id=0x1A][postcard \|\| crc16_le]` |
| UART | `COBS(postcard \|\| crc16_le) + 0x00` |
| ZMQ | Same COBS bytes as UART (without delimiter) |

## Development

```bash
# Host services (no ESP toolchain)
cargo test --workspace --lib
cargo test -p control-plane --test sim_pipeline
cargo test -p hil-simulator --lib

# Firmware (requires esp toolchain + source ~/export-esp.sh)
./scripts/flash_gw.sh /dev/cu.usbmodem1101 115200
./scripts/flash_tx.sh /dev/cu.usbserial-<visible-tx-port> 115200
```

First firmware build downloads ESP-IDF into `.embuild/` (~10–20 min).

## CI/CD

- **CI**: fmt, clippy, unit tests, sim_pipeline integration test, firmware cross-compile
- **HIL**: Self-hosted runner (`esp32-hil`), weekly or manual (`HIL_ENABLED=1`)

## License

MIT OR Apache-2.0
