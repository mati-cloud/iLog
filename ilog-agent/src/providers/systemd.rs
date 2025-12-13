use tokio::sync::mpsc;
use anyhow::{Result, Context};
use std::sync::Arc;
use tracing::{info, error, warn};

use crate::config::AgentConfig;
use crate::sender::LogEntry;
use super::LogProvider;

pub struct SystemdProvider {
    config: Arc<AgentConfig>,
}

impl SystemdProvider {
    pub fn new(config: Arc<AgentConfig>) -> Self {
        Self { config }
    }
}

#[async_trait]
impl LogProvider for SystemdProvider {
    async fn start(&self, tx: mpsc::Sender<LogEntry>) -> Result<()> {
        let journald_config = match &self.config.sources.journald {
            Some(cfg) if cfg.enabled => cfg,
            _ => {
                warn!("Systemd provider is not enabled");
                return Ok(());
            }
        };

        info!("Starting systemd journald provider for {} units", journald_config.units.len());

        // Open the journal
        let mut journal = Journal::open(systemd::journal::JournalFiles::All, false, true)
            .context("Failed to open systemd journal")?;

        // Seek to end (only new entries)
        journal.seek_tail()
            .context("Failed to seek to end of journal")?;

        info!("Watching journald for units: {:?}", journald_config.units);

        // Follow the journal
        loop {
            match journal.next_entry() {
                Ok(Some(entry)) => {
                    let unit = entry.get("_SYSTEMD_UNIT")
                        .or_else(|| entry.get("UNIT"))
                        .unwrap_or("unknown");

                    if !journald_config.units.iter().any(|u| unit.contains(u)) {
                        continue;
                    }

                    let message = entry.get("MESSAGE").unwrap_or("");
                    let priority = entry.get("PRIORITY").unwrap_or("6"); // 6 = INFO
                    
                    let level = match priority {
                        "0" | "1" | "2" | "3" => "ERROR",
                        "4" => "WARN",
                        "5" | "6" => "INFO",
                        "7" => "DEBUG",
                        _ => "INFO",
                    };

                    let timestamp = if let Some(ts_str) = entry.get("__REALTIME_TIMESTAMP") {
                        if let Ok(ts_micros) = ts_str.parse::<i64>() {
                            chrono::DateTime::from_timestamp_micros(ts_micros)
                                .unwrap_or_else(|| chrono::Utc::now())
                        } else {
                            chrono::Utc::now()
                        }
                    } else {
                        chrono::Utc::now()
                    };

                    let log_entry = LogEntry {
                        timestamp,
                        level: level.to_string(),
                        service: unit.to_string(),
                        message: message.to_string(),
                        attributes: Some(serde_json::json!({
                            "source_type": "journald",
                            "unit": unit,
                            "pid": entry.get("_PID").unwrap_or(""),
                            "uid": entry.get("_UID").unwrap_or(""),
                        })),
                    };

                    if let Err(e) = tx.send(log_entry).await {
                        error!("Failed to send log entry: {}", e);
                        break;
                    }
                }
                Ok(None) => {
                    // No new entries, wait a bit
                    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                }
                Err(e) => {
                    error!("Error reading journal: {}", e);
                    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                }
            }
        }

        Ok(())
    }
    
    fn name(&self) -> &str {
        "systemd"
    }
}
