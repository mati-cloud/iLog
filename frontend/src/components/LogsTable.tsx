"use client";

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Virtuoso, type VirtuosoHandle } from "react-virtuoso";
import { LogLine } from "@/components/logs/LogLine";
import { QueryBar } from "@/components/logs/QueryBar";
import { ServiceSelector } from "@/components/ServiceSelector";
import { token } from "@/lib/auth-client";
import {
  filterTerm,
  type LogEntry,
  matches,
  normalize,
  parseQuery,
  withTerm,
} from "@/lib/log-query";
import { config } from "@/lib/runtime-config";

const RANGES = [
  ["15m", 15 * 60e3],
  ["1h", 60 * 60e3],
  ["6h", 6 * 60 * 60e3],
  ["24h", 24 * 60 * 60e3],
  ["3d", 3 * 24 * 60 * 60e3],
] as const;
type Range = (typeof RANGES)[number][0];
const rangeMs = (r: Range) => RANGES.find(([k]) => k === r)?.[1] ?? 60 * 60e3;

const PAGE = 1000;
const MAX_LINES = 3000;

interface Service {
  id: string;
  name: string;
}

function readUrl() {
  const p = new URLSearchParams(window.location.search);
  const r = p.get("range");
  return {
    service: p.get("service"),
    q: p.get("q") ?? "",
    range: (RANGES.some(([k]) => k === r) ? r : "1h") as Range,
  };
}

export default function LogsTable() {
  const [services, setServices] = useState<Service[]>([]);
  const [service, setService] = useState<Service | null>(null);
  const [pickService, setPickService] = useState(false);

  const [draft, setDraft] = useState("");
  const [query, setQuery] = useState("");
  const [range, setRange] = useState<Range>("1h");
  const [live, setLive] = useState(true);

  const [logs, setLogs] = useState<LogEntry[]>([]);
  const [pending, setPending] = useState<LogEntry[]>([]);
  const [status, setStatus] = useState<"loading" | "ready" | "error">(
    "loading",
  );
  const [error, setError] = useState("");
  const [open, setOpen] = useState<Set<string>>(new Set());

  const seen = useRef(new Set<string>());
  const atTop = useRef(true);
  const list = useRef<VirtuosoHandle>(null);
  const terms = useMemo(() => parseQuery(query), [query]);

  // Initial state from the URL, then the service list.
  useEffect(() => {
    const u = readUrl();
    setDraft(u.q);
    setQuery(u.q);
    setRange(u.range);
    fetch("/api/proxy/services", { credentials: "include" })
      .then((r) =>
        r.ok ? r.json() : Promise.reject(new Error(`HTTP ${r.status}`)),
      )
      .then((data: Service[]) => {
        setServices(data);
        const found =
          data.find((s) => s.id === u.service) ??
          (data.length === 1 ? data[0] : null);
        if (found) setService(found);
        else setPickService(true);
      })
      .catch((e) => {
        setStatus("error");
        setError(`Could not load services: ${e.message}`);
      });
  }, []);

  // Keep the URL shareable.
  useEffect(() => {
    if (!service) return;
    const p = new URLSearchParams({ service: service.id, range });
    if (query) p.set("q", query);
    window.history.replaceState(null, "", `?${p}`);
  }, [service, query, range]);

  // History for the current service, query and range.
  useEffect(() => {
    if (!service) return;
    const ctrl = new AbortController();
    setStatus("loading");
    setPending([]);
    setOpen(new Set());
    const p = new URLSearchParams({
      service: service.id,
      start_time: new Date(Date.now() - rangeMs(range)).toISOString(),
      limit: String(PAGE),
    });
    if (query.trim()) p.set("q", query);
    fetch(`/api/proxy/logs/query?${p}`, {
      credentials: "include",
      signal: ctrl.signal,
    })
      .then(async (r) => {
        if (!r.ok)
          throw new Error(
            (await r.json().catch(() => null))?.error ?? `HTTP ${r.status}`,
          );
        return r.json();
      })
      .then((rows: unknown[]) => {
        const entries = rows.map((r) =>
          normalize(r as Parameters<typeof normalize>[0]),
        );
        seen.current = new Set(entries.map((e) => e.key));
        setLogs(entries);
        setStatus("ready");
      })
      .catch((e) => {
        if (ctrl.signal.aborted) return;
        setStatus("error");
        setError(e.message);
      });
    return () => ctrl.abort();
  }, [service, query, range]);

  // Live tail. New lines that match go on top; while the reader is scrolled
  // down they wait in `pending` so the page never moves under them.
  useEffect(() => {
    if (!live || !service) return;
    let ws: WebSocket | null = null;
    let closed = false;
    token().then((res) => {
      const jwt = (res as { data?: { token?: string } })?.data?.token;
      if (!jwt || closed) return;
      ws = new WebSocket(
        `${config.NEXT_PUBLIC_WS_URL}/api/logs/stream?service=${service.id}`,
        ["ilog.v1", `bearer.${jwt}`],
      );
      ws.onmessage = (ev) => {
        let entry: LogEntry;
        try {
          entry = normalize(JSON.parse(ev.data));
        } catch {
          return;
        }
        if (seen.current.has(entry.key) || !matches(entry, terms)) return;
        seen.current.add(entry.key);
        if (atTop.current)
          setLogs((prev) => [entry, ...prev].slice(0, MAX_LINES));
        else setPending((prev) => [entry, ...prev].slice(0, MAX_LINES));
      };
    });
    return () => {
      closed = true;
      ws?.close();
    };
  }, [live, service, terms]);

  const showPending = useCallback(() => {
    setLogs((prev) => [...pending, ...prev].slice(0, MAX_LINES));
    setPending([]);
    list.current?.scrollToIndex({ index: 0 });
  }, [pending]);

  const applyQuery = (q: string) => {
    setDraft(q);
    setQuery(q.trim());
  };

  const onFilter = (field: string, value: string, negate: boolean) =>
    applyQuery(withTerm(query, filterTerm(field, value, negate)));

  const fields = useMemo(() => {
    const set = new Set(["level", "source", "message"]);
    for (const l of logs.slice(0, 300))
      for (const k of Object.keys(l.attrs)) set.add(k);
    return [...set].sort();
  }, [logs]);

  const multiSource = useMemo(
    () => new Set(logs.map((l) => l.service)).size > 1,
    [logs],
  );
  const rangeLabel = RANGES.find(([k]) => k === range)?.[0];

  return (
    <div className="logs-view">
      <ServiceSelector
        open={pickService}
        onOpenChange={(o) => {
          if (!o && service) setPickService(false);
        }}
        onServiceSelect={(id) => {
          const s = services.find((x) => x.id === id);
          if (s) {
            setService(s);
            setPickService(false);
          }
        }}
      />

      <header className="logs-head">
        <div className="logs-head-top">
          <label className="logs-service">
            <span className="sr-only">Service</span>
            <select
              value={service?.id ?? ""}
              onChange={(e) =>
                setService(
                  services.find((s) => s.id === e.target.value) ?? null,
                )
              }
            >
              {!service && <option value="">Choose a service</option>}
              {services.map((s) => (
                <option key={s.id} value={s.id}>
                  {s.name}
                </option>
              ))}
            </select>
          </label>

          <fieldset className="logs-range" aria-label="Time range">
            {RANGES.map(([k]) => (
              <label key={k} data-active={k === range || undefined}>
                <input
                  type="radio"
                  name="range"
                  value={k}
                  checked={k === range}
                  onChange={() => setRange(k)}
                  className="sr-only"
                />
                {k}
              </label>
            ))}
          </fieldset>

          <button
            type="button"
            className="logs-live"
            data-on={live || undefined}
            onClick={() => setLive((v) => !v)}
            aria-pressed={live}
          >
            <span className="logs-live-dot" aria-hidden />
            {live ? "Live" : "Paused"}
          </button>
        </div>

        <QueryBar
          value={draft}
          onChange={setDraft}
          onSubmit={() => applyQuery(draft)}
          fields={fields}
          dirty={draft.trim() !== query}
        />
      </header>

      <div className="logs-status" aria-live="polite">
        {status === "loading" && <span>Searching…</span>}
        {status === "ready" && (
          <span>
            {logs.length.toLocaleString()}
            {logs.length >= PAGE && !live ? "+" : ""}{" "}
            {logs.length === 1 ? "line" : "lines"} · last {rangeLabel}
            {query && (
              <>
                {" · "}
                <button
                  type="button"
                  className="logs-link"
                  onClick={() => applyQuery("")}
                >
                  Clear search
                </button>
              </>
            )}
          </span>
        )}
        {status === "error" && <span className="logs-error">{error}</span>}
      </div>

      <div className="logs-list">
        {pending.length > 0 && (
          <button type="button" className="logs-pending" onClick={showPending}>
            {pending.length} new {pending.length === 1 ? "line" : "lines"} ↑
          </button>
        )}
        {status === "ready" && logs.length === 0 ? (
          <div className="logs-empty">
            {query ? (
              <>
                <p>
                  Nothing matches <code>{query}</code> in the last {rangeLabel}.
                </p>
                <div className="flex gap-3">
                  {range !== "3d" && (
                    <button
                      type="button"
                      className="logs-link"
                      onClick={() => setRange("3d")}
                    >
                      Search the last 3 days
                    </button>
                  )}
                  <button
                    type="button"
                    className="logs-link"
                    onClick={() => applyQuery("")}
                  >
                    Clear search
                  </button>
                </div>
              </>
            ) : (
              <p>
                No lines in the last {rangeLabel}.{" "}
                {live
                  ? "New ones appear here as they arrive."
                  : "Turn on Live to follow new ones."}
              </p>
            )}
          </div>
        ) : (
          <Virtuoso
            ref={list}
            data={logs}
            computeItemKey={(_, l) => l.key}
            atTopStateChange={(top) => {
              atTop.current = top;
              if (top && pending.length > 0) showPending();
            }}
            itemContent={(_, l) => (
              <LogLine
                log={l}
                open={open.has(l.key)}
                showService={multiSource}
                onFilter={onFilter}
                onToggle={() =>
                  setOpen((prev) => {
                    const next = new Set(prev);
                    if (!next.delete(l.key)) next.add(l.key);
                    return next;
                  })
                }
              />
            )}
          />
        )}
      </div>
    </div>
  );
}
