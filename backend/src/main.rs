mod auth;
mod db;
mod models;
mod otel;
mod services;
mod streaming;

use axum::{
    extract::{Query, State, WebSocketUpgrade},
    http::{header, HeaderMap, Method, StatusCode},
    middleware,
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use std::{net::SocketAddr, sync::Arc};
use tower_http::cors::CorsLayer;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

use crate::{
    auth::{auth_middleware, LoginRequest, RegisterRequest},
    db::Database,
    models::{LogQuery, OtelLog},
    streaming::handle_websocket,
};

#[derive(Clone)]
pub struct AppState {
    db: Arc<Database>,
    jwt_secret: String,
    allow_public_signup: bool,
    frontend_url: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv::dotenv().ok();

    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    info!("Starting iLog backend...");

    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://ilog_user:changeme123@localhost:5432/ilog".to_string());
    let db = Database::new(&database_url).await?;
    info!("Database connection established");

    let jwt_secret = std::env::var("JWT_SECRET")
        .unwrap_or_else(|_| "your-secret-key-change-in-production".to_string());
    
    let allow_public_signup = std::env::var("ALLOW_PUBLIC_SIGNUP")
        .unwrap_or_else(|_| "false".to_string())
        .parse::<bool>()
        .unwrap_or(false);
    
    let frontend_url = std::env::var("FRONTEND_URL")
        .unwrap_or_else(|_| "http://localhost:3000".to_string());

    let db_arc = Arc::new(db);
    
    // Check if this is first run (no users exist) and generate registration token
    let user_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users")
        .fetch_one(db_arc.pool())
        .await?;
    
    if user_count.0 == 0 {
        info!("No users found. This appears to be a fresh installation.");
        let token = auth::create_registration_token(&db_arc).await?;
        info!("=".repeat(80));
        info!("🔐 INITIAL ADMIN REGISTRATION TOKEN GENERATED");
        info!("=".repeat(80));
        info!("This is a ONE-TIME registration token that expires in 24 hours.");
        info!("Use this URL to create the initial admin account:");
        info!("");
        info!("    {}/sign-up?token={}", frontend_url, token);
        info!("");
        info!("The token will be automatically deleted after use.");
        info!("=".repeat(80));
    } else if !allow_public_signup {
        info!("Public signup is disabled. Users can only register with a valid registration token.");
    }

    let state = AppState {
        db: db_arc,
        jwt_secret,
        allow_public_signup,
        frontend_url,
    };

    let app = Router::new()
        .route("/health", get(health_check))
        .route("/api/auth/login", post(login))
        .route("/api/auth/register", post(register))
        .route("/api/auth/registration-status", get(check_registration_status))
        .route("/api/services", post(services::create_service))
        .route("/api/services", get(services::list_services))
        .route("/api/services/:id", get(services::get_service))
        .route("/api/services/:id", axum::routing::patch(services::update_service))
        .route("/api/services/:id", axum::routing::delete(services::delete_service))
        .route("/api/services/:id/agents", post(services::create_agent))
        .route("/api/services/:id/agents", get(services::list_agents))
        .route("/api/services/:id/agents/:agent_id", axum::routing::delete(services::revoke_agent))
        .route("/api/logs/query", get(query_logs))
        .route("/api/logs/stream", get(stream_logs))
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            auth_middleware,
        ))
        .route("/v1/logs", post(ingest_logs))
        .layer(
            CorsLayer::new()
                .allow_origin("http://localhost:3000".parse::<axum::http::HeaderValue>().unwrap())
                .allow_methods([
                    Method::GET,
                    Method::POST,
                    Method::PUT,
                    Method::PATCH,
                    Method::DELETE,
                    Method::OPTIONS,
                ])
                .allow_headers([
                    header::CONTENT_TYPE,
                    header::AUTHORIZATION,
                    header::COOKIE,
                ])
                .allow_credentials(true)
        )
        .with_state(state);

    let _host = std::env::var("SERVER_HOST").unwrap_or_else(|_| "0.0.0.0".to_string());
    let port = std::env::var("SERVER_PORT")
        .unwrap_or_else(|_| "8080".to_string())
        .parse::<u16>()?;
    let addr = SocketAddr::from(([0, 0, 0, 0], port));

    info!("Server listening on {}", addr);
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

async fn health_check() -> impl IntoResponse {
    Json(serde_json::json!({
        "status": "healthy",
        "service": "ilog-backend"
    }))
}

async fn login(
    State(state): State<AppState>,
    Json(payload): Json<LoginRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let token = auth::login(&state.db, &state.jwt_secret, payload).await?;
    Ok(Json(serde_json::json!({ "token": token })))
}

async fn register(
    State(state): State<AppState>,
    Json(payload): Json<RegisterRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let user_id = auth::register(&state.db, payload, state.allow_public_signup).await?;
    Ok(Json(serde_json::json!({
        "message": "User registered successfully",
        "user_id": user_id
    })))
}

async fn check_registration_status(
    State(state): State<AppState>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> Result<Json<serde_json::Value>, AppError> {
    let token = params.get("token").map(|s| s.as_str());
    let allowed = auth::check_registration_allowed(&state.db, token, state.allow_public_signup).await?;
    
    Ok(Json(serde_json::json!({
        "allowed": allowed,
        "public_signup_enabled": state.allow_public_signup
    })))
}

async fn ingest_logs(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(logs): Json<Vec<OtelLog>>,
) -> Result<StatusCode, AppError> {
    let token = headers
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .ok_or_else(|| anyhow::anyhow!("Missing or invalid Authorization header"))?;

    let claims = services::validate_agent_token(state.db.pool(), token)
        .await
        .map_err(|_| anyhow::anyhow!("Invalid or expired token"))?;

    otel::ingest_logs(&state.db, logs, claims.service_id).await?;
    Ok(StatusCode::ACCEPTED)
}

async fn query_logs(
    State(state): State<AppState>,
    Query(query): Query<LogQuery>,
) -> Result<Json<Vec<OtelLog>>, AppError> {
    let logs = otel::query_logs(&state.db, query).await?;
    Ok(Json(logs))
}

async fn stream_logs(
    ws: WebSocketUpgrade,
    Query(params): Query<LogQuery>,
    State(state): State<AppState>,
) -> Response {
    ws.on_upgrade(move |socket| handle_websocket(socket, state.db, params))
}

pub struct AppError(anyhow::Error);

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({
                "error": self.0.to_string()
            })),
        )
            .into_response()
    }
}

impl<E> From<E> for AppError
where
    E: Into<anyhow::Error>,
{
    fn from(err: E) -> Self {
        Self(err.into())
    }
}
