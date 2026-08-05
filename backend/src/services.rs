use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Extension,
    Json,
};
use chrono::{Duration, Utc};
use rand::Rng;
use uuid::Uuid;
use sha2::{Sha256, Digest};

use crate::{
    models::{
        Agent, AgentCreatedResponse, AgentResponse, Claims, CreateAgent, CreateService, Service,
        UpdateService,
    },
    AppState,
};

// Generate a slug from a service name
fn generate_slug(name: &str) -> String {
    name.to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

// Generate a secure random token
/// Mint an agent token that carries the agent's own id.
///
/// Format: `agt_<agent_id_simple>_<key_secret>`, where `key_secret` is 32 random
/// alphanumerics.
///
/// Embedding the id lets an agent name itself on the wire without a second
/// config field, which in turn lets the backend resolve the right decryption key
/// with one primary-key lookup instead of trying every registered token. The id
/// is not a secret and carries no authority on its own: `key_secret` is the
/// actual credential, and possession is proven by the AEAD tag.
///
/// Returns the full token and `key_secret` separately. Only the latter is
/// persisted (encrypted, see [`crate::token_crypto`]); the full token is shown
/// to the operator once at creation and is not recoverable afterward.
fn generate_token(agent_id: Uuid) -> (String, String) {
    let key_secret: String = rand::thread_rng()
        .sample_iter(&rand::distributions::Alphanumeric)
        .take(32)
        .map(char::from)
        .collect();
    let token = format!("agt_{}_{}", agent_id.simple(), key_secret);
    (token, key_secret)
}

/// Rebuild the token string for an agent from its id and stored `key_secret`.
///
/// The transport key is derived from the whole token rather than from
/// `key_secret` alone, so the backend has to reassemble the exact string the
/// agent holds. Keeping the derivation input unchanged is what lets the agent
/// side stay untouched by the at-rest encryption work.
pub fn token_from_parts(agent_id: Uuid, key_secret: &str) -> String {
    format!("agt_{}_{}", agent_id.simple(), key_secret)
}

// `agent_id_from_token` used to live here for `validate_agent_token`'s benefit.
// The backend no longer receives a token string on any path: the TCP frame header
// carries the agent id directly, and the token is only ever built outward via
// [`token_from_parts`]. The agent still parses its own configured token; see
// `ilog-agent/src/protocol.rs`.

fn better_auth_id_to_uuid(better_auth_id: &str) -> Uuid {
    // Hash the Better Auth ID to get a deterministic UUID
    let mut hasher = Sha256::new();
    hasher.update(better_auth_id.as_bytes());
    let hash = hasher.finalize();
    
    // Take first 16 bytes of hash and convert to UUID
    let mut bytes = [0u8; 16];
    bytes.copy_from_slice(&hash[0..16]);
    
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    
    Uuid::from_bytes(bytes)
}

fn get_user_uuid(claims: &Claims) -> Uuid {
    Uuid::parse_str(&claims.sub).unwrap_or_else(|_| better_auth_id_to_uuid(&claims.sub))
}

fn get_user_id(claims: &Claims) -> &str {
    &claims.sub
}

pub async fn create_service(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(payload): Json<CreateService>,
) -> Result<impl IntoResponse, StatusCode> {
    let user_id = get_user_uuid(&claims);
    tracing::info!("Creating service for user: {} (UUID: {})", claims.sub, user_id);

    let slug = generate_slug(&payload.name);

    let service = sqlx::query_as::<_, Service>(
        r#"
        INSERT INTO services (name, slug, description, owner_id, source_type)
        VALUES ($1, $2, $3, $4, $5)
        RETURNING id, name, slug, description, owner_id, source_type, created_at, updated_at
        "#,
    )
    .bind(&payload.name)
    .bind(&slug)
    .bind(&payload.description)
    .bind(&claims.sub)
    .bind(&payload.source_type)
    .fetch_one(state.db.pool())
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    sqlx::query(
        r#"
        INSERT INTO service_members (service_id, user_id, role)
        VALUES ($1, $2, 'owner')
        "#,
    )
    .bind(service.id)
    .bind(&claims.sub)
    .execute(state.db.pool())
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok((StatusCode::CREATED, Json(service)))
}

// List user's services
pub async fn list_services(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> Result<impl IntoResponse, StatusCode> {
    let user_id = get_user_uuid(&claims);
    tracing::info!("Listing services for user: {} (UUID: {})", claims.sub, user_id);

    let services = sqlx::query_as::<_, Service>(
        r#"
        SELECT p.id, p.name, p.slug, p.description, p.owner_id, p.source_type, p.created_at, p.updated_at
        FROM services p
        INNER JOIN service_members pm ON p.id = pm.service_id
        WHERE pm.user_id = $1
        ORDER BY p.created_at DESC
        "#,
    )
    .bind(&claims.sub)
    .fetch_all(state.db.pool())
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(services))
}

pub async fn get_service(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(service_id): Path<Uuid>,
) -> Result<impl IntoResponse, StatusCode> {
    let user_id = get_user_id(&claims);

    let has_access = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM service_members WHERE service_id = $1 AND user_id = $2)",
    )
    .bind(service_id)
    .bind(user_id)
    .fetch_one(state.db.pool())
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if !has_access {
        return Err(StatusCode::FORBIDDEN);
    }

    let service = sqlx::query_as::<_, Service>(
        "SELECT id, name, slug, description, owner_id, source_type, created_at, updated_at FROM services WHERE id = $1",
    )
    .bind(service_id)
    .fetch_optional(state.db.pool())
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    .ok_or(StatusCode::NOT_FOUND)?;

    Ok(Json(service))
}

pub async fn update_service(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(service_id): Path<Uuid>,
    Json(payload): Json<UpdateService>,
) -> Result<impl IntoResponse, StatusCode> {
    let user_id = get_user_id(&claims);

    let role = sqlx::query_scalar::<_, Option<String>>(
        "SELECT role FROM service_members WHERE service_id = $1 AND user_id = $2",
    )
    .bind(service_id)
    .bind(user_id)
    .fetch_optional(state.db.pool())
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    .flatten()
    .ok_or(StatusCode::FORBIDDEN)?;

    if role != "owner" && role != "admin" {
        return Err(StatusCode::FORBIDDEN);
    }

    let mut query = String::from("UPDATE services SET updated_at = NOW()");
    let mut params: Vec<String> = vec![];

    if let Some(name) = &payload.name {
        params.push(format!("name = '{}'", name));
    }
    if let Some(description) = &payload.description {
        params.push(format!("description = '{}'", description));
    }
    if let Some(source_type) = &payload.source_type {
        params.push(format!("source_type = '{}'", source_type));
    }

    if !params.is_empty() {
        query.push_str(", ");
        query.push_str(&params.join(", "));
    }

    query.push_str(&format!(" WHERE id = '{}' RETURNING id, name, slug, description, owner_id, source_type, created_at, updated_at", service_id));

    let service = sqlx::query_as::<_, Service>(&query)
        .fetch_one(state.db.pool())
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(service))
}

pub async fn delete_service(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(service_id): Path<Uuid>,
) -> Result<impl IntoResponse, StatusCode> {
    let user_id = get_user_id(&claims);

    let role = sqlx::query_scalar::<_, Option<String>>(
        "SELECT role FROM service_members WHERE service_id = $1 AND user_id = $2",
    )
    .bind(service_id)
    .bind(user_id)
    .fetch_optional(state.db.pool())
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    .flatten()
    .ok_or(StatusCode::FORBIDDEN)?;

    if role != "owner" {
        return Err(StatusCode::FORBIDDEN);
    }

    sqlx::query("DELETE FROM services WHERE id = $1")
        .bind(service_id)
        .execute(state.db.pool())
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(StatusCode::NO_CONTENT)
}

pub async fn create_agent(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(service_id): Path<Uuid>,
    Json(payload): Json<CreateAgent>,
) -> Result<impl IntoResponse, StatusCode> {
    let user_id = get_user_id(&claims);

    let has_access = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM service_members WHERE service_id = $1 AND user_id = $2 AND role IN ('owner', 'admin'))",
    )
    .bind(service_id)
    .bind(user_id)
    .fetch_one(state.db.pool())
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if !has_access {
        return Err(StatusCode::FORBIDDEN);
    }

    // The id is generated here rather than by the database so it can be
    // embedded in the token itself.
    let agent_id = Uuid::new_v4();
    let (token, key_secret) = generate_token(agent_id);
    let expires_at = payload
        .expires_in_days
        .map(|days| Utc::now() + Duration::days(days));

    // Wrapped before it ever reaches the database. A failure here must not fall
    // through to storing anything: no agent is better than an agent whose secret
    // is readable.
    let key_secret_encrypted = state.token_crypto.wrap(&key_secret).map_err(|e| {
        tracing::error!("Failed to wrap key secret for new agent: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let agent: Agent = sqlx::query_as(
        r#"
        INSERT INTO agents (id, service_id, name, key_secret_encrypted, source_type, expires_at)
        VALUES ($1, $2, $3, $4, $5, $6)
        RETURNING id, service_id, name, source_type, expires_at, last_used_at, created_at
        "#,
    )
    .bind(agent_id)
    .bind(service_id)
    .bind(&payload.name)
    .bind(&key_secret_encrypted)
    .bind(&payload.source_type)
    .bind(expires_at)
    .fetch_one(state.db.pool())
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // The only time the token is ever returned. It cannot be recovered from the
    // stored ciphertext without TOKEN_ENCRYPTION_KEY, and no other route exposes
    // it -- if the operator loses it, the agent has to be reissued.
    Ok((
        StatusCode::CREATED,
        Json(AgentCreatedResponse {
            id: agent.id,
            service_id: agent.service_id,
            name: agent.name,
            source_type: agent.source_type,
            expires_at: agent.expires_at,
            last_used_at: agent.last_used_at,
            created_at: agent.created_at,
            token,
        }),
    ))
}

// List agents
pub async fn list_agents(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(service_id): Path<Uuid>,
) -> Result<impl IntoResponse, StatusCode> {
    let user_id = get_user_id(&claims);

    let has_access = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM service_members WHERE service_id = $1 AND user_id = $2)",
    )
    .bind(service_id)
    .bind(user_id)
    .fetch_one(state.db.pool())
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if !has_access {
        return Err(StatusCode::FORBIDDEN);
    }

    let agents: Vec<Agent> = sqlx::query_as(
        r#"
        SELECT id, service_id, name, source_type, expires_at, last_used_at, created_at
        FROM agents
        WHERE service_id = $1
        ORDER BY created_at DESC
        "#,
    )
    .bind(service_id)
    .fetch_all(state.db.pool())
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Convert to safe response that excludes tokens
    let safe_agents: Vec<AgentResponse> = agents
        .into_iter()
        .map(|agent| AgentResponse {
            id: agent.id,
            service_id: agent.service_id,
            name: agent.name,
            source_type: agent.source_type,
            expires_at: agent.expires_at,
            last_used_at: agent.last_used_at,
            created_at: agent.created_at,
        })
        .collect();

    Ok(Json(safe_agents))
}

// Revoke agent
pub async fn revoke_agent(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path((service_id, agent_id)): Path<(Uuid, Uuid)>,
) -> Result<impl IntoResponse, StatusCode> {
    let user_id = get_user_id(&claims);

    let has_access: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM service_members WHERE service_id = $1 AND user_id = $2 AND role IN ('owner', 'admin'))",
    )
    .bind(service_id)
    .bind(user_id)
    .fetch_one(state.db.pool())
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if !has_access {
        return Err(StatusCode::FORBIDDEN);
    }

    let _result = sqlx::query(
        "DELETE FROM agents WHERE id = $1 AND service_id = $2",
    )
    .bind(agent_id)
    .bind(service_id)
    .execute(state.db.pool())
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(StatusCode::NO_CONTENT)
}

// `validate_agent_token` used to live here, serving the HTTP bearer ingest path
// at `POST /v1/logs`. Both are gone: the agent has only ever spoken TCP since it
// began refusing `protocol: "http"`, and the route had no other client. Its
// lookup was `WHERE token = $1`, which the encrypted-at-rest column cannot
// support anyway -- there is no plaintext column left to match on.
//
// On the TCP path the AEAD tag is what authenticates a batch. See
// `tcp_server::process_log_batch`.
