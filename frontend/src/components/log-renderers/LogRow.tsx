import { detectLogSourceType } from "@/lib/log-utils";
import type { RawLogData } from "@/types/log";
import { DockerLogRow } from "./DockerLogRow";
import { FileLogRow } from "./FileLogRow";
import { HttpLogRow } from "./HttpLogRow";
import { JournaldLogRow } from "./JournaldLogRow";

interface LogRowProps {
  log: RawLogData;
  isExpanded: boolean;
  onToggleExpand: () => void;
}

export function LogRow({ log, isExpanded, onToggleExpand }: LogRowProps) {
  const sourceType = detectLogSourceType(log.log_attributes);

  switch (sourceType) {
    case "http":
      return (
        <HttpLogRow
          log={log}
          isExpanded={isExpanded}
          onToggleExpand={onToggleExpand}
        />
      );
    case "docker":
      return (
        <DockerLogRow
          log={log}
          isExpanded={isExpanded}
          onToggleExpand={onToggleExpand}
        />
      );
    case "journald":
      return (
        <JournaldLogRow
          log={log}
          isExpanded={isExpanded}
          onToggleExpand={onToggleExpand}
        />
      );
    default:
      return (
        <FileLogRow
          log={log}
          isExpanded={isExpanded}
          onToggleExpand={onToggleExpand}
        />
      );
  }
}
