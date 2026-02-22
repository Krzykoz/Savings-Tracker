use crate::errors::CoreError;
use crate::models::portfolio::Portfolio;

use super::encryption::{self, KdfParams};
use super::format;

/// High-level storage operations: save/load portfolio to/from encrypted bytes or files.
pub struct StorageManager;

impl StorageManager {
    /// Encrypt and serialize a portfolio to raw bytes (portable, platform-independent).
    ///
    /// Flow: Portfolio → bincode → AES-256-GCM(Argon2id(password)) → SVTK format bytes
    pub fn save_to_bytes(portfolio: &Portfolio, password: &str) -> Result<Vec<u8>, CoreError> {
        // 1. Serialize portfolio to binary
        let plaintext = bincode::serialize(portfolio)
            .map_err(|e| CoreError::Serialization(format!("Failed to serialize portfolio: {e}")))?;

        // 2. Generate fresh salt and nonce
        let salt = encryption::generate_salt()?;
        let nonce = encryption::generate_nonce()?;

        // 3. Derive encryption key from password
        let kdf_params = KdfParams::default();
        let key = encryption::derive_key(password, &salt, &kdf_params)?;

        // 4. Encrypt
        let ciphertext = encryption::encrypt(&plaintext, &key, &nonce)?;

        // 5. Assemble file format
        let file_bytes = format::write_file(
            format::CURRENT_VERSION,
            &kdf_params,
            &salt,
            &nonce,
            &ciphertext,
        );

        Ok(file_bytes)
    }

    /// Decrypt and deserialize a portfolio from raw bytes.
    ///
    /// Flow: SVTK bytes → parse header → Argon2id(password, salt) → AES-256-GCM decrypt → bincode → Portfolio
    pub fn load_from_bytes(data: &[u8], password: &str) -> Result<Portfolio, CoreError> {
        // 1. Parse file header
        let (header, ciphertext) = format::read_file(data)?;

        // 2. Re-derive key from password + stored salt + stored params
        let key = encryption::derive_key(password, &header.salt, &header.kdf_params)?;

        // 3. Decrypt
        let plaintext = encryption::decrypt(ciphertext, &key, &header.nonce)?;

        // 4. Deserialize
        let portfolio: Portfolio = bincode::deserialize(&plaintext)
            .map_err(|e| CoreError::Deserialization(format!("Failed to deserialize portfolio: {e}")))?;

        Ok(portfolio)
    }

    /// Save portfolio to an encrypted file on disk (native only).
    #[cfg(not(target_arch = "wasm32"))]
    pub fn save_to_file(
        portfolio: &Portfolio,
        path: &str,
        password: &str,
    ) -> Result<(), CoreError> {
        let bytes = Self::save_to_bytes(portfolio, password)?;
        std::fs::write(path, bytes)?;
        Ok(())
    }

    /// Load portfolio from an encrypted file on disk (native only).
    #[cfg(not(target_arch = "wasm32"))]
    pub fn load_from_file(path: &str, password: &str) -> Result<Portfolio, CoreError> {
        let bytes = std::fs::read(path)?;
        Self::load_from_bytes(&bytes, password)
    }
}
