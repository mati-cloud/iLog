import { ChevronDown, ChevronUp } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { TableCell, TableRow } from "@/components/ui/table";
import { extractJournaldLogData } from "@/lib/log-utils";
import type { RawLogData } from "@/types/log";

interface JournaldLogRowProps {
  log: RawLogData;
  isExpanded: boolean;
  onToggleExpand: () => void;
}

export function JournaldLogRow({
  log,
  isExpanded,
  onToggleExpand,
}: JournaldLogRowProps) {
  const journaldData = extractJournaldLogData(log.log_attributes);

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
          {log.time ? new Date(log.time).toLocaleTimeString() : "-"}
        </TableCell>
        <TableCell>
          <Badge variant={getLevelBadgeVariant(log.severity_text || "INFO")}>
            {log.severity_text || "INFO"}
          </Badge>
        </TableCell>
        <TableCell className="font-mono text-sm">
          {journaldData?.unit || "-"}
        </TableCell>
        <TableCell className="max-w-2xl truncate">{log.body}</TableCell>
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
          <TableCell colSpan={5} className="bg-muted/30 p-4">
            <div className="space-y-4">
              <div>
                <h4 className="text-sm font-semibold mb-2">Full Message</h4>
                <pre className="bg-background p-3 rounded-md text-sm whitespace-pre-wrap">
                  {log.body}
                </pre>
              </div>

              {journaldData?.unit && (
                <div>
                  <h4 className="text-sm font-semibold mb-2">Unit Details</h4>
                  <div className="text-sm">
                    <span className="text-muted-foreground">Systemd Unit:</span>
                    <span className="ml-2 font-mono">{journaldData.unit}</span>
                  </div>
                </div>
              )}

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
