import type { NextConfig } from "next";

const nextConfig: NextConfig = {
  /* config options here */
  reactCompiler: true,
  // Note: Backend proxying is now handled by Next.js API routes
  // in /src/app/api/projects/* which use BACKEND_URL env var at runtime
  generateBuildId: async () => {
    // Force new build ID to bust cache
    return `${Date.now()}-${Math.random().toString(36).substring(7)}`;
  },
};

export default nextConfig;
