"use client";

import type { BitAnalysis } from "@/lib/types";

interface BitCompareProps {
  bits: BitAnalysis;
  packetOk: boolean;
  crcOk: boolean;
}

export function BitCompare({ bits, packetOk, crcOk }: BitCompareProps) {
  const status = packetOk
    ? "✅ 封包完整"
    : !crcOk
      ? "❌ 封包損壞 (CRC Error)"
      : "❌ 位元錯誤";

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
      <div className="bit-line">
        <span className="label">原始位元：</span>
        {renderBits(bits.original, false)}
      </div>
      <div className="bit-line">
        <span className="label">還原位元：</span>
        {renderBits(bits.recovered, true)}
      </div>
      <div className="bit-status">{status}</div>
      <div className="ber">誤碼率 (BER)：{(bits.ber * 100).toFixed(0)}%</div>
    </div>
  );
}
