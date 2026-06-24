"use client";

import { useCallback, useEffect, useState } from "react";
import { fetchGateway, sendGatewayCommand } from "@/lib/api";
import type { GatewayCommand, GatewayResponse, GatewaySnapshot } from "@/lib/types";

function heapPercent(snap: GatewaySnapshot): number {
  if (snap.heap_total_bytes === 0) return 0;
  return Math.round((snap.free_heap_bytes / snap.heap_total_bytes) * 100);
}

export function GatewayPanel() {
  const [snap, setSnap] = useState<GatewaySnapshot | null>(null);
  const [lastResponse, setLastResponse] = useState<GatewayResponse | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const [oid, setOid] = useState("1.3.6.1.4.1.custom.isolate");
  const [value, setValue] = useState("true");
  const [mac, setMac] = useState("AA:BB:CC:DD:EE:FF");
  const [ip, setIp] = useState("192.168.4.3");

  const refresh = useCallback(() => {
    fetchGateway()
      .then((s) => {
        setSnap(s);
        setError(null);
      })
      .catch(() => setError("無法連線到閘道 API / Cannot reach gateway API"));
  }, []);

  useEffect(() => {
    refresh();
  }, [refresh]);

  const run = async (command: GatewayCommand) => {
    setBusy(true);
    try {
      const resp = await sendGatewayCommand(command);
      setLastResponse(resp);
      setSnap(resp.snapshot);
      setError(null);
    } catch {
      setError("指令失敗 / Command failed");
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="dashboard">
      <header className="header">
        <div>
          <h1>安全遙測閘道 / Secure Telemetry Gateway</h1>
          <p className="subtitle">
            ESP32-S3 AP-STA 閘道：下行切換、模擬 SNMP、系統健康、MAC 過濾
          </p>
        </div>
      </header>

      {error && (
        <div className="backend-error panel" role="alert">
          {error}
        </div>
      )}

      <section className="kpi-row">
        <div className="kpi">
          <div className="kpi-label">Wi-Fi 模式 / Wi-Fi Mode</div>
          <div className="kpi-value">{snap?.wifi_mode ?? "-"}</div>
          <p>ap_sta = 上行+下行；sta = 下行已切斷</p>
        </div>
        <div className={snap?.downstream_online ? "kpi" : "kpi alert"}>
          <div className="kpi-label">下行鏈路 / Downstream</div>
          <div className="kpi-value">{snap ? (snap.downstream_online ? "ONLINE" : "OFFLINE") : "-"}</div>
          <p>隔離 AP 是否啟用</p>
        </div>
        <div className="kpi">
          <div className="kpi-label">可用記憶體 / Free Heap</div>
          <div className="kpi-value">{snap ? `${heapPercent(snap)}%` : "-"}</div>
          <p>{snap ? `${snap.free_heap_bytes} / ${snap.heap_total_bytes} bytes` : ""}</p>
        </div>
        <div className="kpi">
          <div className="kpi-label">指令數 / Commands</div>
          <div className="kpi-value">{snap?.command_count ?? 0}</div>
          <p>已處理的閘道指令</p>
        </div>
      </section>

      <div className="panel">
        <h3>閘道指令 / Gateway Commands</h3>
        <div className="controls">
          <button
            className="trigger-btn"
            type="button"
            disabled={busy}
            onClick={() => run({ command: "net_toggle_downstream" })}
          >
            CMD_NET_TOGGLE_DOWNSTREAM
          </button>
          <button
            className="trigger-btn secondary"
            type="button"
            disabled={busy}
            onClick={() => run({ command: "sys_health" })}
          >
            CMD_SYS_HEALTH
          </button>
        </div>

        <div className="controls" style={{ marginTop: "1rem" }}>
          <label>
            <span>OID</span>
            <input className="mono" value={oid} onChange={(e) => setOid(e.target.value)} />
          </label>
          <label>
            <span>Value</span>
            <input className="mono" value={value} onChange={(e) => setValue(e.target.value)} />
          </label>
          <div className="controls">
            <button
              className="trigger-btn"
              type="button"
              disabled={busy}
              onClick={() => run({ command: "snmp_set", oid, value })}
            >
              CMD_SNMP_SET
            </button>
            <button
              className="trigger-btn secondary"
              type="button"
              disabled={busy}
              onClick={() => run({ command: "snmp_get", oid })}
            >
              CMD_SNMP_GET
            </button>
          </div>
        </div>

        <div className="controls" style={{ marginTop: "1rem" }}>
          <label>
            <span>MAC</span>
            <input className="mono" value={mac} onChange={(e) => setMac(e.target.value)} />
          </label>
          <label>
            <span>IP</span>
            <input className="mono" value={ip} onChange={(e) => setIp(e.target.value)} />
          </label>
          <div className="controls">
            <button
              className="trigger-btn"
              type="button"
              disabled={busy}
              onClick={() => run({ command: "register_node", mac, ip })}
            >
              CMD_REGISTER_NODE
            </button>
            <button
              className="trigger-btn secondary"
              type="button"
              disabled={busy}
              onClick={() => run({ command: "deauth_sta", mac })}
            >
              CMD_DEAUTH_STA
            </button>
          </div>
        </div>
      </div>

      {lastResponse && (
        <div className="panel">
          <h3>最後回應 / Last Response</h3>
          <p className={lastResponse.ok ? "" : "backend-error"}>{lastResponse.message}</p>
          {lastResponse.snmp && (
            <pre className="mono">{JSON.stringify(lastResponse.snmp, null, 2)}</pre>
          )}
        </div>
      )}

      <div className="panel">
        <h3>下行端點 / Downstream Endpoints</h3>
        <table>
          <thead>
            <tr>
              <th>MAC</th>
              <th>IP</th>
              <th>Free Heap</th>
              <th>Status</th>
            </tr>
          </thead>
          <tbody>
            {snap?.nodes.length ? (
              snap.nodes.map((n) => (
                <tr key={n.mac}>
                  <td className="mono">{n.mac}</td>
                  <td className="mono">{n.ip}</td>
                  <td>{n.free_heap_bytes}</td>
                  <td>{n.online ? "online" : "offline"}</td>
                </tr>
              ))
            ) : (
              <tr>
                <td colSpan={4}>無連線端點 / No connected endpoints</td>
              </tr>
            )}
          </tbody>
        </table>
      </div>

      <div className="panel">
        <h3>模擬 MIB / Simulated MIB</h3>
        <table>
          <thead>
            <tr>
              <th>OID</th>
              <th>Value</th>
            </tr>
          </thead>
          <tbody>
            {snap?.oids.map((o) => (
              <tr key={o.oid}>
                <td className="mono">{o.oid}</td>
                <td className="mono">{o.value}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>

      <div className="panel">
        <h3>指令紀錄 / Command Log</h3>
        <ul className="command-log">
          {snap?.command_log.length ? (
            snap.command_log.map((line, i) => (
              <li key={i} className="mono">
                {line}
              </li>
            ))
          ) : (
            <li>尚無指令 / No commands yet</li>
          )}
        </ul>
      </div>
    </div>
  );
}
