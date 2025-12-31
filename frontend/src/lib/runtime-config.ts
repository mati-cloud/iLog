// Runtime configuration that can be set via environment variables at container startup
// This allows self-hosted deployments to configure the app without rebuilding

declare global {
  interface Window {
    __RUNTIME_CONFIG__?: {
      NEXT_PUBLIC_API_URL: string;
      NEXT_PUBLIC_WS_URL: string;
      NEXT_PUBLIC_BETTER_AUTH_URL: string;
      NEXT_PUBLIC_FRONTEND_URL: string;
      NEXT_PUBLIC_ILOG_ENDPOINT: string;
      NEXT_PUBLIC_SERVICE_NAME: string;
    };
  }
}

function getRuntimeConfig() {
  // In browser, use runtime config if available
  if (typeof window !== 'undefined' && window.__RUNTIME_CONFIG__) {
    return window.__RUNTIME_CONFIG__;
  }
  
  // Fallback to build-time env vars (for development)
  return {
    NEXT_PUBLIC_API_URL: process.env.NEXT_PUBLIC_API_URL || 'http://localhost:8080',
    NEXT_PUBLIC_WS_URL: process.env.NEXT_PUBLIC_WS_URL || 'ws://localhost:8080',
    NEXT_PUBLIC_BETTER_AUTH_URL: process.env.NEXT_PUBLIC_BETTER_AUTH_URL || 'http://localhost:3000',
    NEXT_PUBLIC_FRONTEND_URL: process.env.NEXT_PUBLIC_FRONTEND_URL || 'http://localhost:3000',
    NEXT_PUBLIC_ILOG_ENDPOINT: process.env.NEXT_PUBLIC_ILOG_ENDPOINT || 'http://localhost:8080/v1/logs',
    NEXT_PUBLIC_SERVICE_NAME: process.env.NEXT_PUBLIC_SERVICE_NAME || 'ilog-frontend',
  };
}

export const config = getRuntimeConfig();
