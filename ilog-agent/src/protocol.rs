use anyhow::{Context, Result};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use uuid::Uuid;

const MAGIC_BYTES: &[u8; 4] = b"ILOG";

/// Wire protocol version.
///
/// v2 adds a 16-byte agent id to the header. Without it the backend had to try
/// decrypting each batch against every registered agent token in turn, which
/// meant a full `agents` table scan plus an AEAD attempt per agent on every
/// batch. The id lets it do one indexed lookup instead. The id is an
/// authentication *hint* only — it selects which key to try, and the AEAD tag
/// still decides whether the batch is genuine, so claiming another agent's id
/// gains nothing without that agent's token.
const VERSION: u8 = 2;

const MAX_PAYLOAD: u32 = 100 * 1024 * 1024;

/// Extract the agent id embedded in a token of the form
/// `agt_<agent_id_simple>_<key_secret>`.
///
/// The backend mints this format in `services.rs::generate_token` and rebuilds it
/// from storage in `token_from_parts`; the frame codec is likewise duplicated
/// across the two crates. Worth extracting into a shared `ilog-proto` crate once
/// the wire format settles.
pub fn agent_id_from_token(token: &str) -> Option<Uuid> {
    let rest = token.strip_prefix("agt_")?;
    let (id, _secret) = rest.split_once('_')?;
    Uuid::parse_str(id).ok()
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
    pub agent_id: Option<Uuid>,
    pub payload: Vec<u8>,
}

impl Frame {
    pub fn log_batch(agent_id: Uuid, payload: Vec<u8>) -> Self {
        Self {
            frame_type: FrameType::LogBatch,
            agent_id: Some(agent_id),
            payload,
        }
    }

    pub fn heartbeat(agent_id: Uuid) -> Self {
        Self {
            frame_type: FrameType::Heartbeat,
            agent_id: Some(agent_id),
            payload: Vec::new(),
        }
    }

    pub fn ack() -> Self {
        Self {
            frame_type: FrameType::Ack,
            agent_id: None,
            payload: Vec::new(),
        }
    }

    pub async fn write_to<W>(&self, stream: &mut W) -> Result<()>
    where
        W: AsyncWrite + Unpin,
    {
        stream.write_all(MAGIC_BYTES).await?;
        stream.write_u8(VERSION).await?;
        stream.write_u8(self.frame_type as u8).await?;
        stream
            .write_all(self.agent_id.unwrap_or(Uuid::nil()).as_bytes())
            .await?;
        stream.write_u32(self.payload.len() as u32).await?;
        stream.write_all(&self.payload).await?;
        stream.flush().await?;
        Ok(())
    }

    pub async fn read_from<R>(stream: &mut R) -> Result<Self>
    where
        R: AsyncRead + Unpin,
    {
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

        let frame_type = FrameType::try_from(
            stream
                .read_u8()
                .await
                .context("Failed to read frame type")?,
        )?;

        let mut id_bytes = [0u8; 16];
        stream
            .read_exact(&mut id_bytes)
            .await
            .context("Failed to read agent id")?;
        let parsed = Uuid::from_bytes(id_bytes);
        let agent_id = (!parsed.is_nil()).then_some(parsed);

        let payload_len = stream
            .read_u32()
            .await
            .context("Failed to read payload length")?;

        if payload_len > MAX_PAYLOAD {
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_frame_roundtrip() {
        let (mut client, mut server) = tokio::io::duplex(1024);
        let agent_id = Uuid::new_v4();

        tokio::spawn(async move {
            Frame::log_batch(agent_id, b"test payload".to_vec())
                .write_to(&mut client)
                .await
                .unwrap();
        });

        let received = Frame::read_from(&mut server).await.unwrap();

        assert_eq!(received.frame_type, FrameType::LogBatch);
        assert_eq!(received.agent_id, Some(agent_id));
        assert_eq!(received.payload, b"test payload");
    }

    #[tokio::test]
    async fn test_ack_has_no_agent_id() {
        let (mut client, mut server) = tokio::io::duplex(1024);

        tokio::spawn(async move {
            Frame::ack().write_to(&mut client).await.unwrap();
        });

        let received = Frame::read_from(&mut server).await.unwrap();

        assert_eq!(received.frame_type, FrameType::Ack);
        assert_eq!(received.agent_id, None);
    }

    #[tokio::test]
    async fn test_rejects_wrong_version() {
        let (mut client, mut server) = tokio::io::duplex(1024);

        tokio::spawn(async move {
            client.write_all(MAGIC_BYTES).await.unwrap();
            client.write_u8(1).await.unwrap(); // v1 is no longer accepted
            client.write_u8(0x01).await.unwrap();
            client.flush().await.unwrap();
        });

        assert!(Frame::read_from(&mut server).await.is_err());
    }
}
