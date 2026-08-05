import { headers } from "next/headers";
import { type NextRequest, NextResponse } from "next/server";
import { auth } from "@/lib/auth";
import { serverConfig } from "@/lib/server-config";

async function proxyRequest(request: NextRequest, path: string[]) {
  console.log("[Proxy] Request to:", path);
  const incomingHeaders = await headers();

  const session = await auth.api.getSession({
    headers: incomingHeaders,
  });

  if (!session) {
    console.log("[Proxy] No session, returning 401");
    return NextResponse.json({ error: "Unauthorized" }, { status: 401 });
  }

  const backendPath = path.join("/");
  const url = new URL(`/api/${backendPath}`, serverConfig.backendUrl);

  request.nextUrl.searchParams.forEach((value, key) => {
    url.searchParams.append(key, value);
  });

  // Mint a signed JWT for the session. The backend verifies its Ed25519
  // signature against our JWKS endpoint, so identity is proven cryptographically
  // rather than asserted by a header this proxy sets.
  const { token } = await auth.api.getToken({ headers: incomingHeaders });

  if (!token) {
    console.error("[Proxy] Session valid but token issuance failed");
    return NextResponse.json({ error: "Unauthorized" }, { status: 401 });
  }

  const requestHeaders = new Headers();
  requestHeaders.set("Content-Type", "application/json");
  requestHeaders.set("Authorization", `Bearer ${token}`);

  const options: RequestInit = {
    method: request.method,
    headers: requestHeaders,
  };

  if (["POST", "PUT", "PATCH"].includes(request.method)) {
    options.body = JSON.stringify(await request.json());
  }

  try {
    const response = await fetch(url.toString(), options);
    const data = await response.text();

    return new NextResponse(data, {
      status: response.status,
      headers: {
        "Content-Type": "application/json",
      },
    });
  } catch (error) {
    console.error("Proxy error:", error);
    return NextResponse.json(
      { error: "Failed to proxy request" },
      { status: 500 },
    );
  }
}

export async function GET(
  request: NextRequest,
  context: { params: Promise<{ path: string[] }> },
) {
  try {
    const { path } = await context.params;
    console.log("[GET] Params resolved:", path);
    return proxyRequest(request, path);
  } catch (error) {
    console.error("[GET] Error resolving params:", error);
    return NextResponse.json({ error: "Internal error" }, { status: 500 });
  }
}

export async function POST(
  request: NextRequest,
  context: { params: Promise<{ path: string[] }> },
) {
  const { path } = await context.params;
  return proxyRequest(request, path);
}

export async function PUT(
  request: NextRequest,
  context: { params: Promise<{ path: string[] }> },
) {
  const { path } = await context.params;
  return proxyRequest(request, path);
}

export async function PATCH(
  request: NextRequest,
  context: { params: Promise<{ path: string[] }> },
) {
  const { path } = await context.params;
  return proxyRequest(request, path);
}

export async function DELETE(
  request: NextRequest,
  context: { params: Promise<{ path: string[] }> },
) {
  const { path } = await context.params;
  return proxyRequest(request, path);
}
