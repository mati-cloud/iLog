use tokio::sync::mpsc;
use anyhow::{Result, Context};
use async_trait::async_trait;
use std::sync::Arc;
use bollard::{Docker, container::LogsOptions};
use bollard::container::LogOutput;
use futures::StreamExt;
use tracing::{info, error, warn};
use regex::Regex;

use crate::config::AgentConfig;
use crate::tcp_sender::LogEntry;
use super::LogProvider;

pub struct DockerProvider {
    config: Arc<AgentConfig>,
}

impl DockerProvider {
    pub fn new(config: Arc<AgentConfig>) -> Self {
        Self { config }
    }
}

/// Strip ANSI color codes from text
fn strip_ansi_codes(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    
    while let Some(ch) = chars.next() {
        if ch == '\x1b' {
            if chars.peek() == Some(&'[') {
                chars.next();
                while let Some(&c) = chars.peek() {
                    chars.next();
                    if c.is_ascii_alphabetic() {
                        break;
                    }
                }
            }
        } else {
            result.push(ch);
        }
    }
    
    result
}

/// Parse log level from message and return cleaned message
fn parse_log_level_and_clean(message: &str) -> (Option<String>, String) {
    let upper = message.to_uppercase();
    
    let patterns = [
        ("ERROR", "ERROR"),
        ("ERR", "ERROR"),
        ("WARN", "WARN"),
        ("WARNING", "WARN"),
        ("INFO", "INFO"),
        ("DEBUG", "DEBUG"),
    ];
    
    for (pattern, level) in patterns.iter() {
        let pattern_variations = vec![
            format!(" {} ", pattern),
            format!("[{}]", pattern),
            format!("({})", pattern),
            format!("{} ", pattern),
            format!(" {}", pattern),
        ];
        
        for var in pattern_variations {
            if upper.contains(&var.to_uppercase()) {
                let mut cleaned = message.to_string();
                cleaned = cleaned.replace(&var, " ");
                cleaned = cleaned.replace(&var.to_lowercase(), " ");
                cleaned = cleaned.replace(&var.to_uppercase(), " ");
                
                while cleaned.contains("  ") {
                    cleaned = cleaned.replace("  ", " ");
                }
                
                return (Some(level.to_string()), cleaned.trim().to_string());
            }
        }
    }
    
    (None, message.to_string())
}

/// Parse timestamp from message and return cleaned message
fn parse_timestamp_and_clean(message: &str) -> (Option<chrono::DateTime<chrono::Utc>>, String) {
    use chrono::DateTime;
    use regex::Regex;
    
    let re = Regex::new(r"^\d{4}-\d{2}-\d{2}[T ]\d{2}:\d{2}:\d{2}(?:\.\d+)?(?:Z|[+-]\d{2}:\d{2})?(?:\s+UTC)?(?:\s+\[\d+\])?(?:\s+\w+:)?").unwrap();
    
    if let Some(mat) = re.find(message) {
        let timestamp_str = mat.as_str();
        let rest = &message[mat.end()..].trim_start();
        
        let iso_part = timestamp_str
            .split_whitespace()
            .next()
            .unwrap_or(timestamp_str);
        
        if let Ok(dt) = DateTime::parse_from_rfc3339(iso_part) {
            return (Some(dt.with_timezone(&chrono::Utc)), rest.to_string());
        }
        
        let with_z = if !iso_part.ends_with('Z') && !iso_part.contains('+') {
            format!("{}Z", iso_part)
        } else {
            iso_part.to_string()
        };
        
        if let Ok(dt) = DateTime::parse_from_rfc3339(&with_z) {
            return (Some(dt.with_timezone(&chrono::Utc)), rest.to_string());
        }
    }
    
    (None, message.to_string())
}

async fn watch_container(
    docker: Docker,
    container_name: String,
    tx: mpsc::Sender<LogEntry>,
) -> Result<()> {
    info!("Starting to watch container: {}", container_name);
    
    let options = LogsOptions::<String> {
        follow: true,
        stdout: true,
        stderr: true,
        tail: "10".to_string(),
        ..Default::default()
    };

    let mut stream = docker.logs(&container_name, Some(options));
    
    let mut pending_log: Option<(chrono::DateTime<chrono::Utc>, String, String, String)> = None;
    let mut bracket_depth: i32 = 0;

    while let Some(log_result) = stream.next().await {
        match log_result {
            Ok(log_output) => {
                let log_text = match log_output {
                    LogOutput::StdOut { message } => {
                        String::from_utf8_lossy(&message).to_string()
                    }
                    LogOutput::StdErr { message } => {
                        String::from_utf8_lossy(&message).to_string()
                    }
                    _ => continue,
                };

                info!("Raw log from {}: {}", container_name, log_text.trim());

                let log_text = strip_ansi_codes(&log_text);
                let log_text = log_text.trim();
                
                if log_text.is_empty() {
                    continue;
                }

                let (parsed_timestamp, cleaned_message) = parse_timestamp_and_clean(&log_text);
                
                for ch in log_text.chars() {
                    match ch {
                        '{' => bracket_depth += 1,
                        '}' => bracket_depth = bracket_depth.saturating_sub(1),
                        _ => {}
                    }
                }
                
                if let Some(ts) = parsed_timestamp {
                    if bracket_depth == 0 {
                        if let Some((pending_ts, pending_level, pending_service, pending_msg)) = pending_log.take() {
                            let mut attributes = serde_json::Map::new();
                            attributes.insert("container".to_string(), serde_json::Value::String(container_name.clone()));
                            attributes.insert("source_type".to_string(), serde_json::Value::String("docker".to_string()));

                            let entry = LogEntry {
                                timestamp: pending_ts,
                                level: pending_level,
                                service: pending_service,
                                message: pending_msg,
                                attributes: Some(serde_json::Value::Object(attributes)),
                            };

                            if let Err(e) = tx.send(entry).await {
                                error!("Failed to send log entry: {}", e);
                                break;
                            }
                        }
                    }
                    
                    let (parsed_level, final_message) = parse_log_level_and_clean(&cleaned_message);
                    let level = parsed_level.unwrap_or_else(|| "INFO".to_string());
                    
                    if let Some((ref _ts, ref _level, ref _service, ref mut msg)) = pending_log {
                        msg.push('\n');
                        msg.push_str(&final_message);
                    } else {
                        pending_log = Some((ts, level, container_name.clone(), final_message));
                    }
                } else {
                    if let Some((ref _ts, ref _level, ref _service, ref mut msg)) = pending_log {
                        msg.push('\n');
                        msg.push_str(log_text);
                        
                        if bracket_depth == 0 {
                            if let Some((pending_ts, pending_level, pending_service, pending_msg)) = pending_log.take() {
                                let mut attributes = serde_json::Map::new();
                                attributes.insert("container".to_string(), serde_json::Value::String(container_name.clone()));
                                attributes.insert("source_type".to_string(), serde_json::Value::String("docker".to_string()));

                                let entry = LogEntry {
                                    timestamp: pending_ts,
                                    level: pending_level,
                                    service: pending_service,
                                    message: pending_msg,
                                    attributes: Some(serde_json::Value::Object(attributes)),
                                };

                                if let Err(e) = tx.send(entry).await {
                                    error!("Failed to send log entry: {}", e);
                                    break;
                                }
                            }
                        }
                    } else {
                        let (parsed_level, final_message) = parse_log_level_and_clean(&log_text);
                        let level = parsed_level.unwrap_or_else(|| "INFO".to_string());
                        
                        let mut attributes = serde_json::Map::new();
                        attributes.insert("container".to_string(), serde_json::Value::String(container_name.clone()));
                        attributes.insert("source_type".to_string(), serde_json::Value::String("docker".to_string()));

                        let entry = LogEntry {
                            timestamp: chrono::Utc::now(),
                            level,
                            service: container_name.clone(),
                            message: final_message,
                            attributes: Some(serde_json::Value::Object(attributes)),
                        };

                        if let Err(e) = tx.send(entry).await {
                            error!("Failed to send log entry: {}", e);
                            break;
                        }
                    }
                }
            }
            Err(e) => {
                error!("Error reading logs from container {}: {}", container_name, e);
                break;
            }
        }
    }

    Ok(())
}

#[async_trait]
impl LogProvider for DockerProvider {
    async fn start(&self, tx: mpsc::Sender<LogEntry>) -> Result<()> {
        let docker_config = match &self.config.sources.docker {
            Some(cfg) if cfg.enabled => cfg,
            _ => {
                warn!("Docker provider is not enabled");
                return Ok(());
            }
        };

        info!("Starting Docker provider...");
        
        let docker = Docker::connect_with_local_defaults()
            .context("Failed to connect to Docker daemon")?;
        
        info!("Connected to Docker daemon");
        info!("Watching {} containers: {:?}", docker_config.containers.len(), docker_config.containers);

        let mut handles = vec![];
        
        for container_name in &docker_config.containers {
            let docker_clone = docker.clone();
            let container_name_clone = container_name.clone();
            let tx_clone = tx.clone();
            
            let handle = tokio::spawn(async move {
                if let Err(e) = watch_container(docker_clone, container_name_clone.clone(), tx_clone).await {
                    error!("Error watching container {}: {}", container_name_clone, e);
                }
            });
            
            handles.push(handle);
        }

        for handle in handles {
            let _ = handle.await;
        }

        Ok(())
    }
    
    fn name(&self) -> &str {
        "docker"
    }
}
