/** @type {import('next').NextConfig} */
const nextConfig = {
  async rewrites() {
    const hilApi = process.env.HIL_API_URL ?? "http://127.0.0.1:8090";
    const liveCp = process.env.LIVE_CP_URL ?? "http://127.0.0.1:8092";
    const liveEdge = process.env.LIVE_EDGE_URL ?? "http://127.0.0.1:8081";
    return [
      { source: "/api/v1/live/:path*", destination: `${liveCp}/api/v1/live/:path*` },
      { source: "/live/cp/:path*", destination: `${liveCp}/:path*` },
      { source: "/live/edge/:path*", destination: `${liveEdge}/:path*` },
      { source: "/api/:path*", destination: `${hilApi}/api/:path*` },
      { source: "/ws/:path*", destination: `${hilApi}/ws/:path*` },
    ];
  },
};

export default nextConfig;
