FROM oven/bun:1.3-alpine AS base

# Install dependencies only when needed
FROM base AS deps
WORKDIR /app

# Copy only dependency files first for better caching
COPY package.json bun.lock ./
RUN bun install --frozen-lockfile

# Rebuild the source code only when needed
FROM base AS builder
WORKDIR /app
COPY --from=deps /app/node_modules ./node_modules
COPY . .

ENV NEXT_TELEMETRY_DISABLED 1

RUN bun run build

# Production image - use Node.js for running the app
FROM oven/bun:1.3-alpine AS runner
WORKDIR /app

ENV NODE_ENV production
ENV NEXT_TELEMETRY_DISABLED 1

RUN addgroup --system --gid 1001 nodejs
RUN adduser --system --uid 1001 --ingroup nodejs nextjs

# Copy public assets
COPY --from=builder /app/public ./public

# Copy node_modules (needed for runtime)
COPY --from=builder /app/node_modules ./node_modules

# Copy the entire .next directory (Turbopack needs all chunks)
COPY --from=builder --chown=nextjs:nodejs /app/.next ./.next

# Copy package.json for runtime
COPY --from=builder /app/package.json ./package.json

RUN apk add curl

USER nextjs

EXPOSE 3000

ENV PORT 3000
ENV HOSTNAME "0.0.0.0"

CMD ["bun", "run", "start"]
