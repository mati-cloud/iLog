import { ChevronDown, ChevronUp, Copy } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { TableCell, TableRow } from "@/components/ui/table";
import { extractHttpLogData, generateCurlCommand } from "@/lib/log-utils";
import type { RawLogData } from "@/types/log";

interface HttpLogRowProps {
  log: RawLogData;
  isExpanded: boolean;
  onToggleExpand: () => void;
}

export function HttpLogRow({
  log,
  isExpanded,
  onToggleExpand,
}: HttpLogRowProps) {
  const httpData = extractHttpLogData(log.log_attributes);

  if (!httpData) return null;

  const getStatusBadgeVariant = (status: number) => {
    if (status >= 500) return "destructive";
    if (status >= 400) return "secondary";
    if (status >= 300) return "outline";
    return "default";
  };

  const getMethodColor = (method: string) => {
    switch (method) {
      case "GET":
        return "text-blue-500";
      case "POST":
        return "text-green-500";
      case "PUT":
        return "text-yellow-500";
      case "PATCH":
        return "text-orange-500";
      case "DELETE":
        return "text-red-500";
      default:
        return "text-gray-500";
    }
  };

  const copyCurl = () => {
    const host =
      (log.log_attributes?.["http.host"] as string) || "localhost:3000";
    const curl = generateCurlCommand(httpData, undefined, host);
    navigator.clipboard.writeText(curl);
  };

  return (
    <>
      <TableRow
        className="cursor-pointer hover:bg-muted/50"
        onClick={onToggleExpand}
      >
        <TableCell className="font-mono text-xs text-muted-foreground">
          {log.timestamp}
        </TableCell>
        <TableCell>
          <span className={`font-semibold ${getMethodColor(httpData.method)}`}>
            {httpData.method}
          </span>
        </TableCell>
        <TableCell className="font-mono text-sm max-w-md truncate">
          {httpData.path}
        </TableCell>
        <TableCell>
          <Badge variant={getStatusBadgeVariant(httpData.statusCode)}>
            {httpData.statusCode}
          </Badge>
        </TableCell>
        <TableCell className="text-muted-foreground">
          {httpData.duration !== undefined ? `${httpData.duration}ms` : "-"}
        </TableCell>
        <TableCell className="font-mono text-xs text-muted-foreground">
          {httpData.ip || "-"}
        </TableCell>
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
          <TableCell colSpan={7} className="bg-muted/30 p-4">
            <div className="space-y-4">
              <div>
                <div className="flex items-center justify-between mb-2">
                  <h4 className="text-sm font-semibold">cURL Command</h4>
                  <Button size="sm" variant="ghost" onClick={copyCurl}>
                    <Copy className="h-3 w-3 mr-1" />
                    Copy
                  </Button>
                </div>
                <pre className="bg-background p-3 rounded-md text-xs overflow-x-auto whitespace-pre-wrap">
                  {generateCurlCommand(
                    httpData,
                    undefined,
                    (log.log_attributes?.["http.host"] as string) ||
                      "localhost:3000",
                  )}
                </pre>
              </div>

              {log.log_attributes && (
                <div>
                  <h4 className="text-sm font-semibold mb-2">
                    Request Details
                  </h4>
                  <div className="grid grid-cols-2 gap-2 text-sm">
                    <div>
                      <span className="text-muted-foreground">Method:</span>
                      <span className="ml-2 font-mono">{httpData.method}</span>
                    </div>
                    <div>
                      <span className="text-muted-foreground">Status:</span>
                      <span className="ml-2 font-mono">
                        {httpData.statusCode}
                      </span>
                    </div>
                    <div>
                      <span className="text-muted-foreground">Duration:</span>
                      <span className="ml-2 font-mono">
                        {httpData.duration !== undefined
                          ? httpData.duration
                          : 0}
                        ms
                      </span>
                    </div>
                    <div>
                      <span className="text-muted-foreground">Client IP:</span>
                      <span className="ml-2 font-mono">
                        {httpData.ip || "N/A"}
                      </span>
                    </div>
                  </div>
                </div>
              )}
            </div>
          </TableCell>
        </TableRow>
      )}
    </>
  );
}
