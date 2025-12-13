use tokio::sync::mpsc;
use anyhow::Result;
use crate::sender::LogEntry;

/// Provider trait - each log source implements this
#[async_trait::async_trait]
pub trait LogProvider: Send + Sync {
    /// Start collecting logs and send them through the channel
    async fn start(&self, tx: mpsc::Sender<LogEntry>) -> Result<()>;
    
    /// Provider name for logging/debugging
    fn name(&self) -> &str;
}

#[cfg(feature = "file")]
pub mod file;

#[cfg(feature = "docker")]
pub mod docker;

#[cfg(all(feature = "journald", target_os = "linux"))]
pub mod systemd;

// Future providers
// pub mod kubernetes;
// pub mod http;
