import type { PipelineSnapshot, SimConfig, TelemetryEvent, Kpis, LiveEvent, LiveStatus, FirmwareConfigResponse, SidecarTransport, GatewaySnapshot, GatewayCommand, GatewayResponse, GatewayStatus } from "./types";

const API_BASE = "";

export async function fetchStatus(): Promise<{
  hardware_mode: string;
  sidecar_transport?: SidecarTransport;
  kpis: Kpis;
  config: SimConfig;
}> {
  const res = await fetch(`${API_BASE}/api/v1/status`);
  if (!res.ok) throw new Error("status fetch failed");
  return res.json();
}

export async function updateConfig(config: SimConfig): Promise<SimConfig> {
  const res = await fetch(`${API_BASE}/api/v1/config`, {
    method: "PUT",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(config),
  });
  if (!res.ok) throw new Error("config update failed");
  return res.json();
}

export async function triggerCommand(value: boolean): Promise<PipelineSnapshot> {
  const res = await fetch(`${API_BASE}/api/v1/trigger`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ value }),
  });
  if (!res.ok) throw new Error("trigger failed");
  const body = await res.json();
  return body.snapshot as PipelineSnapshot;
}

export async function fetchEvents(): Promise<TelemetryEvent[]> {
  const res = await fetch(`${API_BASE}/api/v1/events`);
  if (!res.ok) throw new Error("events fetch failed");
  return res.json();
}

export function wsUrl(): string {
  const env = process.env.NEXT_PUBLIC_HIL_WS_URL;
  if (env) return env;
  if (typeof window !== "undefined") {
    return "ws://127.0.0.1:8090/ws/live";
  }
  return "";
}

export async function fetchLiveEvents(): Promise<LiveEvent[]> {
  const res = await fetch(`${API_BASE}/api/v1/live/events`, { cache: "no-store" });
  if (!res.ok) throw new Error("live events fetch failed");
  return res.json();
}

export async function fetchLiveStatus(): Promise<LiveStatus> {
  const res = await fetch(`${API_BASE}/api/v1/live/status`, { cache: "no-store" });
  if (!res.ok) throw new Error("live status fetch failed");
  return res.json();
}

export function liveStreamUrl(): string {
  if (typeof window !== "undefined") {
    return `${window.location.origin}/api/v1/live/stream`;
  }
  return "";
}

export async function probeEdgeHealth(): Promise<boolean> {
  try {
    const res = await fetch(`${API_BASE}/live/edge/health`);
    return res.ok;
  } catch {
    return false;
  }
}

export async function probeControlPlaneHealth(): Promise<boolean> {
  try {
    const res = await fetch(`${API_BASE}/live/cp/health`);
    return res.ok;
  } catch {
    return false;
  }
}

export async function fetchGateway(): Promise<GatewaySnapshot> {
  const res = await fetch(`${API_BASE}/api/v1/gateway`, { cache: "no-store" });
  if (!res.ok) throw new Error("gateway fetch failed");
  return res.json();
}

export async function fetchGatewayStatus(): Promise<GatewayStatus> {
  const res = await fetch(`${API_BASE}/api/v1/gateway/status`, { cache: "no-store" });
  if (!res.ok) throw new Error("gateway status fetch failed");
  return res.json();
}

export function gatewayWsUrl(): string {
  const env = process.env.NEXT_PUBLIC_HIL_WS_URL;
  if (env) return env.replace(/\/ws\/live$/, "/ws/gateway");
  if (typeof window !== "undefined") {
    return "ws://127.0.0.1:8090/ws/gateway";
  }
  return "";
}

export async function sendGatewayCommand(command: GatewayCommand): Promise<GatewayResponse> {
  const res = await fetch(`${API_BASE}/api/v1/gateway/command`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(command),
  });
  if (!res.ok) throw new Error("gateway command failed");
  const body = await res.json();
  return body.response as GatewayResponse;
}

export async function applyFirmwareConfig(config: SimConfig): Promise<FirmwareConfigResponse> {
  const res = await fetch(`${API_BASE}/api/v1/firmware/config`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(config),
  });
  if (!res.ok) throw new Error("firmware config update failed");
  return res.json();
}
