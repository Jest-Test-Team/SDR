import { NextRequest } from "next/server";

export const dynamic = "force-dynamic";

const LIVE_CP_URL = process.env.LIVE_CP_URL ?? "http://127.0.0.1:8092";

export async function GET(
  request: NextRequest,
  { params }: { params: { path: string[] } },
) {
  const upstream = new URL(`/api/v1/live/${params.path.join("/")}`, LIVE_CP_URL);
  upstream.search = request.nextUrl.search;

  const response = await fetch(upstream, {
    cache: "no-store",
    headers: {
      accept: request.headers.get("accept") ?? "*/*",
    },
  });

  return new Response(response.body, {
    status: response.status,
    statusText: response.statusText,
    headers: {
      "cache-control": "no-store",
      "content-type": response.headers.get("content-type") ?? "application/json",
    },
  });
}
