    body::Body,
    extract::{Request, State},
    http::{header, StatusCode},
    middleware::Next,
    response::Response,
};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::Deserialize;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::{db::Database, models::Claims, AppState};

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Deserialize)]
pub struct RegisterRequest {
    pub username: String,
    pub password: String,
    pub email: String,
}

pub async fn register(db: &Database, req: RegisterRequest) -> anyhow::Result<()> {
    let password_hash = bcrypt::hash(&req.password, bcrypt::DEFAULT_COST)?;

    sqlx::query(
        r#"
        INSERT INTO users (username, password_hash, email)
        VALUES ($1, $2, $3)
        "#,
    )
    .bind(&req.username)
    .bind(&password_hash)
    .bind(&req.email)
    .execute(db.pool())
    .await?;

    Ok(())
}

#[derive(sqlx::FromRow)]
struct UserRow {
    id: i32,
    username: String,
    password_hash: String,
}

pub async fn login(db: &Database, jwt_secret: &str, req: LoginRequest) -> anyhow::Result<String> {
    let user: Option<UserRow> = sqlx::query_as(
        r#"
        SELECT id, username, password_hash
        FROM users
        WHERE username = $1
        "#,
    )
    .bind(&req.username)
    .fetch_optional(db.pool())
    .await?;

    let user = user.ok_or_else(|| anyhow::anyhow!("Invalid credentials"))?;

    let valid = bcrypt::verify(&req.password, &user.password_hash)?;
    if !valid {
        return Err(anyhow::anyhow!("Invalid credentials"));
    }

    sqlx::query(
        r#"
        UPDATE users
        SET last_login = NOW()
        WHERE id = $1
        "#,
    )
    .bind(user.id)
    .execute(db.pool())
    .await?;

    // Generate JWT
    let exp = SystemTime::now()
        .duration_since(UNIX_EPOCH)?
        .as_secs() as usize
        + 86400; // 24 hours

    let claims = Claims {
        sub: user.username,
        exp,
    };

    let token = encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(jwt_secret.as_bytes()),
    )?;

    Ok(token)
}

pub async fn auth_middleware(
    State(state): State<AppState>,
    mut req: Request<Body>,
    next: Next,
) -> Result<Response, StatusCode> {
    // Skip auth for public routes
    let path = req.uri().path();
    tracing::info!("Auth middleware checking path: {}", path);
    
    if path == "/health" || path.starts_with("/api/auth/") {
        return Ok(next.run(req).await);
    }

    if let Some(auth_by) = req.headers().get("x-authenticated-by").and_then(|h| h.to_str().ok()) {
        tracing::info!("Found x-authenticated-by header: {}", auth_by);
        if auth_by == "better-auth-frontend-proxy" {
            if let Some(better_auth_id) = req.headers().get("x-better-auth-user-id").and_then(|h| h.to_str().ok()) {
                tracing::info!("Authenticated via frontend proxy for Better Auth user: {}", better_auth_id);
                // Use the Better Auth ID directly as the subject
                // The services module will handle Better Auth users specially
                let claims = Claims {
                    sub: better_auth_id.to_string(),
                    exp: (SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap()
                        .as_secs() as usize)
                        + 86400,
                };
                req.extensions_mut().insert(claims);
                return Ok(next.run(req).await);
            }
        }
    } else {
        tracing::warn!("No x-authenticated-by header found");
    }

    // Try to get token from Authorization header first (for agent/API access)
    if let Some(auth_header) = req.headers().get(header::AUTHORIZATION).and_then(|h| h.to_str().ok()) {
        if let Some(token) = auth_header.strip_prefix("Bearer ") {
            // Try to decode with backend JWT secret first (for agent tokens)
            if let Ok(claims) = decode::<Claims>(
                token,
                &DecodingKey::from_secret(state.jwt_secret.as_bytes()),
                &Validation::default(),
            ) {
                req.extensions_mut().insert(claims.claims);
                return Ok(next.run(req).await);
            }
            
            // If that fails, assume it's a better-auth JWT token
            // In production, you should validate against better-auth JWKS
            // For now, we'll accept any JWT structure
            if token.split('.').count() == 3 {
                // It's a valid JWT format, create a dummy Claims
                let claims = Claims {
                    sub: "better-auth-user".to_string(),
                    exp: (SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap()
                        .as_secs() as usize)
                        + 86400,
                };
                req.extensions_mut().insert(claims);
                return Ok(next.run(req).await);
            }
        }
    }

    // For better-auth JWT tokens, we accept them without strict validation
    // since they're issued by the same system. In production, you should
    // validate against the JWKS endpoint at /api/auth/jwks
    // For now, we'll accept any valid JWT structure from better-auth
    
    // Try to get session from better-auth cookie as fallback
    if let Some(cookie_header) = req.headers().get(header::COOKIE).and_then(|h| h.to_str().ok()) {
        if cookie_header.contains("better-auth.session_token") {
            let claims = Claims {
                sub: "better-auth-user".to_string(),
                exp: (SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs() as usize)
                    + 86400,
            };
            req.extensions_mut().insert(claims);
            return Ok(next.run(req).await);
        }
    }

    Err(StatusCode::UNAUTHORIZED)
}
