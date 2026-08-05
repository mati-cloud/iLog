//! Wrapping of agent `key_secret` values at rest.
//!
//! An agent token is both a credential and key material: `Decryptor` runs it
//! through HKDF to derive the per-agent AEAD transport key for every batch. That
//! rules out storing a hash -- the backend has to recover the input. So the
//! secret is encrypted instead, under a key derived from `TOKEN_ENCRYPTION_KEY`.
//!
//! The database therefore holds nothing usable on its own: `key_secret_encrypted`
//! is inert without the env var, and the env var lives outside the database.

use anyhow::{anyhow, Context, Result};
use chacha20poly1305::{
    aead::{Aead, KeyInit, OsRng},
    ChaCha20Poly1305, Nonce,
};
use hkdf::Hkdf;
use rand::RngCore;
use sha2::Sha256;

const NONCE_SIZE: usize = 12;

/// Domain separation for the wrapping key. Distinct from the transport-key
/// parameters in `tcp_server.rs` so the same input cannot yield both keys.
const WRAP_SALT: &[u8] = b"ilog-token-wrap-v1";
const WRAP_INFO: &[u8] = b"ilog agent key_secret wrapping v1";

/// Minimum accepted length for `TOKEN_ENCRYPTION_KEY`. HKDF accepts any input
/// length, so this is a floor on operator-supplied entropy, not a crypto
/// requirement.
const MIN_KEY_LEN: usize = 32;

/// Wraps and unwraps agent `key_secret` values.
///
/// Built once at startup from the environment. The wrapping key is derived
/// deterministically, so a restart recovers every existing agent's secret --
/// generating it per boot would silently break ingest on the first restart.
#[derive(Clone)]
pub struct TokenCrypto {
    cipher: ChaCha20Poly1305,
}

impl TokenCrypto {
    /// Build from `TOKEN_ENCRYPTION_KEY`.
    ///
    /// Deliberately fails rather than falling back to a default. A baked-in
    /// default would mean every deployment that forgot the var shares one
    /// wrapping key, which is the same class of bug as the plaintext column
    /// this replaces.
    pub fn from_env() -> Result<Self> {
        let secret = std::env::var("TOKEN_ENCRYPTION_KEY").map_err(|_| {
            anyhow!(
                "TOKEN_ENCRYPTION_KEY is not set. It wraps agent key secrets at rest; \
                 there is no default because a shared default key would make the \
                 encryption pointless. Generate one with `openssl rand -base64 48`."
            )
        })?;

        if secret.len() < MIN_KEY_LEN {
            return Err(anyhow!(
                "TOKEN_ENCRYPTION_KEY is {} chars; at least {} are required. \
                 Generate one with `openssl rand -base64 48`.",
                secret.len(),
                MIN_KEY_LEN
            ));
        }

        Ok(Self::from_passphrase(secret.as_bytes()))
    }

    /// Derive the wrapping key from arbitrary input material.
    ///
    /// HKDF means the env var can be a passphrase rather than raw key bytes, so
    /// operators need no hex or base64 decoding step.
    fn from_passphrase(ikm: &[u8]) -> Self {
        let hk = Hkdf::<Sha256>::new(Some(WRAP_SALT), ikm);
        let mut key = [0u8; 32];
        hk.expand(WRAP_INFO, &mut key)
            // 32 bytes is well under HKDF-SHA256's output limit, so expand
            // cannot fail for this length.
            .expect("HKDF expand of 32 bytes cannot fail");
        Self {
            cipher: ChaCha20Poly1305::new((&key).into()),
        }
    }

    /// Encrypt a `key_secret` for storage. Output is `nonce || ciphertext || tag`,
    /// matching the transport layout in `tcp_server.rs`.
    pub fn wrap(&self, key_secret: &str) -> Result<Vec<u8>> {
        let mut nonce_bytes = [0u8; NONCE_SIZE];
        OsRng.fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);

        let ciphertext = self
            .cipher
            .encrypt(nonce, key_secret.as_bytes())
            .map_err(|_| anyhow!("Failed to wrap agent key secret"))?;

        let mut out = Vec::with_capacity(NONCE_SIZE + ciphertext.len());
        out.extend_from_slice(&nonce_bytes);
        out.extend_from_slice(&ciphertext);
        Ok(out)
    }

    /// Recover a stored `key_secret`.
    ///
    /// Fails if the row was written under a different `TOKEN_ENCRYPTION_KEY` or
    /// has been tampered with -- the AEAD tag covers both cases.
    pub fn unwrap(&self, stored: &[u8]) -> Result<String> {
        if stored.len() < NONCE_SIZE {
            return Err(anyhow!("Stored key secret is too short to contain a nonce"));
        }

        let (nonce_bytes, ciphertext) = stored.split_at(NONCE_SIZE);
        let plaintext = self
            .cipher
            .decrypt(Nonce::from_slice(nonce_bytes), ciphertext)
            .map_err(|_| {
                anyhow!(
                    "Failed to unwrap agent key secret. The row was written under a \
                     different TOKEN_ENCRYPTION_KEY, or has been altered."
                )
            })?;

        String::from_utf8(plaintext).context("Unwrapped key secret is not valid UTF-8")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_crypto() -> TokenCrypto {
        TokenCrypto::from_passphrase(b"test-wrapping-passphrase-at-least-32-chars")
    }

    #[test]
    fn wrap_then_unwrap_round_trips() {
        let crypto = test_crypto();
        let secret = "aB3dE6gH9jK2mN5pQ8sT1vW4xY7zC0eF";
        let wrapped = crypto.wrap(secret).unwrap();

        assert_ne!(wrapped.as_slice(), secret.as_bytes());
        assert_eq!(crypto.unwrap(&wrapped).unwrap(), secret);
    }

    #[test]
    fn same_secret_wraps_to_different_ciphertext() {
        // Random nonce per call: equal secrets must not produce equal rows, or
        // the table would leak which agents share a secret.
        let crypto = test_crypto();
        let a = crypto.wrap("same-secret-value").unwrap();
        let b = crypto.wrap("same-secret-value").unwrap();
        assert_ne!(a, b);
    }

    #[test]
    fn tampered_ciphertext_is_rejected() {
        let crypto = test_crypto();
        let mut wrapped = crypto.wrap("aB3dE6gH9jK2mN5pQ8sT1vW4xY7zC0eF").unwrap();
        let last = wrapped.len() - 1;
        wrapped[last] ^= 0x01;
        assert!(crypto.unwrap(&wrapped).is_err());
    }

    #[test]
    fn wrong_wrapping_key_is_rejected() {
        let wrapped = test_crypto().wrap("aB3dE6gH9jK2mN5pQ8sT1vW4xY7zC0eF").unwrap();
        let other = TokenCrypto::from_passphrase(b"a-different-passphrase-also-32-chars-long");
        assert!(other.unwrap(&wrapped).is_err());
    }

    #[test]
    fn wrapping_key_is_deterministic() {
        // The property step 8 of the verification plan checks: a restart must
        // derive the same key, or every existing agent stops ingesting.
        let wrapped = test_crypto().wrap("aB3dE6gH9jK2mN5pQ8sT1vW4xY7zC0eF").unwrap();
        assert_eq!(
            test_crypto().unwrap(&wrapped).unwrap(),
            "aB3dE6gH9jK2mN5pQ8sT1vW4xY7zC0eF"
        );
    }

    #[test]
    fn truncated_input_is_rejected() {
        let crypto = test_crypto();
        assert!(crypto.unwrap(&[0u8; 4]).is_err());
    }
}
