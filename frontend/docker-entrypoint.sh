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

# Run Better Auth database migrations if DATABASE_URL is set
if [ -n "$DATABASE_URL" ]; then
  echo "Running Better Auth database migrations..."
  
  # Check if psql is available (for running SQL migrations)
  if command -v psql >/dev/null 2>&1; then
    # Extract database connection details from DATABASE_URL
    DB_HOST=$(echo $DATABASE_URL | sed -n 's/.*@\([^:]*\):.*/\1/p')
    DB_PORT=$(echo $DATABASE_URL | sed -n 's/.*:\([0-9]*\)\/.*/\1/p')
    DB_NAME=$(echo $DATABASE_URL | sed -n 's/.*\/\([^?]*\).*/\1/p')
    DB_USER=$(echo $DATABASE_URL | sed -n 's/.*:\/\/\([^:]*\):.*/\1/p')
    
    export PGPASSWORD=$(echo $DATABASE_URL | sed -n 's/.*:\/\/[^:]*:\([^@]*\)@.*/\1/p')
    
    # Run each migration file
    for migration in /app/better-auth_migrations/*.sql; do
      if [ -f "$migration" ]; then
        echo "Running migration: $(basename $migration)"
        psql -h "$DB_HOST" -p "$DB_PORT" -U "$DB_USER" -d "$DB_NAME" -f "$migration" 2>&1 | grep -v "already exists" || true
      fi
    done
    
    echo "Migrations completed"
  else
    echo "Warning: psql not found, skipping automatic migrations"
    echo "Please run migrations manually or install postgresql-client in the Docker image"
  fi
fi

# Execute the CMD
exec "$@"
