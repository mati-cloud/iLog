use axum::{extract::State, Json};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::sync::Arc;

use crate::{db::Database, AppError, AppState};

#[derive(Debug, Serialize, Deserialize)]
pub struct DashboardMetrics {
    pub total_storage_bytes: i64,
    pub total_storage_gb: f64,
    pub agents_online: i64,
    pub agents_offline: i64,
    pub agents_total: i64,
    pub logs_today: i64,
    pub logs_yesterday: i64,
    pub logs_today_change_percent: f64,
    pub errors_24h: i64,
    pub errors_previous_24h: i64,
    pub errors_change_percent: f64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LogVolumeDataPoint {
    pub hour: String,
    pub logs: i64,
    pub errors: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StorageByServiceDataPoint {
    pub service: String,
    pub storage_bytes: i64,
    pub storage_gb: f64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AgentInfo {
    pub id: String,
    pub name: String,
    pub service_name: String,
    pub status: String,
    pub last_seen: Option<DateTime<Utc>>,
    pub last_seen_human: String,
    pub logs_today: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DailyIngestionDataPoint {
    pub day: String,
    pub date: String,
    pub logs: i64,
    pub storage_gb: f64,
}

pub async fn get_dashboard_metrics(
    State(state): State<AppState>,
) -> Result<Json<DashboardMetrics>, AppError> {
    let pool = state.db.pool();

    let now = Utc::now();
    let today_start = now.date_naive().and_hms_opt(0, 0, 0).unwrap().and_utc();
    let yesterday_start = (now - Duration::days(1)).date_naive().and_hms_opt(0, 0, 0).unwrap().and_utc();
    let hours_24_ago = now - Duration::hours(24);
    let hours_48_ago = now - Duration::hours(48);

    // Total storage
    let total_storage: Option<i64> = sqlx::query_scalar(
        "SELECT pg_total_relation_size('logs')"
    )
    .fetch_optional(pool)
    .await?
    .flatten();

    let total_storage_bytes = total_storage.unwrap_or(0);
    let total_storage_gb = total_storage_bytes as f64 / (1024.0 * 1024.0 * 1024.0);

    // Agents online/offline (consider online if last_used_at within 5 minutes)
    let five_minutes_ago = now - Duration::minutes(5);
    
    let agents_online: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM agents WHERE last_used_at > $1"
    )
    .bind(five_minutes_ago)
    .fetch_one(pool)
    .await?;

    let agents_total: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM agents"
    )
    .fetch_one(pool)
    .await?;

    let agents_offline = agents_total - agents_online;

    // Logs today
    let logs_today: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM logs WHERE timestamp >= $1"
    )
    .bind(today_start)
    .fetch_one(pool)
    .await?;

    // Logs yesterday
    let logs_yesterday: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM logs WHERE timestamp >= $1 AND timestamp < $2"
    )
    .bind(yesterday_start)
    .bind(today_start)
    .fetch_one(pool)
    .await?;

    let logs_today_change_percent = if logs_yesterday > 0 {
        ((logs_today - logs_yesterday) as f64 / logs_yesterday as f64) * 100.0
    } else {
        0.0
    };

    // Errors in last 24h (severity_number >= 17 is ERROR level in OTEL)
    let errors_24h: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM logs WHERE timestamp >= $1 AND severity_number >= 17"
    )
    .bind(hours_24_ago)
    .fetch_one(pool)
    .await?;

    // Errors in previous 24h
    let errors_previous_24h: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM logs WHERE timestamp >= $1 AND timestamp < $2 AND severity_number >= 17"
    )
    .bind(hours_48_ago)
    .bind(hours_24_ago)
    .fetch_one(pool)
    .await?;

    let errors_change_percent = if errors_previous_24h > 0 {
        ((errors_24h - errors_previous_24h) as f64 / errors_previous_24h as f64) * 100.0
    } else {
        0.0
    };

    Ok(Json(DashboardMetrics {
        total_storage_bytes,
        total_storage_gb,
        agents_online,
        agents_offline,
        agents_total,
        logs_today,
        logs_yesterday,
        logs_today_change_percent,
        errors_24h,
        errors_previous_24h,
        errors_change_percent,
    }))
}

pub async fn get_log_volume_24h(
    State(state): State<AppState>,
) -> Result<Json<Vec<LogVolumeDataPoint>>, AppError> {
    let pool = state.db.pool();
    let now = Utc::now();
    let hours_24_ago = now - Duration::hours(24);

    let data: Vec<(String, i64, i64)> = sqlx::query_as(
        r#"
        SELECT 
            TO_CHAR(date_trunc('hour', timestamp), 'HH24:MI') as hour,
            COUNT(*) as logs,
            COUNT(*) FILTER (WHERE severity_number >= 17) as errors
        FROM logs
        WHERE timestamp >= $1
        GROUP BY date_trunc('hour', timestamp)
        ORDER BY date_trunc('hour', timestamp)
        "#
    )
    .bind(hours_24_ago)
    .fetch_all(pool)
    .await?;

    let result = data.into_iter().map(|(hour, logs, errors)| {
        LogVolumeDataPoint { hour, logs, errors }
    }).collect();

    Ok(Json(result))
}

pub async fn get_storage_by_service(
    State(state): State<AppState>,
) -> Result<Json<Vec<StorageByServiceDataPoint>>, AppError> {
    let pool = state.db.pool();

    let data: Vec<(String, i64)> = sqlx::query_as(
        r#"
        SELECT 
            service_name,
            COUNT(*) as log_count
        FROM logs
        GROUP BY service_name
        ORDER BY log_count DESC
        LIMIT 10
        "#
    )
    .fetch_all(pool)
    .await?;

    // Estimate storage (rough approximation: avg log size ~500 bytes)
    let result = data.into_iter().map(|(service, count)| {
        let storage_bytes = count * 500; // Rough estimate
        let storage_gb = storage_bytes as f64 / (1024.0 * 1024.0 * 1024.0);
        StorageByServiceDataPoint {
            service,
            storage_bytes,
            storage_gb,
        }
    }).collect();

    Ok(Json(result))
}

pub async fn get_connected_agents(
    State(state): State<AppState>,
) -> Result<Json<Vec<AgentInfo>>, AppError> {
    let pool = state.db.pool();
    let now = Utc::now();
    let today_start = now.date_naive().and_hms_opt(0, 0, 0).unwrap().and_utc();

    let agents: Vec<(String, String, String, Option<DateTime<Utc>>)> = sqlx::query_as(
        r#"
        SELECT 
            a.id::text,
            a.name,
            s.name as service_name,
            a.last_used_at
        FROM agents a
        JOIN services s ON a.service_id = s.id
        ORDER BY a.last_used_at DESC NULLS LAST
        "#
    )
    .fetch_all(pool)
    .await?;

    let mut result = Vec::new();
    for (id, name, service_name, last_used_at) in agents {
        let five_minutes_ago = now - Duration::minutes(5);
        let status = if let Some(last_used) = last_used_at {
            if last_used > five_minutes_ago {
                "online"
            } else {
                "offline"
            }
        } else {
            "offline"
        };

        let last_seen_human = if let Some(last_used) = last_used_at {
            let duration = now.signed_duration_since(last_used);
            if duration.num_seconds() < 60 {
                format!("{}s ago", duration.num_seconds())
            } else if duration.num_minutes() < 60 {
                format!("{}m ago", duration.num_minutes())
            } else if duration.num_hours() < 24 {
                format!("{}h ago", duration.num_hours())
            } else {
                format!("{}d ago", duration.num_days())
            }
        } else {
            "never".to_string()
        };

        // Get logs today for this agent's service
        let logs_today: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM logs WHERE service_name = $1 AND timestamp >= $2"
        )
        .bind(&service_name)
        .bind(today_start)
        .fetch_one(pool)
        .await?;

        result.push(AgentInfo {
            id,
            name,
            service_name,
            status: status.to_string(),
            last_seen: last_used_at,
            last_seen_human,
            logs_today,
        });
    }

    Ok(Json(result))
}

pub async fn get_7day_ingestion(
    State(state): State<AppState>,
) -> Result<Json<Vec<DailyIngestionDataPoint>>, AppError> {
    let pool = state.db.pool();
    let now = Utc::now();
    let seven_days_ago = now - Duration::days(7);

    let data: Vec<(String, i64)> = sqlx::query_as(
        r#"
        SELECT 
            TO_CHAR(date_trunc('day', timestamp), 'YYYY-MM-DD') as date,
            COUNT(*) as logs
        FROM logs
        WHERE timestamp >= $1
        GROUP BY date_trunc('day', timestamp)
        ORDER BY date_trunc('day', timestamp)
        "#
    )
    .bind(seven_days_ago)
    .fetch_all(pool)
    .await?;

    let result = data.into_iter().map(|(date, logs)| {
        // Parse date and get day name
        let parsed_date = chrono::NaiveDate::parse_from_str(&date, "%Y-%m-%d")
            .unwrap_or_else(|_| Utc::now().date_naive());
        let day = parsed_date.format("%a").to_string();
        
        // Estimate storage (rough approximation: avg log size ~500 bytes)
        let storage_gb = (logs * 500) as f64 / (1024.0 * 1024.0 * 1024.0);
        
        DailyIngestionDataPoint {
            day,
            date,
            logs,
            storage_gb,
        }
    }).collect();

    Ok(Json(result))
}
