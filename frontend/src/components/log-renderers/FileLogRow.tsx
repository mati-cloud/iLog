import { useState } from "react";
import { ChevronDown, ChevronUp, Clock, Server, FileText, Tag, Terminal, Copy, Check } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { TableCell, TableRow } from "@/components/ui/table";
import { extractFileLogData } from "@/lib/log-utils";
import type { Log } from "@/types/log";
import { cn } from "@/lib/utils";

interface FileLogRowProps {
  log: Log;
  isExpanded: boolean;
  onToggleExpand: () => void;
}

interface ParsedHttpRequest {
  method: string;
  path: string;
  protocol?: string;
  statusCode?: string;
  ip?: string;
  userAgent?: string;
  body?: string;
}

function parseHttpRequest(message: string): ParsedHttpRequest | null {
  const nginxPattern = /^([\d.]+)\s+-\s+-\s+\[.*?\]\s+"(\w+)\s+([^\s]+)\s+(HTTP\/[\d.]+)"\s+(\d+)/;
  const match = message.match(nginxPattern);
  
  if (match) {
    return {
      ip: match[1],
      method: match[2],
      path: match[3],
      protocol: match[4],
      statusCode: match[5],
      userAgent: message.match(/"([^"]*)"$/)?.[1]
    };
  }
  
  const simplePattern = /"(GET|POST|PUT|DELETE|PATCH|HEAD|OPTIONS)\s+([^\s"]+)/;
  const simpleMatch = message.match(simplePattern);
  
  if (simpleMatch) {
    return {
      method: simpleMatch[1],
      path: simpleMatch[2]
    };
  }
  
  return null;
}

function generateCurlCommand(request: ParsedHttpRequest, log: Log): string {
  const host = log.log_attributes?.host || "localhost";
  const url = request.path.startsWith("http") ? request.path : `https://${host}${request.path}`;
  
  let curl = "curl";
  
  if (request.method !== "GET") {
    curl += ` -X ${request.method}`;
  }
  
  curl += ` "${url}"`;
  
  if (request.userAgent) {
    curl += ` \\\n  -H "User-Agent: ${request.userAgent}"`;
  }
  
  if (request.method === "POST" || request.method === "PUT" || request.method === "PATCH") {
    curl += ` \\\n  -H "Content-Type: application/json"`;
    
    const requestBody = log.requestData?.body;
    if (requestBody && typeof requestBody === "string") {
      try {
        const parsed = JSON.parse(requestBody);
        const sanitized = sanitizeRequestBody(parsed);
        curl += ` \\\n  -d '${JSON.stringify(sanitized)}'`;
      } catch {
        curl += ` \\\n  -d '{}'`;
      }
    } else {
      curl += ` \\\n  -d '{}'`;
    }
  }
  
  return curl;
}

function sanitizeRequestBody(body: any): any {
  if (typeof body !== "object" || body === null) return body;
  
  const sensitiveFields = ["password", "passwd", "pwd", "secret", "token", "api_key", "apikey", "auth", "authorization"];
  const sanitized = Array.isArray(body) ? [...body] : { ...body };
  
  for (const key in sanitized) {
    if (sensitiveFields.some(field => key.toLowerCase().includes(field))) {
      sanitized[key] = "***REDACTED***";
    } else if (typeof sanitized[key] === "object" && sanitized[key] !== null) {
      sanitized[key] = sanitizeRequestBody(sanitized[key]);
    }
  }
  
  return sanitized;
}

function CopyButton({ text }: { text: string }) {
  const [copied, setCopied] = useState(false);
  
  const handleCopy = async (e: React.MouseEvent) => {
    e.stopPropagation();
    await navigator.clipboard.writeText(text);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };
  
  return (
    <button
      onClick={handleCopy}
      className="p-1.5 text-muted-foreground hover:text-foreground hover:bg-secondary rounded transition-colors"
    >
      {copied ? <Check className="w-3.5 h-3.5" /> : <Copy className="w-3.5 h-3.5" />}
    </button>
  );
}

export function FileLogRow({
  log,
  isExpanded,
  onToggleExpand,
}: FileLogRowProps) {
  const fileData = extractFileLogData(log.log_attributes);
  const httpRequest = parseHttpRequest(log.message);
  const curlCommand = httpRequest ? generateCurlCommand(httpRequest, log) : null;

  const getLevelBadgeVariant = (level: string) => {
    switch (level?.toUpperCase()) {
      case "ERROR":
        return "destructive";
      case "WARN":
        return "secondary";
      case "INFO":
        return "default";
      case "DEBUG":
        return "outline";
      default:
        return "outline";
    }
  };

  return (
    <>
      <TableCell className="font-mono text-xs text-muted-foreground cursor-pointer" onClick={onToggleExpand}>
        {log.timestamp || "-"}
      </TableCell>
      <TableCell className="cursor-pointer" onClick={onToggleExpand}>
        <Badge variant={getLevelBadgeVariant(log.level)}>
          {log.level}
        </Badge>
      </TableCell>
      <TableCell className="font-mono text-sm cursor-pointer" onClick={onToggleExpand}>
        {log.source || "-"}
      </TableCell>
      <TableCell className="font-mono text-xs text-muted-foreground max-w-xs truncate cursor-pointer" onClick={onToggleExpand}>
        {log.filePath || "-"}
      </TableCell>
      <TableCell className="max-w-2xl truncate cursor-pointer" onClick={onToggleExpand}>{log.message}</TableCell>
      <TableCell className="text-right cursor-pointer" onClick={onToggleExpand}>
        {isExpanded ? (
          <ChevronUp className="h-4 w-4" />
        ) : (
          <ChevronDown className="h-4 w-4" />
        )}
      </TableCell>
    </>
  );
}
