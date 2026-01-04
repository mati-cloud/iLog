export interface DashboardMetrics {
  total_storage_bytes: number;
  total_storage_gb: number;
  agents_online: number;
  agents_offline: number;
  agents_total: number;
  logs_today: number;
  logs_yesterday: number;
  logs_today_change_percent: number;
  errors_24h: number;
  errors_previous_24h: number;
  errors_change_percent: number;
}

export interface LogVolumeDataPoint {
  hour: string;
  logs: number;
  errors: number;
}

export interface StorageByServiceDataPoint {
  service: string;
  storage_bytes: number;
  storage_gb: number;
}

export interface AgentInfo {
  id: string;
  name: string;
  service_name: string;
  status: string;
  last_seen: string | null;
  last_seen_human: string;
  logs_today: number;
}

export interface DailyIngestionDataPoint {
  day: string;
  date: string;
  logs: number;
  storage_gb: number;
}

export async function fetchDashboardMetrics(backendUrl: string, userId?: string): Promise<DashboardMetrics> {
  const headers: HeadersInit = {
    "Content-Type": "application/json",
  };
  
  if (userId) {
    headers["x-authenticated-by"] = "better-auth-frontend-proxy";
    headers["x-better-auth-user-id"] = userId;
  }
  
  const response = await fetch(`${backendUrl}/api/dashboard/metrics`, {
    headers,
    cache: "no-store",
  });
  
  if (!response.ok) {
    throw new Error(`Failed to fetch dashboard metrics: ${response.status} ${response.statusText}`);
  }
  
  return response.json();
}

export async function fetchLogVolume(backendUrl: string, userId?: string): Promise<LogVolumeDataPoint[]> {
  const headers: HeadersInit = {
    "Content-Type": "application/json",
  };
  
  if (userId) {
    headers["x-authenticated-by"] = "better-auth-frontend-proxy";
    headers["x-better-auth-user-id"] = userId;
  }
  
  const response = await fetch(`${backendUrl}/api/dashboard/log-volume`, {
    headers,
    cache: "no-store",
  });
  
  if (!response.ok) {
    throw new Error(`Failed to fetch log volume data: ${response.status} ${response.statusText}`);
  }
  
  return response.json();
}

export async function fetchStorageByService(backendUrl: string, userId?: string): Promise<StorageByServiceDataPoint[]> {
  const headers: HeadersInit = {
    "Content-Type": "application/json",
  };
  
  if (userId) {
    headers["x-authenticated-by"] = "better-auth-frontend-proxy";
    headers["x-better-auth-user-id"] = userId;
  }
  
  const response = await fetch(`${backendUrl}/api/dashboard/storage-by-service`, {
    headers,
    cache: "no-store",
  });
  
  if (!response.ok) {
    throw new Error(`Failed to fetch storage by service data: ${response.status} ${response.statusText}`);
  }
  
  return response.json();
}

export async function fetchConnectedAgents(backendUrl: string, userId?: string): Promise<AgentInfo[]> {
  const headers: HeadersInit = {
    "Content-Type": "application/json",
  };
  
  if (userId) {
    headers["x-authenticated-by"] = "better-auth-frontend-proxy";
    headers["x-better-auth-user-id"] = userId;
  }
  
  const response = await fetch(`${backendUrl}/api/dashboard/agents`, {
    headers,
    cache: "no-store",
  });
  
  if (!response.ok) {
    throw new Error(`Failed to fetch connected agents: ${response.status} ${response.statusText}`);
  }
  
  return response.json();
}

export async function fetch7DayIngestion(backendUrl: string, userId?: string): Promise<DailyIngestionDataPoint[]> {
  const headers: HeadersInit = {
    "Content-Type": "application/json",
  };
  
  if (userId) {
    headers["x-authenticated-by"] = "better-auth-frontend-proxy";
    headers["x-better-auth-user-id"] = userId;
  }
  
  const response = await fetch(`${backendUrl}/api/dashboard/7day-ingestion`, {
    headers,
    cache: "no-store",
  });
  
  if (!response.ok) {
    throw new Error(`Failed to fetch 7-day ingestion data: ${response.status} ${response.statusText}`);
  }
  
  return response.json();
}
