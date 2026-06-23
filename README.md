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

## Hardware

| Role | Device | Qty | Interface |
|------|--------|-----|-----------|
| TX Node | ESP32 (WROOM-32) | 2 | ESP-NOW + GPIO0 button + UART CLI |
| Gateway | ESP32-S3 (WROOM-1U) | 1 | ESP-NOW + USB/UART @ 921600 |

Compile-time env (firmware):

- `GATEWAY_MAC` — gateway peer MAC (default `FF:FF:FF:FF:FF:FF`)
- `NODE_ID` — TX node id (default `1`)

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
- `espup` + `cargo-espflash` for firmware
- `libzmq` (`brew install zeromq pkgconf` on macOS, `apt install libzmq3-dev` on Linux)
- Python 3 + `pyzmq` for simulation injector

### 1. Flash Firmware

```bash
./scripts/flash_tx.sh /dev/ttyUSB0 460800 --monitor
./scripts/flash_gw.sh /dev/ttyUSB1 921600 --monitor
```

UART CLI on TX node: `TRIGGER` / `RELEASE`

### 2. Run Pipeline (PC)

```bash
./scripts/run_local.sh
```

### 3. Simulate Without Hardware

```bash
cargo run -p control-plane --release &
python3 dsp-core/scripts/inject_zmq.py --replay-last
```

Expect `ACTION_TRIGGERED` in control-plane logs for unique `BoolCmd(true)` frames.

### 4. Verify

- Press GPIO0 or send `TRIGGER` on TX UART
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
cargo test --workspace --lib
cargo test -p control-plane --test sim_pipeline
cargo build --release -p esp32-tx-node -p esp32s3-gateway
```

## CI/CD

- **CI**: fmt, clippy, unit tests, sim_pipeline integration test, firmware cross-compile
- **HIL**: Self-hosted runner (`esp32-hil`), weekly or manual (`HIL_ENABLED=1`)

## License

MIT OR Apache-2.0
