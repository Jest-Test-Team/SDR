"use client";

import {
  CartesianGrid,
  Line,
  LineChart,
  ReferenceLine,
  ResponsiveContainer,
  XAxis,
  YAxis,
} from "recharts";

interface WaveformPanelProps {
  title: string;
  description?: string;
  data: number[];
  color: string;
  yDomain?: [number, number];
  threshold?: number;
  thresholdLabel?: string;
  unit?: string;
}

export function WaveformPanel({
  title,
  description,
  data,
  color,
  yDomain,
  threshold,
  thresholdLabel = "",
  unit = "",
}: WaveformPanelProps) {
  const chartData = data.map((v, i) => ({ i, v }));

  return (
    <div className="panel waveform-panel">
      <h3>{title}</h3>
      {description && <p className="panel-note">{description}</p>}
      <ResponsiveContainer width="100%" height={180}>
        <LineChart data={chartData} margin={{ top: 8, right: 8, left: 0, bottom: 0 }}>
          <CartesianGrid strokeDasharray="3 3" stroke="#333" />
          <XAxis dataKey="i" tick={{ fill: "#888", fontSize: 10 }} />
          <YAxis
            domain={yDomain ?? ["auto", "auto"]}
            tick={{ fill: "#888", fontSize: 10 }}
            label={{
              value: unit,
              angle: -90,
              position: "insideLeft",
              fill: "#888",
              fontSize: 10,
            }}
          />
          {threshold !== undefined && (
            <ReferenceLine
              y={threshold}
              stroke="#4da3ff"
              strokeDasharray="6 4"
              label={{
                value: thresholdLabel ? `${thresholdLabel}: ${threshold}` : threshold,
                fill: "#4da3ff",
                fontSize: 10,
              }}
            />
          )}
          <Line
            type="monotone"
            dataKey="v"
            stroke={color}
            dot={false}
            strokeWidth={1.5}
            isAnimationActive={false}
          />
        </LineChart>
      </ResponsiveContainer>
    </div>
  );
}
