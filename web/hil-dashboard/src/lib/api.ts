import type { PipelineSnapshot, SimConfig, TelemetryEvent, Kpis } from "./types";

const API_BASE = "";

export async function fetchStatus(): Promise<{
  hardware_mode: string;
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
