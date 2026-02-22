use thiserror::Error;

/// Unified error type for the entire savings-tracker-core library.
/// Every public function returns `Result<T, CoreError>`.
#[derive(Debug, Error)]
pub enum CoreError {
    // ── Storage / File ──────────────────────────────────────────────
    #[error("Invalid file format: {0}")]
    InvalidFileFormat(String),

    #[error("Unsupported file version: {0}")]
    UnsupportedVersion(u16),

    #[error("Encryption failed: {0}")]
    Encryption(String),

    #[error("Decryption failed — wrong password or corrupted file")]
    Decryption,

    #[error("Serialization error: {0}")]
    Serialization(String),

    #[error("Deserialization error: {0}")]
    Deserialization(String),

    // ── File I/O (native only) ──────────────────────────────────────
    #[error("File I/O error: {0}")]
    FileIO(String),

    // ── API / Network ───────────────────────────────────────────────
    #[error("API error ({provider}): {message}")]
    Api {
        provider: String,
        message: String,
    },

    #[error("Network error: {0}")]
    Network(String),

    #[error("No provider available for asset type: {0}")]
    NoProvider(String),

    // ── Business Logic ──────────────────────────────────────────────
    #[error("Event validation failed: {0}")]
    ValidationError(String),

    #[error("Event not found: {0}")]
    EventNotFound(String),

    #[error("Price not available for {symbol} in {currency} on {date}")]
    PriceNotAvailable {
        symbol: String,
        currency: String,
        date: String,
    },
}

// ── Conversion helpers (From impls) ─────────────────────────────────

impl From<std::io::Error> for CoreError {
    fn from(e: std::io::Error) -> Self {
        CoreError::FileIO(e.to_string())
    }
}

impl From<bincode::Error> for CoreError {
    fn from(e: bincode::Error) -> Self {
        CoreError::Serialization(e.to_string())
    }
}

impl From<serde_json::Error> for CoreError {
    fn from(e: serde_json::Error) -> Self {
        CoreError::Deserialization(e.to_string())
    }
}

impl From<reqwest::Error> for CoreError {
    fn from(e: reqwest::Error) -> Self {
        // Sanitize error message: strip query parameters from URLs to prevent
        // API key leakage. reqwest errors often contain full URLs with secrets.
        let msg = e.to_string();
        let sanitized = if let Some(idx) = msg.find('?') {
            format!("{}?<query redacted>", &msg[..idx])
        } else {
            msg
        };
        CoreError::Network(sanitized)
    }
}

impl From<aes_gcm::Error> for CoreError {
    fn from(_: aes_gcm::Error) -> Self {
        CoreError::Decryption
    }
}
