export type TransmissionMode = "esp_now" | "ble_advertisement" | "ook433_mhz";

export interface SimConfig {
  mode: "EspNow" | "BleAdvertisement" | "Ook433Mhz";
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

export interface FirmwareConfigResponse {
  ok: boolean;
  applied: string[];
  unsupported: string[];
  command: string;
}
