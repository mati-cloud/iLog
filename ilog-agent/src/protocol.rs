use anyhow::{Context, Result};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

const MAGIC_BYTES: &[u8; 4] = b"ILOG";
const VERSION: u8 = 1;

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
    pub fn new(frame_type: FrameType, payload: Vec<u8>) -> Self {
        Self {
            frame_type,
            payload,
        }
    }

    pub fn log_batch(payload: Vec<u8>) -> Self {
        Self::new(FrameType::LogBatch, payload)
    }

    pub fn heartbeat() -> Self {
        Self::new(FrameType::Heartbeat, Vec::new())
    }

    pub fn ack() -> Self {
        Self::new(FrameType::Ack, Vec::new())
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::DuplexStream;

    #[tokio::test]
    async fn test_frame_roundtrip() {
        let (mut client, mut server) = tokio::io::duplex(1024);

        let original_frame = Frame::log_batch(b"test payload".to_vec());

        tokio::spawn(async move {
            original_frame.write_to(&mut client).await.unwrap();
        });

        let received_frame = Frame::read_from(&mut server).await.unwrap();

        assert_eq!(received_frame.payload, b"test payload");
    }
}
