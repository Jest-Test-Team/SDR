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
| Transport mode | Partial | Firmware is ESP-NOW only. `SoftwareSim` and `433 MHz OOK` are simulator/SDR-path behavior. |
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

## Software-sim helpers

The workspace now includes a host-side Rust crate at `firmware/software-sim`
that emits the same frame/control bytes used by the real boards:

- `TelemetryFrame` bytes for the ESP32 TX node path.
- UART `SDRCTL,...` control lines for the ESP32-S3 gateway.
- Shared helpers for software-sim tooling and tests.
- Secure Telemetry Gateway command model (`gateway` module): hardware-free
  simulation of the ESP32-S3 AP-STA gateway used by `hil-simulator` and tests.

### Secure Telemetry Gateway commands

The `firmware-software-sim::gateway` module models the AP-STA gateway described
in the implementation guide. The same command set is mapped onto `esp-idf-svc`
calls on the device and onto the hardware-free `GatewaySim` for the dashboard /
HIL simulator:

| Command | On-device mapping | Effect |
| --- | --- | --- |
| `CMD_NET_TOGGLE_DOWNSTREAM` | `wifi.set_configuration(Client \| Mixed)` | Sever/restore the downstream AP (`Sta` ↔ `ApSta`) |
| `CMD_SNMP_SET` | TCP JSON to endpoint | Write a simulated OID (`sim_snmp_v3`) |
| `CMD_SNMP_GET` | TCP JSON to endpoint | Read a simulated OID |
| `CMD_DEAUTH_STA` | `esp_wifi_deauth_sta` | Kick a station by MAC |
| `CMD_SYS_HEALTH` | `esp_get_free_heap_size` | Report heap / link / station count |
| `CMD_REGISTER_NODE` | AP STA-join event (sim-only) | Track a downstream endpoint |
| `CMD_ENROLL_DEVICE` | `GwMsg::EnrollReq` → on-device registry | Create identity, issue credential (`pending`) |
| `CMD_CLAIM_DEVICE` | `GwMsg::ClaimReq` → on-device registry | Activate an enrolled device (`active`) |
| `CMD_ROTATE_CREDENTIAL` | `GwMsg::RotateReq` → on-device registry | Reissue credential, bump version |
| `CMD_REVOKE_DEVICE` | `GwMsg::RevokeReq` → on-device registry | Revoke identity (`revoked`) |

The HIL simulator exposes these over `GET /api/v1/gateway` and
`POST /api/v1/gateway/command`, and the web dashboard's **Secure Gateway** page
drives them.

### On-device provisioning (real boards)

Provisioning is a **real on-device operation**, not sim-only. The ESP32 gateway
(`esp32-tx-node` crate) holds the device registry in RAM and answers four new
`GwMsg` requests, replying `ProvisionResp { device_id, state, fingerprint,
version, ok }`. The state machine is `pending → active → revoked`, with negative
paths rejected (duplicate enroll, claim of a non-pending device, rotate after
revoke). The ESP32-S3 node (`esp32s3-gateway` crate) accepts these USB lines and
relays them over ESP-NOW, printing replies as `GWRESP PROVISION ...`:

| USB line | ESP-NOW request |
| --- | --- |
| `GW,ENROLL,<device_id>,<mac>` | `GwMsg::EnrollReq` |
| `GW,CLAIM,<device_id>` | `GwMsg::ClaimReq` |
| `GW,ROTATE,<device_id>` | `GwMsg::RotateReq` |
| `GW,REVOKE,<device_id>` | `GwMsg::RevokeReq` |

The credential fingerprint is a deterministic FNV-1a stand-in (no real crypto
yet). In hardware mode the dashboard's provisioning panel drives the boards;
`hil-simulator` parses `GWRESP PROVISION` lines into `GatewaySnapshot.devices`.

This crate is for protocol generation and test harnesses. It does not replace
the embedded ESP32 firmware images.

## Flashing with default TX power

Example:

```bash
GATEWAY_MAC="14:C1:9F:CB:51:B4" NODE_ID=1 TX_POWER_DBM=10 \
  ./scripts/flash_tx.sh /dev/cu.usbserial-TX1 115200 --monitor
```

If `TX_POWER_DBM` is unset, firmware leaves the ESP-IDF default Wi-Fi TX power
unchanged. If it is set outside the supported ESP32 range, firmware clamps it
and logs the value used at startup.
