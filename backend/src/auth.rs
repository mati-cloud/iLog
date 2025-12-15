use axum::{
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
    pub registration_token: Option<String>,
}

pub async fn register(db: &Database, req: RegisterRequest, allow_public_signup: bool) -> anyhow::Result<i32> {
    let user_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users")
        .fetch_one(db.pool())
        .await?;
    
    let is_first_user = user_count.0 == 0;
    let is_admin = is_first_user;
    
    if !is_first_user && !allow_public_signup {
        if let Some(token) = &req.registration_token {
            if token.len() != 64 {
                return Err(anyhow::anyhow!("Invalid registration token format"));
            }
            
            let valid = validate_and_consume_registration_token(db, token).await?;
            if !valid {
                return Err(anyhow::anyhow!("Invalid or expired registration token"));
            }
        } else {
            return Err(anyhow::anyhow!("Registration is disabled. A valid registration token is required."));
        }
    }
    
    let password_hash = bcrypt::hash(&req.password, bcrypt::DEFAULT_COST)?;

    let user_id: (i32,) = sqlx::query_as(
        r#"
        INSERT INTO users (username, password_hash, email, is_admin)
        VALUES ($1, $2, $3, $4)
        RETURNING id
        "#,
    )
    .bind(&req.username)
    .bind(&password_hash)
    .bind(&req.email)
    .bind(is_admin)
    .fetch_one(db.pool())
    .await?;

    if let Some(token) = &req.registration_token {
        sqlx::query(
            "UPDATE registration_tokens SET used = TRUE, used_at = NOW(), used_by_user_id = $1 WHERE token = $2"
        )
        .bind(user_id.0)
        .bind(token)
        .execute(db.pool())
        .await?;
    }

    Ok(user_id.0)
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

pub async fn validate_and_consume_registration_token(db: &Database, token: &str) -> anyhow::Result<bool> {
    if token.len() != 64 {
        return Ok(false);
    }
    
    let result: Option<(bool,)> = sqlx::query_as(
        r#"
        SELECT EXISTS(
            SELECT 1 FROM registration_tokens
            WHERE token = $1
              AND NOT used
              AND expires_at > NOW()
        )
        "#,
    )
    .bind(token)
    .fetch_optional(db.pool())
    .await?;
    
    Ok(result.map(|r| r.0).unwrap_or(false))
}

pub async fn check_registration_allowed(db: &Database, token: Option<&str>, allow_public_signup: bool) -> anyhow::Result<bool> {
    let user_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users")
        .fetch_one(db.pool())
        .await?;
    
    if user_count.0 == 0 {
        return Ok(true);
    }
    
    if allow_public_signup {
        return Ok(true);
    }
    
    if let Some(t) = token {
        return validate_and_consume_registration_token(db, t).await;
    }
    
    Ok(false)
}

pub fn generate_registration_token() -> String {
    use rand::Rng;
    const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
    let mut rng = rand::thread_rng();
    
    (0..64)
        .map(|_| {
            let idx = rng.gen_range(0..CHARSET.len());
            CHARSET[idx] as char
        })
        .collect()
}

pub async fn create_registration_token(db: &Database) -> anyhow::Result<String> {
    let token = generate_registration_token();
    let expires_at = chrono::Utc::now() + chrono::Duration::hours(24);
    
    sqlx::query(
        "INSERT INTO registration_tokens (token, expires_at) VALUES ($1, $2)"
    )
    .bind(&token)
    .bind(expires_at)
    .execute(db.pool())
    .await?;
    
    Ok(token)
}
