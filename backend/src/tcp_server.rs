use anyhow::{Context, Result};
use chacha20poly1305::{
    aead::{Aead, KeyInit},
    ChaCha20Poly1305, Nonce,
};
use hkdf::Hkdf;
use sha2::Sha256;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::broadcast;
use tracing::{error, info, warn};

use crate::{db::Database, models::OtelLog, otel, token_crypto::TokenCrypto};

const MAGIC_BYTES: &[u8; 4] = b"ILOG";

/// Wire protocol version. v2 added the agent id to the frame header; see
/// `ilog-agent/src/protocol.rs` for the full rationale. Both sides must agree.
const VERSION: u8 = 2;
const NONCE_SIZE: usize = 12;

/// Agent-key derivation parameters. Must match `ilog-agent/src/crypto.rs`.
const KEY_SALT: &[u8] = b"ilog-agent-transport-v2";
const KEY_INFO: &[u8] = b"ilog agent transport key v2";

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
    /// Sending agent. `None` on frames the backend originates, such as acks.
    pub agent_id: Option<uuid::Uuid>,
    pub payload: Vec<u8>,
}

impl Frame {
    pub fn ack() -> Self {
        Self {
            frame_type: FrameType::Ack,
            agent_id: None,
            payload: Vec::new(),
        }
    }

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

        let mut id_bytes = [0u8; 16];
        stream
            .read_exact(&mut id_bytes)
            .await
            .context("Failed to read agent id")?;
        let parsed = uuid::Uuid::from_bytes(id_bytes);
        let agent_id = (!parsed.is_nil()).then_some(parsed);

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
            agent_id,
            payload,
        })
    }

    pub async fn write_to(&self, stream: &mut TcpStream) -> Result<()> {
        stream.write_all(MAGIC_BYTES).await?;
        stream.write_u8(VERSION).await?;
        stream.write_u8(self.frame_type as u8).await?;
        stream
            .write_all(self.agent_id.unwrap_or(uuid::Uuid::nil()).as_bytes())
            .await?;
        stream.write_u32(self.payload.len() as u32).await?;
        stream.write_all(&self.payload).await?;
        stream.flush().await?;
        Ok(())
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

    /// Derive the AEAD key from an agent token with HKDF-SHA256.
    ///
    /// Must stay byte-for-byte identical to the agent's derivation in
    /// `ilog-agent/src/crypto.rs`, including the salt and info strings. The
    /// former `DefaultHasher` scheme yielded a 32-byte key holding only 64 bits
    /// of entropy.
    fn derive_key_from_token(token: &str) -> Result<[u8; 32]> {
        let hk = Hkdf::<Sha256>::new(Some(KEY_SALT), token.as_bytes());
        let mut key = [0u8; 32];
        hk.expand(KEY_INFO, &mut key)
            .map_err(|_| anyhow::anyhow!("HKDF expand failed"))?;
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

async fn handle_client(
    mut stream: TcpStream,
    db: Arc<Database>,
    token_crypto: TokenCrypto,
    log_tx: broadcast::Sender<OtelLog>,
) {
    let peer_addr = stream.peer_addr().ok();
    info!("✓ Agent connection established from {:?}", peer_addr);

    loop {
        info!("Waiting for frame from {:?}", peer_addr);
        match Frame::read_from(&mut stream).await {
            Ok(frame) => {
                info!("Received frame type {:?} with {} bytes payload from {:?}", frame.frame_type, frame.payload.len(), peer_addr);
                match frame.frame_type {
                    FrameType::LogBatch => {
                        match process_log_batch(
                            frame.agent_id,
                            &frame.payload,
                            &db,
                            &token_crypto,
                            &log_tx,
                        )
                        .await
                        {
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
    claimed_agent_id: Option<uuid::Uuid>,
    encrypted_payload: &[u8],
    db: &Database,
    token_crypto: &TokenCrypto,
    log_tx: &broadcast::Sender<OtelLog>,
) -> Result<(uuid::Uuid, usize)> {
    // The agent names itself in the frame header, so we fetch exactly one
    // candidate key by primary key. Previously this loaded every active agent
    // and attempted an AEAD decrypt against each, making ingest cost scale with
    // fleet size. The id is only a hint about which key to try; the decrypt
    // below is what actually authenticates the batch, so a forged id fails.
    let agent_id = claimed_agent_id
        .ok_or_else(|| anyhow::anyhow!("LogBatch frame carried no agent id"))?;

    // Selected into a local tuple rather than a struct: the wrapped secret must
    // not land in any type that derives `Serialize`.
    let agent: Option<(uuid::Uuid, Vec<u8>)> = sqlx::query_as(
        r#"
        SELECT service_id, key_secret_encrypted
        FROM agents
        WHERE id = $1
          AND (expires_at IS NULL OR expires_at > NOW())
        "#,
    )
    .bind(agent_id)
    .fetch_optional(db.pool())
    .await?;

    let (service_id, key_secret_encrypted) =
        agent.ok_or_else(|| anyhow::anyhow!("Unknown or expired agent {}", agent_id))?;

    // Unwrap the stored secret, then rebuild the exact token string the agent
    // holds. The derivation input is the whole token, unchanged from before this
    // column was encrypted, which is what keeps the agent side in step.
    let key_secret = token_crypto.unwrap(&key_secret_encrypted).map_err(|e| {
        anyhow::anyhow!(
            "Cannot recover key secret for agent {}: {}. The row was written under \
             a different TOKEN_ENCRYPTION_KEY, or has been altered.",
            agent_id,
            e
        )
    })?;
    let token = crate::services::token_from_parts(agent_id, &key_secret);

    let compressed = Decryptor::from_token(&token)
        .and_then(|d| d.decrypt(encrypted_payload))
        .map_err(|_| anyhow::anyhow!("Authentication failed for agent {}", agent_id))?;

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
    token_crypto: TokenCrypto,
    log_tx: broadcast::Sender<OtelLog>,
) -> Result<()> {
    let listener = TcpListener::bind(addr).await?;
    info!("TCP server listening on {}", addr);

    loop {
        match listener.accept().await {
            Ok((stream, _)) => {
                let db = Arc::clone(&db);
                let token_crypto = token_crypto.clone();
                let log_tx = log_tx.clone();
                tokio::spawn(async move {
                    handle_client(stream, db, token_crypto, log_tx).await;
                });
            }
            Err(e) => {
                error!("Failed to accept connection: {}", e);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Pins the exact bytes HKDF produces for a known token.
    ///
    /// The agent derives its transport key independently in
    /// `ilog-agent/src/crypto.rs` and asserts this same vector. Nothing at
    /// compile time links the two implementations, so if one side's salt, info
    /// string, or hash changes, the only symptom in production would be every
    /// agent silently failing to authenticate. These paired tests turn that into
    /// a build failure instead.
    #[test]
    fn derived_key_matches_agent_vector() {
        let key = Decryptor::derive_key_from_token("agt_test_token").unwrap();
        let hex: String = key.iter().map(|b| format!("{:02x}", b)).collect();

        assert_eq!(
            hex, "26491796d51fdc101b48bc0a41d707eeab13e958e00cb4caf4a4a0e7f9901dc4",
            "agent key derivation changed; update ilog-agent crypto.rs to match"
        );
    }
}
