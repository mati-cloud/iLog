
import { TableHead, TableHeader, TableRow } from "@/components/ui/table";
import type { LogSourceType } from "@/lib/log-utils";

interface LogTableHeadersProps {
  sourceType: LogSourceType;
}

export function LogTableHeaders({ sourceType }: LogTableHeadersProps) {
  switch (sourceType) {
    case "http":
      return (
        <TableHeader>
          <TableRow>
            <TableHead className="w-[120px]">Time</TableHead>
            <TableHead className="w-[100px]">Method</TableHead>
            <TableHead>Path</TableHead>
            <TableHead className="w-[100px]">Status</TableHead>
            <TableHead className="w-[100px]">Duration</TableHead>
            <TableHead className="w-[140px]">Client IP</TableHead>
            <TableHead className="w-[50px]"></TableHead>
          </TableRow>
        </TableHeader>
      );

    case "docker":
      return (
        <TableHeader>
          <TableRow>
            <TableHead className="w-[120px]">Time</TableHead>
            <TableHead className="w-[100px]">Level</TableHead>
            <TableHead className="w-[200px]">Container</TableHead>
            <TableHead className="w-[200px]">Image</TableHead>
            <TableHead>Message</TableHead>
            <TableHead className="w-[50px]"></TableHead>
          </TableRow>
        </TableHeader>
      );

    case "journald":
      return (
        <TableHeader>
          <TableRow>
            <TableHead className="w-[120px]">Time</TableHead>
            <TableHead className="w-[100px]">Level</TableHead>
            <TableHead className="w-[200px]">Unit</TableHead>
            <TableHead>Message</TableHead>
            <TableHead className="w-[50px]"></TableHead>
          </TableRow>
        </TableHeader>
      );
    default:
      return (
        <TableHeader>
          <TableRow>
            <TableHead className="w-[120px]">Time</TableHead>
            <TableHead className="w-[100px]">Level</TableHead>
            <TableHead className="w-[150px]">Source</TableHead>
            <TableHead className="w-[200px]">Path</TableHead>
            <TableHead>Message</TableHead>
            <TableHead className="w-[50px]"></TableHead>
          </TableRow>
        </TableHeader>
      );
  }
}
