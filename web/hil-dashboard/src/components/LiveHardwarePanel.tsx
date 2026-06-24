"use client";

import { useCallback, useEffect, useRef, useState } from "react";
import {
  fetchLiveEvents,
  fetchLiveStatus,
  liveStreamUrl,
  probeEdgeHealth,
  probeControlPlaneHealth,
} from "@/lib/api";
import type { Dictionary } from "@/lib/i18n";
import type { LiveEvent, LiveStatus } from "@/lib/types";

function formatTime(tsMs: number): string {
  const d = new Date(tsMs);
  return d.toLocaleTimeString(undefined, {
    hour12: false,
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
    fractionalSecondDigits: 3,
  } as Intl.DateTimeFormatOptions);
}

function levelClass(level: string): string {
  if (level === "action") return "log-action";
  if (level === "warn") return "log-warn";
  return "log-info";
}

export function LiveHardwarePanel({ copy }: { copy: Dictionary["live"] }) {
  const [events, setEvents] = useState<LiveEvent[]>([]);
  const [status, setStatus] = useState<LiveStatus | null>(null);
  const [edgeOk, setEdgeOk] = useState(false);
  const [cpOk, setCpOk] = useState(false);
  const [streamOk, setStreamOk] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const logRef = useRef<HTMLDivElement>(null);
  const seenRef = useRef<Set<string>>(new Set());

  const pushEvent = useCallback((ev: LiveEvent) => {
    const key = `${ev.ts_ms}-${ev.message}`;
    if (seenRef.current.has(key)) return;
    seenRef.current.add(key);
    setEvents((prev) => [ev, ...prev].slice(0, 200));
  }, []);

  useEffect(() => {
    let cancelled = false;

    const pollHealth = async () => {
      const [edge, cp] = await Promise.all([
        probeEdgeHealth(),
        probeControlPlaneHealth(),
      ]);
      if (cancelled) return;
      setEdgeOk(edge);
      setCpOk(cp);
      if (!edge && !cp) {
        setError(copy.backendError);
      } else {
        setError(null);
      }
    };

    const pollStatus = async () => {
      try {
        const s = await fetchLiveStatus();
        if (!cancelled) setStatus(s);
      } catch {
        if (!cancelled) setCpOk(false);
      }
    };

    pollHealth();
    pollStatus();
    const healthTimer = setInterval(() => {
      pollHealth();
      pollStatus();
    }, 3000);

    return () => {
      cancelled = true;
      clearInterval(healthTimer);
    };
  }, [copy.backendError]);

  useEffect(() => {
    fetchLiveEvents()
      .then((initial) => {
        seenRef.current.clear();
        initial.forEach((ev) => {
          seenRef.current.add(`${ev.ts_ms}-${ev.message}`);
        });
        setEvents(initial.slice(0, 200));
      })
      .catch(() => setError(copy.backendError));

    const url = liveStreamUrl();
    if (!url) return;

    const es = new EventSource(url);
    es.onopen = () => {
      setStreamOk(true);
      setError(null);
    };
    es.onerror = () => {
      setStreamOk(false);
    };
    es.onmessage = (msg) => {
      try {
        const ev = JSON.parse(msg.data) as LiveEvent;
        pushEvent(ev);
      } catch {
        // ignore malformed events
      }
    };

    return () => es.close();
  }, [copy.backendError, pushEvent]);

  const connected = edgeOk && cpOk && streamOk;

  return (
    <>
      <div className="live-header panel">
        <div>
          <h3>{copy.title}</h3>
          <p className="panel-note">{copy.intro}</p>
        </div>
        <div className="live-status-row">
          <span className={`live-pill ${edgeOk ? "ok" : ""}`}>
            {copy.edge}: {edgeOk ? copy.up : copy.down}
          </span>
          <span className={`live-pill ${cpOk ? "ok" : ""}`}>
            {copy.controlPlane}: {cpOk ? copy.up : copy.down}
          </span>
          <span className={`ws-dot ${connected ? "on" : ""}`} />
          <span className="live-stream-label">
            {connected ? copy.streamConnected : copy.streamConnecting}
          </span>
        </div>
      </div>

      {error && (
        <div className="backend-error panel" role="alert">
          {error}
        </div>
      )}

      <div className="kpi-row">
        <div className="kpi">
          <div className="kpi-label">{copy.framesDecoded}</div>
          <div className="kpi-value">{status?.frames_decoded ?? "-"}</div>
        </div>
        <div className="kpi">
          <div className="kpi-label">{copy.eventsBuffered}</div>
          <div className="kpi-value">{status?.events_buffered ?? events.length}</div>
        </div>
        <div className={`kpi ${status?.last_action ? "alert" : ""}`}>
          <div className="kpi-label">{copy.lastAction}</div>
          <div className="kpi-value">
            {status?.last_action
              ? `node ${status.last_action.node_id} seq ${status.last_action.seq}`
              : copy.noActionYet}
          </div>
        </div>
      </div>

      <div className="panel live-instructions">
        <h3>{copy.instructionsTitle}</h3>
        <ol>
          {copy.instructions.map((line) => (
            <li key={line}>{line}</li>
          ))}
        </ol>
      </div>

      <div className="panel live-log-panel">
        <h3>{copy.logTitle}</h3>
        <p className="panel-note">{copy.logIntro}</p>
        <div className="live-log" ref={logRef}>
          {events.length ? (
            events.map((ev, i) => (
              <div className={`live-log-line ${levelClass(ev.level)}`} key={`${ev.ts_ms}-${i}`}>
                <span className="live-log-time">{formatTime(ev.ts_ms)}</span>
                <span className="live-log-level">{ev.level.toUpperCase()}</span>
                <span className="live-log-msg mono">{ev.message}</span>
              </div>
            ))
          ) : (
            <p className="panel-note">{copy.logEmpty}</p>
          )}
        </div>
      </div>
    </>
  );
}
