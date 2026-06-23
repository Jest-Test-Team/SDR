"use client";

import type { PipelineSnapshot } from "@/lib/types";
import type { Dictionary } from "@/lib/i18n";

interface PipelineFlowProps {
  snapshot: PipelineSnapshot | null;
  copy: Dictionary["pipeline"];
  title: string;
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

export function PipelineFlow({ snapshot, copy, title }: PipelineFlowProps) {
  const wf = snapshot?.waveforms;
  const waveforms = [
    wf?.baseband ?? [],
    wf?.rf_tx ?? [],
    wf?.rf_rx ?? [],
    wf?.magnitude ?? [],
    [],
  ];
  const iconClasses = ["esp", "rf", "sdr", "zmq", "cp"];
  const colors = ["#3ecf8e", "#f5a623", "#e8a838", "#4da3ff", "#b388ff"];

  return (
    <div className="pipeline-flow panel">
      <h3>{title}</h3>
      <div className="flow-track">
        {copy.nodes.map((node, index) => (
          <div className="flow-step" key={node.label}>
            <div className="flow-node">
              <div className={`node-icon ${iconClasses[index]}`}>{node.icon}</div>
              <div className="node-label">{node.label}</div>
              <div className="node-sub">{node.sub}</div>
              <MiniWave data={waveforms[index]} color={colors[index]} />
              <p>{node.explanation}</p>
            </div>
            {index < copy.nodes.length - 1 && <div className="flow-arrow">→</div>}
          </div>
        ))}
      </div>
    </div>
  );
}
