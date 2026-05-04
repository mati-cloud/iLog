use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{info, error};
use tracing_subscriber;

use ilog_agent::config::AgentConfig;
use ilog_agent::tcp_sender::TcpLogSender;
use ilog_agent::parser::ParserConfig;
use ilog_agent::providers::{LogProvider, file_parsed::ParsedFileProvider};

#[derive(Parser, Debug)]
#[command(author, version, about = "iLog Agent - Config-driven log parser", long_about = None)]
struct Args {
    /// Path to agent configuration file (server, auth, etc.)
    #[arg(short, long, value_name = "FILE")]
    config: PathBuf,
    
    /// Path to parser configuration file (log sources)
    #[arg(short, long, value_name = "FILE")]
    parser: PathBuf,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let args = Args::parse();

    // Load agent config (server, auth)
    let agent_config = AgentConfig::load(&args.config)?;
    info!("Loaded agent config from {:?}", args.config);
    info!("Server: {}", agent_config.agent.server);

    // Load parser config (log sources)
    let parser_config_str = std::fs::read_to_string(&args.parser)?;
    let parser_config: ParserConfig = serde_yaml::from_str(&parser_config_str)?;
    info!("Loaded parser config from {:?}", args.parser);
    info!("Sources: {}", parser_config.sources.len());

    let agent_config = Arc::new(agent_config);
    let (tx, rx) = mpsc::channel(1000);

    // Start TCP sender
    let config_clone = agent_config.clone();
    let sender_handle = tokio::spawn(async move {
        info!("Starting TCP sender with ChaCha20-Poly1305 + LZ4");
        TcpLogSender::start(config_clone, rx).await
    });

    // Start parsed file provider
    let provider = ParsedFileProvider::new(parser_config);
    let provider_handle = tokio::spawn(async move {
        if let Err(e) = provider.start(tx).await {
            error!("Provider error: {}", e);
        }
    });

    // Wait for both
    tokio::select! {
        result = sender_handle => {
            if let Err(e) = result {
                error!("Sender task failed: {}", e);
            }
        }
        result = provider_handle => {
            if let Err(e) = result {
                error!("Provider task failed: {}", e);
            }
        }
    }

    Ok(())
}
