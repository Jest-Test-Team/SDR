# RF Simulation Workspace - ESP32 Telemetry Pipeline

End-to-end boolean command telemetry using ESP32/ESP32-S3 with ESP-NOW (hardware track) and GNU Radio / ZMQ injection (simulation track).

## Architecture

```
Hardware Track:
  ESP32 TX (×2) --ESP-NOW--> ESP32-S3 Gateway --UART/COBS--> edge-gateway --ZMQ--> control-plane

Simulation Track:
  dsp-core/inject_zmq.py ------------------------------------ZMQ-------------> control-plane
  hil-simulator sidecar ----ZMQ or HTTPS/TLS 1.3 mTLS ingest----------------> control-plane
```

Hardware and simulator tracks publish the same `TelemetryFrame` values into the control-plane processing path.

## Current Status

- Live hardware path has been verified end to end:
  `ESP32 TX -> ESP-NOW -> ESP32-S3 Gateway -> USB -> edge-gateway -> ZMQ -> control-plane`.
- Both boards re-verified flashing on 2026-06-25:
  - Gateway (ESP32-S3, MAC `14:C1:9F:CB:51:B4`) on `/dev/cu.usbmodem1101` flashed at `921600` baud via `./scripts/flash_gw.sh`.
  - TX node (ESP32, MAC `CC:7B:5C:25:9E:20`, `NODE_ID=1`, `TX_POWER_DBM=10`) on `/dev/cu.usbserial-57860443631` flashed at `460800` baud via `./scripts/flash_tx.sh`.
  - After flashing, the TX serial monitor shows `ESP-NOW sent node=1 seq=N payload=BoolCmd(false)` heartbeats ~every 2s; BOOT/GPIO0 press sends `BoolCmd(true)`.
  - `espflash`'s `Setting baud rate higher than 115,200 can cause issues` is a benign warning; these boards flash reliably at the higher rates above. Drop to `115200` only if a flash fails.
- Current macOS gateway port is `/dev/cu.usbmodem1101`; run the PC pipeline with
  `GW_PORT=/dev/cu.usbmodem1101 GW_BAUD=115200 ./scripts/run_local.sh`.
- `edge-gateway` listens on `:8081`, `control-plane` listens on `:8092`, and the live dashboard proxies those through Next.js.
- TX node heartbeats are decoded as `BoolCmd(false)` about every 2 seconds; BOOT press should produce `ACTION_TRIGGERED`.
- If `control-plane` fails with sled corruption, preserve the local DB with:
  `mv data/telemetry.db data/telemetry.db.corrupt-$(date +%Y%m%d-%H%M)` and restart the pipeline.
- `scripts/run_live_dashboard.sh` clears stale `.next`, binds Next.js to `127.0.0.1`, and uses the next free dashboard port if `3001` is busy.
- Firmware-real dashboard controls are ESP-NOW live telemetry, sequence numbers, BOOT action, heartbeat, runtime TX power, and runtime 8-bit BOOT payload.
- Simulator-only or future-hardware controls are SNR, noise level, filter bandwidth, OOK threshold, and non-ESP-NOW modes.
- `hil-simulator` is now a first-class software-sim sidecar: it can publish valid simulated frames through local ZMQ or through secure TLS 1.3/mTLS ingest at `control-plane`.
- Secure Telemetry Gateway (software-sim) verified on real hardware on 2026-06-25:
  ESP32-S3 software-sim node (USB to Mac) drives an ESP32 gateway over ESP-NOW;
  `GW,HEALTH`/`TOGGLE`/`DEAUTH`/`STALIST`/`SNMP_*` all confirmed on-device, and the
  dashboard `/gateway` page auto-detects real vs simulation mode. See
  [Secure Telemetry Gateway](#secure-telemetry-gateway-software-sim-hardware-verified).

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
- 「發送布林指令」成功時會以軟體 sidecar 方式送出 `TelemetryFrame`，預設發布至 ZMQ（`ZMQ_ENDPOINT`）供 control-plane 接收
- 安全 sidecar 模式可改走 `SECURE_INGEST_URL=https://localhost:<port>/api/v1/ingest/frame`，並設定 `HIL_SIM_TLS_CERT`、`HIL_SIM_TLS_KEY`、`HIL_SIM_SERVER_CA`

Secure ingest on `control-plane`:

```bash
SECURE_INGEST_ONLY=1 \
CONTROL_PLANE_TLS_CERT=certs/server.pem \
CONTROL_PLANE_TLS_KEY=certs/server.key \
CONTROL_PLANE_CLIENT_CA=certs/ca.pem \
cargo run -p control-plane -- --health-port 8092
```

When `SECURE_INGEST_ONLY=1` is set, the ZMQ subscriber is disabled and clients must use TLS 1.3 with a trusted client certificate.

## Secure Telemetry Gateway (software-sim, hardware-verified)

A second, command-oriented topology that runs the same gateway control surface as
hardware **or** pure software, with the dashboard showing which mode is live.

```text
Mac (control-plane / dashboard)
  | USB serial  (ESP32-S3 usbmodem)
  v
ESP32-S3  = software-sim sender + receiver  (server-facing endpoint)
  ^  ESP-NOW  (protocol::gwlink, vendor id 0x1B)
  |
ESP32     = gateway (AP-STA: GATEWAY_ISOLATED_NET on ch 1)
```

### Board / role / script map

> Note: the firmware crate names are legacy and **inverted** vs. these roles.

| Board | Port | Flash script | Crate built | Role |
|-------|------|--------------|-------------|------|
| ESP32 (normal) | `usbserial` | `./scripts/flash_tx.sh` | `esp32-tx-node` | **Gateway** (AP-STA, executes commands) |
| ESP32-S3 | `usbmodem` | `./scripts/flash_gw.sh` | `esp32s3-gateway` | **Software-sim node** (USB↔Mac, ESP-NOW↔gateway) |

### Flash both boards

```bash
# ESP32 gateway (note its MAC from the boot log; default assumes CC:7B:5C:25:9E:20)
./scripts/flash_tx.sh "" 460800 --monitor      # expect: "AP 'GATEWAY_ISOLATED_NET' up"

# ESP32-S3 sim node (flash WITHOUT --monitor so the USB port stays free)
./scripts/flash_gw.sh                          # bakes GATEWAY_MAC into the S3 build
#   different ESP32 MAC:  GATEWAY_MAC="aa:bb:.." ./scripts/flash_gw.sh
```

### On-device command set

The S3 accepts comma-separated USB lines and relays them to the gateway over
ESP-NOW (`protocol::gwlink::GwMsg`), printing the replies back as
`GWRESP ...` / `SIMRECV ...`:

| USB line | Gateway action | Reply |
|----------|----------------|-------|
| `GW,HEALTH` | real `esp_get_free_heap_size()` + uptime + Wi-Fi mode | `GWRESP HEALTH free_heap=… wifi_mode=ApSta` |
| `GW,TOGGLE` | AP up/down (`Mixed` ↔ `Client`) | `GWRESP TOGGLE downstream_online=… wifi_mode=…` |
| `GW,DEAUTH` | `esp_wifi_deauth_sta(0)` (kick all) | `GWRESP DEAUTH kicked=…` |
| `GW,STALIST` | connected station count | `GWRESP STALIST count=…` |
| `GW,SNMP_SET,<oid>,<val>` | write simulated OID (in-RAM MIB) | `GWRESP SNMP oid=… value=… ok=true` |
| `GW,SNMP_GET,<oid>` | read simulated OID | `GWRESP SNMP oid=… value=… ok=…` |
| `SIM,SEND,<0\|1>` | send TelemetryFrame; gateway echoes it | `SIMRECV node=… seq=… payload=ByteCmd(172)` |

### Verify from the CLI (no dashboard)

`scripts/sim_node.py` is a stdlib-only client that drives the S3 over USB. The S3
port can only be held by one process — close any espflash monitor first.

```bash
./scripts/sim_node.py GW,HEALTH
./scripts/sim_node.py GW,SNMP_SET,1.3.6.1.4.1.custom.relay,on
./scripts/sim_node.py GW,SNMP_GET,1.3.6.1.4.1.custom.relay
./scripts/sim_node.py GW,TOGGLE          # GATEWAY_ISOLATED_NET disappears from Wi-Fi scan
./scripts/sim_node.py SIM,SEND,1
```

Proof it is real hardware (not the sim): `free_heap` is a live, changing number
(e.g. `221272`), and `GW,TOGGLE` makes the `GATEWAY_ISOLATED_NET` AP physically
appear/disappear in a phone's Wi-Fi list.

### Drive it from the dashboard (real ⟷ simulation)

`hil-simulator` opens the S3 serial port when `HIL_GW_SERIAL` is set; otherwise it
serves the in-memory `GatewaySim`. The `/gateway` page shows a **REAL HARDWARE**
vs **SIMULATION MODE** badge and a live serial monitor (`/ws/gateway`).

```bash
# IMPORTANT: kill any stale hil-simulator first, or it keeps serving the old mode
pkill -f hil-simulator; sleep 1
lsof /dev/cu.usbmodem1101                       # must be empty (no monitor/CLI holding it)

# hardware mode (auto-detect the S3 port, or pass the path)
HIL_GW_SERIAL=auto ./scripts/run_hil_dashboard.sh
#   explicit:  HIL_GW_SERIAL=/dev/cu.usbmodem1101 ./scripts/run_hil_dashboard.sh
```

Expected startup log: `gateway hardware backend connected on /dev/cu.usbmodem1101`
and `Gateway backend: Hardware`. Confirm with:

```bash
curl -s localhost:8090/api/v1/gateway/status
# {"mode":"hardware","connected":true,"port":"/dev/cu.usbmodem1101"}
```

Then open `/gateway` (port 3001/3002): the badge reads **REAL HARDWARE**, command
buttons hit the boards, and the live monitor streams `GWRESP …` lines. Without
`HIL_GW_SERIAL` the same page runs against the simulator and the badge reads
**SIMULATION MODE** — the Robot suite `tests/gateway_commands.robot` exercises that
path with no hardware.

### Notes / gotchas

- Only one process can own the S3 USB port. Don't run `sim_node.py` or an espflash
  monitor while the hardware-mode dashboard is up.
- `CMD_DEAUTH_STA` maps to "deauth all" on hardware; the MAC field is ignored.
- `CMD_REGISTER_NODE` is simulation-only (returns a note in hardware mode).
- Wire protocol and command translation are host-tested: `cargo test -p protocol`
  (gwlink round-trips) and `cargo test -p firmware-software-sim` (gateway model).

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

**ESP32-S3 Gateway** (already flashed example MAC `14:C1:9F:CB:51:B4`). With no
arguments the script auto-detects the first `/dev/cu.usbmodem*` and flashes at
`921600` baud:

```bash
./scripts/flash_gw.sh                       # flash only (verified working)
./scripts/flash_gw.sh "" 921600 --monitor   # flash + open serial monitor
```

> Running without `--monitor` may print `Monitor options were provided, but
> --monitor flag isn't set` — harmless. Pass `--monitor` to actually watch the
> gateway bridge decoded ESP-NOW frames out over USB.

**ESP32 TX Node** (×2, set unique `NODE_ID`; point `GATEWAY_MAC` at gateway).
Empty first arg auto-detects the first `/dev/cu.usbserial*`:

```bash
# TX #1 (verified working)
GATEWAY_MAC="14:C1:9F:CB:51:B4" NODE_ID=1 TX_POWER_DBM=10 \
  ./scripts/flash_tx.sh "" 460800 --monitor

# TX #2 (unplug gateway, plug second ESP32, use its port)
GATEWAY_MAC="14:C1:9F:CB:51:B4" NODE_ID=2 TX_POWER_DBM=10 \
  ./scripts/flash_tx.sh "" 460800 --monitor
```

Expected TX monitor output after a successful flash:

```text
I (817) esp32_tx_node: ESP-NOW ready, gateway=14:C1:9F:CB:51:B4 ch=1
I (837) esp32_tx_node: main loop started (BOOT=GPIO0 trigger)
I (2847) esp32_tx_node: ESP-NOW sent node=1 seq=1 payload=BoolCmd(false)
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
protocol/           Shared TelemetryFrame, COBS/UART + ESP-NOW framing, gwlink, ReplayGuard
firmware/           esp32-tx-node (now ESP32 gateway role), esp32s3-gateway (now S3 sim node),
                    software-sim (host-side gateway model + helpers)
edge-gateway/       UART -> ZMQ PUB
control-plane/      ZMQ SUB -> rules -> sled
hil-simulator/      software-sim sidecar + gateway backend (hardware/simulation)
web/hil-dashboard/  Next.js dashboard (HIL + /gateway real/sim page)
dsp-core/           inject_zmq.py, optional GNU Radio Docker image
infrastructure/     Dockerfiles
scripts/            flash_tx.sh, flash_gw.sh, sim_node.py, run_*.sh
```

## Protocol

| Layer | Format |
|-------|--------|
| ESP-NOW (telemetry) | `[vendor_id=0x1A][postcard \|\| crc16_le]` |
| ESP-NOW (gateway link) | `[vendor_id=0x1B][postcard(GwMsg) \|\| crc16_le]` (`protocol::gwlink`) |
| UART | `COBS(postcard \|\| crc16_le) + 0x00` |
| ZMQ | Same COBS bytes as UART (without delimiter) |

## Development

```bash
# Host services (no ESP toolchain)
cargo test --workspace --lib
cargo test -p protocol                 # frame + gwlink wire round-trips
cargo test -p firmware-software-sim    # gateway command model
cargo test -p control-plane --test sim_pipeline
cargo test -p hil-simulator --lib

# Gateway command surface, end to end in simulation mode (Robot Framework)
robot --outputdir /tmp/sdr-robot tests/gateway_commands.robot

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
