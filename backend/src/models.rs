use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct OtelLog {
    #[serde(rename = "timeUnixNano")]
    pub time_unix_nano: String,
    #[serde(rename = "traceId", skip_serializing_if = "Option::is_none")]
    pub trace_id: Option<String>,
    #[serde(rename = "spanId", skip_serializing_if = "Option::is_none")]
    pub span_id: Option<String>,
    #[serde(rename = "traceFlags", skip_serializing_if = "Option::is_none")]
    pub trace_flags: Option<i32>,
    #[serde(rename = "severityText", skip_serializing_if = "Option::is_none")]
    pub severity_text: Option<String>,
    #[serde(rename = "severityNumber", skip_serializing_if = "Option::is_none")]
    pub severity_number: Option<i32>,
    #[serde(rename = "serviceName")]
    pub service_name: String,
    pub body: String,
    #[serde(rename = "resourceAttributes", skip_serializing_if = "Option::is_none")]
    pub resource_attributes: Option<JsonValue>,
    #[serde(rename = "logAttributes", skip_serializing_if = "Option::is_none")]
    pub log_attributes: Option<JsonValue>,
    #[serde(rename = "scopeName", skip_serializing_if = "Option::is_none")]
    pub scope_name: Option<String>,
    #[serde(rename = "scopeVersion", skip_serializing_if = "Option::is_none")]
    pub scope_version: Option<String>,
    #[serde(rename = "scopeAttributes", skip_serializing_if = "Option::is_none")]
    pub scope_attributes: Option<JsonValue>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LogQuery {
    pub service: Option<String>,
    pub service_name: Option<String>,
    pub severity: Option<i32>,
    pub trace_id: Option<String>,
    pub start_time: Option<DateTime<Utc>>,
    pub end_time: Option<DateTime<Utc>>,
    pub limit: Option<i64>,
    pub search: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    pub sub: String,
    pub exp: usize,
}

// Service models
#[derive(Debug, Serialize, Deserialize, Clone, sqlx::FromRow)]
pub struct Service {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub owner_id: String,
    pub source_type: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateService {
    pub name: String,
    pub description: Option<String>,
    pub source_type: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdateService {
    pub name: Option<String>,
    pub description: Option<String>,
    pub source_type: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, sqlx::FromRow)]
pub struct Agent {
    pub id: Uuid,
    pub service_id: Uuid,
    pub name: String,
    pub token: String,
    pub source_type: String,
    pub expires_at: Option<DateTime<Utc>>,
    pub last_used_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateAgent {
    pub name: String,
    pub source_type: String,
    pub expires_in_days: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AgentClaims {
    pub service_id: Uuid,
    pub agent_id: Uuid,
}
