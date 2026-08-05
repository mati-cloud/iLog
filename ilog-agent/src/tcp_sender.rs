use anyhow::{Context, Result};
use chrono::Utc;
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio::time::sleep;
use tracing::{debug, error, info, trace, warn};

use crate::config::AgentConfig;

/// Upper bound on logs per batch. Was 50, which put a hard ceiling on ingest
/// throughput regardless of how fast either side could go: a host emitting
/// 10k logs/sec needed 200 round-trips per second just to keep up.
const MAX_BATCH_LOGS: usize = 1000;

/// Upper bound on uncompressed message bytes per batch, so a burst of very large
/// log lines can't build an oversized frame. The wire format caps payloads at
/// 100 MB; this keeps batches far below that.
const MAX_BATCH_BYTES: usize = 4 * 1024 * 1024;

/// Once this many logs are buffered, flush without waiting for more.
const MICRO_BATCH_MIN_LOGS: usize = 64;

/// How long to wait for more logs before shipping a small batch. Bounds the
/// added latency on a mostly-idle host.
const MICRO_BATCH_DELAY: Duration = Duration::from_millis(10);
use crate::crypto::Encryptor;
use crate::protocol::{agent_id_from_token, Frame};
use uuid::Uuid;

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
    /// This agent's id, parsed out of its token. Sent in every frame header so
    /// the backend can look up our key directly.
    agent_id: Uuid,
    buffer: Vec<LogEntry>,
    stream: Option<TcpStream>,
}

impl TcpLogSender {
    pub fn new(config: Arc<AgentConfig>) -> Result<Self> {
        let token = &config.agent.token;
        let encryptor = Encryptor::from_token(token)?;
        let agent_id = agent_id_from_token(token).context(
            "Agent token is malformed: expected the form agt_<agent_id>_<secret>. \
             Re-issue the token from the iLog dashboard.",
        )?;

        Ok(Self {
            config,
            encryptor,
            agent_id,
            buffer: Vec::new(),
            stream: None,
        })
    }

    pub async fn start(
        config: Arc<AgentConfig>,
        mut rx: mpsc::Receiver<LogEntry>,
    ) -> Result<()> {
        let mut sender = Self::new(config.clone())?;
        let mut heartbeat_interval = tokio::time::interval(Duration::from_secs(30));

        loop {
            tokio::select! {
                Some(log) = rx.recv() => {
                    // Per-log logging stays at trace: at `info` this allocated a
                    // String for every line the agent shipped, which is real
                    // overhead in the hot path and doubles the log volume of any
                    // host running the agent at debug level.
                    trace!(service = %log.service, "buffered log entry");
                    let mut batch_bytes = log.message.len();
                    sender.buffer.push(log);

                    // Drain whatever is already queued before waiting. Only pay
                    // the micro-batch delay if the burst is small, so a backlog
                    // flushes immediately instead of at 10ms per batch.
                    loop {
                        while let Ok(log) = rx.try_recv() {
                            batch_bytes += log.message.len();
                            sender.buffer.push(log);
                            if sender.buffer.len() >= MAX_BATCH_LOGS || batch_bytes >= MAX_BATCH_BYTES {
                                break;
                            }
                        }

                        if sender.buffer.len() >= MAX_BATCH_LOGS
                            || batch_bytes >= MAX_BATCH_BYTES
                            || sender.buffer.len() >= MICRO_BATCH_MIN_LOGS
                        {
                            break;
                        }

                        tokio::time::sleep(MICRO_BATCH_DELAY).await;

                        // Nothing new arrived during the delay; ship what we have.
                        if rx.is_empty() {
                            break;
                        }
                    }

                    debug!("Flushing {} logs ({} bytes)", sender.buffer.len(), batch_bytes);

                    if let Err(e) = sender.flush().await {
                        error!("Failed to flush logs: {}", e);
                        if sender.stream.is_some() {
                            warn!("✗ Disconnected from backend server: {}", sender.config.agent.server);
                            sender.stream = None;
                        }
                    }
                }
                _ = heartbeat_interval.tick() => {
                    info!("Sending heartbeat to server");
                    if let Err(e) = sender.send_heartbeat().await {
                        warn!("Failed to send heartbeat: {}", e);
                        if sender.stream.is_some() {
                            warn!("✗ Disconnected from backend server: {}", sender.config.agent.server);
                            sender.stream = None;
                        }
                    } else {
                        info!("Heartbeat sent successfully");
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
            info!("✓ Connection established with backend server: {}", self.config.agent.server);
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
        let encrypted_len = encrypted.len();

        let frame = Frame::log_batch(self.agent_id, encrypted);

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
                                encrypted_len
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
        // Read the id before `ensure_connected` takes a mutable borrow of self.
        let frame = Frame::heartbeat(self.agent_id);
        let stream = self.ensure_connected().await?;
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
        // Raw LZ4 block, no size prefix. The backend decompresses with
        // `lz4_flex::block::decompress` and an explicit capacity, so the two
        // sides must use this same crate and calling convention.
        Ok(lz4_flex::block::compress(data))
    }
}
