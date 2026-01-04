"use client";

import { cn } from "@/lib/utils";

interface Agent {
  id: string;
  name: string;
  service_name: string;
  status: string;
  last_seen_human: string;
  logs_today: number;
}

interface AgentsListProps {
  agents: Agent[];
  onAgentClick?: (agent: Agent) => void;
}

export default function AgentsList({ agents, onAgentClick }: AgentsListProps) {
  const formatLogsCount = (count: number) => {
    if (count >= 1000000) {
      return `${(count / 1000000).toFixed(1)}M`;
    } else if (count >= 1000) {
      return `${(count / 1000).toFixed(1)}K`;
    }
    return count.toString();
  };

  const hasData = agents && agents.length > 0;

  return (
    <div className="bg-card border border-border rounded-lg p-4">
      <div className="mb-4">
        <h3 className="text-sm font-medium text-foreground">Connected Agents</h3>
        <p className="text-xs text-muted-foreground">Active log collectors</p>
      </div>
      <div className="space-y-2">
        {hasData ? (
          agents.map((agent) => (
            <div
              key={agent.id}
              className={cn(
                "flex items-center justify-between py-2 px-2.5 rounded bg-secondary/50 transition-colors hover:bg-secondary",
                onAgentClick && "cursor-pointer"
              )}
              onClick={() => onAgentClick?.(agent)}
            >
              <div className="flex items-center gap-2.5">
                <div
                  className={cn(
                    "h-2 w-2 rounded-full",
                    agent.status === "online" ? "bg-accent" : "bg-muted-foreground"
                  )}
                />
                <div>
                  <p className="text-xs font-medium text-foreground font-mono">
                    {agent.name}
                  </p>
                  <p className="text-[11px] text-muted-foreground font-mono">
                    {agent.service_name}
                  </p>
                </div>
              </div>
              <div className="text-right">
                <p className="text-xs font-medium text-foreground">
                  {formatLogsCount(agent.logs_today)}
                </p>
                <p className="text-[11px] text-muted-foreground">
                  {agent.last_seen_human}
                </p>
              </div>
            </div>
          ))
        ) : (
          <div className="text-center py-8">
            <p className="text-sm text-muted-foreground">No agents connected</p>
          </div>
        )}
      </div>
    </div>
  );
}
