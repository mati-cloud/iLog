use tokio::sync::mpsc;
use anyhow::{Result, Context};
use std::sync::Arc;
use std::path::PathBuf;
use tokio::fs::File;
use tokio::io::{AsyncBufReadExt, AsyncSeekExt, BufReader};
use tracing::{info, error, warn};
use notify::{Watcher, RecursiveMode, EventKind};

use crate::config::AgentConfig;
use crate::sender::LogEntry;
use super::LogProvider;

pub struct FileProvider {
    config: Arc<AgentConfig>,
}

impl FileProvider {
    pub fn new(config: Arc<AgentConfig>) -> Self {
        Self { config }
    }

    async fn tail_file(path: PathBuf, tx: mpsc::Sender<LogEntry>) -> Result<()> {
        info!("Starting to tail file: {}", path.display());
        
        let file = File::open(&path).await
            .context(format!("Failed to open file: {}", path.display()))?;
        
        let mut reader = BufReader::new(file);
        
        // Seek to end of file (like tail -f)
        reader.seek(std::io::SeekFrom::End(0)).await?;
        info!("Positioned at end of file: {}", path.display());
        
        let (watch_tx, mut watch_rx) = tokio::sync::mpsc::channel(100);
        let watch_path = path.clone();
        
        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                let (notify_tx, notify_rx) = std::sync::mpsc::channel();
                
                let mut watcher = notify::recommended_watcher(notify_tx)
                    .expect("Failed to create file watcher");
                
                watcher.watch(&watch_path, RecursiveMode::NonRecursive)
                    .expect("Failed to watch file");
                
                info!("File watcher started for: {}", watch_path.display());
                
                loop {
                    match notify_rx.recv() {
                        Ok(Ok(event)) => {
                            if matches!(event.kind, EventKind::Modify(_) | EventKind::Create(_)) {
                                let _ = watch_tx.send(()).await;
                            }
                        }
                        Ok(Err(e)) => {
                            error!("Watch error: {:?}", e);
                        }
                        Err(e) => {
                            error!("Channel error: {:?}", e);
                            break;
                        }
                    }
                }
            });
        });
        
        loop {
            // Wait for file change notification
            let _ = watch_rx.recv().await;
            
            // Read all available complete lines
            let mut line = String::new();
            loop {
                line.clear();
                let bytes_read = reader.read_line(&mut line).await?;
                
                if bytes_read == 0 {
                    break; // No more data available
                }
                
                // Only process if we got a complete line (ends with newline)
                if !line.ends_with('\n') {
                    // Incomplete line, seek back and wait for next event
                    let current_pos = reader.stream_position().await?;
                    reader.seek(std::io::SeekFrom::Start(current_pos.saturating_sub(bytes_read as u64))).await?;
                    break;
                }
                
                let log_text = line.trim();
                if log_text.is_empty() {
                    continue;
                }
                
                let entry = LogEntry {
                    timestamp: chrono::Utc::now(),
                    level: "INFO".to_string(),
                    service: path.file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("unknown")
                        .to_string(),
                    message: log_text.to_string(),
                    attributes: Some(serde_json::json!({
                        "source_type": "file",
                        "file_path": path.to_string_lossy(),
                    })),
                };
                
                if let Err(e) = tx.send(entry).await {
                    error!("Failed to send log entry: {}", e);
                    return Ok(());
                }
            }
        }
    }
}

#[async_trait::async_trait]
impl LogProvider for FileProvider {
    async fn start(&self, tx: mpsc::Sender<LogEntry>) -> Result<()> {
        let file_config = match &self.config.sources.file {
            Some(cfg) if cfg.enabled => cfg,
            _ => {
                warn!("File provider is not enabled");
                return Ok(());
            }
        };
        
        info!("Starting file provider with {} paths", file_config.paths.len());
        
        let mut handles = vec![];
        let mut discovered_files = vec![];
        
        for path_pattern in &file_config.paths {
            let path = std::path::Path::new(path_pattern.as_str());
            if path.is_dir() {
                // Recursively find all .log files in directory
                info!("Discovering *.log files in directory: {}", path.display());
                let pattern = format!("{}/**/*.log", path_pattern.trim_end_matches('/'));
                match glob::glob(&pattern) {
                    Ok(paths) => {
                        for path_result in paths {
                            if let Ok(file_path) = path_result {
                                if file_path.is_file() {
                                    discovered_files.push(file_path);
                                }
                            }
                        }
                    }
                    Err(e) => {
                        error!("Failed to glob directory {}: {}", path.display(), e);
                    }
                }
            } else {
                // Expand glob patterns for files
                match glob::glob(path_pattern) {
                    Ok(paths) => {
                        for path_result in paths {
                            if let Ok(file_path) = path_result {
                                if file_path.is_file() {
                                    discovered_files.push(file_path);
                                }
                            }
                        }
                    }
                    Err(e) => {
                        error!("Invalid glob pattern {}: {}", path_pattern, e);
                    }
                }
            }
        }
        
        info!("Discovered {} log files to tail", discovered_files.len());
        
        for path in discovered_files {
            let tx_clone = tx.clone();
            
            let handle = tokio::spawn(async move {
                if let Err(e) = Self::tail_file(path.clone(), tx_clone).await {
                    error!("Error tailing file {}: {}", path.display(), e);
                }
            });
            
            handles.push(handle);
        }
        
        // Wait for all file watchers
        for handle in handles {
            let _ = handle.await;
        }
        
        Ok(())
    }
    
    fn name(&self) -> &str {
        "file"
    }
}
