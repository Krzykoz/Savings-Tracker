use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use argon2::{Argon2, Algorithm, Version, Params};

use crate::errors::CoreError;

/// Argon2id parameters for key derivation.
/// Stored in the file header so they can be upgraded in future versions.
#[derive(Debug, Clone, Copy)]
pub struct KdfParams {
    /// Memory cost in KiB (default: 65536 = 64 MB)
    pub memory_cost: u32,
    /// Number of iterations (default: 3)
    pub time_cost: u32,
    /// Degree of parallelism (default: 4)
    pub parallelism: u32,
}

impl Default for KdfParams {
    fn default() -> Self {
        Self {
            memory_cost: 65_536, // 64 MB
            time_cost: 3,
            parallelism: 4,
        }
    }
}

/// Derive a 256-bit encryption key from a password using Argon2id.
///
/// Argon2id is the recommended variant — resistant to both side-channel
/// and GPU-based attacks. The salt must be random and unique per file save.
pub fn derive_key(password: &str, salt: &[u8; 16], params: &KdfParams) -> Result<[u8; 32], CoreError> {
    let argon2_params = Params::new(
        params.memory_cost,
        params.time_cost,
        params.parallelism,
        Some(32), // output length = 256 bits
    )
    .map_err(|e| CoreError::Encryption(format!("Invalid Argon2 params: {e}")))?;

    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, argon2_params);

    let mut key = [0u8; 32];
    argon2
        .hash_password_into(password.as_bytes(), salt, &mut key)
        .map_err(|e| CoreError::Encryption(format!("Argon2 key derivation failed: {e}")))?;

    Ok(key)
}

/// Encrypt plaintext using AES-256-GCM.
///
/// Returns ciphertext with the 16-byte authentication tag appended.
/// The tag provides both confidentiality and integrity — no separate HMAC needed.
pub fn encrypt(plaintext: &[u8], key: &[u8; 32], nonce: &[u8; 12]) -> Result<Vec<u8>, CoreError> {
    let cipher = Aes256Gcm::new_from_slice(key)
        .map_err(|e| CoreError::Encryption(format!("Failed to create cipher: {e}")))?;
    let nonce = Nonce::from_slice(nonce);

    cipher
        .encrypt(nonce, plaintext)
        .map_err(|e| CoreError::Encryption(format!("Encryption failed: {e}")))
}

/// Decrypt ciphertext using AES-256-GCM.
///
/// Verifies the authentication tag automatically. Returns `CoreError::Decryption`
/// if the password is wrong or the data has been tampered with.
pub fn decrypt(ciphertext: &[u8], key: &[u8; 32], nonce: &[u8; 12]) -> Result<Vec<u8>, CoreError> {
    let cipher = Aes256Gcm::new_from_slice(key)
        .map_err(|e| CoreError::Encryption(format!("Failed to create cipher: {e}")))?;
    let nonce = Nonce::from_slice(nonce);

    cipher
        .decrypt(nonce, ciphertext)
        .map_err(|_| CoreError::Decryption)
}

/// Generate cryptographically secure random bytes for a salt.
pub fn generate_salt() -> Result<[u8; 16], CoreError> {
    let mut salt = [0u8; 16];
    getrandom::getrandom(&mut salt)
        .map_err(|e| CoreError::Encryption(format!("Failed to generate random salt: {e}")))?;
    Ok(salt)
}

/// Generate cryptographically secure random bytes for a nonce.
pub fn generate_nonce() -> Result<[u8; 12], CoreError> {
    let mut nonce = [0u8; 12];
    getrandom::getrandom(&mut nonce)
        .map_err(|e| CoreError::Encryption(format!("Failed to generate random nonce: {e}")))?;
    Ok(nonce)
}
