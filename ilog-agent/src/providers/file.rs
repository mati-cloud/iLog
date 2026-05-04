use tokio::sync::mpsc;
use anyhow::{Result, Context};
use std::sync::Arc;
use std::path::PathBuf;
use tokio::fs::File;
use tokio::io::{AsyncBufReadExt, AsyncSeekExt, BufReader};
use tracing::{info, error, warn};
use notify::{Watcher, RecursiveMode, EventKind};

use crate::tcp_sender::LogEntry;
use crate::parser::{ParserConfig, ParserType, JsonParser, RegexParser, LogParser};
use super::LogProvider;

pub struct FileProvider {
    parser_config: Arc<ParserConfig>,
}

impl FileProvider {
    pub fn new(parser_config: ParserConfig) -> Self {
        Self {
            parser_config: Arc::new(parser_config),
        }
    }

    async fn tail_file(
        path: PathBuf,
        parser: Arc<dyn LogParser>,
        service_name: String,
        tx: mpsc::Sender<LogEntry>,
    ) -> Result<()> {
        info!("Starting to tail file: {}", path.display());
        
        let file = File::open(&path).await
            .context(format!("Failed to open file: {}", path.display()))?;
        
        let mut reader = BufReader::new(file);
        reader.seek(std::io::SeekFrom::End(0)).await?;
        
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
                        Ok(Err(e)) => error!("Watch error: {:?}", e),
                        Err(e) => {
                            error!("Channel error: {:?}", e);
                            break;
                        }
                    }
                }
            });
        });
        
        loop {
            let _ = watch_rx.recv().await;
            
            let mut line = String::new();
            loop {
                line.clear();
                let bytes_read = reader.read_line(&mut line).await?;
                
                if bytes_read == 0 {
                    break;
                }
                
                if !line.ends_with('\n') {
                    let current_pos = reader.stream_position().await?;
                    reader.seek(std::io::SeekFrom::Start(current_pos.saturating_sub(bytes_read as u64))).await?;
                    break;
                }
                
                let log_text = line.trim();
                if log_text.is_empty() {
                    continue;
                }
                
                // Parse the log line
                let parsed_attrs = parser.parse(log_text);
                
                let entry = LogEntry {
                    timestamp: chrono::Utc::now(),
                    level: parsed_attrs.as_ref()
                        .and_then(|m| m.get("severity").or(m.get("level")))
                        .and_then(|v| v.as_str())
                        .unwrap_or("INFO")
                        .to_string(),
                    service: service_name.clone(),
                    message: log_text.to_string(),
                    attributes: parsed_attrs.map(|m| serde_json::to_value(m).unwrap_or(serde_json::json!({}))),
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
        info!("Starting FileProvider with {} sources", self.parser_config.sources.len());
        
        let mut handles = vec![];
        
        for source in &self.parser_config.sources {
            info!("Setting up source: {} ({})", source.name, source.path);
            
            // Create parser based on type
            let parser: Arc<dyn LogParser> = match &source.parser {
                ParserType::Json { fields } => {
                    Arc::new(JsonParser::new(fields.clone()))
                }
                ParserType::Regex { pattern, fields } => {
                    match RegexParser::new(pattern, fields.clone()) {
                        Ok(p) => Arc::new(p),
                        Err(e) => {
                            error!("Invalid regex pattern for {}: {}", source.name, e);
                            continue;
                        }
                    }
                }
            };
            
            // Expand glob pattern
            let paths: Vec<PathBuf> = match glob::glob(&source.path) {
                Ok(paths) => paths.filter_map(Result::ok).filter(|p| p.is_file()).collect(),
                Err(e) => {
                    error!("Invalid glob pattern {}: {}", source.path, e);
                    continue;
                }
            };
            
            if paths.is_empty() {
                warn!("No files found for pattern: {}", source.path);
                continue;
            }
            
            info!("Found {} files for source {}", paths.len(), source.name);
            
            for path in paths {
                let tx_clone = tx.clone();
                let parser_clone = parser.clone();
                let service_name = source.name.clone();
                
                let handle = tokio::spawn(async move {
                    if let Err(e) = Self::tail_file(path.clone(), parser_clone, service_name, tx_clone).await {
                        error!("Error tailing file {}: {}", path.display(), e);
                    }
                });
                
                handles.push(handle);
            }
        }
        
        for handle in handles {
            let _ = handle.await;
        }
        
        Ok(())
    }
    
    fn name(&self) -> &str {
        "file"
    }
}
