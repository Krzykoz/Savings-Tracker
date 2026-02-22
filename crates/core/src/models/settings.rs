use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// User-configurable settings, stored inside the encrypted portfolio file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    /// The currency in which all portfolio values are displayed (e.g., "PLN", "USD", "EUR").
    pub default_currency: String,

    /// Optional API keys for providers that require them.
    /// Keys: provider name (e.g., "metals_dev", "alphavantage").
    /// Values: the API key string.
    pub api_keys: HashMap<String, String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            default_currency: "USD".to_string(),
            api_keys: HashMap::new(),
        }
    }
}
