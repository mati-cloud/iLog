import type { ReactNode } from "react";
import { Card } from "@/components/ui/card";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";

export interface Column<T> {
  key: string;
  label: string;
  width?: string;
  align?: "left" | "center" | "right";
  render: (item: T) => ReactNode;
}

interface DataTableProps<T> {
  columns: Column<T>[];
  data: T[];
  getRowKey: (item: T) => string;
  emptyState?: ReactNode;
}

export function DataTable<T>({
  columns,
  data,
  getRowKey,
  emptyState,
}: DataTableProps<T>) {
  if (data.length === 0 && emptyState) {
    return <>{emptyState}</>;
  }

  return (
    <Card>
      <Table>
        <TableHeader>
          <TableRow>
            {columns.map((column, index) => (
              <TableHead
                key={column.key}
                className={`
                  ${column.width || ""}
                  ${column.align === "right" ? "text-right" : ""}
                  ${column.align === "center" ? "text-center" : ""}
                  ${index === 0 ? "pl-6" : ""}
                  ${index === columns.length - 1 ? "pr-6" : ""}
                `}
              >
                {column.label}
              </TableHead>
            ))}
          </TableRow>
        </TableHeader>
        <TableBody>
          {data.map((item) => (
            <TableRow key={getRowKey(item)} className="group">
              {columns.map((column, index) => (
                <TableCell
                  key={`${getRowKey(item)}-${column.key}`}
                  className={`
                    ${column.align === "right" ? "text-right" : ""}
                    ${column.align === "center" ? "text-center" : ""}
                    ${index === 0 ? "pl-6" : ""}
                    ${index === columns.length - 1 ? "pr-6" : ""}
                  `}
                >
                  {column.render(item)}
                </TableCell>
              ))}
            </TableRow>
          ))}
        </TableBody>
      </Table>
    </Card>
  );
}
