
export type LogLevel = "INFO" | "WARN" | "ERROR" | "DEBUG";

export interface RawLogData {
  id?: string;
  time?: string;
  time_unix_nano?: string;
  timeUnixNano?: string;
  timestamp?: string;
  body?: string;
  message?: string;
  severity_text?: string;
  level?: string;
  serviceName?: string;
  service_name?: string;
  service?: string;
  logAttributes?: LogAttributes;
  log_attributes?: LogAttributes;
}

export interface Log {
  id: string;
  timestamp: string;
  level: LogLevel;
  method?: string;
  status?: string;
  ipAddress?: string;
  source: string;
  filePath?: string;
  message: string;
  sourceType?: string;
  container?: string;
  log_attributes?: LogAttributes;
  requestData?: {
    headers?: Record<string, string>;
    body?: string;
    query?: Record<string, string>;
    duration?: number;
  };
}
