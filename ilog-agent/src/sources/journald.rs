use anyhow::Result;
use chrono::Utc;
use std::process::Stdio;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::mpsc;
use tracing::{error, info};

use crate::config::AgentConfig;
use crate::sender::LogEntry;

pub async fn watch_journald(
    config: Arc<AgentConfig>,
    tx: mpsc::Sender<LogEntry>,
) -> Result<()> {
    let journald_config = match &config.sources.journald {
        Some(cfg) if cfg.enabled => cfg,
        _ => {
            info!("Journald watching disabled");
            return Ok(());
        }
    };

    info!("Starting journald watcher for {} units", journald_config.units.len());

    for unit in &journald_config.units {
        let tx = tx.clone();
        let unit = unit.clone();
        
        tokio::spawn(async move {
            if let Err(e) = follow_unit(&unit, tx).await {
                error!("Error following unit {}: {}", unit, e);
            }
        });
    }

    Ok(())
}

async fn follow_unit(unit: &str, tx: mpsc::Sender<LogEntry>) -> Result<()> {
    info!("Following journald unit: {}", unit);

    let mut child = Command::new("journalctl")
        .args(&["-u", unit, "-f", "-o", "json"])
        .stdout(Stdio::piped())
        .spawn()?;

    let stdout = child.stdout.take().expect("Failed to capture stdout");
    let reader = BufReader::new(stdout);
    let mut lines = reader.lines();

    while let Some(line) = lines.next_line().await? {
        if let Some(log) = parse_journald_entry(&line, unit) {
            if let Err(e) = tx.send(log).await {
                error!("Failed to send log: {}", e);
            }
        }
    }

    Ok(())
}

fn parse_journald_entry(line: &str, unit: &str) -> Option<LogEntry> {
    let json: serde_json::Value = serde_json::from_str(line).ok()?;

    let message = json.get("MESSAGE")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    if message.is_empty() {
        return None;
    }

    let priority = json.get("PRIORITY")
        .and_then(|v| v.as_str())
        .unwrap_or("6");

    let level = match priority {
        "0" | "1" | "2" | "3" => "ERROR",
        "4" => "WARN",
        "5" | "6" => "INFO",
        "7" => "DEBUG",
        _ => "INFO",
    };

    Some(LogEntry {
        timestamp: Utc::now(),
        level: level.to_string(),
        service: unit.to_string(),
        message,
        attributes: Some(json),
    })
}
