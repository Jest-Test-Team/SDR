import { ctlProxy } from "@/lib/ctlProxy";

// Static route (no dynamic segment) so it resolves BEFORE the `/api/:path*`
// rewrite in next.config.mjs, which would otherwise shadow it.
export const dynamic = "force-dynamic";

export async function GET() {
  return ctlProxy("/status", "GET");
}
