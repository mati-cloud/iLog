use anyhow::{Context, Result};
use chacha20poly1305::{
    aead::{Aead, KeyInit},
    ChaCha20Poly1305, Nonce,
};
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::broadcast;
use tracing::{error, info, warn};

use crate::{db::Database, models::OtelLog, otel};

const MAGIC_BYTES: &[u8; 4] = b"ILOG";
const VERSION: u8 = 1;
const NONCE_SIZE: usize = 12;

#[repr(u8)]
#[derive(Debug, Clone, Copy)]
pub enum FrameType {
    LogBatch = 0x01,
    Heartbeat = 0x02,
    Ack = 0x03,
}

impl TryFrom<u8> for FrameType {
    type Error = anyhow::Error;

    fn try_from(value: u8) -> Result<Self> {
        match value {
            0x01 => Ok(FrameType::LogBatch),
            0x02 => Ok(FrameType::Heartbeat),
            0x03 => Ok(FrameType::Ack),
            _ => anyhow::bail!("Unknown frame type: {}", value),
        }
    }
}

pub struct Frame {
    pub frame_type: FrameType,
    pub payload: Vec<u8>,
}

impl Frame {
    pub async fn read_from(stream: &mut TcpStream) -> Result<Self> {
        let mut magic = [0u8; 4];
        stream
            .read_exact(&mut magic)
            .await
            .context("Failed to read magic bytes")?;

        if &magic != MAGIC_BYTES {
            anyhow::bail!("Invalid magic bytes");
        }

        let version = stream.read_u8().await.context("Failed to read version")?;
        if version != VERSION {
            anyhow::bail!("Unsupported protocol version: {}", version);
        }

        let frame_type_byte = stream
            .read_u8()
            .await
            .context("Failed to read frame type")?;
        let frame_type = FrameType::try_from(frame_type_byte)?;

        let payload_len = stream
            .read_u32()
            .await
            .context("Failed to read payload length")?;

        if payload_len > 100 * 1024 * 1024 {
            anyhow::bail!("Payload too large: {} bytes", payload_len);
        }

        let mut payload = vec![0u8; payload_len as usize];
        stream
            .read_exact(&mut payload)
            .await
            .context("Failed to read payload")?;

        Ok(Self {
            frame_type,
            payload,
        })
    }

    pub async fn write_to(&self, stream: &mut TcpStream) -> Result<()> {
        stream.write_all(MAGIC_BYTES).await?;
        stream.write_u8(VERSION).await?;
        stream.write_u8(self.frame_type as u8).await?;
        stream.write_u32(self.payload.len() as u32).await?;
        stream.write_all(&self.payload).await?;
        stream.flush().await?;
        Ok(())
    }

    pub fn ack() -> Self {
        Self {
            frame_type: FrameType::Ack,
            payload: Vec::new(),
        }
    }
}

pub struct Decryptor {
    cipher: ChaCha20Poly1305,
}

impl Decryptor {
    pub fn new(key: &[u8; 32]) -> Self {
        Self {
            cipher: ChaCha20Poly1305::new(key.into()),
        }
    }

    pub fn from_token(token: &str) -> Result<Self> {
        let key = Self::derive_key_from_token(token)?;
        Ok(Self::new(&key))
    }

    fn derive_key_from_token(token: &str) -> Result<[u8; 32]> {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        token.hash(&mut hasher);
        let hash = hasher.finish();

        let mut key = [0u8; 32];
        for (i, chunk) in key.chunks_mut(8).enumerate() {
            let mut h = DefaultHasher::new();
            (hash, i).hash(&mut h);
            chunk.copy_from_slice(&h.finish().to_le_bytes());
        }

        Ok(key)
    }

    pub fn decrypt(&self, encrypted: &[u8]) -> Result<Vec<u8>> {
        if encrypted.len() < NONCE_SIZE {
            anyhow::bail!("Encrypted data too short");
        }

        let (nonce_bytes, ciphertext) = encrypted.split_at(NONCE_SIZE);
        let nonce = Nonce::from_slice(nonce_bytes);

        let plaintext = self
            .cipher
            .decrypt(nonce, ciphertext)
            .map_err(|_| anyhow::anyhow!("Decryption failed"))?;

        Ok(plaintext)
    }
}

async fn handle_client(mut stream: TcpStream, db: Arc<Database>, log_tx: broadcast::Sender<OtelLog>) {
    let peer_addr = stream.peer_addr().ok();
    info!("✓ Agent connection established from {:?}", peer_addr);

    loop {
        info!("Waiting for frame from {:?}", peer_addr);
        match Frame::read_from(&mut stream).await {
            Ok(frame) => {
                info!("Received frame type {:?} with {} bytes payload from {:?}", frame.frame_type, frame.payload.len(), peer_addr);
                match frame.frame_type {
                    FrameType::LogBatch => {
                        match process_log_batch(&frame.payload, &db, &log_tx).await {
                            Ok((service_id, count)) => {
                                info!("Processed {} logs from {:?} for service {}", count, peer_addr, service_id);
                                
                                // Send ACK
                                if let Err(e) = Frame::ack().write_to(&mut stream).await {
                                    error!("Failed to send ACK: {}", e);
                                    break;
                                }
                            }
                            Err(e) => {
                                error!("Failed to process log batch from {:?}: {}", peer_addr, e);
                                break;
                            }
                        }
                    }
                    FrameType::Heartbeat => {
                        info!("Received heartbeat from {:?}", peer_addr);
                        if let Err(e) = Frame::ack().write_to(&mut stream).await {
                            error!("Failed to send heartbeat ACK: {}", e);
                            break;
                        } else {
                            info!("Sent heartbeat ACK to {:?}", peer_addr);
                        }
                    }
                    FrameType::Ack => {
                        warn!("Received unexpected ACK from client");
                    }
                }
            }
            Err(e) => {
                if e.to_string().contains("UnexpectedEof") || e.to_string().contains("Connection reset") {
                    info!("✗ Agent disconnected gracefully: {:?}", peer_addr);
                } else {
                    error!("✗ Agent disconnected with error from {:?}: {}", peer_addr, e);
                }
                break;
            }
        }
    }

    info!("✗ Connection closed for agent: {:?}", peer_addr);
}

async fn process_log_batch(
    encrypted_payload: &[u8],
    db: &Database,
    log_tx: &broadcast::Sender<OtelLog>,
) -> Result<(uuid::Uuid, usize)> {
    // Fetch all active agent tokens from database
    let agents: Vec<(uuid::Uuid, uuid::Uuid, String)> = sqlx::query_as(
        r#"
        SELECT id, service_id, token
        FROM agents
        WHERE expires_at IS NULL OR expires_at > NOW()
        "#,
    )
    .fetch_all(db.pool())
    .await?;

    // Try to decrypt with each agent token
    let mut decrypted_data = None;
    let mut authenticated_service_id = None;
    let mut authenticated_agent_id = None;

    for (agent_id, service_id, token) in agents {
        if let Ok(decryptor) = Decryptor::from_token(&token) {
            if let Ok(data) = decryptor.decrypt(encrypted_payload) {
                decrypted_data = Some(data);
                authenticated_service_id = Some(service_id);
                authenticated_agent_id = Some(agent_id);
                break;
            }
        }
    }

    let compressed = decrypted_data.ok_or_else(|| {
        anyhow::anyhow!("Failed to decrypt with any valid agent token - authentication failed")
    })?;

    let service_id = authenticated_service_id.unwrap();
    let agent_id = authenticated_agent_id.unwrap();

    // Update last_used_at for the agent
    let _ = sqlx::query("UPDATE agents SET last_used_at = NOW() WHERE id = $1")
        .bind(agent_id)
        .execute(db.pool())
        .await;

    // Decompress (agent uses raw block compression without size prefix)
    let json_bytes = lz4_flex::block::decompress(&compressed, 10 * 1024 * 1024)
        .context("Failed to decompress log batch")?;

    // Deserialize
    let mut logs: Vec<OtelLog> =
        serde_json::from_slice(&json_bytes).context("Failed to deserialize logs")?;

    let count = logs.len();

    // Set service_id on each log for filtering
    for log in &mut logs {
        log.service_id = Some(service_id);
    }

    // Insert logs with authenticated service_id
    otel::ingest_logs(db, logs.clone(), service_id).await?;

    // Broadcast logs to WebSocket clients for real-time streaming
    for log in logs {
        // Ignore send errors - it's okay if no clients are listening
        let _ = log_tx.send(log);
    }

    Ok((service_id, count))
}

pub async fn start_tcp_server(
    addr: std::net::SocketAddr,
    db: Arc<Database>,
    log_tx: broadcast::Sender<OtelLog>,
) -> Result<()> {
    let listener = TcpListener::bind(addr).await?;
    info!("TCP server listening on {}", addr);

    loop {
        match listener.accept().await {
            Ok((stream, _)) => {
                let db = Arc::clone(&db);
                let log_tx = log_tx.clone();
                tokio::spawn(async move {
                    handle_client(stream, db, log_tx).await;
                });
            }
            Err(e) => {
                error!("Failed to accept connection: {}", e);
            }
        }
    }
}
