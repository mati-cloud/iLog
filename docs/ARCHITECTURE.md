# iLog — Architecture & Design

> Centralized log pipeline. Rust agent + Rust backend + Next.js UI. OpenTelemetry data model on TimescaleDB.

---

## 1. What iLog Is

`@mati.cloud/iLog` = self-hosted log aggregation platform. Three components:

| Component | Stack | Role |
|-----------|-------|------|
| `ilog-agent` | Rust (tokio) | Collect logs on host. Tail files, journald, Docker. Ship to backend. |
| `backend` | Rust (axum) + TimescaleDB | Ingest, store, query, stream. HTTP + custom TCP. |
| `frontend` | Next.js 16 + React 19 | Dashboard, log table, live tail, RBAC, service mgmt. |

Storage = PostgreSQL 17 + TimescaleDB 2.18 hypertable. Schema = OpenTelemetry logs (`trace_id`, `span_id`, `severity_number`, `resource_attributes`, `log_attributes`, `body`).

---

## 2. Problem Being Solved

Ops reality:
- Logs scattered across hosts (`/var/log/`, journald, Docker stdout).
- `ssh + grep + tail -f` no scale past 2 servers.
- Splunk / Datadog / Loggly = per-GB pricing, vendor lock, sends data offsite.
- ELK = heavy (JVM Elasticsearch ~2GB RAM minimum), shard tuning, ILM rollover headaches.
- Loki = good but Grafana-dependent, LogQL learning curve, label cardinality footguns.
- Most OSS agents (Fluentd Ruby, Logstash JVM) = fat. 100-500 MB RAM per host.

iLog target use case = small/medium fleet (single-digit to ~100 hosts), self-host, tiny resource budget, OTel-native, multi-tenant by "service".

What it cuts:
- Vendor cost → zero.
- Agent footprint → ~5-12 MB RAM (Rust, statically linked).
- Storage cost → TimescaleDB native compression (>10× on log workloads).
- Setup ceremony → `compose.yaml` + one binary on each host.

---

## 3. How It Solves It

### 3.1 Data Flow (end-to-end)

```
[ host process ] → file / stdout → /var/log/pods (k8s) or /var/log/... (host)
        │
        ▼
[ ilog-agent ]  (tokio task per tailed file)
   providers/       parser/           tcp_sender
   ├─ file (tail)   ├─ json           │
   ├─ docker        ├─ regex          │  micro-batch 10ms / 50 logs
   └─ systemd       └─ cri → inner    │  LZ4 compress
                       (strips CRI    │  ChaCha20-Poly1305 encrypt
                        envelope)     │  persistent TCP socket
                                    ▼
                            ┌──────────────────┐
                            │  backend :8081   │  (custom binary frame)
                            │  tcp_server.rs   │  ─ magic "ILOG" + ver
                            │                  │  ─ decrypt with agent
                            │                  │    token-derived key
                            │                  │  ─ decompress LZ4
                            │                  │  ─ serde_json → OtelLog
                            └──────┬───────────┘
                                   │
                       ┌───────────┼──────────────────┐
                       ▼                              ▼
                 INSERT into            tokio::broadcast::Sender<OtelLog>
                 TimescaleDB                          │
                 (hypertable `logs`)                  ▼
                       │                     [ WebSocket /api/logs/stream ]
                       │                              │
                       ▼                              ▼
              compression policy            React virtual list
              (chunks > 7d → compress)      (react-virtuoso)
              retention policy
              (chunks > 90d → drop)
              continuous aggregate
              (logs_hourly)
```

### 3.2 Agent (`ilog-agent`)

- **Tokio runtime**, channel-based fan-in: each tailed file = task, all push to `mpsc<LogEntry>(1000)`.
- **Providers** (compile-time feature gates):
  - `file` (default) — glob + polling tail. Globs are **re-expanded every `discovery_interval_secs`** (default 15) rather than once at startup, so files created later are picked up. This is what makes `/var/log/pods` viable: the matching set turns over as pods are scheduled. Files present at boot are tailed from the end (a restart does not re-ingest history); files found by a later sweep are read from position 0. Polling also handles rotation and truncation.
  - `journald` (`systemd` crate, Linux only, cannot link into a static musl build).
  - `docker` (`bollard`, optional).
- **Parser layer** (`format:` per source):
  - `json` — field extraction with optional rename/type mapping.
  - `regex` — named capture groups, typed field mapping.
  - `cri` — Kubernetes container logs. Strips the CRI envelope
    (`<RFC3339Nano> <stream> <F|P> <payload>`), reassembles lines the runtime
    split at 16 KiB, then delegates the payload to an inner parser (JSON by
    default). Non-JSON payloads fall back to `message` rather than being dropped,
    since container stdout mixes structured logs with panics and banners.
    Reassembly is stateful, so parsers are built **per file**, not shared across a
    glob expansion.
  - `exclude_paths` per source — needed to stop the agent tailing the backend it
    ships to, which would otherwise be a self-sustaining ingest loop.
- **Config** — TOML file plus env overrides with prefix `ILOG`, so env names mirror
  config paths: `ILOG_AGENT_TOKEN` → `agent.token`, `ILOG_AGENT_SERVER` →
  `agent.server`. This is how the Kubernetes DaemonSet injects the token from a
  Secret without baking it into the ConfigMap.
- **Wire path** (`tcp_sender.rs`):
  1. Buffer logs, flush on 10 ms tick OR 50-log threshold.
  2. Serialize OTLP-ish JSON.
  3. `lz4::block::compress` → `ChaCha20Poly1305::encrypt`.
  4. Frame: `"ILOG"` magic + version + type + u32 len + payload.
  5. Heartbeat every 30s. Reconnect with exponential backoff (`1 << retry_count`).
- **Footprint**: release profile `opt-level="z"`, `lto`, `panic="abort"`, `strip`. ~3–6 MB binary.

### 3.3 Backend (`backend/`)

- **axum 0.7** HTTP server (port 8080) + **raw TCP** server (port 8081, spawned task).
- **HTTP endpoints**:
  - `POST /v1/logs` — OTLP-style bearer-auth ingest (HTTP fallback).
  - `GET /api/logs/query` — filtered query (service, severity, trace_id, search, time window).
  - `GET /api/logs/stream` — WebSocket; sends last 100 logs from past 24h then subscribes to broadcast.
  - `GET /api/dashboard/*` — metrics, log-volume, storage-by-service, agents, 7-day ingestion.
  - `*` `/api/services` + `/agents` — multi-tenant CRUD, RBAC roles `owner|admin|member|viewer`.
- **Auth**:
  - User auth = JWT middleware (Better Auth issues; backend validates).
  - Agent auth = unique token per agent row in `agents` table. TCP layer authenticates by *trial decrypt*: load active agent tokens, try each → first that decrypts wins, sets `service_id` on the batch. (Trade-off: O(n) tokens per batch — fine at small fleet scale, would need re-think at >1000 agents.)
- **Real-time fanout**: `tokio::sync::broadcast::channel::<OtelLog>(1000)` — TCP ingest task pushes, every WebSocket subscriber filters by `service_id`. Slow client = lagged → skip (no head-of-line block).
- **DB layer** = `sqlx` (compile-time-checked queries off, runtime checked).

### 3.4 Storage (TimescaleDB)

- `logs` = **hypertable** on `time` column → automatic per-time-range chunking.
- Indexes:
  - B-tree on `(service_name, time DESC)`, `(severity_number, time DESC)`, `(service_id, time DESC)`.
  - Partial on `trace_id WHERE NOT NULL`.
  - **GIN on `to_tsvector('english', body)`** → full-text search on body.
  - GIN on `resource_attributes`, `log_attributes` JSONB.
- **Compression** (migration 003): `segmentby = service_name`, `orderby = time DESC`, kick in after 7 days. Columnar storage on the cold chunks → typically 10–20× shrink on logs.
- **Retention**: drop chunks > 90 days (configurable).
- **Continuous aggregate**: `logs_hourly` rolls up `count(*)` per (hour, service_name, severity_number) → dashboard queries don't scan raw rows.

### 3.5 Frontend (`frontend/`)

- **Next.js 16** App Router, **React 19** with React Compiler enabled.
- **Better Auth** for session/login (Postgres-backed).
- `react-virtuoso` virtual list for `LogsTable` → handle 10k+ row windows without DOM blow-up.
- `recharts` for dashboard time-series.
- WebSocket client subscribes to `/api/logs/stream?service=<uuid>` for live tail.
- Tailwind 4 + shadcn/radix primitives.

### 3.6 Kubernetes Deployment

The agent runs as a **DaemonSet**, one pod per node.

Why not a Deployment: the kubelet already writes every container's stdout/stderr
to `/var/log/pods/<ns>_<pod>_<uid>/<container>/N.log` on the node it runs on, so
collection is inherently per-node work. The alternative — streaming from the
`pods/log` API — needs RBAC, holds one long-lived apiserver connection per
container, and re-subscribes on every pod restart.

**No ServiceAccount, ClusterRole or ClusterRoleBinding.** Reading logs off the
node filesystem requires no Kubernetes API access at all. RBAC would only become
necessary to enrich events with pod labels (resolving `pod=api-7d9f-x2k` to
`app=api`), which is not implemented — granting it now would be permission
without a consumer.

Mounts `/var/log/pods` read-only and nothing else. On containerd those are real
files and the symlinks in `/var/log/containers` point at them; a
`/var/lib/docker/containers` mount only applies to Docker's `json-file` driver.
Pod logs are `0640 root:root` inside a `0750` directory, so the container runs as
root — an unprivileged UID cannot read them. It drops all capabilities, disallows
privilege escalation, and uses a read-only root filesystem, so root buys it
nothing beyond that read.

Note that iLog **cannot reduce disk usage on the nodes it collects from**. The
agent is a reader; the kubelet writes the file before the agent sees a byte.
What iLog does is make it *safe* to shrink node-local retention, since the
durable copy lives centrally — via kubelet `containerLogMaxSize` /
`containerLogMaxFiles`. Do not shrink to zero: those files back `kubectl logs`
and act as the replay buffer if the agent or backend is briefly unavailable.

Requires secret `ilog-agent-token` (key `token`) in the deployment namespace,
minted from the dashboard. The backend Service must expose the TCP ingest port
(8081) alongside HTTP (8080).

---

## 4. Compare vs. Existing Solutions

### 4.1 vs. ELK (Elasticsearch + Logstash + Kibana / Filebeat)

| Axis | ELK | iLog |
|------|-----|------|
| Agent RAM | Filebeat ~30–80 MB, Logstash 500 MB+ | Agent ~5–12 MB |
| Backend baseline | ES JVM ≥ 2 GB, dedicated cluster work | Rust backend < 100 MB + Postgres |
| Storage engine | Lucene shards, ILM tiers | TimescaleDB hypertable + native compression |
| Search | Lucene full-text, very rich | Postgres GIN tsvector + JSONB. Less expressive, plenty for logs. |
| Ops burden | Shard sizing, hot/warm, mapping explosion, ILM, snapshots | One Postgres. Done. |
| Modernity | Mature, sprawling, JVM-rooted | Rust + Postgres extensions. Lean. |

**Verdict**: iLog ~10× lighter, ~10× simpler ops. Loses on query power and ecosystem (Kibana visualisations, Watcher alerts).

### 4.2 vs. Loki + Promtail / Grafana Agent

| Axis | Loki | iLog |
|------|------|------|
| Storage model | Index labels, chunk body in object store | Index full body via GIN, single Postgres |
| Cost model | Cheap object storage, but label cardinality is a footgun | Predictable Postgres disk, compression on cold chunks |
| Search | LogQL — powerful for label-filtered tail, weak for "grep across everything" without labels | Plain SQL + `body ILIKE` + full-text. "Search anything" works out of box. |
| UI | Grafana required | First-party dashboard included |
| Multi-tenant | Tenant header, no built-in RBAC UI | `services` + `service_members` + roles in DB |
| Tracing tie-in | Native (Tempo) | `trace_id`/`span_id` columns + OTel semantics |

**Verdict**: iLog wins for "I want to search arbitrary text" + bundled UI. Loki wins at PB scale and on cloud-native label workflows.

### 4.3 vs. Splunk / Datadog / Sumo / Better Stack

| Axis | SaaS | iLog |
|------|------|------|
| Cost | $0.10–$5 / GB ingested | Self-host. CapEx only. |
| Data residency | Vendor cloud | Your box. |
| Lock-in | High (query lang, dashboards) | OTel schema in Postgres — fully portable |
| Features | Alerting, ML anomaly, SOC pkgs | Storage + query + live tail. No alerting yet. |
| Time-to-value | Sign up, paste API key | `docker compose up` + install agent script |

**Verdict**: iLog = cost/privacy win. SaaS = feature breadth win.

### 4.4 vs. Vector (agent) + ClickHouse + custom UI

Probably the closest serious peer.

| Axis | Vector + ClickHouse | iLog |
|------|---------------------|------|
| Agent | Vector (Rust, ~30 MB, very feature-rich VRL transforms) | ilog-agent (Rust, ~10 MB, focused) |
| Backend | ClickHouse — best-in-class columnar OLAP for logs | TimescaleDB — strong, not ClickHouse-fast at PB |
| Wire format | gRPC / Vector protocol, TLS | Custom framed TCP + ChaCha20-Poly1305 |
| UI | DIY (Grafana plugin / build your own) | First-party Next.js dashboard |
| Compile-time agent slimming | Vector features | iLog cargo features (`file`/`docker`/`journald`/`all`) |

**Verdict**: ClickHouse stack scales further. iLog ships a turnkey UI + auth + RBAC out of the box. Smaller pieces, less glue.

### 4.5 vs. Fluentd / Fluent Bit

- Fluent Bit (C) ~1 MB binary, ~10 MB RAM — *the* leanest competitor.
- Fluentd (Ruby) ~80 MB+ RAM.
- iLog agent is in the same ballpark as Fluent Bit on resources, but **iLog ships an opinionated full stack** (agent + backend + UI + DB schema). Fluent Bit is just the shipper — you still bring your own sink.

---

## 5. Where iLog Is Strong

1. **Modernity** — Rust + axum + tokio + sqlx; React 19 + Next 16 + React Compiler; Better Auth instead of homegrown JWT plumbing. No legacy JVM / Ruby anywhere.
2. **Low footprint** — agent fits a Raspberry Pi. Backend fits a 256 MB VPS plus Postgres.
3. **Custom binary protocol** — TCP frame + LZ4 + ChaCha20-Poly1305. Sub-ms latency, persistent socket, no TLS handshake per batch. Per-token-derived keys → revoke an agent = delete a row.
4. **OTel-native schema** — `trace_id`, `span_id`, `severity_number`, scope/resource/log attributes match OpenTelemetry. Trivial to ingest from existing OTel SDKs (HTTP `/v1/logs` path exists).
5. **TimescaleDB lever** — hypertables + native columnar compression + continuous aggregates do the heavy lifting Postgres alone wouldn't.
6. **Bundled UX** — dashboard, log table, live tail, services CRUD, RBAC — all in repo. No "now go install Grafana" step.
7. **Multi-tenant from day one** — `services` table + `service_members` + role check + per-service agent tokens. Most OSS stacks bolt this on later (or never).

---

## 6. Where iLog Is Weak / Honest Trade-offs

1. ~~**Agent auth = trial decrypt over all tokens.**~~ **Fixed.** The v2 frame header carries the agent id, so `tcp_server.rs` does a single primary-key lookup (`WHERE id = $1`) and derives that agent's key. No longer O(n) per batch.
2. **Ingest loop is single-row INSERT in a `for log in logs` loop** (`otel.rs:11-41`). No `COPY` / batched `INSERT … VALUES (…),(…)`. Throughput cap will hit here first under load — easy fix, but currently a real bottleneck.
3. **WebSocket fanout is in-process `broadcast::channel`**. Single backend node only. Horizontal scale would need Redis pub/sub or NATS shim.
4. **No alerting / no anomaly detection / no log-based metrics export**. Pure ingest+search+view today.
5. ~~**Key derivation = `DefaultHasher` of token.**~~ **Fixed.** Now HKDF-SHA256 (`tcp_server.rs`), with the agent side deriving identically. The former scheme produced a 32-byte key holding only 64 bits of entropy. Agent `key_secret` is also encrypted at rest under `TOKEN_ENCRYPTION_KEY` rather than stored plaintext (`token_crypto.rs`), and is no longer returned by the agent-list API.
6. **No TLS on the TCP port** — security relies on AEAD + token secrecy. Defensible (and intentional: skip handshake overhead), but a NAT-traversing operator may want TLS-wrapped fallback.
7. **HTTP ingest path is gone**, not just deprecated — `POST /v1/logs` and `validate_agent_token` were both removed, so TCP is the only ingest route. Fronting iLog behind a generic L7 load balancer is awkward today.
8. **Schema migrations are forward-only SQL files**. No `down`, no versioned tooling beyond `sqlx::migrate!`.
9. **Search is `body ILIKE`** in addition to GIN tsvector. `ILIKE` won't use the GIN index — full-text path needs `to_tsquery` wiring to actually exploit the index.
10. **Frontend talks directly to Postgres** (`pg` in `package.json`) for some Better Auth paths. Two write paths to the same DB → operational surface area larger than necessary.

---

## 7. Verdict — How Good Is iLog?

Scoring on the niche it actually targets — **self-hosted, OTel-shaped, small-to-medium fleet, "I don't want to think about Elasticsearch on a Sunday"**:

| Dimension | Score | Notes |
|-----------|-------|-------|
| Modernity of stack | **9/10** | Rust + Postgres17 + React19 + Next16. Few projects this current. |
| Resource efficiency | **9/10** | Beats ELK/Fluentd by an order of magnitude. Matches Vector/FluentBit class. |
| Operational simplicity | **9/10** | Two binaries + one DB. `compose up`. |
| Out-of-box UX | **8/10** | Dashboard + RBAC + multi-tenant included. Most rivals ship bare. |
| Scalability ceiling | **5/10** | Single-row inserts + single-node broadcast + O(n) token match. Designed for ~100 hosts, not 10k. |
| Feature breadth | **5/10** | No alerts, no anomaly, no metric export, no parsers marketplace. |
| Search power | **6/10** | Full-text via GIN works, but query path mixes ILIKE; not Lucene/LogQL/SQL-on-ClickHouse class. |
| Security posture | **7/10** | AEAD + per-agent tokens is solid; KDF + TLS would push to 9. |

**Overall**: iLog is a *very strong* "logging for humans" play in the 1–~200 host range. The architecture is clean, the stack is genuinely modern, and the bundled UX + RBAC give it an unfair head start over "agent + sink + DIY UI" combos like Vector/ClickHouse or Fluent Bit/Loki. It is **not** trying to be Splunk, Elastic, or ClickHouse-at-petabytes, and it shouldn't pretend to. Within its target box it punches well above its line count (~4.6k LOC of Rust + a Next app).

Closest single-sentence pitch: **"Loki's footprint, Splunk's UX, a Rust agent, one Postgres."**

---

## 8. Outstanding Verification

Carried over from the token-encryption work. The code is implemented and compiles,
but `.sqlx` is empty so the SQL is only validated at runtime — `cargo check`
passing proves nothing about these paths.

1. Boot the backend with `TOKEN_ENCRYPTION_KEY` unset — it must refuse to start.
   If it boots, every deployment that forgot the env var shares one wrapping key.
2. `GET /api/services/:id/agents` — confirm no `token`, `auth_secret_hash`, or
   `key_secret_encrypted` in the JSON. Check the response body, not the Rust
   types.
3. `SELECT * FROM agents` — `key_secret_encrypted` must be bytes that are not the
   token.
4. Point an agent at a freshly minted token, confirm logs arrive. Proves the
   encrypt → store → decrypt → HKDF → AEAD round trip.
5. Corrupt one byte of `key_secret_encrypted`, confirm that agent's batches are
   rejected and the error names the agent id.
6. **Restart the backend, confirm the same agent still ingests.** Proves the
   wrapping key is derived deterministically from env rather than generated per
   boot. This is the one most likely to be skipped and the only one that fails
   silently until the first restart.
