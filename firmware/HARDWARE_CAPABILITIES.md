# Firmware hardware capabilities

The two-board live firmware path is:

```text
ESP32 TX node -> ESP-NOW -> ESP32-S3 gateway -> USB serial -> edge-gateway -> control-plane
```

This path is real hardware for ESP-NOW frame transmission, sequence numbers,
BOOT-button actions, periodic heartbeats, gateway receive/decode, and USB
forwarding. It is not an SDR receiver and it does not implement the simulator's
OOK demodulator.

## What the boards can control

| Dashboard concept | Firmware support | Notes |
| --- | --- | --- |
| Transport mode | Partial | Firmware is ESP-NOW only. `433 MHz OOK` is simulator/SDR-path behavior. |
| 8-bit transmit data | Yes, runtime | The dashboard/edge gateway can set the TX node BOOT payload byte. Heartbeats remain `BoolCmd(false)`. |
| TX power | Yes, build-time and runtime | Set `TX_POWER_DBM` when flashing, or apply it from the dashboard through the live edge gateway. ESP-IDF accepts quarter-dBm units; firmware exposes a clamped integer dBm setting. |
| SNR | No | SNR is an observed or simulated channel condition, not a setting the two ESP32 boards can force by themselves. |
| Noise level | No | Artificial noise belongs in the simulator, an SDR/RF test setup, or external interference source. |
| Filter bandwidth | No | The ESP32-S3 gateway receives decoded ESP-NOW packets, not raw SDR samples. |
| Decision threshold | No | There is no firmware slicer threshold in the ESP-NOW path. |
| Replay guard | Control-plane | Firmware emits monotonically increasing sequence numbers; duplicate rejection is handled by control-plane rules. |

## Runtime control path

Runtime firmware control uses this path:

```text
dashboard -> Next.js proxy -> edge-gateway HTTP -> USB serial -> ESP32-S3 gateway -> ESP-NOW broadcast -> ESP32 TX node
```

The edge gateway accepts `POST /api/v1/firmware/config`. The dashboard calls this
through `/api/v1/firmware/config` when you press **Apply to live firmware**.

The runtime command currently applies:

- `node_id`: target node, or `0` for all TX nodes.
- `tx_power_dbm`: applied on the TX node with ESP-IDF Wi-Fi TX power.
- `data_bits`: parsed as an 8-bit byte and sent as `ByteCmd(0xNN)` on the next
  BOOT press.

The same API reports `snr_db`, `noise_level`, `filter_bw_mhz`, `threshold`,
non-ESP-NOW modes, and `replay_guard` as unsupported for firmware because they
belong to the simulator, SDR path, or control-plane rules.

## Flashing with default TX power

Example:

```bash
GATEWAY_MAC="14:C1:9F:CB:51:B4" NODE_ID=1 TX_POWER_DBM=10 \
  ./scripts/flash_tx.sh /dev/cu.usbserial-TX1 115200 --monitor
```

If `TX_POWER_DBM` is unset, firmware leaves the ESP-IDF default Wi-Fi TX power
unchanged. If it is set outside the supported ESP32 range, firmware clamps it
and logs the value used at startup.
