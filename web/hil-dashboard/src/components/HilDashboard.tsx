"use client";

import { useCallback, useEffect, useState } from "react";
import {
  fetchStatus,
  triggerCommand,
  updateConfig,
  wsUrl,
} from "@/lib/api";
import { dictionaries, type Dictionary, type Locale } from "@/lib/i18n";
import type { Kpis, PipelineSnapshot, SimConfig, TelemetryEvent } from "@/lib/types";
import { BitCompare } from "./BitCompare";
import { LiveHardwarePanel } from "./LiveHardwarePanel";
import { PipelineFlow } from "./PipelineFlow";
import { WaveformPanel } from "./WaveformPanel";

type Tab = "hil" | "ook" | "bits" | "live";

const MODE_OPTIONS: { value: SimConfig["mode"] }[] = [
  { value: "EspNow" },
  { value: "BleAdvertisement" },
  { value: "Ook433Mhz" },
];

function KpiCard({
  explanation,
  value,
  alert = false,
}: {
  explanation: { title: string; body: string };
  value: string;
  alert?: boolean;
}) {
  return (
    <div className={alert ? "kpi alert" : "kpi"}>
      <div className="kpi-label">{explanation.title}</div>
      <div className="kpi-value">{value}</div>
      <p>{explanation.body}</p>
    </div>
  );
}

function ConceptGuide({ concepts }: { concepts: Dictionary["concepts"] }) {
  return (
    <div className="concept-grid">
      {concepts.map((concept) => (
        <div className="concept-item" key={concept.title}>
          <strong>{concept.title}</strong>
          <span>{concept.body}</span>
        </div>
      ))}
    </div>
  );
}

export function HilDashboard() {
  const [tab, setTab] = useState<Tab>("hil");
  const [locale, setLocale] = useState<Locale>("zh-Hant");
  const [config, setConfig] = useState<SimConfig | null>(null);
  const [kpis, setKpis] = useState<Kpis | null>(null);
  const [events, setEvents] = useState<TelemetryEvent[]>([]);
  const [snapshot, setSnapshot] = useState<PipelineSnapshot | null>(null);
  const [connected, setConnected] = useState(false);
  const [busy, setBusy] = useState(false);
  const [backendError, setBackendError] = useState<string | null>(null);
  const t = dictionaries[locale];

  const applySnapshot = useCallback((snap: PipelineSnapshot) => {
    setSnapshot(snap);
    setKpis(snap.kpis);
    setEvents((prev) => [snap.event, ...prev].slice(0, 50));
    setBackendError(null);
  }, []);

  useEffect(() => {
    fetchStatus()
      .then((s) => {
        setConfig(s.config);
        setKpis(s.kpis);
        setBackendError(null);
      })
      .catch(() => {
        setBackendError(dictionaries[locale].backendFetchError);
      });
  }, [locale]);

  useEffect(() => {
    const url = wsUrl();
    if (!url) return;
    const ws = new WebSocket(url);
    ws.onopen = () => {
      setConnected(true);
      setBackendError(null);
    };
    ws.onclose = () => setConnected(false);
    ws.onerror = () => {
      setBackendError(dictionaries[locale].backendWsError);
    };
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
    return () => ws.close();
  }, [applySnapshot, locale]);

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
    } catch {
      setBackendError(t.triggerError);
    } finally {
      setBusy(false);
    }
  };

  const wf = snapshot?.waveforms;

  return (
    <div className="dashboard" lang={locale}>
      <header className="header">
        <div>
          <h1>{t.title}</h1>
          <p className="subtitle">
            {t.subtitle}
            <span className={`ws-dot ${connected ? "on" : ""}`} />
            {connected ? t.connected : t.connecting}
          </p>
        </div>
        <div className="header-actions">
          <label className="language-picker">
            {t.languageToggle}
            <select
              value={locale}
              onChange={(event) => setLocale(event.target.value as Locale)}
            >
              <option value="zh-Hant">{dictionaries["zh-Hant"].languageName}</option>
              <option value="en">{dictionaries.en.languageName}</option>
            </select>
          </label>
          <div className="tabs">
            {(
              [
                ["hil", t.tabs.hil],
                ["ook", t.tabs.ook],
                ["bits", t.tabs.bits],
                ["live", t.tabs.live],
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
        </div>
      </header>

      {backendError && (
        <div className="backend-error panel" role="alert">
          {backendError}
        </div>
      )}

      {tab === "live" && <LiveHardwarePanel copy={t.live} />}

      {tab === "hil" && (
        <>
          <PipelineFlow snapshot={snapshot} copy={t.pipeline} title={t.sections.flow} />

          <div className="panel events-panel">
            <h3>{t.sections.events}</h3>
            <p className="panel-note">{t.events.intro}</p>
            <table>
              <thead>
                <tr>
                  <th>{t.events.time}</th>
                  <th>{t.events.node}</th>
                  <th>{t.events.payload}</th>
                  <th>{t.events.rssi}</th>
                  <th>{t.events.status}</th>
                </tr>
              </thead>
              <tbody>
                {events.length ? (
                  events.map((e, i) => (
                    <tr key={`${e.seq}-${i}`}>
                      <td>{e.time}</td>
                      <td>{t.events.nodePrefix} {e.node_id}</td>
                      <td className="mono">{e.payload_json}</td>
                      <td>{e.rssi_dbm.toFixed(1)} dBm</td>
                      <td>{e.status}</td>
                    </tr>
                  ))
                ) : (
                  <tr>
                    <td colSpan={5}>{t.events.empty}</td>
                  </tr>
                )}
              </tbody>
            </table>
          </div>

          <h3 className="section-title">{t.sections.kpis}</h3>
          <div className="kpi-row">
            <KpiCard
              explanation={t.kpis.prr}
              value={`${kpis?.prr_percent.toFixed(1) ?? "-"}%`}
            />
            <KpiCard
              explanation={t.kpis.latency}
              value={`${kpis?.latency_ms ?? "-"} ms`}
            />
            <KpiCard
              explanation={t.kpis.bool}
              value={String(kpis?.last_bool ?? false)}
            />
            <KpiCard
              explanation={t.kpis.alerts}
              value={String(kpis?.security_alerts ?? 0)}
              alert
            />
          </div>
          <ConceptGuide concepts={t.concepts} />
        </>
      )}

      {tab === "ook" && (
        <>
          <h3 className="section-title">{t.sections.ook}</h3>
          <div className="charts-grid">
            <WaveformPanel
              title={t.charts.baseband.title}
              description={t.charts.baseband.body}
              data={wf?.baseband ?? []}
              color="#4da3ff"
              yDomain={[-0.2, 1.2]}
              unit="Level"
            />
            <WaveformPanel
              title={t.charts.rfTx.title}
              description={t.charts.rfTx.body}
              data={wf?.rf_tx ?? []}
              color="#3ecf8e"
              yDomain={[-1.2, 1.2]}
              unit="Amp"
            />
            <WaveformPanel
              title={t.charts.rfRx.title}
              description={t.charts.rfRx.body}
              data={wf?.rf_rx ?? []}
              color="#f5a623"
              yDomain={[-2, 2]}
              unit="Amp"
            />
            <WaveformPanel
              title={t.charts.magnitude.title}
              description={t.charts.magnitude.body}
              data={wf?.magnitude ?? []}
              color="#e85d5d"
              threshold={wf?.threshold ?? config?.threshold ?? 0.75}
              thresholdLabel={t.charts.threshold}
              yDomain={[0, 1.5]}
              unit="Mag"
            />
          </div>
        </>
      )}

      {tab === "bits" && snapshot && (
        <>
          <WaveformPanel
            title={t.charts.bitMagnitude.title}
            description={t.charts.bitMagnitude.body}
            data={wf?.magnitude ?? []}
            color="#e85d5d"
            threshold={wf?.threshold ?? 0.75}
            thresholdLabel={t.charts.threshold}
            yDomain={[0, 1.5]}
            unit="Mag"
          />
          <BitCompare
            bits={snapshot.bits}
            packetOk={snapshot.packet_ok}
            crcOk={snapshot.crc_ok}
            copy={t.bitCompare}
          />
        </>
      )}

      {tab === "bits" && !snapshot && (
        <div className="panel">
          <h3>{t.sections.bits}</h3>
          <p className="panel-note">{t.bitCompare.empty}</p>
        </div>
      )}

      {tab !== "live" && (
      <div className="control-panel panel">
        <h3>{t.sections.controls}</h3>
        <p className="panel-note">{t.controls.intro}</p>
        <div className="controls">
          <label>
            <span>{t.controls.mode.title}</span>
            <select
              value={config?.mode ?? "EspNow"}
              onChange={(e) =>
                patchConfig({ mode: e.target.value as SimConfig["mode"] })
              }
            >
              {MODE_OPTIONS.map((o) => (
                <option key={o.value} value={o.value}>
                  {t.controls.modeOptions[o.value]}
                </option>
              ))}
            </select>
            <small>{t.controls.mode.body}</small>
          </label>

          <label>
            <span>{t.controls.dataBits.title}</span>
            <input
              type="text"
              value={config?.data_bits ?? "10110010"}
              onChange={(e) => patchConfig({ data_bits: e.target.value })}
              maxLength={8}
              className="mono"
            />
            <small>{t.controls.dataBits.body}</small>
          </label>

          <label>
            <span>{t.controls.txPower.title}: {config?.tx_power_dbm ?? 0}</span>
            <input
              type="range"
              min={-10}
              max={10}
              step={1}
              value={config?.tx_power_dbm ?? 0}
              onChange={(e) => patchConfig({ tx_power_dbm: Number(e.target.value) })}
            />
            <small>{t.controls.txPower.body}</small>
          </label>

          <label>
            <span>{t.controls.snr.title}: {config?.snr_db ?? 15}</span>
            <input
              type="range"
              min={-5}
              max={40}
              step={1}
              value={config?.snr_db ?? 15}
              onChange={(e) => patchConfig({ snr_db: Number(e.target.value) })}
            />
            <small>{t.controls.snr.body}</small>
          </label>

          <label>
            <span>{t.controls.noise.title}: {config?.noise_level?.toFixed(2) ?? "0.20"}</span>
            <input
              type="range"
              min={0}
              max={1}
              step={0.05}
              value={config?.noise_level ?? 0.2}
              onChange={(e) => patchConfig({ noise_level: Number(e.target.value) })}
            />
            <small>{t.controls.noise.body}</small>
          </label>

          <label>
            <span>{t.controls.filter.title}: {config?.filter_bw_mhz ?? 1}</span>
            <input
              type="range"
              min={0.5}
              max={4}
              step={0.5}
              value={config?.filter_bw_mhz ?? 1}
              onChange={(e) => patchConfig({ filter_bw_mhz: Number(e.target.value) })}
            />
            <small>{t.controls.filter.body}</small>
          </label>

          <label>
            <span>{t.controls.threshold.title}: {config?.threshold?.toFixed(2) ?? "0.75"}</span>
            <input
              type="range"
              min={0.1}
              max={1.2}
              step={0.05}
              value={config?.threshold ?? 0.75}
              onChange={(e) => patchConfig({ threshold: Number(e.target.value) })}
            />
            <small>{t.controls.threshold.body}</small>
          </label>

          <label className="toggle">
            <input
              type="checkbox"
              checked={config?.replay_guard ?? true}
              onChange={(e) => patchConfig({ replay_guard: e.target.checked })}
            />
            <span>{t.controls.replayGuard.title}</span>
            <small>{t.controls.replayGuard.body}</small>
          </label>
        </div>

        <button className="trigger-btn" onClick={onTrigger} disabled={busy} type="button">
          {busy ? t.controls.sending : t.controls.send}
        </button>
      </div>
      )}
    </div>
  );
}
