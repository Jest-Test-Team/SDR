import { NextRequest } from "next/server";
import { readFileSync } from "node:fs";

// Server-side proxy to the local pipelinectl service. The bearer token is read
// from a 0600 file written by pipelinectl and is NEVER exposed to the browser.
export const dynamic = "force-dynamic";

const CTL_URL = process.env.PIPELINECTL_URL ?? "http://127.0.0.1:8099";
const TOKEN_FILE = process.env.PIPELINECTL_TOKEN_FILE ?? "/tmp/sdr-pipelinectl.token";

function token(): string {
  try {
    return readFileSync(TOKEN_FILE, "utf8").trim();
  } catch {
    return "";
  }
}

async function proxy(request: NextRequest, path: string[], method: "GET" | "POST") {
  const tok = token();
  if (!tok) {
    return Response.json(
      { error: "pipelinectl not running (no token). Start it: ./scripts/pipelinectl.py" },
      { status: 503 },
    );
  }
  const upstream = new URL(`/${path.join("/")}`, CTL_URL);
  const init: RequestInit = {
    method,
    headers: { Authorization: `Bearer ${tok}`, "Content-Type": "application/json" },
    cache: "no-store",
  };
  if (method === "POST") {
    init.body = await request.text();
  }
  try {
    const res = await fetch(upstream, init);
    const text = await res.text();
    return new Response(text, {
      status: res.status,
      headers: { "content-type": res.headers.get("content-type") ?? "application/json" },
    });
  } catch {
    return Response.json({ error: "pipelinectl unreachable" }, { status: 503 });
  }
}

export async function GET(request: NextRequest, ctx: { params: { path: string[] } }) {
  return proxy(request, ctx.params.path, "GET");
}

export async function POST(request: NextRequest, ctx: { params: { path: string[] } }) {
  return proxy(request, ctx.params.path, "POST");
}
