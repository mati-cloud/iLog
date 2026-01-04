"use client";

import {
  Bar,
  BarChart,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
  Cell,
} from "recharts";

interface StorageByServiceProps {
  data: Array<{
    service: string;
    storage_gb: number;
  }>;
}

const COLORS = [
  "hsl(265 80% 60%)",
  "hsl(265 80% 55%)",
  "hsl(265 80% 50%)",
  "hsl(265 80% 45%)",
  "hsl(265 80% 40%)",
  "hsl(265 80% 35%)",
];

const CustomTooltip = ({ active, payload, label }: any) => {
  if (active && payload && payload.length) {
    return (
      <div className="rounded border bg-card p-3 shadow-md border-border">
        <p className="text-xs font-medium text-foreground">{label}</p>
        <p className="text-xs text-muted-foreground mt-0.5">
          {payload[0]?.value?.toFixed(1)} GB
        </p>
      </div>
    );
  }
  return null;
};

export default function StorageByService({ data }: StorageByServiceProps) {
  const hasData = data && data.length > 0;
  const totalStorage = hasData ? data.reduce((acc, item) => acc + item.storage_gb, 0) : 0;

  return (
    <div className="bg-card border border-border rounded-lg p-4">
      <div className="mb-4">
        <div className="flex items-center justify-between">
          <h3 className="text-sm font-medium text-foreground">Storage by Service</h3>
          {hasData && (
            <span className="text-xs text-muted-foreground">
              {totalStorage.toFixed(2)} GB total
            </span>
          )}
        </div>
        <p className="text-xs text-muted-foreground">Disk usage per service</p>
      </div>
      <div className="h-[200px]">
        {hasData ? (
          <ResponsiveContainer width="100%" height="100%">
            <BarChart data={data} layout="vertical" margin={{ top: 0, right: 0, left: 0, bottom: 0 }}>
              <XAxis
                type="number"
                axisLine={false}
                tickLine={false}
                tick={{ fill: "hsl(0 0% 50%)", fontSize: 10 }}
                tickFormatter={(value: number) => `${value}GB`}
              />
              <YAxis
                type="category"
                dataKey="service"
                axisLine={false}
                tickLine={false}
                tick={{ fill: "hsl(0 0% 50%)", fontSize: 10 }}
                width={75}
              />
              <Tooltip content={<CustomTooltip />} cursor={{ fill: "hsl(0 0% 12%)" }} />
              <Bar dataKey="storage_gb" radius={[0, 3, 3, 0]} maxBarSize={18}>
                {data.map((entry, index) => (
                  <Cell key={`cell-${index}`} fill={COLORS[index % COLORS.length]} />
                ))}
              </Bar>
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
