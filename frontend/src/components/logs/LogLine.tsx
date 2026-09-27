"use client";

import { Check, Copy } from "lucide-react";
import { useState } from "react";
import { type LogEntry, summarize, tone } from "@/lib/log-query";

const timeFmt = new Intl.DateTimeFormat(undefined, {
  hour: "2-digit",
  minute: "2-digit",
  second: "2-digit",
  hour12: false,
});

export function formatTime(d: Date) {
  return `${timeFmt.format(d)}.${String(d.getMilliseconds()).padStart(3, "0")}`;
}

interface LogLineProps {
  log: LogEntry;
  open: boolean;
  onToggle: () => void;
  /** Add `field:value` (or its negation) to the query. */
  onFilter: (field: string, value: string, negate: boolean) => void;
  showService: boolean;
}

export function LogLine({
  log,
  open,
  onToggle,
  onFilter,
  showService,
}: LogLineProps) {
  const s = summarize(log);
  const t = tone(log);
  return (
    <div
      className="log-line"
      data-tone={t ?? undefined}
      data-open={open || undefined}
    >
      <button
        type="button"
        className="log-row"
        onClick={onToggle}
        aria-expanded={open}
      >
        <time className="log-time" dateTime={log.time.toISOString()}>
          {formatTime(log.time)}
        </time>
        {showService && <span className="log-service">{log.service}</span>}
        <span className="log-msg">
          {t && (
            <span className="log-level">
              {t === "error" ? "error" : "warn"}
            </span>
          )}
          {s.method ? (
            <>
              <span className="log-method">{s.method}</span>{" "}
              <span>{s.path}</span>
              {s.status !== undefined && (
                <span className="log-status"> {s.status}</span>
              )}
              {s.duration && <span className="log-dim"> · {s.duration}</span>}
            </>
          ) : (
            s.text
          )}
        </span>
      </button>
      {open && <LogDetail log={log} onFilter={onFilter} />}
    </div>
  );
}

function LogDetail({ log, onFilter }: Pick<LogLineProps, "log" | "onFilter">) {
  const entries = Object.entries(log.attrs).sort(([a], [b]) =>
    a.localeCompare(b),
  );
  let pretty = log.body;
  try {
    const parsed = JSON.parse(log.body);
    if (parsed && typeof parsed === "object")
      pretty = JSON.stringify(parsed, null, 2);
  } catch {}

  return (
    <div className="log-detail">
      <dl className="log-fields">
        <Field name="source" value={log.service} onFilter={onFilter} />
        <Field name="level" value={log.level} onFilter={onFilter} />
        {entries.map(([k, v]) => (
          <Field
            key={k}
            name={k}
            value={
              v !== null && typeof v === "object"
                ? JSON.stringify(v)
                : String(v)
            }
            onFilter={onFilter}
            filterable={v === null || typeof v !== "object"}
          />
        ))}
      </dl>
      <div className="log-body">
        <div className="log-body-head">
          <span>Raw line</span>
          <CopyButton text={log.body} />
        </div>
        <pre>{pretty}</pre>
      </div>
    </div>
  );
}

function Field({
  name,
  value,
  onFilter,
  filterable = true,
}: {
  name: string;
  value: string;
  onFilter: LogLineProps["onFilter"];
  filterable?: boolean;
}) {
  return (
    <div className="log-field">
      <dt>{name}</dt>
      <dd>
        <span className="log-field-value">{value}</span>
        {filterable && (
          <span className="log-field-actions">
            <button
              type="button"
              onClick={() => onFilter(name, value, false)}
              title={`Show only ${name}:${value}`}
            >
              Only
            </button>
            <button
              type="button"
              onClick={() => onFilter(name, value, true)}
              title={`Hide ${name}:${value}`}
            >
              Hide
            </button>
          </span>
        )}
      </dd>
    </div>
  );
}

function CopyButton({ text }: { text: string }) {
  const [done, setDone] = useState(false);
  return (
    <button
      type="button"
      className="log-copy"
      onClick={async () => {
        await navigator.clipboard.writeText(text);
        setDone(true);
        setTimeout(() => setDone(false), 1500);
      }}
      aria-label="Copy raw line"
    >
      {done ? <Check className="size-3.5" /> : <Copy className="size-3.5" />}
      {done ? "Copied" : "Copy"}
    </button>
  );
}
