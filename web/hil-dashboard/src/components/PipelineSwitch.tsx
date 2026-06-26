"use client";

import Link from "next/link";
import { usePathname } from "next/navigation";

/**
 * Prominent toggle between the two board pipelines. Each dashboard view is
 * backed by a different `up.sh` pipeline that owns the single S3 USB port, so
 * the button navigates to the other view AND surfaces the command that brings
 * its backend up (the live pipeline swap itself requires restarting up.sh).
 */
export function PipelineSwitch() {
  const pathname = usePathname();
  if (pathname.startsWith("/about")) return null;

  const onGateway = pathname.startsWith("/gateway");
  const target = onGateway
    ? {
        href: "/",
        label: "切換到即時遙測 / Live Telemetry",
        cmd: "./scripts/up.sh --telemetry",
      }
    : {
        href: "/gateway",
        label: "切換到安全閘道 / Secure Gateway",
        cmd: "./scripts/up.sh",
      };

  return (
    <Link
      href={target.href}
      className="pipeline-switch"
      title={`需要重啟後端管線 / needs backend restart: ${target.cmd}`}
    >
      <span className="pipeline-switch-label">⇄ {target.label}</span>
      <code className="pipeline-switch-cmd">{target.cmd}</code>
    </Link>
  );
}
