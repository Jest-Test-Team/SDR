import { readFileSync } from "node:fs";

// Server-side proxy helper to the local pipelinectl service. The bearer token is
// read from a 0600 file written by pipelinectl and is NEVER sent to the browser.
const CTL_URL = process.env.PIPELINECTL_URL ?? "http://127.0.0.1:8099";
const TOKEN_FILE = process.env.PIPELINECTL_TOKEN_FILE ?? "/tmp/sdr-pipelinectl.token";

function token(): string {
  try {
    return readFileSync(TOKEN_FILE, "utf8").trim();
  } catch {
    return "";
  }
}

export async function ctlProxy(path: string, method: "GET" | "POST", body?: string) {
  const tok = token();
  if (!tok) {
    return Response.json(
      { error: "pipelinectl not running (no token). Start it: ./scripts/up.sh --control" },
      { status: 503 },
    );
  }
  try {
    const res = await fetch(new URL(path, CTL_URL), {
      method,
      headers: { Authorization: `Bearer ${tok}`, "Content-Type": "application/json" },
      body,
      cache: "no-store",
    });
    const text = await res.text();
    return new Response(text, {
      status: res.status,
      headers: { "content-type": res.headers.get("content-type") ?? "application/json" },
    });
  } catch {
    return Response.json({ error: "pipelinectl unreachable" }, { status: 503 });
  }
}
