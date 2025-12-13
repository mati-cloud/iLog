use sqlx::types::JsonValue;
use uuid::Uuid;

use crate::{
    db::Database,
    models::{LogQuery, OtelLog},
};

pub async fn ingest_logs(db: &Database, logs: Vec<OtelLog>, service_id: Uuid) -> anyhow::Result<()> {
    for log in logs {
        let time = parse_unix_nano(&log.time_unix_nano)?;

        sqlx::query(
            r#"
            INSERT INTO logs (
                time, service_id, trace_id, span_id, trace_flags,
                severity_text, severity_number, service_name,
                body, resource_attributes, log_attributes,
                scope_name, scope_version, scope_attributes
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)
            "#,
        )
        .bind(time)
        .bind(service_id)
        .bind(log.trace_id)
        .bind(log.span_id)
        .bind(log.trace_flags)
        .bind(log.severity_text)
        .bind(log.severity_number)
        .bind(log.service_name)
        .bind(log.body)
        .bind(log.resource_attributes.as_ref().map(|v| v as &JsonValue))
        .bind(log.log_attributes.as_ref().map(|v| v as &JsonValue))
        .bind(log.scope_name)
        .bind(log.scope_version)
        .bind(log.scope_attributes.as_ref().map(|v| v as &JsonValue))
        .execute(db.pool())
        .await?;
    }

    Ok(())
}

pub async fn query_logs(db: &Database, query: LogQuery) -> anyhow::Result<Vec<OtelLog>> {
    let limit = query.limit.unwrap_or(100).min(1000);
    let start_time = query.start_time.unwrap_or_else(|| {
        Utc::now() - chrono::Duration::hours(24)
    });
    let end_time = query.end_time.unwrap_or_else(Utc::now);

    let mut sql = String::from(
        r#"
        SELECT 
            time, trace_id, span_id, trace_flags,
            severity_text, severity_number, service_name,
            body, resource_attributes, log_attributes,
            scope_name, scope_version, scope_attributes
        FROM logs
        WHERE time >= $1 AND time <= $2
        "#,
    );

    let mut param_count = 2;

    if query.service.is_some() {
        param_count += 1;
        sql.push_str(&format!(" AND service_id = ${}::uuid", param_count));
    }

    if query.service_name.is_some() {
        param_count += 1;
        sql.push_str(&format!(" AND service_name = ${}", param_count));
    }

    if query.severity.is_some() {
        param_count += 1;
        sql.push_str(&format!(" AND severity_number >= ${}", param_count));
    }

    if query.trace_id.is_some() {
        param_count += 1;
        sql.push_str(&format!(" AND trace_id = ${}", param_count));
    }

    if query.search.is_some() {
        param_count += 1;
        sql.push_str(&format!(" AND body ILIKE ${}", param_count));
    }

    sql.push_str(" ORDER BY time DESC");
    param_count += 1;
    sql.push_str(&format!(" LIMIT ${}", param_count));

    let mut query_builder = sqlx::query_as::<_, LogRow>(&sql)
        .bind(start_time)
        .bind(end_time);

    if let Some(service) = &query.service {
        query_builder = query_builder.bind(service);
    }

    if let Some(service) = &query.service_name {
        query_builder = query_builder.bind(service);
    }

    if let Some(severity) = query.severity {
        query_builder = query_builder.bind(severity);
    }

    if let Some(trace_id) = &query.trace_id {
        query_builder = query_builder.bind(trace_id);
    }

    if let Some(search) = &query.search {
        query_builder = query_builder.bind(format!("%{}%", search));
    }

    query_builder = query_builder.bind(limit);

    let rows = query_builder.fetch_all(db.pool()).await?;

    let logs = rows
        .into_iter()
        .map(|row| OtelLog {
            time_unix_nano: row.time.timestamp_nanos_opt().unwrap_or(0).to_string(),
            trace_id: row.trace_id,
            span_id: row.span_id,
            trace_flags: row.trace_flags,
            severity_text: row.severity_text,
            severity_number: row.severity_number,
            service_name: row.service_name,
            body: row.body,
            resource_attributes: row.resource_attributes,
            log_attributes: row.log_attributes,
            scope_name: row.scope_name,
            scope_version: row.scope_version,
            scope_attributes: row.scope_attributes,
        })
        .collect();

    Ok(logs)
}

#[derive(sqlx::FromRow)]
struct LogRow {
    time: DateTime<Utc>,
    trace_id: Option<String>,
    span_id: Option<String>,
    trace_flags: Option<i32>,
    severity_text: Option<String>,
    severity_number: Option<i32>,
    service_name: String,
    body: String,
    resource_attributes: Option<JsonValue>,
    log_attributes: Option<JsonValue>,
    scope_name: Option<String>,
    scope_version: Option<String>,
    scope_attributes: Option<JsonValue>,
}

fn parse_unix_nano(nano_str: &str) -> anyhow::Result<DateTime<Utc>> {
    let nanos: i64 = nano_str.parse()?;
    let secs = nanos / 1_000_000_000;
    let nsecs = (nanos % 1_000_000_000) as u32;
    Ok(DateTime::from_timestamp(secs, nsecs).unwrap_or_else(Utc::now))
}
