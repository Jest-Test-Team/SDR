import { NextRequest } from "next/server";
import { ctlProxy } from "@/lib/ctlProxy";

// Static route so it resolves BEFORE the `/api/:path*` rewrite in next.config.mjs.
export const dynamic = "force-dynamic";

export async function POST(request: NextRequest) {
  return ctlProxy("/switch", "POST", await request.text());
}
