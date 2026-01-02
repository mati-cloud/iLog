# @mati.cloud/ilog

A centralized logging software solution for collecting, processing, and visualizing logs.

## Installation

See the [Releases](https://github.com/mati-cloud/iLog/releases) page for pre-built binaries and Docker images.

## Environment Variables (`.env.dist`)

### Frontend

```bash
# Backend API URL
NEXT_PUBLIC_API_URL=https://api.yourdomain.com

# WebSocket URL for real-time logs
NEXT_PUBLIC_WS_URL=wss://api.yourdomain.com

# Database connection
DATABASE_URL=postgresql://user:password@host:5432/ilog

# Authentication (minimum 32 characters)
BETTER_AUTH_SECRET=your-secure-random-string-min-32-chars
BETTER_AUTH_URL=https://yourdomain.com
```

### Backend

See backend configuration for database and service settings.

## Docker Images

Public Docker images are available on GitHub Container Registry:

```bash
# Pull frontend image
docker pull ghcr.io/mati-cloud/ilog-frontend:latest

# Pull backend image
docker pull ghcr.io/mati-cloud/ilog-backend:latest
```

## Agent Binaries

Public pre-built - for both ARM64 and AMD64 (x86_64) architectures - are available under:
- https://github.com/mati-cloud/ilog/releases

## Quick Start

1. Copy `.env.dist` to `.env` in the frontend directory
2. Update environment variables with your production values
3. Deploy using Docker Compose or Kubernetes
