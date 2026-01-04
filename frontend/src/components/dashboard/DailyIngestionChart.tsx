"use client";

import { Bar, BarChart, ResponsiveContainer, Tooltip, XAxis } from "recharts";

interface DailyIngestionChartProps {
  data: Array<{
    day: string;
    storage_gb: number;
  }>;
}

const CustomTooltip = ({ active, payload, label }: any) => {
  if (active && payload && payload.length) {
    return (
      <div className="rounded border bg-card p-3 shadow-md border-border">
        <p className="text-xs font-medium text-foreground">{label}</p>
        <p className="text-xs text-muted-foreground mt-0.5">
          {payload[0]?.value?.toFixed(1)} GB ingested
        </p>
      </div>
    );
  }
  return null;
};

export default function DailyIngestionChart({ data }: DailyIngestionChartProps) {
  const hasData = data && data.length > 0;

  return (
    <div className="bg-card border border-border rounded-lg p-4">
      <div className="mb-4">
        <h3 className="text-sm font-medium text-foreground">7-Day Ingestion</h3>
        <p className="text-xs text-muted-foreground">Daily log volume (GB)</p>
      </div>
      <div className="h-[140px]">
        {hasData ? (
          <ResponsiveContainer width="100%" height="100%">
            <BarChart data={data}>
              <XAxis
                dataKey="day"
                axisLine={false}
                tickLine={false}
                tick={{ fill: "hsl(0 0% 50%)", fontSize: 10 }}
              />
              <Tooltip content={<CustomTooltip />} cursor={{ fill: "hsl(0 0% 12%)" }} />
              <Bar
                dataKey="storage_gb"
                fill="hsl(265 80% 60%)"
                radius={[3, 3, 0, 0]}
                maxBarSize={32}
              />
            </BarChart>
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
