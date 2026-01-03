// Server-side configuration that reads from environment variables at runtime
// This is separate from runtime-config.ts which is for browser/client-side code

export const serverConfig = {
  // Use internal Kubernetes service for server-side requests, not the public URL
  backendUrl: process.env.BACKEND_URL || "http://backend:8080",
  wsUrl: process.env.WS_URL || "ws://backend:8080",
};
