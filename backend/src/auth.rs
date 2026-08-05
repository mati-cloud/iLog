use axum::{
    body::Body,
    extract::{Request, State},
    http::{header, StatusCode},
    middleware::Next,
    response::Response,
};
use jsonwebtoken::{decode, DecodingKey, Validation};

use crate::{models::Claims, AppState};

/// Authenticate a request and attach verified [`Claims`] to its extensions.
///
/// Two credential types are accepted, both cryptographically verified:
///
/// 1. A backend-issued HS256 JWT, signed with `state.jwt_secret`.
/// 2. A better-auth EdDSA JWT, verified against the better-auth JWKS endpoint.
///
/// Both must arrive as `Authorization: Bearer <token>`, except on the WebSocket
/// upgrade route, which cannot set headers from the browser and instead passes
/// the token via `Sec-WebSocket-Protocol` (see [`websocket_token`]).
///
/// Anything else is rejected with 401. In particular there is deliberately no
/// path that trusts an unverified token, a proxy-supplied identity header, or
/// the mere presence of a session cookie.
pub async fn auth_middleware(
    State(state): State<AppState>,
    mut req: Request<Body>,
    next: Next,
) -> Result<Response, StatusCode> {
    if req.uri().path() == "/health" {
        return Ok(next.run(req).await);
    }

    let token = bearer_token(&req)
        .or_else(|| websocket_token(&req))
        .ok_or(StatusCode::UNAUTHORIZED)?;

    let claims = verify_token(&state, &token)
        .await
        .ok_or(StatusCode::UNAUTHORIZED)?;

    req.extensions_mut().insert(claims);
    Ok(next.run(req).await)
}

/// Verify `token` as either a backend-signed JWT or a better-auth JWT.
async fn verify_token(state: &AppState, token: &str) -> Option<Claims> {
    if let Ok(data) = decode::<Claims>(
        token,
        &DecodingKey::from_secret(state.jwt_secret.as_bytes()),
        &Validation::default(),
    ) {
        return Some(data.claims);
    }

    match state.jwks.verify(token).await {
        Ok(claims) => Some(claims),
        Err(e) => {
            // Logged without the token so credentials stay out of the log stream.
            tracing::warn!("rejected token: {}", e);
            None
        }
    }
}

fn bearer_token(req: &Request<Body>) -> Option<String> {
    req.headers()
        .get(header::AUTHORIZATION)?
        .to_str()
        .ok()?
        .strip_prefix("Bearer ")
        .map(str::to_owned)
}

/// Extract a token from the WebSocket subprotocol header.
///
/// Browsers cannot set `Authorization` on a WebSocket handshake, and putting the
/// token in the query string leaks it into access logs and referrers. The
/// standard workaround is to smuggle it through `Sec-WebSocket-Protocol` as a
/// second protocol entry: `ilog.v1, bearer.<token>`.
fn websocket_token(req: &Request<Body>) -> Option<String> {
    req.headers()
        .get("sec-websocket-protocol")?
        .to_str()
        .ok()?
        .split(',')
        .map(str::trim)
        .find_map(|p| p.strip_prefix("bearer.").map(str::to_owned))
}
