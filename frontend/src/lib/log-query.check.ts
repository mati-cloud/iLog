// Self-check for the search language: `bun src/lib/log-query.check.ts`.
// Mirrors backend/src/query.rs tests; a failure throws.
import {
  filterTerm,
  matches,
  normalize,
  parseQuery,
  withTerm,
} from "./log-query";

const log = normalize({
  timeUnixNano: "1759000000000000000",
  serviceName: "gitlab-nginx",
  body: "GET /api/v4/x 502 connection reset",
  severityText: "INFO",
  logAttributes: {
    status: "502",
    path: "/api/v4/x",
    method: "GET",
    user: "bob",
  },
});
const cases: [string, boolean][] = [
  ["status:>=500", true],
  ["status:<500", false],
  ["status:500,502", true],
  ["-status:502", false],
  ["path:/api/*", true],
  ["path:/web/*", false],
  ['"connection reset"', true],
  ["-healthz", true],
  ["level:info", true],
  ["source:gitlab-nginx", true],
  ["user_id:*", false],
  ["-user_id:*", true],
  ["message:reset", true],
  ["https://x", false],
  ['"error: x"', false],
];
for (const [q, want] of cases) {
  if (matches(log, parseQuery(q)) !== want)
    throw new Error(`${q}: expected ${want}`);
}
if (
  withTerm("status:200 foo", filterTerm("status", "500")) !== "foo status:500"
)
  throw new Error("withTerm should replace same-field term");
if (filterTerm("path", "/a b") !== 'path:"/a b"')
  throw new Error("filterTerm should quote");
if (parseQuery("-status:5*")[0].keyLen !== 8) throw new Error("keyLen");
console.log("log-query: ok");
