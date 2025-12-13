use anyhow::Result;
use chrono::Utc;
use notify::{Config, Event, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::Path;
use std::sync::Arc;
use tokio::fs::File;
use tokio::io::{AsyncBufReadExt, AsyncSeekExt, BufReader};
use tokio::sync::mpsc;
use tracing::{error, info};

use crate::config::AgentConfig;
use crate::sender::LogEntry;

pub async fn watch_files(
    config: Arc<AgentConfig>,
    tx: mpsc::Sender<LogEntry>,
) -> Result<()> {
    let file_config = match &config.sources.file {
        Some(cfg) if cfg.enabled => cfg,
        _ => {
            info!("File watching disabled");
            return Ok(());
        }
    };

    info!("Starting file watcher for {} paths", file_config.paths.len());

    for path_pattern in &file_config.paths {
        let tx = tx.clone();
        let path = path_pattern.clone();
        
        tokio::spawn(async move {
            if let Err(e) = tail_file(&path, tx).await {
                error!("Error tailing file {}: {}", path, e);
            }
        });
    }

    Ok(())
}

async fn tail_file(path: &str, tx: mpsc::Sender<LogEntry>) -> Result<()> {
    let file = File::open(path).await?;
    let mut reader = BufReader::new(file);
    
    // Seek to end of file
    reader.seek(std::io::SeekFrom::End(0)).await?;
    
    info!("Tailing file: {}", path);
    
    let mut line = String::new();
    loop {
        line.clear();
        match reader.read_line(&mut line).await {
            Ok(0) => {
                // EOF, wait a bit and try again
                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            }
            Ok(_) => {
                if let Some(log) = parse_log_line(&line, path) {
                    if let Err(e) = tx.send(log).await {
                        error!("Failed to send log: {}", e);
                    }
                }
            }
            Err(e) => {
                error!("Error reading file {}: {}", path, e);
                tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
            }
        }
    }
}

fn parse_log_line(line: &str, source: &str) -> Option<LogEntry> {
    let line = line.trim();
    if line.is_empty() {
        return None;
    }

    // Try to parse as JSON first
    if let Ok(json) = serde_json::from_str::<serde_json::Value>(line) {
        return Some(LogEntry {
            timestamp: Utc::now(),
            level: json.get("level")
                .and_then(|v| v.as_str())
                .unwrap_or("INFO")
                .to_string(),
            service: extract_service_name(source),
            message: json.get("message")
                .or_else(|| json.get("msg"))
                .and_then(|v| v.as_str())
                .unwrap_or(line)
                .to_string(),
            attributes: Some(json),
        });
    }

    // Try common log formats (nginx, apache, etc.)
    let level = detect_log_level(line);
    
    Some(LogEntry {
        timestamp: Utc::now(),
        level,
        service: extract_service_name(source),
        message: line.to_string(),
        attributes: None,
    })
}

fn detect_log_level(line: &str) -> String {
    let line_lower = line.to_lowercase();
    
    if line_lower.contains("error") || line_lower.contains("err") {
        "ERROR".to_string()
    } else if line_lower.contains("warn") || line_lower.contains("warning") {
        "WARN".to_string()
    } else if line_lower.contains("debug") {
        "DEBUG".to_string()
    } else {
        "INFO".to_string()
    }
}

fn extract_service_name(path: &str) -> String {
    Path::new(path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown")
        .to_string()
}
