mod config;
mod sender;
mod tcp_sender;
mod crypto;
mod protocol;
mod providers;

use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{error, info};
use tracing_subscriber;

use config::AgentConfig;
use sender::LogSender;
use tcp_sender::TcpLogSender;
use providers::LogProvider;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Path to configuration file
    #[arg(short, long, value_name = "FILE")]
    config: PathBuf,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let args = Args::parse();

    let config = AgentConfig::load(&args.config)?;
    info!("Loaded configuration from {:?}", args.config);
    info!("Server: {}", config.agent.server);

    let config = Arc::new(config);

    let (tx, rx) = mpsc::channel(1000);

    let config_clone = config.clone();
    let sender_handle = tokio::spawn(async move {
        match config_clone.agent.protocol.as_str() {
            "tcp" => {
                info!("Using TCP protocol with ChaCha20-Poly1305 encryption and LZ4 compression");
                TcpLogSender::start(config_clone, rx).await
            }
            "http" => {
                #[cfg(feature = "http")]
                {
                    info!("Using HTTP protocol");
                    LogSender::start(config_clone, rx).await
                }
                #[cfg(not(feature = "http"))]
                {
                    error!("HTTP protocol requested but 'http' feature not enabled");
                    Err(anyhow::anyhow!("HTTP feature not enabled"))
                }
            }
            proto => {
                error!("Unknown protocol: {}", proto);
                Err(anyhow::anyhow!("Unknown protocol: {}", proto))
            }
        }
    });

    // Collect all enabled providers
    let mut provider_handles = vec![];

    // File provider
    #[cfg(feature = "file")]
    {
        if config.sources.file.as_ref().map(|f| f.enabled).unwrap_or(false) {
            let provider = providers::file::FileProvider::new(config.clone());
            let tx_clone = tx.clone();
            info!("Starting {} provider", provider.name());
            let handle = tokio::spawn(async move {
                if let Err(e) = provider.start(tx_clone).await {
                    error!("File provider error: {}", e);
                }
            });
            provider_handles.push(handle);
        }
    }

    // Docker provider
    #[cfg(feature = "docker")]
    {
        if config.sources.docker.as_ref().map(|d| d.enabled).unwrap_or(false) {
            let provider = providers::docker::DockerProvider::new(config.clone());
            let tx_clone = tx.clone();
            info!("Starting {} provider", provider.name());
            let handle = tokio::spawn(async move {
                if let Err(e) = provider.start(tx_clone).await {
                    error!("Docker provider error: {}", e);
                }
            });
            provider_handles.push(handle);
        }
    }

    // Systemd provider (Linux only)
    #[cfg(all(feature = "journald", target_os = "linux"))]
    {
        if config.sources.journald.as_ref().map(|j| j.enabled).unwrap_or(false) {
            let provider = providers::systemd::SystemdProvider::new(config.clone());
            let tx_clone = tx.clone();
            info!("Starting {} provider", provider.name());
            let handle = tokio::spawn(async move {
                if let Err(e) = provider.start(tx_clone).await {
                    error!("Systemd provider error: {}", e);
                }
            });
            provider_handles.push(handle);
        }
    }

    info!("Started {} providers", provider_handles.len());

    // Wait for sender to complete
    sender_handle.await??;

    // Wait for shutdown signal
    tokio::signal::ctrl_c().await?;
    info!("Shutting down...");

    Ok(())
}
