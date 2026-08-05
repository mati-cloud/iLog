//! JWKS client for verifying better-auth EdDSA (Ed25519) JWTs.
//!
//! better-auth's `jwt()` plugin signs session tokens with Ed25519 and publishes
//! the public keys at `{BETTER_AUTH_URL}/api/auth/jwks`. We fetch that key set,
//! cache it, and verify signatures against it. Keys are refetched when we see an
//! unknown `kid` (better-auth rotates keys) and are otherwise cached.

use std::sync::Arc;
use std::time::{Duration, Instant};

use jsonwebtoken::jwk::JwkSet;
use jsonwebtoken::{decode, decode_header, Algorithm, DecodingKey, Validation};
use tokio::sync::RwLock;

use crate::models::Claims;

/// Minimum gap between JWKS refetches, so an attacker spraying bogus `kid`
/// values can't turn our verifier into a request amplifier against the frontend.
const MIN_REFETCH_INTERVAL: Duration = Duration::from_secs(30);

/// Refetch keys older than this even on a cache hit, to pick up rotations.
const MAX_KEY_AGE: Duration = Duration::from_secs(600);

#[derive(Debug, thiserror::Error)]
pub enum JwksError {
    #[error("token header has no kid")]
    MissingKid,
    #[error("no key matching kid {0}")]
    UnknownKid(String),
    #[error("jwks fetch failed: {0}")]
    Fetch(String),
    #[error("jwks payload malformed: {0}")]
    Malformed(String),
    #[error("signature or claim validation failed: {0}")]
    Invalid(#[from] jsonwebtoken::errors::Error),
}

struct Cache {
    keys: JwkSet,
    fetched_at: Instant,
}

pub struct JwksClient {
    url: String,
    http: reqwest::Client,
    cache: RwLock<Option<Cache>>,
}

impl JwksClient {
    /// `base_url` is the better-auth origin, e.g. `http://localhost:3000`.
    pub fn new(base_url: &str) -> Arc<Self> {
        let url = format!("{}/api/auth/jwks", base_url.trim_end_matches('/'));
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(5))
            .build()
            .expect("failed to build JWKS http client");

        Arc::new(Self {
            url,
            http,
            cache: RwLock::new(None),
        })
    }

    async fn fetch(&self) -> Result<JwkSet, JwksError> {
        let resp = self
            .http
            .get(&self.url)
            .send()
            .await
            .map_err(|e| JwksError::Fetch(e.to_string()))?;

        if !resp.status().is_success() {
            return Err(JwksError::Fetch(format!("HTTP {}", resp.status())));
        }

        resp.json::<JwkSet>()
            .await
            .map_err(|e| JwksError::Malformed(e.to_string()))
    }

    /// Return a decoding key for `kid`, refetching the key set if it isn't
    /// cached or the cache is stale. Refetches are rate limited.
    async fn key_for(&self, kid: &str) -> Result<DecodingKey, JwksError> {
        {
            let guard = self.cache.read().await;
            if let Some(cache) = guard.as_ref() {
                if cache.fetched_at.elapsed() < MAX_KEY_AGE {
                    if let Some(jwk) = cache.keys.find(kid) {
                        return DecodingKey::from_jwk(jwk).map_err(JwksError::from);
                    }
                }
            }
        }

        let mut guard = self.cache.write().await;

        // Another task may have refreshed while we waited for the write lock.
        if let Some(cache) = guard.as_ref() {
            if let Some(jwk) = cache.keys.find(kid) {
                if cache.fetched_at.elapsed() < MAX_KEY_AGE {
                    return DecodingKey::from_jwk(jwk).map_err(JwksError::from);
                }
            }
            if cache.fetched_at.elapsed() < MIN_REFETCH_INTERVAL {
                return Err(JwksError::UnknownKid(kid.to_string()));
            }
        }

        let keys = self.fetch().await?;
        let found = keys.find(kid).map(DecodingKey::from_jwk).transpose()?;

        *guard = Some(Cache {
            keys,
            fetched_at: Instant::now(),
        });

        found.ok_or_else(|| JwksError::UnknownKid(kid.to_string()))
    }

    /// Verify a better-auth JWT and return its claims.
    ///
    /// Checks the Ed25519 signature and `exp`. `aud`/`iss` are not checked: the
    /// key set itself is origin-scoped, so a valid signature already proves the
    /// token came from our better-auth instance.
    pub async fn verify(&self, token: &str) -> Result<Claims, JwksError> {
        let header = decode_header(token)?;
        let kid = header.kid.ok_or(JwksError::MissingKid)?;
        let key = self.key_for(&kid).await?;

        let mut validation = Validation::new(Algorithm::EdDSA);
        validation.validate_exp = true;
        validation.validate_aud = false;

        let data = decode::<Claims>(token, &key, &validation)?;
        Ok(data.claims)
    }
}
