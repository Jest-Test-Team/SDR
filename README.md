# RF Simulation Workspace - ESP32 Telemetry Pipeline

End-to-end boolean command telemetry system using ESP32/ESP32-S3 with ESP-NOW, Rust firmware, and Rust control plane.

## Architecture

```
┌─────────────┐     ESP-NOW (2.4GHz)     ┌──────────────────┐     UART      ┌──────────────┐     ZMQ PUB/SUB     ┌───────────────┐
│ ESP32 ×2    │ ──────────────────────▶ │ ESP32-S3 Gateway │ ────────────▶ │ Edge Gateway │ ───────────────▶ │ Control Plane │
│ (TX Nodes)  │   BoolCmd + Seq + CRC    │ (RX + Bridge)    │  COBS Frames  │ (UART→ZMQ)   │  TelemetryFrame │ (Rules/Store) │
└─────────────┘                          └──────────────────┘               └──────────────┘                   └───────────────┘
```

## Hardware

| Role | Device | Qty | Interface |
|------|--------|-----|-----------|
| TX Node | ESP32 (WROOM-32) | 2 | ESP-NOW |
| Gateway | ESP32-S3 (WROOM-1U) | 1 | ESP-NOW + USB/UART |

## Quick Start

### Prerequisites

- Rust 1.82+ with `espup` toolchain
- `cargo-espflash` for flashing
- Two ESP32 + one ESP32-S3 with U.FL antennas

### 1. Flash Firmware

```bash
# Flash TX nodes (two ESP32s)
./scripts/flash_tx.sh /dev/ttyUSB0 460800 --monitor
./scripts/flash_tx.sh /dev/ttyUSB2 460800 --monitor

# Flash Gateway (ESP32-S3)
./scripts/flash_gw.sh /dev/ttyUSB1 921600 --monitor
```

### 2. Run Pipeline (PC)

```bash
# One command starts both services
./scripts/run_local.sh
```

Or manually:
```bash
# Terminal 1: Edge Gateway
cargo run -p edge-gateway --release

# Terminal 2: Control Plane
cargo run -p control-plane --release
```

### 3. Verify

- Press button on TX node → Control Plane logs `ACTION_TRIGGERED: BoolCmd(true)`
- Check metrics at `http://localhost:9090/metrics`
- Health at `http://localhost:8080/health`

## Development

### Build All
```bash
cargo build --workspace --release
```

### Test
```bash
cargo test --workspace --lib
```

### Firmware Only
```bash
cargo build --release -p esp32-tx-node -p esp32s3-gateway
```

## Configuration

Each service uses TOML config (see `config.toml.example` in each crate):
- `edge-gateway`: UART port, baud, ZMQ endpoint
- `control-plane`: ZMQ endpoint, DB path, rules

## CI/CD

- **CI**: `cargo check`, `clippy`, `fmt`, unit tests, firmware cross-compile
- **HIL**: Self-hosted runner with physical hardware (manual/weekly)

## Protocol

`protocol` crate defines:
- `TelemetryFrame`: seq, timestamp, node_id, payload, crc16
- `Payload::BoolCmd(bool)` - core PoC payload
- COBS framing over UART
- Postcard serialization for ESP-NOW

## License

MIT OR Apache-2.0