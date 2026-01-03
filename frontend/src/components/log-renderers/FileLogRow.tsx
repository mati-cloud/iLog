import { ChevronDown, ChevronUp } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { TableCell, TableRow } from "@/components/ui/table";
import { extractFileLogData } from "@/lib/log-utils";
import type { Log } from "@/types/log";

interface FileLogRowProps {
  log: Log;
  isExpanded: boolean;
  onToggleExpand: () => void;
}

export function FileLogRow({
  log,
  isExpanded,
  onToggleExpand,
}: FileLogRowProps) {
  const fileData = extractFileLogData(log.log_attributes);

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
          <TableCell colSpan={6} className="bg-muted/30 p-4">
            <div className="space-y-4">
              <div>
                <h4 className="text-sm font-semibold mb-2">Full Message</h4>
                <pre className="bg-background p-3 rounded-md text-sm whitespace-pre-wrap">
                  {log.message}
                </pre>
              </div>

              {log.log_attributes && (
                <div>
                  <h4 className="text-sm font-semibold mb-2">Attributes</h4>
                  <pre className="bg-background p-3 rounded-md text-xs overflow-x-auto">
                    {JSON.stringify(log.log_attributes, null, 2)}
                  </pre>
                </div>
              )}
            </div>
          </TableCell>
        </TableRow>
      )}
    </>
  );
}
