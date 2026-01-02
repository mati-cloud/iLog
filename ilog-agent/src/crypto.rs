use chacha20poly1305::{
    aead::{Aead, KeyInit, OsRng},
    ChaCha20Poly1305, Nonce,
};
use anyhow::Result;
use rand::RngCore;

const NONCE_SIZE: usize = 12;

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

    fn derive_key_from_token(token: &str) -> Result<[u8; 32]> {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        token.hash(&mut hasher);
        let hash = hasher.finish();

        let mut key = [0u8; 32];
        for (i, chunk) in key.chunks_mut(8).enumerate() {
            let mut h = DefaultHasher::new();
            (hash, i).hash(&mut h);
            chunk.copy_from_slice(&h.finish().to_le_bytes());
        }

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
