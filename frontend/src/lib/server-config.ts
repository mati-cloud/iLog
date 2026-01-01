// Server-side configuration that reads from environment variables at runtime
// This is separate from runtime-config.ts which is for browser/client-side code

export const serverConfig = {
  backendUrl: process.env.NEXT_PUBLIC_API_URL || process.env.BACKEND_URL || "http://localhost:8080",
  wsUrl: process.env.NEXT_PUBLIC_WS_URL || process.env.WS_URL || "ws://localhost:8080",
};
