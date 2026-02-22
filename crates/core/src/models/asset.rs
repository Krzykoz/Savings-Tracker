use serde::{Deserialize, Serialize};

/// The type/category of a tracked asset.
/// Determines which price provider to use for fetching market data.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AssetType {
    /// Cryptocurrencies (BTC, ETH, etc.) — uses CoinCap API
    Crypto,
    /// Fiat currencies (USD, EUR, PLN, etc.) — uses Frankfurter API
    Fiat,
    /// Precious metals (XAU, XAG, etc.) — uses metals.dev API
    Metal,
    /// Stocks / equities (AAPL, MSFT, etc.) — uses Alpha Vantage API
    Stock,
}

impl std::fmt::Display for AssetType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AssetType::Crypto => write!(f, "Crypto"),
            AssetType::Fiat => write!(f, "Fiat"),
            AssetType::Metal => write!(f, "Metal"),
            AssetType::Stock => write!(f, "Stock"),
        }
    }
}

/// Represents a trackable asset (currency, crypto, metal, stock).
///
/// **Equality and hashing** are based solely on `(symbol, asset_type)`,
/// NOT on `name`. This ensures consistent HashMap lookups regardless
/// of the display name used when creating the asset.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Asset {
    /// Ticker symbol, uppercased (e.g., "BTC", "USD", "XAU", "AAPL")
    pub symbol: String,

    /// Human-readable name (e.g., "Bitcoin", "US Dollar", "Gold", "Apple Inc.")
    pub name: String,

    /// Asset category — determines which API provider to use
    pub asset_type: AssetType,
}

impl PartialEq for Asset {
    fn eq(&self, other: &Self) -> bool {
        self.symbol == other.symbol && self.asset_type == other.asset_type
    }
}

impl Eq for Asset {}

impl std::hash::Hash for Asset {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.symbol.hash(state);
        self.asset_type.hash(state);
    }
}

impl Asset {
    pub fn new(symbol: impl Into<String>, name: impl Into<String>, asset_type: AssetType) -> Self {
        Self {
            symbol: symbol.into().to_uppercase(),
            name: name.into(),
            asset_type,
        }
    }

    /// Convenience constructors for common asset types
    pub fn crypto(symbol: impl Into<String>, name: impl Into<String>) -> Self {
        Self::new(symbol, name, AssetType::Crypto)
    }

    pub fn fiat(symbol: impl Into<String>, name: impl Into<String>) -> Self {
        Self::new(symbol, name, AssetType::Fiat)
    }

    pub fn metal(symbol: impl Into<String>, name: impl Into<String>) -> Self {
        Self::new(symbol, name, AssetType::Metal)
    }

    pub fn stock(symbol: impl Into<String>, name: impl Into<String>) -> Self {
        Self::new(symbol, name, AssetType::Stock)
    }
}
