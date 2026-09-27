"use client";

import { useMemo, useRef, useState } from "react";
import { parseQuery } from "@/lib/log-query";

interface QueryBarProps {
  value: string;
  onChange: (v: string) => void;
  onSubmit: () => void;
  /** Attribute names seen in the current results, offered as completions. */
  fields: string[];
  dirty: boolean;
}

const EXAMPLES = [
  ["status:>=500", "server errors"],
  ["-path:/health*", "hide health checks"],
  ["method:POST,PUT", "any of several"],
  ['"connection reset"', "exact phrase"],
  ["user_id:*", "field is present"],
] as const;

/**
 * Plain input over a mirrored copy of its text. The mirror paints the syntax;
 * the input keeps native caret, selection, IME and undo. Both share one font
 * metric, so they line up character for character.
 */
export function QueryBar({
  value,
  onChange,
  onSubmit,
  fields,
  dirty,
}: QueryBarProps) {
  const inputRef = useRef<HTMLInputElement>(null);
  const mirrorRef = useRef<HTMLDivElement>(null);
  const [focused, setFocused] = useState(false);
  const [caret, setCaret] = useState(value.length);

  const segments = useMemo(() => {
    const out: { text: string; cls: string }[] = [];
    let at = 0;
    for (const t of parseQuery(value)) {
      if (t.start > at) out.push({ text: value.slice(at, t.start), cls: "" });
      const raw = value.slice(t.start, t.end);
      const neg = t.negate ? "q-neg" : "";
      if (t.field) {
        out.push({ text: raw.slice(0, t.keyLen), cls: `q-key ${neg}` });
        out.push({ text: raw.slice(t.keyLen), cls: `q-val ${neg}` });
      } else {
        out.push({ text: raw, cls: t.negate ? "q-neg" : "q-text" });
      }
      at = t.end;
    }
    if (at < value.length) out.push({ text: value.slice(at), cls: "" });
    return out;
  }, [value]);

  // The word under the caret, for field completion.
  const partial =
    value.slice(0, caret).match(/(^|\s)-?([A-Za-z0-9_.@-]*)$/)?.[2] ?? "";
  const completions = focused
    ? fields.filter((f) => f.startsWith(partial) && f !== partial).slice(0, 8)
    : [];

  const complete = (field: string) => {
    const before = value.slice(0, caret - partial.length);
    const next = `${before}${field}:${value.slice(caret)}`;
    onChange(next);
    requestAnimationFrame(() => {
      const pos = before.length + field.length + 1;
      inputRef.current?.setSelectionRange(pos, pos);
      setCaret(pos);
      inputRef.current?.focus();
    });
  };

  return (
    <div className="relative min-w-0 flex-1">
      <form
        onSubmit={(e) => {
          e.preventDefault();
          onSubmit();
        }}
        className="query-shell"
        data-dirty={dirty || undefined}
      >
        <span aria-hidden className="query-prompt">
          ›
        </span>
        <div className="relative min-w-0 flex-1">
          <div ref={mirrorRef} aria-hidden className="query-mirror">
            {segments.map((s, i) => (
              // biome-ignore lint/suspicious/noArrayIndexKey: segments are positional
              <span key={i} className={s.cls}>
                {s.text}
              </span>
            ))}
          </div>
          <input
            ref={inputRef}
            value={value}
            onChange={(e) => onChange(e.target.value)}
            onSelect={(e) =>
              setCaret(e.currentTarget.selectionStart ?? value.length)
            }
            onScroll={(e) => {
              if (mirrorRef.current)
                mirrorRef.current.scrollLeft = e.currentTarget.scrollLeft;
            }}
            onFocus={() => setFocused(true)}
            onBlur={() => setTimeout(() => setFocused(false), 120)}
            onKeyDown={(e) => {
              if (e.key === "Tab" && completions.length > 0 && partial) {
                e.preventDefault();
                complete(completions[0]);
              }
              if (e.key === "Escape") inputRef.current?.blur();
            }}
            spellCheck={false}
            autoComplete="off"
            aria-label="Search logs"
            placeholder="Search, or filter with field:value"
            className="query-input"
          />
        </div>
        {dirty && (
          <button type="submit" className="query-run">
            Search <kbd>↵</kbd>
          </button>
        )}
      </form>

      {focused && (
        // Keeps focus in the input while clicking a suggestion; the buttons
        // inside are the interactive elements.
        // biome-ignore lint/a11y/noStaticElementInteractions: focus guard only
        <div className="query-help" onMouseDown={(e) => e.preventDefault()}>
          {completions.length > 0 && (
            <div className="query-help-row">
              <span className="query-help-label">Fields</span>
              <div className="flex flex-wrap gap-1">
                {completions.map((f) => (
                  <button
                    key={f}
                    type="button"
                    className="query-chip"
                    onClick={() => complete(f)}
                  >
                    {f}
                  </button>
                ))}
              </div>
            </div>
          )}
          {!value && (
            <div className="query-help-row">
              <span className="query-help-label">Try</span>
              <dl className="grid grid-cols-[auto_1fr] gap-x-4 gap-y-1">
                {EXAMPLES.map(([q, what]) => (
                  <div key={q} className="contents">
                    <dt>
                      <button
                        type="button"
                        className="query-example"
                        onClick={() => onChange(q)}
                      >
                        {q}
                      </button>
                    </dt>
                    <dd className="text-[var(--ink-soft)]">{what}</dd>
                  </div>
                ))}
              </dl>
            </div>
          )}
          <p className="query-help-foot">
            Terms combine with AND. Prefix <code>-</code> to exclude.{" "}
            <code>*</code> matches anything. Tab completes a field.
          </p>
        </div>
      )}
    </div>
  );
}
