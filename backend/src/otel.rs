use chrono::{DateTime, Utc};
use sqlx::types::JsonValue;
use uuid::Uuid;

use crate::{
    db::Database,
    models::{LogQuery, OtelLog},
};

/// Insert a batch of logs in a single statement.
///
/// Each column is sent as an array and expanded server-side with `UNNEST`, so a
/// batch of any size costs one round-trip. The previous implementation issued
/// one `INSERT` per log, making latency scale linearly with batch size and
/// putting the network round-trip in the hot path.
pub async fn ingest_logs(db: &Database, logs: Vec<OtelLog>, service_id: Uuid) -> anyhow::Result<()> {
    if logs.is_empty() {
        return Ok(());
    }

    let n = logs.len();
    let mut times = Vec::with_capacity(n);
    let mut trace_ids = Vec::with_capacity(n);
    let mut span_ids = Vec::with_capacity(n);
    let mut trace_flags = Vec::with_capacity(n);
    let mut severity_texts = Vec::with_capacity(n);
    let mut severity_numbers = Vec::with_capacity(n);
    let mut service_names = Vec::with_capacity(n);
    let mut bodies = Vec::with_capacity(n);
    let mut resource_attrs = Vec::with_capacity(n);
    let mut log_attrs = Vec::with_capacity(n);
    let mut scope_names = Vec::with_capacity(n);
    let mut scope_versions = Vec::with_capacity(n);
    let mut scope_attrs = Vec::with_capacity(n);

    for log in logs {
        times.push(parse_unix_nano(&log.time_unix_nano)?);
        trace_ids.push(log.trace_id);
        span_ids.push(log.span_id);
        trace_flags.push(log.trace_flags);
        severity_texts.push(log.severity_text);
        severity_numbers.push(log.severity_number);
        service_names.push(log.service_name);
        bodies.push(log.body);
        resource_attrs.push(log.resource_attributes);
        log_attrs.push(log.log_attributes);
        scope_names.push(log.scope_name);
        scope_versions.push(log.scope_version);
        scope_attrs.push(log.scope_attributes);
    }

    sqlx::query(
        r#"
        INSERT INTO logs (
            time, service_id, trace_id, span_id, trace_flags,
            severity_text, severity_number, service_name,
            body, resource_attributes, log_attributes,
            scope_name, scope_version, scope_attributes
        )
        SELECT
            t, $1, trace_id, span_id, trace_flag,
            severity_text, severity_number, service_name,
            body, resource_attributes, log_attributes,
            scope_name, scope_version, scope_attributes
        FROM UNNEST(
            $2::timestamptz[], $3::varchar[], $4::varchar[], $5::int[],
            $6::varchar[], $7::int[], $8::varchar[], $9::text[],
            $10::jsonb[], $11::jsonb[], $12::varchar[], $13::varchar[], $14::jsonb[]
        ) AS batch(
            t, trace_id, span_id, trace_flag,
            severity_text, severity_number, service_name, body,
            resource_attributes, log_attributes,
            scope_name, scope_version, scope_attributes
        )
        "#,
    )
    .bind(service_id)
    .bind(&times)
    .bind(&trace_ids)
    .bind(&span_ids)
    .bind(&trace_flags)
    .bind(&severity_texts)
    .bind(&severity_numbers)
    .bind(&service_names)
    .bind(&bodies)
    .bind(&resource_attrs)
    .bind(&log_attrs)
    .bind(&scope_names)
    .bind(&scope_versions)
    .bind(&scope_attrs)
    .execute(db.pool())
    .await?;

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
            service_id: None,
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
