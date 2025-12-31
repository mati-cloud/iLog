#!/bin/sh
set -e

# Generate runtime config that can be injected into the app
cat > /app/public/runtime-config.js << EOF
window.__RUNTIME_CONFIG__ = {
  NEXT_PUBLIC_API_URL: "${NEXT_PUBLIC_API_URL:-http://localhost:8080}",
  NEXT_PUBLIC_WS_URL: "${NEXT_PUBLIC_WS_URL:-ws://localhost:8080}",
  NEXT_PUBLIC_BETTER_AUTH_URL: "${NEXT_PUBLIC_BETTER_AUTH_URL:-http://localhost:3000}",
  NEXT_PUBLIC_FRONTEND_URL: "${NEXT_PUBLIC_FRONTEND_URL:-http://localhost:3000}",
  NEXT_PUBLIC_ILOG_ENDPOINT: "${NEXT_PUBLIC_ILOG_ENDPOINT:-http://localhost:8080/v1/logs}",
  NEXT_PUBLIC_SERVICE_NAME: "${NEXT_PUBLIC_SERVICE_NAME:-ilog-frontend}"
};
EOF

echo "Runtime configuration generated:"
echo "  NEXT_PUBLIC_API_URL: ${NEXT_PUBLIC_API_URL:-http://localhost:8080}"
echo "  NEXT_PUBLIC_WS_URL: ${NEXT_PUBLIC_WS_URL:-ws://localhost:8080}"
echo "  NEXT_PUBLIC_BETTER_AUTH_URL: ${NEXT_PUBLIC_BETTER_AUTH_URL:-http://localhost:3000}"
echo "  NEXT_PUBLIC_FRONTEND_URL: ${NEXT_PUBLIC_FRONTEND_URL:-http://localhost:3000}"
echo "  NEXT_PUBLIC_ILOG_ENDPOINT: ${NEXT_PUBLIC_ILOG_ENDPOINT:-http://localhost:8080/v1/logs}"
echo "  NEXT_PUBLIC_SERVICE_NAME: ${NEXT_PUBLIC_SERVICE_NAME:-ilog-frontend}"

# Execute the CMD
exec "$@"
