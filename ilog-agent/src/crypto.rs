use anyhow::Result;
use chacha20poly1305::{
    aead::{Aead, KeyInit, OsRng},
    ChaCha20Poly1305, Nonce,
};
use hkdf::Hkdf;
use rand::RngCore;
use sha2::Sha256;

const NONCE_SIZE: usize = 12;

/// Fixed salt for agent-key derivation. A salt need not be secret; its role is
/// to separate this key-derivation domain from any other use of the same token.
const KEY_SALT: &[u8] = b"ilog-agent-transport-v2";

/// HKDF info string, binding derived keys to this protocol version.
const KEY_INFO: &[u8] = b"ilog agent transport key v2";

pub struct Encryptor {
    cipher: ChaCha20Poly1305,
}

impl Encryptor {
    pub fn new(key: &[u8; 32]) -> Self {
        let cipher = ChaCha20Poly1305::new(key.into());
        Self { cipher }
    }

    pub fn from_token(token: &str) -> Result<Self> {
        let key = Self::derive_key_from_token(token)?;
        Ok(Self::new(&key))
    }

    /// Derive the AEAD key from an agent token with HKDF-SHA256.
    ///
    /// The previous implementation ran the token through `DefaultHasher`, which
    /// is a non-cryptographic 64-bit hash: the 32-byte key it produced carried
    /// at most 64 bits of entropy and was trivially reversible. HKDF-SHA256
    /// preserves the token's full entropy and is domain-separated by `INFO`, so
    /// the same token cannot yield a colliding key for another purpose.
    ///
    /// The backend derives keys identically; both sides must stay in step.
    fn derive_key_from_token(token: &str) -> Result<[u8; 32]> {
        let hk = Hkdf::<Sha256>::new(Some(KEY_SALT), token.as_bytes());
        let mut key = [0u8; 32];
        hk.expand(KEY_INFO, &mut key)
            .map_err(|_| anyhow::anyhow!("HKDF expand failed"))?;
        Ok(key)
    }

    pub fn encrypt(&self, plaintext: &[u8]) -> Result<Vec<u8>> {
        let mut nonce_bytes = [0u8; NONCE_SIZE];
        OsRng.fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);

        let ciphertext = self
            .cipher
            .encrypt(nonce, plaintext)
            .map_err(|_| anyhow::anyhow!("Encryption failed"))?;

        let mut result = Vec::with_capacity(NONCE_SIZE + ciphertext.len());
        result.extend_from_slice(&nonce_bytes);
        result.extend_from_slice(&ciphertext);

        Ok(result)
    }

    pub fn decrypt(&self, encrypted: &[u8]) -> Result<Vec<u8>> {
        if encrypted.len() < NONCE_SIZE {
            anyhow::bail!("Encrypted data too short");
        }

        let (nonce_bytes, ciphertext) = encrypted.split_at(NONCE_SIZE);
        let nonce = Nonce::from_slice(nonce_bytes);

        let plaintext = self
            .cipher
            .decrypt(nonce, ciphertext)
            .map_err(|_| anyhow::anyhow!("Decryption failed"))?;

        Ok(plaintext)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encrypt_decrypt() {
        let key = [42u8; 32];
        let encryptor = Encryptor::new(&key);

        let plaintext = b"Hello, World!";
        let encrypted = encryptor.encrypt(plaintext).unwrap();
        let decrypted = encryptor.decrypt(&encrypted).unwrap();

        assert_eq!(plaintext, decrypted.as_slice());
    }

    /// Pins the exact bytes HKDF produces for a known token.
    ///
    /// The backend derives agent keys independently in `tcp_server.rs` and has
    /// the identical vector. If either side's salt, info string, or hash changes,
    /// these two tests disagree and CI fails — without them the only symptom
    /// would be every agent in the fleet silently failing authentication, since
    /// nothing at compile time ties the two implementations together.
    #[test]
    fn derived_key_matches_known_vector() {
        let key = Encryptor::derive_key_from_token("agt_test_token").unwrap();
        let hex: String = key.iter().map(|b| format!("{:02x}", b)).collect();

        assert_eq!(
            hex, "26491796d51fdc101b48bc0a41d707eeab13e958e00cb4caf4a4a0e7f9901dc4",
            "agent key derivation changed; update backend tcp_server.rs to match"
        );
    }

    #[test]
    fn test_from_token() {
        let token = "proj_abc123_xyz789";
        let encryptor = Encryptor::from_token(token).unwrap();

        let plaintext = b"Test message";
        let encrypted = encryptor.encrypt(plaintext).unwrap();
        let decrypted = encryptor.decrypt(&encrypted).unwrap();

        assert_eq!(plaintext, decrypted.as_slice());
    }
}
