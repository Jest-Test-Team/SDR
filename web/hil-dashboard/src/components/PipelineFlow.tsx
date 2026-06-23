"use client";

import type { PipelineSnapshot } from "@/lib/types";
import { WaveformPanel } from "./WaveformPanel";

interface PipelineFlowProps {
  snapshot: PipelineSnapshot | null;
}

function MiniWave({ data, color }: { data: number[]; color: string }) {
  if (!data.length) return <svg viewBox="0 0 80 24" className="mini-wave" />;
  const max = Math.max(...data.map(Math.abs), 0.01);
  const points = data
    .filter((_, i) => i % Math.ceil(data.length / 40) === 0)
    .map((v, i, arr) => {
      const x = (i / Math.max(arr.length - 1, 1)) * 80;
      const y = 12 - (v / max) * 10;
      return `${x},${y}`;
    })
    .join(" ");
  return (
    <svg viewBox="0 0 80 24" className="mini-wave">
      <polyline fill="none" stroke={color} strokeWidth="1.5" points={points} />
    </svg>
  );
}

export function PipelineFlow({ snapshot }: PipelineFlowProps) {
  const wf = snapshot?.waveforms;

  return (
    <div className="pipeline-flow panel">
      <div className="flow-node">
        <div className="node-icon esp">S3</div>
        <div className="node-label">ESP32-S3</div>
        <div className="node-sub">原始指令</div>
        <MiniWave data={wf?.baseband ?? []} color="#3ecf8e" />
      </div>
      <div className="flow-arrow">→</div>
      <div className="flow-node">
        <div className="node-icon rf">RF</div>
        <div className="node-label">RF 空間</div>
        <div className="node-sub">射頻調變 (+雜訊)</div>
        <MiniWave data={wf?.rf_tx ?? []} color="#f5a623" />
      </div>
      <div className="flow-arrow">→</div>
      <div className="flow-node">
        <div className="node-icon sdr">SDR</div>
        <div className="node-label">SDR 接收</div>
        <div className="node-sub">RTL-SDR 模擬</div>
        <MiniWave data={wf?.rf_rx ?? []} color="#e8a838" />
      </div>
      <div className="flow-arrow">→</div>
      <div className="flow-node">
        <div className="node-icon zmq">ZMQ</div>
        <div className="node-label">ZMQ 管道</div>
        <div className="node-sub">還原結果</div>
        <MiniWave data={wf?.magnitude ?? []} color="#4da3ff" />
      </div>
      <div className="flow-arrow">→</div>
      <div className="flow-node">
        <div className="node-icon cp">CP</div>
        <div className="node-label">控制層端</div>
        <div className="node-sub">規則引擎</div>
      </div>
    </div>
  );
}
