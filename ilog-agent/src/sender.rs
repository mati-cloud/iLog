use chrono::Utc;
use reqwest::Client;
use serde_json::json;
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{error, info};

use crate::config::AgentConfig;

#[derive(Debug, Clone)]
pub struct LogEntry {
    pub timestamp: chrono::DateTime<Utc>,
    pub level: String,
    pub service: String,
    pub message: String,
    pub attributes: Option<serde_json::Value>,
}

pub struct LogSender {
    client: Client,
    config: Arc<AgentConfig>,
    buffer: Vec<LogEntry>,
}

impl LogSender {
    pub fn new(config: Arc<AgentConfig>) -> Self {
        Self {
            client: Client::new(),
            config,
            buffer: Vec::new(),
        }
    }

    pub async fn start(
        config: Arc<AgentConfig>,
        mut rx: mpsc::Receiver<LogEntry>,
    ) -> Result<()> {
        let mut sender = Self::new(config.clone());
        let flush_interval = tokio::time::Duration::from_secs(config.agent.flush_interval_secs);
        let mut interval = tokio::time::interval(flush_interval);

        loop {
            tokio::select! {
                Some(log) = rx.recv() => {
                    sender.buffer.push(log);
                    if sender.buffer.len() >= config.agent.batch_size {
                        if let Err(e) = sender.flush().await {
                            error!("Failed to flush logs: {}", e);
                        }
                    }
                }
                _ = interval.tick() => {
                    if !sender.buffer.is_empty() {
                        if let Err(e) = sender.flush().await {
                            error!("Failed to flush logs: {}", e);
                        }
                    }
                }
            }
        }
    }

    async fn flush(&mut self) -> Result<()> {
        if self.buffer.is_empty() {
            return Ok(());
        }

        let logs = self.convert_to_otlp(&self.buffer);
        let endpoint = format!("http://{}/v1/logs", self.config.agent.server);

        let response = self
            .client
            .post(&endpoint)
            .header("Authorization", format!("Bearer {}", self.config.agent.token))
            .header("Content-Type", "application/json")
            .json(&logs)
            .send()
            .await?;

        if response.status().is_success() {
            info!("Successfully sent {} logs", self.buffer.len());
            self.buffer.clear();
        } else {
            let status = response.status();
            let body = response.text().await?;
            error!("Failed to send logs: {} - {}", status, body);
        }

        Ok(())
    }

    fn convert_to_otlp(&self, logs: &[LogEntry]) -> serde_json::Value {
        let otlp_logs: Vec<serde_json::Value> = logs
            .iter()
            .map(|log| {
                json!({
                    "timeUnixNano": log.timestamp.timestamp_nanos_opt().unwrap_or(0).to_string(),
                    "severityText": log.level.to_uppercase(),
                    "serviceName": log.service,
                    "body": log.message,
                    "logAttributes": log.attributes,
                })
            })
            .collect();

        json!(otlp_logs)
    }
}
