"use client";

import { HardDrive, Server, FileText, AlertTriangle } from "lucide-react";
import { cn } from "@/lib/utils";

const iconMap = {
  HardDrive,
  Server,
  FileText,
  AlertTriangle,
};

type IconName = keyof typeof iconMap;

interface MetricCardProps {
  title: string;
  value: string;
  subtitle?: string;
  change?: string;
  changeType?: "positive" | "negative" | "neutral";
  icon: IconName;
  iconColor?: string;
  className?: string;
  onClick?: () => void;
}

export default function MetricCard({
  title,
  value,
  subtitle,
  change,
  changeType = "neutral",
  icon,
  iconColor = "text-primary",
  className,
  onClick,
}: MetricCardProps) {
  const Icon = iconMap[icon];
  
  return (
    <div
      className={cn(
        "bg-card border border-border rounded-lg p-4 transition-colors hover:border-border/80",
        onClick && "cursor-pointer",
        className
      )}
      onClick={onClick}
    >
      <div className="flex items-start justify-between">
        <div className="space-y-1">
          <p className="text-xs font-medium text-muted-foreground uppercase tracking-wide">
            {title}
          </p>
          <p className="text-2xl font-semibold tracking-tight text-foreground">
            {value}
          </p>
          {(subtitle || change) && (
            <div className="flex items-center gap-1.5">
              {change && (
                <span
                  className={cn(
                    "text-xs font-medium",
                    changeType === "positive" && "text-accent",
                    changeType === "negative" && "text-destructive",
                    changeType === "neutral" && "text-muted-foreground"
                  )}
                >
                  {change}
                </span>
              )}
              {subtitle && (
                <span className="text-xs text-muted-foreground">{subtitle}</span>
              )}
            </div>
          )}
        </div>
        <div
          className={cn(
            "flex h-9 w-9 items-center justify-center rounded bg-secondary",
            iconColor
          )}
        >
          <Icon className="h-4 w-4" />
        </div>
      </div>
    </div>
  );
}
