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
      <TableRow
        className="cursor-pointer hover:bg-muted/50"
        onClick={onToggleExpand}
      >
        <TableCell className="font-mono text-xs text-muted-foreground">
          {log.timestamp || "-"}
        </TableCell>
        <TableCell>
          <Badge variant={getLevelBadgeVariant(log.level)}>
            {log.level}
          </Badge>
        </TableCell>
        <TableCell className="font-mono text-sm">
          {log.source || "-"}
        </TableCell>
        <TableCell className="font-mono text-xs text-muted-foreground max-w-xs truncate">
          {log.filePath || "-"}
        </TableCell>
        <TableCell className="max-w-2xl truncate">{log.message}</TableCell>
        <TableCell className="text-right">
          {isExpanded ? (
            <ChevronUp className="h-4 w-4" />
          ) : (
            <ChevronDown className="h-4 w-4" />
          )}
        </TableCell>
      </TableRow>

      {isExpanded && (
        <TableRow>
          <TableCell colSpan={6} className="bg-muted/30 p-0">
            <div className="animate-in slide-in-from-top-1 duration-200">
              <div className="overflow-hidden">
                {/* Context Section */}
                <div className="grid grid-cols-2 gap-px bg-border">
                  <div className="bg-card p-3">
                    <div className="flex items-center gap-2 text-muted-foreground mb-1.5">
                      <Server className="w-3 h-3" />
                      <span className="text-[10px] uppercase tracking-wider font-medium">Source</span>
                    </div>
                    <p className="text-xs font-mono text-foreground">
                      {log.filePath ? `${log.filePath}/${log.source}` : log.source}
                    </p>
                  </div>
                  <div className="bg-card p-3">
                    <div className="flex items-center gap-2 text-muted-foreground mb-1.5">
                      <Clock className="w-3 h-3" />
                      <span className="text-[10px] uppercase tracking-wider font-medium">Timestamp</span>
                    </div>
                    <p className="text-xs font-mono text-foreground">{log.timestamp}</p>
                  </div>
                </div>
                
                {/* Full Message */}
                <div className="p-3 border-t border-border">
                  <div className="flex items-center justify-between mb-2">
                    <div className="flex items-center gap-2 text-muted-foreground">
                      <FileText className="w-3 h-3" />
                      <span className="text-[10px] uppercase tracking-wider font-medium">Message</span>
                    </div>
                    <CopyButton text={log.message} />
                  </div>
                  <p className="text-xs font-mono text-foreground leading-relaxed break-all">
                    {log.message}
                  </p>
                </div>
                
                {/* Curl Command - only for HTTP requests */}
                {curlCommand && (
                  <div className="p-3 border-t border-border">
                    <div className="flex items-center justify-between mb-2">
                      <div className="flex items-center gap-2 text-muted-foreground">
                        <Terminal className="w-3 h-3" />
                        <span className="text-[10px] uppercase tracking-wider font-medium">Reproduce</span>
                        {httpRequest && (
                          <span className="text-[10px] bg-secondary px-1.5 py-0.5 rounded font-mono">
                            {httpRequest.method}
                          </span>
                        )}
                      </div>
                      <CopyButton text={curlCommand} />
                    </div>
                    <pre className="text-xs font-mono text-foreground/80 leading-relaxed whitespace-pre-wrap bg-background/50 p-2 rounded">
                      {curlCommand}
                    </pre>
                  </div>
                )}
                
                {/* Metadata */}
                {log.log_attributes && Object.keys(log.log_attributes).length > 0 && (
                  <div className="p-3 border-t border-border">
                    <div className="flex items-center gap-2 text-muted-foreground mb-2">
                      <Tag className="w-3 h-3" />
                      <span className="text-[10px] uppercase tracking-wider font-medium">Metadata</span>
                    </div>
                    <div className="flex flex-wrap gap-2">
                      {Object.entries(log.log_attributes).map(([key, value]) => (
                        <div key={key} className="inline-flex items-center gap-1.5 bg-secondary/50 px-2 py-1 rounded text-[11px]">
                          <span className="text-muted-foreground">{key}:</span>
                          <span className="text-foreground font-mono">{String(value)}</span>
                        </div>
                      ))}
                    </div>
                  </div>
                )}
              </div>
            </div>
          </TableCell>
        </TableRow>
      )}
    </>
  );
}
