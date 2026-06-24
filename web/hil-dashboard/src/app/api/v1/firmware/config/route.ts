import { NextRequest, NextResponse } from "next/server";

export const dynamic = "force-dynamic";

const LIVE_EDGE_URL = process.env.LIVE_EDGE_URL ?? "http://127.0.0.1:8081";

export async function POST(request: NextRequest) {
  const response = await fetch(`${LIVE_EDGE_URL}/api/v1/firmware/config`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: await request.text(),
    cache: "no-store",
  });

  return new NextResponse(await response.text(), {
    status: response.status,
    statusText: response.statusText,
    headers: {
      "content-type": response.headers.get("content-type") ?? "application/json",
      "cache-control": "no-store",
    },
  });
}
