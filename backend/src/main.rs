mod analytics;
mod auth;
mod db;
mod jwks;
mod models;
mod otel;
mod services;
mod streaming;
mod tcp_server;
mod token_crypto;

use axum::{
    extract::{Query, State, WebSocketUpgrade},
    http::{header, Method, StatusCode},
    middleware,
    response::{IntoResponse, Response},
    routing::{get, post},
    Extension, Json, Router,
};
use std::{net::SocketAddr, sync::Arc};
use tokio::sync::broadcast;
use tower_http::cors::CorsLayer;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

use crate::{
    auth::auth_middleware,
    db::Database,
    jwks::JwksClient,
    models::{Claims, LogQuery, OtelLog},
    streaming::handle_websocket,
    token_crypto::TokenCrypto,
};

#[derive(Clone)]
pub struct AppState {
    db: Arc<Database>,
    jwt_secret: String,
    log_broadcast: broadcast::Sender<OtelLog>,
    jwks: Arc<JwksClient>,
    /// Wraps and unwraps agent key secrets. Cheap to clone (holds a cipher).
    token_crypto: TokenCrypto,
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

    // Required, with no fallback. The previous default
    // ("your-secret-key-change-in-production") meant any deployment that missed
    // this var accepted forged backend JWTs from anyone who had read the source.
    let jwt_secret = std::env::var("JWT_SECRET").map_err(|_| {
        anyhow::anyhow!(
            "JWT_SECRET is not set. It signs backend session tokens; there is no \
             default because a shared default would let anyone forge them. \
             Generate one with `openssl rand -base64 48`."
        )
    })?;
    if jwt_secret.len() < 32 {
        anyhow::bail!(
            "JWT_SECRET is {} chars; at least 32 are required. Generate one with \
             `openssl rand -base64 48`.",
            jwt_secret.len()
        );
    }

    // Wraps agent key secrets at rest. Built before the listeners start so a
    // missing or too-short key fails the boot rather than the first agent batch.
    let token_crypto = token_crypto::TokenCrypto::from_env()?;
    info!("Agent key secret wrapping initialized");

    // better-auth origin, used to fetch the Ed25519 public keys that sign
    // frontend session JWTs.
    let better_auth_url = std::env::var("BETTER_AUTH_URL")
        .unwrap_or_else(|_| "http://localhost:3000".to_string());
    let jwks = JwksClient::new(&better_auth_url);
    info!("JWKS verification against {}", better_auth_url);

    let db_arc = Arc::new(db);

    // Create broadcast channel for real-time log streaming
    // Buffer size of 1000 logs - if a client is slow, older logs will be dropped
    let (log_tx, _log_rx) = broadcast::channel::<OtelLog>(1000);
    info!("Broadcast channel created for real-time log streaming");

    let state = AppState {
        db: Arc::clone(&db_arc),
        jwt_secret,
        log_broadcast: log_tx.clone(),
        jwks,
        token_crypto: token_crypto.clone(),
    };

    let app = Router::new()
        .route("/health", get(health_check))
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
        .route("/api/dashboard/metrics", get(analytics::get_dashboard_metrics))
        .route("/api/dashboard/log-volume", get(analytics::get_log_volume_24h))
        .route("/api/dashboard/storage-by-service", get(analytics::get_storage_by_service))
        .route("/api/dashboard/agents", get(analytics::get_connected_agents))
        .route("/api/dashboard/7day-ingestion", get(analytics::get_7day_ingestion))
        // Every route above sits behind auth. `POST /v1/logs` was the one
        // exception -- an unauthenticated-middleware bearer ingest path -- and it
        // has been removed along with its handler. Agents ingest over TCP on
        // TCP_PORT, where the AEAD tag authenticates each batch.
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            auth_middleware,
        ))
        .layer(
            CorsLayer::new()
                .allow_origin([
                    "http://localhost:3000".parse::<axum::http::HeaderValue>().unwrap(),
                    "https://ilog.mati.cloud".parse::<axum::http::HeaderValue>().unwrap(),
                ])
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
    let http_port = std::env::var("SERVER_PORT")
        .unwrap_or_else(|_| "8080".to_string())
        .parse::<u16>()?;
    let tcp_port = std::env::var("TCP_PORT")
        .unwrap_or_else(|_| "8081".to_string())
        .parse::<u16>()?;

    let http_addr = SocketAddr::from(([0, 0, 0, 0], http_port));
    let tcp_addr = SocketAddr::from(([0, 0, 0, 0], tcp_port));

    // Spawn TCP server for agent connections
    let tcp_db = Arc::clone(&db_arc);
    let tcp_token_crypto = token_crypto;
    let tcp_log_tx = log_tx.clone();
    tokio::spawn(async move {
        if let Err(e) =
            tcp_server::start_tcp_server(tcp_addr, tcp_db, tcp_token_crypto, tcp_log_tx).await
        {
            tracing::error!("TCP server error: {}", e);
        }
    });

    info!("HTTP server listening on {}", http_addr);
    info!("TCP server listening on {} (for agents)", tcp_addr);
    
    let listener = tokio::net::TcpListener::bind(http_addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

async fn health_check() -> impl IntoResponse {
    Json(serde_json::json!({
        "status": "healthy",
        "service": "ilog-backend"
    }))
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
    Extension(_claims): Extension<Claims>,
) -> Response {
    // Authentication is handled by the auth middleware; the Claims extension
    // proves the caller is authenticated. The client sends its credential as a
    // `bearer.<token>` subprotocol entry alongside `ilog.v1`; we must select a
    // protocol the client offered or the browser aborts the handshake, so we
    // echo `ilog.v1` and never the entry containing the token.
    let log_rx = state.log_broadcast.subscribe();
    ws.protocols(["ilog.v1"])
        .on_upgrade(move |socket| handle_websocket(socket, state.db, params, log_rx))
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
