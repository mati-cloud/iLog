use axum::extract::ws::{Message, WebSocket};
use futures::{SinkExt, StreamExt};
use std::sync::Arc;
use tokio::sync::broadcast;
use tracing::{error, info};
use uuid::Uuid;
use crate::{db::Database, models::{LogQuery, OtelLog}};

pub async fn handle_websocket(
    socket: WebSocket,
    db: Arc<Database>,
    query_params: LogQuery,
    log_rx: broadcast::Receiver<OtelLog>,
) {
    let (mut sender, mut receiver) = socket.split();

    info!("WebSocket connection established");

    // Parse service_id filter
    let service_filter: Option<Uuid> = query_params.service.as_ref().and_then(|s| s.parse().ok());
    info!("WebSocket filtering for service: {:?}", service_filter);

    // Spawn a task to send log updates from broadcast channel
    let db_clone = db.clone();
    let mut send_task = tokio::spawn(async move {
        // Send initial historical logs (last 100 from past 24h)
        let initial_query = LogQuery {
            service: query_params.service.clone(),
            service_name: None,
            severity: None,
            trace_id: None,
            start_time: Some(chrono::Utc::now() - chrono::Duration::hours(24)),
            end_time: Some(chrono::Utc::now()),
            limit: Some(100),
            search: None,
            token: None,
        };
        
        match crate::otel::query_logs(&db_clone, initial_query).await {
            Ok(logs) => {
                if !logs.is_empty() {
                    info!("Sending {} initial historical logs to WebSocket client", logs.len());
                    for log in &logs {
                        let json = serde_json::to_string(&log).unwrap_or_default();
                        if sender.send(Message::Text(json)).await.is_err() {
                            return;
                        }
                    }
                } else {
                    info!("No initial logs found for service");
                }
            }
            Err(e) => {
                error!("Error querying initial logs: {}", e);
            }
        }
        
        // Subscribe to real-time broadcast channel
        let mut rx = log_rx;
        info!("Subscribed to real-time log broadcast");

        loop {
            match rx.recv().await {
                Ok(log) => {
                    // Filter by service_id if specified
                    if let Some(filter_id) = service_filter {
                        if log.service_id != Some(filter_id) {
                            continue;
                        }
                    }

                    // Send log to WebSocket client
                    let json = serde_json::to_string(&log).unwrap_or_default();
                    if sender.send(Message::Text(json)).await.is_err() {
                        info!("WebSocket client disconnected, stopping send task");
                        break;
                    }
                }
                Err(broadcast::error::RecvError::Lagged(skipped)) => {
                    error!("WebSocket client lagged, skipped {} logs", skipped);
                    // Continue receiving - client was too slow but is still connected
                }
                Err(broadcast::error::RecvError::Closed) => {
                    info!("Broadcast channel closed, stopping send task");
                    break;
                }
            }
        }
    });

    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            match msg {
                Message::Text(text) => {
                    info!("Received message: {}", text);
                }
                Message::Close(_) => {
                    info!("Client closed connection");
                    break;
                }
                _ => {}
            }
        }
    });

    // Wait for either task to finish
    tokio::select! {
        _ = &mut send_task => {
            recv_task.abort();
        }
        _ = &mut recv_task => {
            send_task.abort();
        }
    }

    info!("WebSocket connection closed");
}
