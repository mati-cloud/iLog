import { headers } from "next/headers";
import { redirect } from "next/navigation";
import LogoutButton from "@/components/LogoutButton";
import { Badge } from "@/components/ui/badge";
import { auth } from "@/lib/auth";
import { serverConfig } from "@/lib/server-config";
import MetricCard from "@/components/dashboard/MetricCard";
import LogVolumeChart from "@/components/dashboard/LogVolumeChart";
import StorageByService from "@/components/dashboard/StorageByService";
import AgentsList from "@/components/dashboard/AgentsList";
import DailyIngestionChart from "@/components/dashboard/DailyIngestionChart";
import {
  fetchDashboardMetrics,
  fetchLogVolume,
  fetchStorageByService,
  fetchConnectedAgents,
  fetch7DayIngestion,
} from "@/lib/api/dashboard";
import "./dashboard.css";

export default async function DashboardPage() {
  const session = await auth.api.getSession({
    headers: await headers(),
  });

  if (!session) {
    redirect("/login");
  }

  const userId = session.user.id;

  // Fetch all dashboard data in parallel
  const [metrics, logVolume, storageByService, agents, dailyIngestion] = await Promise.all([
    fetchDashboardMetrics(serverConfig.backendUrl, userId).catch((e) => {
      console.error("Failed to fetch metrics:", e);
      return null;
    }),
    fetchLogVolume(serverConfig.backendUrl, userId).catch((e) => {
      console.error("Failed to fetch log volume:", e);
      return [];
    }),
    fetchStorageByService(serverConfig.backendUrl, userId).catch((e) => {
      console.error("Failed to fetch storage by service:", e);
      return [];
    }),
    fetchConnectedAgents(serverConfig.backendUrl, userId).catch((e) => {
      console.error("Failed to fetch agents:", e);
      return [];
    }),
    fetch7DayIngestion(serverConfig.backendUrl, userId).catch((e) => {
      console.error("Failed to fetch 7-day ingestion:", e);
      return [];
    }),
  ]);

  const formatNumber = (num: number) => {
    if (num >= 1000000) {
      return `${(num / 1000000).toFixed(2)}M`;
    } else if (num >= 1000) {
      return `${(num / 1000).toFixed(1)}K`;
    }
    return num.toString();
  };

  const formatChange = (percent: number) => {
    const sign = percent >= 0 ? "+" : "";
    return `${sign}${percent.toFixed(1)}%`;
  };

  return (
    <div className="min-h-screen bg-background dashboard-theme">
      <nav className="border-b">
        <div className="max-w-7xl mx-auto flex justify-between items-center p-4">
          <div className="flex items-center gap-2">
            <h1 className="text-2xl font-bold">iLog</h1>
            <Badge variant="outline">Dashboard</Badge>
          </div>
          <div className="flex items-center gap-4">
            <span className="text-sm text-muted-foreground">
              Welcome, {session.user.name || session.user.email}
            </span>
            <LogoutButton />
          </div>
        </div>
      </nav>

      <main className="p-5 lg:p-6">
        {/* Metrics Grid */}
        <div className="mb-5 grid gap-3 sm:grid-cols-2 lg:grid-cols-4">
          <MetricCard
            title="Total Storage"
            value={metrics ? `${metrics.total_storage_gb.toFixed(1)} GB` : "0 GB"}
            subtitle="disk usage"
            icon="HardDrive"
            iconColor="text-primary"
          />
          <MetricCard
            title="Agents Online"
            value={metrics ? `${metrics.agents_online} / ${metrics.agents_total}` : "0 / 0"}
            subtitle={metrics && metrics.agents_offline > 0 ? `${metrics.agents_offline} offline` : "all online"}
            icon="Server"
            iconColor="text-accent"
          />
          <MetricCard
            title="Logs Today"
            value={metrics ? formatNumber(metrics.logs_today) : "0"}
            change={metrics ? formatChange(metrics.logs_today_change_percent) : undefined}
            changeType={metrics && metrics.logs_today_change_percent >= 0 ? "positive" : "negative"}
            subtitle="vs yesterday"
            icon="FileText"
            iconColor="text-primary"
          />
          <MetricCard
            title="Errors (24h)"
            value={metrics ? formatNumber(metrics.errors_24h) : "0"}
            change={metrics ? formatChange(metrics.errors_change_percent) : undefined}
            changeType={metrics && metrics.errors_change_percent <= 0 ? "positive" : "negative"}
            subtitle="vs previous 24h"
            icon="AlertTriangle"
            iconColor="text-warning"
          />
        </div>

        {/* Main Chart */}
        <div className="mb-5">
          <LogVolumeChart data={logVolume} />
        </div>

        {/* Secondary Section */}
        <div className="grid gap-4 lg:grid-cols-3">
          <StorageByService data={storageByService} />
          <AgentsList agents={agents} />
          <DailyIngestionChart data={dailyIngestion} />
        </div>
      </main>
    </div>
  );
}
