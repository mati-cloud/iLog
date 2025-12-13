export type LogSourceType = "http" | "file" | "docker" | "journald" | "unknown";

export type LogAttributes = Record<string, unknown>;

export function detectLogSourceType(
  logAttributes?: LogAttributes,
): LogSourceType {
  if (!logAttributes) return "unknown";

  if (
    logAttributes["http.method"] ||
    logAttributes["http.path"] ||
    logAttributes["http.status_code"]
  ) {
    return "http";
  }

  if (
    logAttributes["container.name"] ||
    logAttributes["container.id"] ||
    logAttributes["docker.container"]
  ) {
    return "docker";
  }

  if (logAttributes["systemd.unit"] || logAttributes["journal.unit"]) {
    return "journald";
  }

  if (logAttributes.file_path || logAttributes.source_type === "file") {
    return "file";
  }

  return "unknown";
}

export interface HttpLogData {
  method: string;
  path: string;
  statusCode: number;
  duration?: number;
  ip?: string;
  userAgent?: string;
}

export function extractHttpLogData(
  logAttributes?: LogAttributes,
): HttpLogData | null {
  if (!logAttributes) return null;

  const method = logAttributes["http.method"] as string;
  const path = logAttributes["http.path"] as string;
  const statusCode = logAttributes["http.status_code"] as number;

  if (!method || !path || !statusCode) return null;

  return {
    method,
    path,
    statusCode,
    duration: logAttributes["http.response_time_ms"]
      ? Number(logAttributes["http.response_time_ms"])
      : undefined,
    ip: logAttributes["http.client_ip"] as string | undefined,
    userAgent: logAttributes["http.user_agent"] as string | undefined,
  };
}

export interface DockerLogData {
  containerName?: string;
  containerId?: string;
  image?: string;
}

export function extractDockerLogData(
  logAttributes?: LogAttributes,
): DockerLogData | null {
  if (!logAttributes) return null;

  return {
    containerName: (logAttributes["container.name"] ||
      logAttributes["docker.container"]) as string | undefined,
    containerId: logAttributes["container.id"] as string | undefined,
    image: (logAttributes["container.image"] ||
      logAttributes["docker.image"]) as string | undefined,
  };
}

export interface JournaldLogData {
  unit?: string;
  message?: string;
}

export function extractJournaldLogData(
  logAttributes?: LogAttributes,
): JournaldLogData | null {
  if (!logAttributes) return null;

  return {
    unit: (logAttributes["systemd.unit"] || logAttributes["journal.unit"]) as
      | string
      | undefined,
    message: logAttributes.message as string | undefined,
  };
}

export interface FileLogData {
  filePath?: string;
}

export function extractFileLogData(
  logAttributes?: LogAttributes,
): FileLogData | null {
  if (!logAttributes) return null;

  return {
    filePath: logAttributes.file_path as string | undefined,
  };
}

export function generateCurlCommand(
  httpData: HttpLogData,
  body?: string,
  host?: string,
): string {
  const { method, path } = httpData;

  const domain = host || "localhost:3000";
  const protocol = domain.includes("localhost") ? "http" : "https";

  let curl = `curl -X ${method}`;

  if (body && ["POST", "PUT", "PATCH"].includes(method)) {
    curl += ` \\\n  -H "Content-Type: application/json" \\\n  -d '${body}'`;
  }

  curl += ` \\\n  "${protocol}://${domain}${path}"`;

  return curl;
}
