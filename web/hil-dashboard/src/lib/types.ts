export type TransmissionMode = "esp_now" | "ble_advertisement" | "ook433_mhz" | "software_sim";

export interface SimConfig {
  mode: "EspNow" | "BleAdvertisement" | "Ook433Mhz" | "SoftwareSim";
  tx_power_dbm: number;
  snr_db: number;
  filter_bw_mhz: number;
  threshold: number;
  noise_level: number;
  replay_guard: boolean;
  data_bits: string;
  node_id: number;
}

export interface Kpis {
  prr_percent: number;
  latency_ms: number;
  last_bool: boolean;
  security_alerts: number;
  packets_sent: number;
  packets_ok: number;
}

export type SidecarTransport = "zmq" | "tls13_mtls";

export type HeaderPathMode = SimConfig["mode"] | "Unknown";

export interface TelemetryEvent {
  time: string;
  node_id: number;
  payload_json: string;
  rssi_dbm: number;
  status: string;
  seq: number;
  latency_ms: number;
}

export interface BitAnalysis {
  original: string;
  recovered: string;
  error_indices: number[];
  ber: number;
}

export interface Waveforms {
  baseband: number[];
  rf_tx: number[];
  rf_rx: number[];
  magnitude: number[];
  threshold: number;
}

export interface PipelineSnapshot {
  mode: SimConfig["mode"];
  hardware_mode: string;
  waveforms: Waveforms;
  bits: BitAnalysis;
  packet_ok: boolean;
  crc_ok: boolean;
  replay_rejected: boolean;
  zmq_published: boolean;
  kpis: Kpis;
  event: TelemetryEvent;
}

export interface LiveEvent {
  ts_ms: number;
  level: "info" | "warn" | "action" | string;
  source: string;
  message: string;
  node_id: number | null;
  seq: number | null;
  payload: string | null;
}

export interface LiveStatus {
  frames_decoded: number;
  events_buffered: number;
  last_action: { node_id: number; seq: number } | null;
}

export type WifiMode = "ap_sta" | "sta";

export interface NodeInfo {
  mac: string;
  ip: string;
  free_heap_bytes: number;
  online: boolean;
}

export interface OidEntry {
  oid: string;
  value: string;
}

export type ProvisioningState = "pending" | "active" | "revoked";

export interface DeviceIdentity {
  device_id: string;
  mac: string;
  state: ProvisioningState;
  credential_fingerprint: string;
  credential_version: number;
}

export interface GatewaySnapshot {
  wifi_mode: WifiMode;
  downstream_online: boolean;
  free_heap_bytes: number;
  heap_total_bytes: number;
  station_count: number;
  command_count: number;
  oids: OidEntry[];
  nodes: NodeInfo[];
  devices: DeviceIdentity[];
  command_log: string[];
}

export interface SnmpResponse {
  protocol: string;
  operation: string;
  oid: string;
  value: string | null;
  ok: boolean;
  message: string;
}

export interface GatewayResponse {
  ok: boolean;
  command: string;
  message: string;
  snmp?: SnmpResponse;
  snapshot: GatewaySnapshot;
}

export type GatewayMode = "hardware" | "simulation";

export interface GatewayStatus {
  mode: GatewayMode;
  connected: boolean;
  port: string | null;
}

export type GatewayCommand =
  | { command: "net_toggle_downstream" }
  | { command: "snmp_set"; oid: string; value: string }
  | { command: "snmp_get"; oid: string }
  | { command: "deauth_sta"; mac: string }
  | { command: "sys_health" }
  | { command: "sta_list" }
  | { command: "register_node"; mac: string; ip: string }
  | { command: "enroll_device"; device_id: string; mac: string }
  | { command: "claim_device"; device_id: string }
  | { command: "rotate_credential"; device_id: string }
  | { command: "revoke_device"; device_id: string };

export interface FirmwareConfigResponse {
  ok: boolean;
  applied: string[];
  unsupported: string[];
  command: string;
}
