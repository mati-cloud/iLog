import { NextResponse } from "next/server";

const ILOG_ENDPOINT =
  process.env.NEXT_PUBLIC_ILOG_ENDPOINT || "http://backend:8080/v1/logs";
const ILOG_TOKEN =
  process.env.ILOG_HTTP_TOKEN ||
  "proj_c71ad61150ac40ab8bbd93e1b62c05c2_32hEhVtorzB4Aj2tQuxh7bmLY7HJcVkh";
const SERVICE_NAME = process.env.NEXT_PUBLIC_SERVICE_NAME || "ilog-frontend";

// Helper to send logs to iLog in OTLP format
async function sendHttpLog(
  method: string,
  path: string,
  statusCode: number,
  responseTimeMs: number,
  userAgent: string | null,
  ip: string | null,
  host: string | null,
) {
  if (!ILOG_TOKEN) {
    return;
  }

  const severityText =
    statusCode >= 500 ? "ERROR" : statusCode >= 400 ? "WARN" : "INFO";
  const severityNumber = statusCode >= 500 ? 17 : statusCode >= 400 ? 13 : 9;

  const logAttributes: Record<string, string | number> = {
    "http.method": method,
    "http.path": path,
    "http.status_code": statusCode,
    "http.response_time_ms": responseTimeMs,
  };

  if (userAgent) {
    logAttributes["http.user_agent"] = userAgent;
  }

  if (ip) {
    logAttributes["http.client_ip"] = ip;
  }

  if (host) {
    logAttributes["http.host"] = host;
  }

  const payload = [
    {
      timeUnixNano: String(Date.now() * 1000000),
      severityText,
      severityNumber,
      serviceName: SERVICE_NAME,
      body: `${method} ${path} ${statusCode}`,
      logAttributes,
    },
  ];

  try {
    await fetch(ILOG_ENDPOINT, {
      method: "POST",
      headers: {
        Authorization: `Bearer ${ILOG_TOKEN}`,
        "Content-Type": "application/json",
      },
      body: JSON.stringify(payload),
    });
  } catch (error) {
    console.error("Failed to send HTTP log to iLog:", error);
  }
}

export async function proxy(request: NextRequest) {
  const startTime = Date.now();

  const method = request.method;
  const path = request.nextUrl.pathname;
  const userAgent = request.headers.get("user-agent");
  const ip =
    request.headers.get("x-forwarded-for") || request.headers.get("x-real-ip");
  const host = request.headers.get("host");

  const response = NextResponse.next();

  const responseTimeMs = Date.now() - startTime;

  sendHttpLog(
    method,
    path,
    response.status,
    responseTimeMs,
    userAgent,
    ip,
    host,
  ).catch(() => {});

  return response;
}

export const config = {
  matcher: [
    /*
     * Match all request paths except:
     * - /api/* (API routes - avoid logging internal proxy requests)
     * - _next/static (static files)
     * - _next/image (image optimization files)
     * - favicon.ico (favicon file)
     * - public files (public folder)
     */
    "/((?!api/|_next/static|_next/image|favicon.ico|.*\\.(?:svg|png|jpg|jpeg|gif|webp)$).*)",
  ],
};
