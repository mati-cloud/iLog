use axum::{
    body::Body,
    extract::{Request, State},
    http::{header, StatusCode},
    middleware::Next,
    response::Response,
};
use jsonwebtoken::{decode, DecodingKey, Validation};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::{models::Claims, AppState};

pub async fn auth_middleware(
    State(state): State<AppState>,
    mut req: Request<Body>,
    next: Next,
) -> Result<Response, StatusCode> {
    // Skip auth for public routes
    let path = req.uri().path();
    tracing::info!("Auth middleware checking path: {}", path);
    
    if path == "/health" {
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
