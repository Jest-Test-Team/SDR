"use client";

import Link from "next/link";
import { usePathname, useRouter } from "next/navigation";
import { useCallback, useEffect, useState } from "react";

/**
 * Toggle between the two board pipelines (each owns the single S3 USB port).
 *
 * If the local `pipelinectl` control service is running, the button actually
 * restarts the backend into the target pipeline, then navigates. If it isn't,
 * the button degrades to plain navigation and shows the `up.sh` command to run.
 */
export function PipelineSwitch() {
  const pathname = usePathname();
  const router = useRouter();
  const [ctlUp, setCtlUp] = useState(false);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    let alive = true;
    fetch("/api/control/status", { cache: "no-store" })
      .then((r) => (r.ok ? r.json() : Promise.reject()))
      .then(() => alive && setCtlUp(true))
      .catch(() => alive && setCtlUp(false));
    return () => {
      alive = false;
    };
  }, [pathname]);

  const onGateway = pathname.startsWith("/gateway");
  const target = onGateway
    ? { href: "/", pipeline: "telemetry", label: "切換到即時遙測 / Live Telemetry", cmd: "./scripts/up.sh --telemetry" }
    : { href: "/gateway", pipeline: "gateway", label: "切換到安全閘道 / Secure Gateway", cmd: "./scripts/up.sh" };

  const switchPipeline = useCallback(async () => {
    setBusy(true);
    try {
      await fetch("/api/control/switch", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ pipeline: target.pipeline }),
      });
    } catch {
      /* ignore — navigate anyway */
    }
    // Give the backend a moment to start, then navigate to the matching page.
    setTimeout(() => {
      setBusy(false);
      router.push(target.href);
    }, 1500);
  }, [router, target.href, target.pipeline]);

  if (pathname.startsWith("/about")) return null;

  // No control service: plain navigation + the command to run by hand.
  if (!ctlUp) {
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

  return (
    <button
      type="button"
      className="pipeline-switch"
      disabled={busy}
      onClick={switchPipeline}
      title={`透過 pipelinectl 重啟後端 / restarts backend via pipelinectl (${target.pipeline})`}
    >
      <span className="pipeline-switch-label">
        {busy ? "切換中… / switching…" : `⇄ ${target.label}`}
      </span>
      <code className="pipeline-switch-cmd">live switch · pipelinectl</code>
    </button>
  );
}
