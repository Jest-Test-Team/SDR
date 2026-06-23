/** @type {import('next').NextConfig} */
const nextConfig = {
  async rewrites() {
    const api = process.env.HIL_API_URL ?? "http://127.0.0.1:8090";
    return [
      { source: "/api/:path*", destination: `${api}/api/:path*` },
      { source: "/ws/:path*", destination: `${api}/ws/:path*` },
    ];
  },
};

export default nextConfig;
