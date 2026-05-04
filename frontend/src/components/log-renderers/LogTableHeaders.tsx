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
          <TableRow className="h-10">
            <TableHead className="w-[110px] py-2">Time</TableHead>
            <TableHead className="w-[80px] py-2">Method</TableHead>
            <TableHead className="py-2">Path</TableHead>
            <TableHead className="w-[80px] py-2">Status</TableHead>
            <TableHead className="w-[90px] py-2">Duration</TableHead>
            <TableHead className="w-[130px] py-2">Client IP</TableHead>
            <TableHead className="w-[150px] py-2">Service</TableHead>
            <TableHead className="w-[40px] py-2"></TableHead>
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
          <TableRow className="h-10">
            <TableHead className="w-[110px] py-2">Time</TableHead>
            <TableHead className="w-[80px] py-2">Level</TableHead>
            <TableHead className="w-[150px] py-2">Source</TableHead>
            <TableHead className="w-[200px] py-2">Path</TableHead>
            <TableHead className="py-2">Message</TableHead>
            <TableHead className="w-[40px] py-2"></TableHead>
          </TableRow>
        </TableHeader>
      );
  }
}
