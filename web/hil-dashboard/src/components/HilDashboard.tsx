"use client";

import { useCallback, useEffect, useState } from "react";
import {
  fetchStatus,
  triggerCommand,
  updateConfig,
  wsUrl,
} from "@/lib/api";
import type { Kpis, PipelineSnapshot, SimConfig, TelemetryEvent } from "@/lib/types";
import { BitCompare } from "./BitCompare";
import { PipelineFlow } from "./PipelineFlow";
import { WaveformPanel } from "./WaveformPanel";

type Tab = "hil" | "ook" | "bits";

const MODE_OPTIONS: { value: SimConfig["mode"]; label: string }[] = [
  { value: "EspNow", label: "ESP-NOW" },
  { value: "BleAdvertisement", label: "BLE Advertisement" },
  { value: "Ook433Mhz", label: "433MHz OOK" },
];

export function HilDashboard() {
  const [tab, setTab] = useState<Tab>("hil");
  const [config, setConfig] = useState<SimConfig | null>(null);
  const [kpis, setKpis] = useState<Kpis | null>(null);
  const [events, setEvents] = useState<TelemetryEvent[]>([]);
  const [snapshot, setSnapshot] = useState<PipelineSnapshot | null>(null);
  const [connected, setConnected] = useState(false);
  const [busy, setBusy] = useState(false);

  const applySnapshot = useCallback((snap: PipelineSnapshot) => {
    setSnapshot(snap);
    setKpis(snap.kpis);
    setEvents((prev) => [snap.event, ...prev].slice(0, 50));
  }, []);

  useEffect(() => {
    fetchStatus()
      .then((s) => {
        setConfig(s.config);
        setKpis(s.kpis);
      })
      .catch(console.error);
  }, []);

  useEffect(() => {
    const url = wsUrl();
    if (!url) return;
    const ws = new WebSocket(url);
    ws.onopen = () => setConnected(true);
    ws.onclose = () => setConnected(false);
    ws.onmessage = (ev) => {
      try {
        const msg = JSON.parse(ev.data);
        if (msg.type === "snapshot") applySnapshot(msg.data);
        if (msg.type === "hello") {
          setConfig(msg.config);
          setKpis(msg.kpis);
          setEvents(msg.events ?? []);
        }
      } catch (e) {
        console.error(e);
      }
    };
  }, [applySnapshot]);

  const patchConfig = async (patch: Partial<SimConfig>) => {
    if (!config) return;
    const next = { ...config, ...patch };
    setConfig(next);
    const saved = await updateConfig(next);
    setConfig(saved);
  };

  const onTrigger = async () => {
    setBusy(true);
    try {
      const snap = await triggerCommand(true);
      applySnapshot(snap);
    } finally {
      setBusy(false);
    }
  };

  const wf = snapshot?.waveforms;

  return (
    <div className="dashboard">
      <header className="header">
        <div>
          <h1>ESP32-S3 至 SDR HIL 模擬器</h1>
          <p className="subtitle">
            軟體模擬模式（ESP32-S3 + ESP32）· 真實 SDR 版本尚未啟用
            <span className={`ws-dot ${connected ? "on" : ""}`} />
            {connected ? "即時連線" : "連線中…"}
          </p>
        </div>
        <div className="tabs">
          {(
            [
              ["hil", "系統總覽"],
              ["ook", "OOK 解調"],
              ["bits", "位元分析"],
            ] as const
          ).map(([id, label]) => (
            <button
              key={id}
              className={tab === id ? "tab active" : "tab"}
              onClick={() => setTab(id)}
              type="button"
            >
              {label}
            </button>
          ))}
        </div>
      </header>

      {tab === "hil" && (
        <>
          <PipelineFlow snapshot={snapshot} />

          <div className="panel events-panel">
            <h3>即時資料流</h3>
            <table>
              <thead>
                <tr>
                  <th>時間</th>
                  <th>來源節點</th>
                  <th>JSON 載荷</th>
                  <th>RSSI</th>
                  <th>狀態</th>
                </tr>
              </thead>
              <tbody>
                {events.map((e, i) => (
                  <tr key={`${e.seq}-${i}`}>
                    <td>{e.time}</td>
                    <td>Node {e.node_id}</td>
                    <td className="mono">{e.payload_json}</td>
                    <td>{e.rssi_dbm.toFixed(1)} dBm</td>
                    <td>{e.status}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>

          <div className="kpi-row">
            <div className="kpi">
              <div className="kpi-label">PRR (封包接收率)</div>
              <div className="kpi-value">{kpis?.prr_percent.toFixed(1) ?? "—"}%</div>
            </div>
            <div className="kpi">
              <div className="kpi-label">延遲 (Latency)</div>
              <div className="kpi-value">{kpis?.latency_ms ?? "—"}ms</div>
            </div>
            <div className="kpi">
              <div className="kpi-label">狀態 (Bool)</div>
              <div className="kpi-value">{String(kpis?.last_bool ?? false)}</div>
            </div>
            <div className="kpi alert">
              <div className="kpi-label">安全警報</div>
              <div className="kpi-value">{kpis?.security_alerts ?? 0}</div>
            </div>
          </div>
        </>
      )}

      {tab === "ook" && (
        <div className="charts-grid">
          <WaveformPanel
            title="ESP32 原始數位訊號 (Baseband)"
            data={wf?.baseband ?? []}
            color="#4da3ff"
            yDomain={[-0.2, 1.2]}
            unit="Level"
          />
          <WaveformPanel
            title="發射端 RF 訊號 (OOK 調變)"
            data={wf?.rf_tx ?? []}
            color="#3ecf8e"
            yDomain={[-1.2, 1.2]}
            unit="Amp"
          />
          <WaveformPanel
            title="RTL-SDR 接收之複合訊號 (含雜訊)"
            data={wf?.rf_rx ?? []}
            color="#f5a623"
            yDomain={[-2, 2]}
            unit="Amp"
          />
          <WaveformPanel
            title="GNU Radio 解調與判定 (Magnitude & Slicer)"
            data={wf?.magnitude ?? []}
            color="#e85d5d"
            threshold={wf?.threshold ?? config?.threshold ?? 0.75}
            yDomain={[0, 1.5]}
            unit="Mag"
          />
        </div>
      )}

      {tab === "bits" && snapshot && (
        <>
          <WaveformPanel
            title="解調 Magnitude"
            data={wf?.magnitude ?? []}
            color="#e85d5d"
            threshold={wf?.threshold ?? 0.75}
            yDomain={[0, 1.5]}
            unit="Mag"
          />
          <BitCompare
            bits={snapshot.bits}
            packetOk={snapshot.packet_ok}
            crcOk={snapshot.crc_ok}
          />
        </>
      )}

      <div className="control-panel panel">
        <h3>控制面板</h3>
        <div className="controls">
          <label>
            傳輸模式
            <select
              value={config?.mode ?? "EspNow"}
              onChange={(e) =>
                patchConfig({ mode: e.target.value as SimConfig["mode"] })
              }
            >
              {MODE_OPTIONS.map((o) => (
                <option key={o.value} value={o.value}>
                  {o.label}
                </option>
              ))}
            </select>
          </label>

          <label>
            傳輸資料 (8-bit)
            <input
              value={config?.data_bits ?? "10110010"}
              onChange={(e) => patchConfig({ data_bits: e.target.value })}
              maxLength={8}
              className="mono"
            />
          </label>

          <label>
            發射功率 (dBm): {config?.tx_power_dbm ?? 0}
            <input
              type="range"
              min={-10}
              max={10}
              step={1}
              value={config?.tx_power_dbm ?? 0}
              onChange={(e) => patchConfig({ tx_power_dbm: Number(e.target.value) })}
            />
          </label>

          <label>
            信噪比 (SNR dB): {config?.snr_db ?? 15}
            <input
              type="range"
              min={-5}
              max={40}
              step={1}
              value={config?.snr_db ?? 15}
              onChange={(e) => patchConfig({ snr_db: Number(e.target.value) })}
            />
          </label>

          <label>
            雜訊強度: {config?.noise_level?.toFixed(2) ?? "0.20"}
            <input
              type="range"
              min={0}
              max={1}
              step={0.05}
              value={config?.noise_level ?? 0.2}
              onChange={(e) => patchConfig({ noise_level: Number(e.target.value) })}
            />
          </label>

          <label>
            濾波器頻寬 (MHz): {config?.filter_bw_mhz ?? 1}
            <input
              type="range"
              min={0.5}
              max={4}
              step={0.5}
              value={config?.filter_bw_mhz ?? 1}
              onChange={(e) => patchConfig({ filter_bw_mhz: Number(e.target.value) })}
            />
          </label>

          <label>
            判定閾值: {config?.threshold?.toFixed(2) ?? "0.75"}
            <input
              type="range"
              min={0.1}
              max={1.2}
              step={0.05}
              value={config?.threshold ?? 0.75}
              onChange={(e) => patchConfig({ threshold: Number(e.target.value) })}
            />
          </label>

          <label className="toggle">
            <input
              type="checkbox"
              checked={config?.replay_guard ?? true}
              onChange={(e) => patchConfig({ replay_guard: e.target.checked })}
            />
            序列號重放校驗
          </label>
        </div>

        <button className="trigger-btn" onClick={onTrigger} disabled={busy} type="button">
          {busy ? "發送中…" : "發送布林指令"}
        </button>
      </div>
    </div>
  );
}
