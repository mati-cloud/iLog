use bollard::container::{LogOutput, LogsOptions};
use bollard::Docker;
use futures::stream::StreamExt;
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{error, info, warn};

use crate::config::AgentConfig;
use crate::sender::LogEntry;

/// Strip ANSI escape codes from text
fn strip_ansi_codes(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    
    while let Some(ch) = chars.next() {
        if ch == '\x1b' {
            // Skip ESC sequence
            if chars.peek() == Some(&'[') {
                chars.next(); // consume '['
                // Skip until we hit a letter (end of ANSI sequence)
                while let Some(&next_ch) = chars.peek() {
                    chars.next();
                    if next_ch.is_ascii_alphabetic() {
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
    
    // Common log level patterns - more aggressive matching
    let patterns = [
        ("ERROR", "ERROR"),
        ("ERR", "ERROR"),
        ("WARN", "WARN"),
        ("WARNING", "WARN"),
        ("INFO", "INFO"),
        ("DEBUG", "DEBUG"),
    ];
    
    for (pattern, level) in patterns.iter() {
        // Look for pattern as a word (surrounded by spaces, brackets, or at start/end)
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
                
                // Clean up multiple spaces
                while cleaned.contains("  ") {
                    cleaned = cleaned.replace("  ", " ");
                }
                
                return (Some(level.to_string()), cleaned.trim().to_string());
            }
        }
    }
    
    (None, message.to_string())
}

/// Parse timestamp from message (ISO 8601 format) and return cleaned message
fn parse_timestamp_and_clean(message: &str) -> (Option<chrono::DateTime<chrono::Utc>>, String) {
    use chrono::DateTime;
    use regex::Regex;
    
    // Pattern for ISO 8601 timestamps with optional timezone and extra info
    // Matches: 2025-12-11T02:53:38.909 UTC [26] LOG: or 2025-12-11T02:53:38.909Z
    let re = Regex::new(r"^\d{4}-\d{2}-\d{2}[T ]\d{2}:\d{2}:\d{2}(?:\.\d+)?(?:Z|[+-]\d{2}:\d{2})?(?:\s+UTC)?(?:\s+\[\d+\])?(?:\s+\w+:)?").unwrap();
    
    if let Some(mat) = re.find(message) {
        let timestamp_str = mat.as_str();
        let rest = &message[mat.end()..].trim_start();
        
        // Try to parse the timestamp part (extract just the ISO portion)
        let iso_part = timestamp_str
            .split_whitespace()
            .next()
            .unwrap_or(timestamp_str);
        
        // Try parsing with Z suffix
        if let Ok(dt) = DateTime::parse_from_rfc3339(iso_part) {
            return (Some(dt.with_timezone(&chrono::Utc)), rest.to_string());
        }
        
        // Try adding Z if missing
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

pub async fn watch_docker(config: Arc<AgentConfig>, tx: mpsc::Sender<LogEntry>) -> Result<()> {
    let docker_config = match &config.sources.docker {
        Some(cfg) if cfg.enabled => cfg,
        _ => {
            info!("Docker watching disabled");
            return Ok(());
        }
    };

    info!("Starting Docker log collection...");
    
    // Connect to Docker daemon
    let docker = Docker::connect_with_local_defaults()?;
    
    // Verify connection
    docker.ping().await?;
    info!("Connected to Docker daemon");

    let containers = docker_config.containers.clone();
    
    if containers.is_empty() {
        warn!("No containers specified for Docker log collection");
        return Ok(());
    }

    info!("Watching {} containers: {:?}", containers.len(), containers);

    // Spawn a task for each container
    for container_name in containers {
        let docker = docker.clone();
        let tx = tx.clone();
        let container = container_name.clone();
        
        tokio::spawn(async move {
            if let Err(e) = watch_container(docker, container.clone(), tx).await {
                error!("Error watching container {}: {}", container, e);
            }
        });
    }

    Ok(())
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
        tail: "10".to_string(), // Start with last 10 lines
        ..Default::default()
    };

    let mut stream = docker.logs(&container_name, Some(options));
    
    // Buffer for multiline logs
    let mut pending_log: Option<(chrono::DateTime<chrono::Utc>, String, String, String)> = None; // (timestamp, level, service, message)
    let mut bracket_depth: i32 = 0; // Track JSON/bracket nesting

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
                    LogOutput::StdIn { message } => {
                        String::from_utf8_lossy(&message).to_string()
                    }
                    LogOutput::Console { message } => {
                        String::from_utf8_lossy(&message).to_string()
                    }
                };

                // Strip ANSI color codes
                let log_text = strip_ansi_codes(&log_text);
                let log_text = log_text.trim();
                
                if log_text.is_empty() {
                    continue;
                }

                let (parsed_timestamp, cleaned_message) = parse_timestamp_and_clean(&log_text);
                
                // Count only curly brackets for JSON object detection
                for ch in log_text.chars() {
                    match ch {
                        '{' => bracket_depth += 1,
                        '}' => bracket_depth = bracket_depth.saturating_sub(1),
                        _ => {}
                    }
                }
                
                if let Some(ts) = parsed_timestamp {
                    // This is a new log entry with timestamp
                    // Only send pending log if brackets are balanced (JSON complete)
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
                        // Already have pending log with open brackets, append
                        msg.push('\n');
                        msg.push_str(&final_message);
                    } else {
                        pending_log = Some((ts, level, container_name.clone(), final_message));
                    }
                } else {
                    // This is a continuation line (no timestamp)
                    if let Some((ref _ts, ref _level, ref _service, ref mut msg)) = pending_log {
                        // Append to existing message with newline
                        msg.push('\n');
                        msg.push_str(log_text);
                        
                        // If brackets are balanced, send the log
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
                        // No pending log, treat as standalone with current time
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
                error!("Error reading logs from {}: {}", container_name, e);
                // Wait a bit before retrying
                tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
            }
        }
    }

    warn!("Container {} log stream ended", container_name);
    Ok(())
}
