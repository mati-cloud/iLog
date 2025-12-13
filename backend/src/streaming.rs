use futures::{SinkExt, StreamExt};
use std::sync::Arc;
use tokio::time::{interval, Duration};
use tracing::{error, info};
use crate::{db::Database, models::LogQuery};

pub async fn handle_websocket(socket: WebSocket, db: Arc<Database>, query_params: LogQuery) {
    let (mut sender, mut receiver) = socket.split();

    info!("WebSocket connection established");

    // Spawn a task to send log updates
    let db_clone = db.clone();
    let service_filter = query_params.service;
    let mut send_task = tokio::spawn(async move {
        let mut ticker = interval(Duration::from_secs(2));
        
        let initial_query = LogQuery {
            service: service_filter.clone(),
            service_name: None,
            severity: None,
            trace_id: None,
            start_time: Some(chrono::Utc::now() - chrono::Duration::hours(24)),
            end_time: Some(chrono::Utc::now()),
            limit: Some(100),
            search: None,
        };
        
        match crate::otel::query_logs(&db_clone, initial_query).await {
            Ok(logs) => {
                if !logs.is_empty() {
                    info!("Sending {} initial logs to WebSocket client", logs.len());
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
        
        let mut last_check = chrono::Utc::now();

        loop {
            ticker.tick().await;

            let now = chrono::Utc::now();
            let query = LogQuery {
                service: service_filter.clone(),
                service_name: None,
                severity: None,
                trace_id: None,
                start_time: Some(last_check),
                end_time: Some(now),
                limit: Some(100),
                search: None,
            };

            match crate::otel::query_logs(&db_clone, query).await {
                Ok(logs) => {
                    if !logs.is_empty() {
                        info!("Sending {} logs to WebSocket client", logs.len());
                        for log in &logs {
                            let json = serde_json::to_string(&log).unwrap_or_default();
                            if sender.send(Message::Text(json)).await.is_err() {
                                break;
                            }
                        }
                    }
                }
                Err(e) => {
                    error!("Error querying logs: {}", e);
                }
            }

            last_check = now;
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
