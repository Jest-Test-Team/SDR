export default function AboutRoute() {
  return (
    <div className="dashboard">
      <header className="header">
        <div>
          <h1>架構 / Architecture</h1>
          <p className="subtitle">Secure Telemetry Gateway — Rust end-to-end</p>
        </div>
      </header>

      <div className="panel">
        <h3>拓樸 / Topology</h3>
        <pre className="mono">{`Mac (Orchestrator, mTLS termination)
   |  TLS 1.3 / mTLS
   v
ESP32-S3 Gateway (AP-STA)
   |  isolated Wi-Fi + simulated SNMP (JSON)
   v
ESP32 Downstream Endpoint (STA)`}</pre>
      </div>

      <div className="panel">
        <h3>角色 / Roles</h3>
        <ul>
          <li>
            <strong>Mac Central Orchestrator</strong> — TLS termination, command issuer,
            telemetry ingestion server (control-plane).
          </li>
          <li>
            <strong>ESP32-S3 Gateway</strong> — AP-STA edge gateway: routes commands, enforces
            access control (deauth/MAC filter), aggregates downstream data, reports system health.
          </li>
          <li>
            <strong>ESP32 Downstream Endpoint</strong> — STA node on the isolated network, runs a
            lightweight server answering simulated SNMP set/get payloads.
          </li>
        </ul>
      </div>

      <div className="panel">
        <h3>指令 / Command Set</h3>
        <table>
          <thead>
            <tr>
              <th>Command</th>
              <th>Effect</th>
            </tr>
          </thead>
          <tbody>
            <tr><td className="mono">CMD_NET_TOGGLE_DOWNSTREAM</td><td>Sever/restore downstream AP (ApSta ↔ Sta)</td></tr>
            <tr><td className="mono">CMD_SNMP_SET</td><td>Write a simulated OID on the endpoint</td></tr>
            <tr><td className="mono">CMD_SNMP_GET</td><td>Read a simulated OID from the endpoint</td></tr>
            <tr><td className="mono">CMD_DEAUTH_STA</td><td>Kick a station by MAC</td></tr>
            <tr><td className="mono">CMD_SYS_HEALTH</td><td>Report free heap / link / station count</td></tr>
            <tr><td className="mono">CMD_REGISTER_NODE</td><td>Simulate an endpoint joining the AP</td></tr>
          </tbody>
        </table>
      </div>
    </div>
  );
}
