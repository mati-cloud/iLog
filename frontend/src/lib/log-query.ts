// Search language shared with the backend (backend/src/query.rs). The server
// runs it against history; this copy filters the live stream the same way and
// drives the query bar's highlighting. Keep the two in step.

export type Matcher =
  | { kind: "text"; value: string }
  | { kind: "eq"; values: string[] }
  | { kind: "like"; re: RegExp }
  | { kind: "num"; op: ">" | ">=" | "<" | "<="; n: number }
  | { kind: "exists" };

export interface Term {
  negate: boolean;
  field: string | null;
  matcher: Matcher;
  /** Source span in the query, for highlighting. */
  start: number;
  end: number;
  /** Length of the `-field:` prefix inside the span (0 for free text). */
  keyLen: number;
}

export interface LogEntry {
  key: string;
  time: Date;
  level: string;
  service: string;
  body: string;
  attrs: Record<string, unknown>;
}

const FIELD = /^[A-Za-z0-9_.@-]{1,128}$/;
const NUM = /^-?\d+(\.\d+)?$/;

export function parseQuery(q: string): Term[] {
  const terms: Term[] = [];
  let i = 0;
  while (i < q.length) {
    while (i < q.length && /\s/.test(q[i])) i++;
    if (i >= q.length) break;
    const start = i;
    let text = "";
    let quoted = false;
    let inQuote = false;
    let colon = -1;
    while (i < q.length && (inQuote || !/\s/.test(q[i]))) {
      const c = q[i++];
      if (c === '"') {
        inQuote = !inQuote;
        quoted = true;
      } else {
        if (c === ":" && !inQuote && colon < 0) colon = text.length;
        text += c;
      }
    }
    const negate = text.length > 1 && text.startsWith("-");
    const skip = negate ? 1 : 0;
    const body = text.slice(skip);
    const at = colon - skip;
    if (colon >= skip) {
      const k = body.slice(0, at);
      const v = body.slice(at + 1);
      if (FIELD.test(k) && v && !v.startsWith("//")) {
        const raw = q.slice(start, i);
        terms.push({
          negate,
          field: k,
          matcher: valueMatcher(v, quoted),
          start,
          end: i,
          keyLen: raw.indexOf(":") + 1,
        });
        continue;
      }
    }
    terms.push({
      negate,
      field: null,
      matcher: { kind: "text", value: body },
      start,
      end: i,
      keyLen: skip,
    });
  }
  return terms;
}

function valueMatcher(v: string, quoted: boolean): Matcher {
  if (quoted) return { kind: "eq", values: [v] };
  if (v === "*") return { kind: "exists" };
  for (const op of [">=", "<=", ">", "<"] as const) {
    if (v.startsWith(op) && NUM.test(v.slice(op.length))) {
      return { kind: "num", op, n: Number(v.slice(op.length)) };
    }
  }
  if (v.includes("*")) {
    const src = v
      .split("*")
      .map((s) => s.replace(/[.+?^${}()|[\]\\]/g, "\\$&"))
      .join(".*");
    return { kind: "like", re: new RegExp(`^${src}$`, "is") };
  }
  return { kind: "eq", values: v.split(",").filter(Boolean) };
}

function fieldValue(log: LogEntry, field: string): string | undefined {
  switch (field) {
    case "level":
    case "severity":
      return log.level;
    case "source":
    case "service":
      return log.service;
    case "message":
    case "msg":
    case "body":
      return log.body;
  }
  let v: unknown = log.attrs[field];
  if (v === undefined && field.includes(".")) {
    v = field
      .split(".")
      .reduce<unknown>(
        (o, k) =>
          o && typeof o === "object"
            ? (o as Record<string, unknown>)[k]
            : undefined,
        log.attrs,
      );
  }
  if (v === undefined || v === null) return undefined;
  return typeof v === "object" ? JSON.stringify(v) : String(v);
}

function termMatches(log: LogEntry, t: Term): boolean {
  const field = t.field ?? "body";
  const v = fieldValue(log, field);
  const m = t.matcher;
  if (m.kind === "exists") return v !== undefined;
  if (v === undefined) return false;
  const lower = v.toLowerCase();
  const isBody = ["body", "message", "msg"].includes(field);
  const isLevel = field === "level" || field === "severity";
  switch (m.kind) {
    case "text":
      return lower.includes(m.value.toLowerCase());
    case "like":
      return m.re.test(v);
    case "num": {
      if (!NUM.test(v)) return false;
      const n = Number(v);
      return m.op === ">"
        ? n > m.n
        : m.op === ">="
          ? n >= m.n
          : m.op === "<"
            ? n < m.n
            : n <= m.n;
    }
    case "eq":
      return m.values.some((x) =>
        isBody
          ? lower.includes(x.toLowerCase())
          : isLevel
            ? lower === x.toLowerCase()
            : v === x,
      );
  }
}

export function matches(log: LogEntry, terms: Term[]): boolean {
  return terms.every((t) => termMatches(log, t) !== t.negate);
}

/** Quote a value if it would otherwise split or parse differently. */
export function filterTerm(
  field: string,
  value: string,
  negate = false,
): string {
  const needsQuotes = /[\s",*<>]/.test(value) || value === "";
  const v = needsQuotes ? `"${value.replace(/"/g, "")}"` : value;
  return `${negate ? "-" : ""}${field}:${v}`;
}

/** Add a term, replacing an existing term on the same field and polarity. */
export function withTerm(q: string, term: string): string {
  const next = parseQuery(term)[0];
  const kept = parseQuery(q)
    .filter(
      (t) =>
        !(
          next &&
          t.field &&
          t.field === next.field &&
          t.negate === next.negate
        ),
    )
    .map((t) => q.slice(t.start, t.end));
  return [...kept, term].join(" ");
}

// ---- Normalising raw backend/WebSocket payloads --------------------------

interface RawLog {
  timeUnixNano?: string;
  time_unix_nano?: string;
  time?: string;
  severityText?: string;
  severity_text?: string;
  serviceName?: string;
  service_name?: string;
  body?: string;
  logAttributes?: unknown;
  log_attributes?: unknown;
}

export function normalize(raw: RawLog): LogEntry {
  const nanos = raw.timeUnixNano ?? raw.time_unix_nano;
  const time =
    nanos && /^\d+$/.test(nanos)
      ? new Date(Number(BigInt(nanos) / 1_000_000n))
      : new Date(raw.time ?? Date.now());
  let attrs = raw.logAttributes ?? raw.log_attributes ?? {};
  if (typeof attrs === "string") {
    try {
      attrs = JSON.parse(attrs);
    } catch {
      attrs = {};
    }
  }
  const body = raw.body ?? "";
  const service = raw.serviceName ?? raw.service_name ?? "unknown";
  return {
    key: `${nanos ?? time.getTime()}|${service}|${body.length}|${body.slice(0, 48)}`,
    time,
    level: (raw.severityText ?? raw.severity_text ?? "INFO").toUpperCase(),
    service,
    body,
    attrs: (attrs && typeof attrs === "object" ? attrs : {}) as Record<
      string,
      unknown
    >,
  };
}

/**
 * One-line reading of a log. Request logs (nginx, Rails JSON) collapse to
 * `GET /path 200 · 43ms`; everything else is its body.
 */
export function summarize(log: LogEntry): {
  method?: string;
  path?: string;
  status?: number;
  duration?: string;
  text: string;
} {
  const a = log.attrs;
  const method = typeof a.method === "string" ? a.method : undefined;
  const path = typeof a.path === "string" ? a.path : undefined;
  if (method && path) {
    const status = Number(a.status ?? a["http.status_code"]);
    const secs = Number(a.duration_s ?? a.duration);
    const ms = Number(a.duration_ms);
    const dur =
      Number.isFinite(ms) && ms > 0
        ? ms
        : Number.isFinite(secs) && secs > 0
          ? secs * 1000
          : NaN;
    return {
      method,
      path,
      status: Number.isFinite(status) && status > 0 ? status : undefined,
      duration: Number.isFinite(dur)
        ? dur >= 1000
          ? `${(dur / 1000).toFixed(1)}s`
          : `${Math.round(dur)}ms`
        : undefined,
      text: `${method} ${path}`,
    };
  }
  return { text: log.body };
}

/** Level to show, if any. INFO is the quiet default and gets none. */
export function tone(log: LogEntry): "error" | "warn" | null {
  const l = log.level;
  const status = Number(log.attrs.status);
  if (l.startsWith("ERR") || l === "FATAL" || l === "CRITICAL" || status >= 500)
    return "error";
  if (l.startsWith("WARN") || status >= 400) return "warn";
  return null;
}
