"use client";

import {
  Area,
  AreaChart,
  CartesianGrid,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from "recharts";

interface LogVolumeChartProps {
  data: Array<{
    hour: string;
    logs: number;
    errors: number;
  }>;
}

const CustomTooltip = ({ active, payload, label }: any) => {
  if (active && payload && payload.length) {
    return (
      <div className="rounded border bg-card p-3 shadow-md border-border">
        <p className="text-xs font-medium text-foreground mb-1.5">{label}</p>
        <div className="space-y-0.5">
          <p className="text-xs">
            <span className="text-muted-foreground">Logs: </span>
            <span className="font-medium text-primary">
              {payload[0]?.value?.toLocaleString()}
            </span>
          </p>
          <p className="text-xs">
            <span className="text-muted-foreground">Errors: </span>
            <span className="font-medium text-destructive">
              {payload[1]?.value?.toLocaleString()}
            </span>
          </p>
        </div>
      </div>
    );
  }
  return null;
};

export default function LogVolumeChart({ data }: LogVolumeChartProps) {
  const hasData = data && data.length > 0;

  return (
    <div className="bg-card border border-border rounded-lg p-4">
      <div className="mb-4 flex items-center justify-between">
        <div>
          <h3 className="text-sm font-medium text-foreground">Log Volume</h3>
          <p className="text-xs text-muted-foreground">Ingestion rate over 24h</p>
        </div>
        {hasData && (
          <div className="flex items-center gap-4">
            <div className="flex items-center gap-1.5">
              <div className="h-2 w-2 rounded-full bg-primary" />
              <span className="text-[11px] text-muted-foreground">Logs</span>
            </div>
            <div className="flex items-center gap-1.5">
              <div className="h-2 w-2 rounded-full bg-destructive" />
              <span className="text-[11px] text-muted-foreground">Errors</span>
            </div>
          </div>
        )}
      </div>
      <div className="h-[240px]">
        {hasData ? (
          <ResponsiveContainer width="100%" height="100%">
            <AreaChart data={data} margin={{ top: 5, right: 5, left: -20, bottom: 0 }}>
              <defs>
                <linearGradient id="logGradient" x1="0" y1="0" x2="0" y2="1">
                  <stop offset="0%" stopColor="hsl(265 80% 60%)" stopOpacity={0.25} />
                  <stop offset="100%" stopColor="hsl(265 80% 60%)" stopOpacity={0} />
                </linearGradient>
                <linearGradient id="errorGradient" x1="0" y1="0" x2="0" y2="1">
                  <stop offset="0%" stopColor="hsl(0 72% 51%)" stopOpacity={0.15} />
                  <stop offset="100%" stopColor="hsl(0 72% 51%)" stopOpacity={0} />
                </linearGradient>
              </defs>
              <CartesianGrid strokeDasharray="3 3" stroke="hsl(0 0% 14%)" vertical={false} />
              <XAxis
                dataKey="hour"
                axisLine={false}
                tickLine={false}
                tick={{ fill: "hsl(0 0% 50%)", fontSize: 10 }}
                dy={8}
              />
              <YAxis
                axisLine={false}
                tickLine={false}
                tick={{ fill: "hsl(0 0% 50%)", fontSize: 10 }}
                tickFormatter={(value: number) => `${(value / 1000).toFixed(0)}k`}
              />
              <Tooltip content={<CustomTooltip />} />
              <Area
                type="monotone"
                dataKey="logs"
                stroke="hsl(265 80% 60%)"
                strokeWidth={1.5}
                fill="url(#logGradient)"
              />
              <Area
                type="monotone"
                dataKey="errors"
                stroke="hsl(0 72% 51%)"
                strokeWidth={1.5}
                fill="url(#errorGradient)"
              />
            </AreaChart>
          </ResponsiveContainer>
        ) : (
          <div className="flex h-full items-center justify-center">
            <p className="text-sm text-muted-foreground">No data available</p>
          </div>
        )}
      </div>
    </div>
  );
}
