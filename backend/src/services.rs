use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Extension,
    Json,
};
use chrono::{Duration, Utc};
use rand::Rng;
use sqlx::PgPool;
use uuid::Uuid;
use sha2::{Sha256, Digest};

use crate::{
    models::{Claims, CreateService, CreateAgent, Service, Agent, AgentClaims, AgentResponse, UpdateService},
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
fn generate_token(service_id: Uuid) -> String {
    let random: String = rand::thread_rng()
        .sample_iter(&rand::distributions::Alphanumeric)
        .take(32)
        .map(char::from)
        .collect();
    format!("proj_{}_{}", service_id.simple(), random)
}

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

    let token = generate_token(service_id);
    let expires_at = payload
        .expires_in_days
        .map(|days| Utc::now() + Duration::days(days));

    let agent: Agent = sqlx::query_as(
        r#"
        INSERT INTO agents (service_id, name, token, source_type, expires_at)
        VALUES ($1, $2, $3, $4, $5)
        RETURNING id, service_id, name, token, source_type, expires_at, last_used_at, created_at
        "#,
    )
    .bind(service_id)
    .bind(&payload.name)
    .bind(&token)
    .bind(&payload.source_type)
    .bind(expires_at)
    .fetch_one(state.db.pool())
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok((StatusCode::CREATED, Json(agent)))
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
        SELECT id, service_id, name, token, source_type, expires_at, last_used_at, created_at
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

pub async fn validate_agent_token(
    pool: &PgPool,
    token: &str,
) -> Result<AgentClaims, StatusCode> {
    let result: Agent = sqlx::query_as(
        r#"
        SELECT id, service_id, name, token, source_type, expires_at, last_used_at, created_at
        FROM agents
        WHERE token = $1
          AND (expires_at IS NULL OR expires_at > NOW())
        "#,
    )
    .bind(token)
    .fetch_optional(pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    .ok_or(StatusCode::UNAUTHORIZED)?;

    let _ = sqlx::query("UPDATE agents SET last_used_at = NOW() WHERE id = $1")
        .bind(result.id)
        .execute(pool)
        .await;

    Ok(AgentClaims {
        service_id: result.service_id,
        agent_id: result.id,
    })
}
