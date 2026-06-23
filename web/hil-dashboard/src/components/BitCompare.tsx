"use client";

import type { BitAnalysis } from "@/lib/types";
import type { Dictionary } from "@/lib/i18n";

interface BitCompareProps {
  bits: BitAnalysis;
  packetOk: boolean;
  crcOk: boolean;
  copy: Dictionary["bitCompare"];
}

export function BitCompare({ bits, packetOk, crcOk, copy }: BitCompareProps) {
  const status = packetOk
    ? copy.packetComplete
    : !crcOk
      ? copy.crcError
      : copy.bitError;

  const renderBits = (text: string, highlightErrors: boolean) => (
    <span className="bit-row">
      {text.split("").map((ch, idx) => (
        <span
          key={idx}
          className={
            highlightErrors && bits.error_indices.includes(idx) ? "bit-error" : "bit-ok"
          }
        >
          {ch}
        </span>
      ))}
    </span>
  );

  return (
    <div className="panel bit-panel">
      <h3>{copy.explanation.title}</h3>
      <p className="panel-note">{copy.explanation.body}</p>
      <div className="bit-line">
        <span className="label">{copy.original}</span>
        {renderBits(bits.original, false)}
      </div>
      <div className="bit-line">
        <span className="label">{copy.recovered}</span>
        {renderBits(bits.recovered, true)}
      </div>
      <div className="bit-status">{status}</div>
      <div className="ber">{copy.ber} {(bits.ber * 100).toFixed(0)}%</div>
    </div>
  );
}
