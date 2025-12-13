import { headers } from "next/headers";
import { type NextRequest, NextResponse } from "next/server";
import { auth } from "@/lib/auth";

const BACKEND_URL = process.env.BACKEND_URL || "http://localhost:8080";

async function proxyRequest(request: NextRequest, path: string[]) {
  const incomingHeaders = await headers();

  const session = await auth.api.getSession({
    headers: incomingHeaders,
  });

  if (!session) {
    return NextResponse.json({ error: "Unauthorized" }, { status: 401 });
  }

  const backendPath = path.join("/");
  const url = new URL(`/api/${backendPath}`, BACKEND_URL);

  request.nextUrl.searchParams.forEach((value, key) => {
    url.searchParams.append(key, value);
  });

  const requestHeaders = new Headers();
  requestHeaders.set("Content-Type", "application/json");
  requestHeaders.set("X-User-Email", session.user.email);
  requestHeaders.set("X-Better-Auth-User-Id", session.user.id);
  requestHeaders.set("X-Authenticated-By", "better-auth-frontend-proxy");

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
  { params }: { params: Promise<{ path: string[] }> },
) {
  const { path } = await params;
  return proxyRequest(request, path);
}

export async function POST(
  request: NextRequest,
  { params }: { params: Promise<{ path: string[] }> },
) {
  const { path } = await params;
  return proxyRequest(request, path);
}

export async function PUT(
  request: NextRequest,
  { params }: { params: Promise<{ path: string[] }> },
) {
  const { path } = await params;
  return proxyRequest(request, path);
}

export async function PATCH(
  request: NextRequest,
  { params }: { params: Promise<{ path: string[] }> },
) {
  const { path } = await params;
  return proxyRequest(request, path);
}

export async function DELETE(
  request: NextRequest,
  { params }: { params: Promise<{ path: string[] }> },
) {
  const { path } = await params;
  return proxyRequest(request, path);
}
