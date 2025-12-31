use anyhow::{Context, Result};
use chrono::Utc;
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio::time::sleep;
use tracing::{error, info, warn};

use crate::config::AgentConfig;
use crate::crypto::Encryptor;
use crate::protocol::{Frame, FrameType};

#[derive(Debug, Clone)]
pub struct LogEntry {
    pub timestamp: chrono::DateTime<Utc>,
    pub level: String,
    pub service: String,
    pub message: String,
    pub attributes: Option<serde_json::Value>,
}

pub struct TcpLogSender {
    config: Arc<AgentConfig>,
    encryptor: Encryptor,
    buffer: Vec<LogEntry>,
    stream: Option<TcpStream>,
}

impl TcpLogSender {
    pub fn new(config: Arc<AgentConfig>) -> Result<Self> {
        let encryptor = Encryptor::from_token(&config.agent.token)?;
        Ok(Self {
            config,
            encryptor,
            buffer: Vec::new(),
            stream: None,
        })
    }

    pub async fn start(
        config: Arc<AgentConfig>,
        mut rx: mpsc::Receiver<LogEntry>,
    ) -> Result<()> {
        let mut sender = Self::new(config.clone())?;
        let micro_batch_delay = Duration::from_millis(10);
        let mut heartbeat_interval = tokio::time::interval(Duration::from_secs(30));

        loop {
            tokio::select! {
                Some(log) = rx.recv() => {
                    sender.buffer.push(log);
                    
                    tokio::time::sleep(micro_batch_delay).await;
                    
                    while let Ok(log) = rx.try_recv() {
                        sender.buffer.push(log);
                        if sender.buffer.len() >= 50 {
                            break;
                        }
                    }
                    
                    if let Err(e) = sender.flush().await {
                        error!("Failed to flush logs: {}", e);
                        sender.stream = None;
                    }
                }
                _ = heartbeat_interval.tick() => {
                    if let Err(e) = sender.send_heartbeat().await {
                        warn!("Failed to send heartbeat: {}", e);
                        sender.stream = None;
                    }
                }
            }
        }
    }

    async fn ensure_connected(&mut self) -> Result<&mut TcpStream> {
        if self.stream.is_none() {
            info!("Connecting to {}", self.config.agent.server);
            let stream = TcpStream::connect(&self.config.agent.server)
                .await
                .context("Failed to connect to server")?;
            stream.set_nodelay(true)?;
            info!("Connected to {}", self.config.agent.server);
            self.stream = Some(stream);
        }
        Ok(self.stream.as_mut().unwrap())
    }

    async fn flush(&mut self) -> Result<()> {
        if self.buffer.is_empty() {
            return Ok(());
        }

        let json_payload = self.serialize_logs(&self.buffer);
        let compressed = self.compress(&json_payload)?;
        let encrypted = self.encryptor.encrypt(&compressed)?;

        let frame = Frame::log_batch(encrypted);

        let mut retry_count = 0;
        const MAX_RETRIES: u32 = 3;

        loop {
            match self.ensure_connected().await {
                Ok(stream) => {
                    match frame.write_to(stream).await {
                        Ok(_) => {
                            info!("Successfully sent {} logs ({} bytes compressed, {} bytes encrypted)",
                                self.buffer.len(),
                                compressed.len(),
                                encrypted.len()
                            );
                            self.buffer.clear();
                            return Ok(());
                        }
                        Err(e) => {
                            error!("Failed to write frame: {}", e);
                            self.stream = None;
                            retry_count += 1;
                            if retry_count >= MAX_RETRIES {
                                anyhow::bail!("Max retries exceeded");
                            }
                            sleep(Duration::from_secs(1 << retry_count)).await;
                        }
                    }
                }
                Err(e) => {
                    error!("Failed to connect: {}", e);
                    retry_count += 1;
                    if retry_count >= MAX_RETRIES {
                        anyhow::bail!("Max retries exceeded");
                    }
                    sleep(Duration::from_secs(1 << retry_count)).await;
                }
            }
        }
    }

    async fn send_heartbeat(&mut self) -> Result<()> {
        let stream = self.ensure_connected().await?;
        let frame = Frame::heartbeat();
        frame.write_to(stream).await?;
        Ok(())
    }

    fn serialize_logs(&self, logs: &[LogEntry]) -> Vec<u8> {
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

        serde_json::to_vec(&otlp_logs).unwrap_or_default()
    }

    fn compress(&self, data: &[u8]) -> Result<Vec<u8>> {
        lz4::block::compress(data, None, false).context("LZ4 compression failed")
    }
}
