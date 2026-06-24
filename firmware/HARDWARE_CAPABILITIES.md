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
| 8-bit transmit data | No | Current shared protocol payload is `BoolCmd(bool)`, so the TX node sends heartbeat `false` and BOOT action `true`. |
| TX power | Yes, build-time | Set `TX_POWER_DBM` when flashing the TX node. ESP-IDF accepts quarter-dBm units; firmware exposes a clamped integer dBm setting. |
| SNR | No | SNR is an observed or simulated channel condition, not a setting the two ESP32 boards can force by themselves. |
| Noise level | No | Artificial noise belongs in the simulator, an SDR/RF test setup, or external interference source. |
| Filter bandwidth | No | The ESP32-S3 gateway receives decoded ESP-NOW packets, not raw SDR samples. |
| Decision threshold | No | There is no firmware slicer threshold in the ESP-NOW path. |
| Replay guard | Control-plane | Firmware emits monotonically increasing sequence numbers; duplicate rejection is handled by control-plane rules. |

## Flashing with real TX power

Example:

```bash
GATEWAY_MAC="14:C1:9F:CB:51:B4" NODE_ID=1 TX_POWER_DBM=10 \
  ./scripts/flash_tx.sh /dev/cu.usbserial-TX1 115200 --monitor
```

If `TX_POWER_DBM` is unset, firmware leaves the ESP-IDF default Wi-Fi TX power
unchanged. If it is set outside the supported ESP32 range, firmware clamps it
and logs the value used at startup.
