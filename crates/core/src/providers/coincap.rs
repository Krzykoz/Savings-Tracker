use async_trait::async_trait;
use chrono::NaiveDate;
use reqwest::Client;
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::Mutex;
#[cfg(not(target_arch = "wasm32"))]
use std::time::Duration;

use crate::errors::CoreError;
use crate::models::asset::AssetType;
use crate::models::price::PricePoint;
use super::traits::PriceProvider;

const BASE_URL: &str = "https://api.coincap.io/v2";

/// CoinCap API provider for cryptocurrency prices.
///
/// - **Free**: No API key required, no strict rate limits.
/// - **Data**: 2000+ cryptocurrencies, real-time and historical.
/// - **Endpoints**: `/assets/{id}`, `/assets/{id}/history`, `/assets?search={symbol}`
///
/// Note: CoinCap uses lowercase ids like "bitcoin", "ethereum".
/// We map common symbols (BTC → bitcoin) and dynamically resolve unknown ones.
pub struct CoinCapProvider {
    client: Client,
    /// Map from uppercase symbol (BTC) to CoinCap asset id (bitcoin).
    /// Seeded with common mappings, extended at runtime via dynamic search.
    symbol_map: Mutex<HashMap<String, String>>,
}

impl CoinCapProvider {
    pub fn new() -> Self {
        let mut symbol_map = HashMap::new();
        // Pre-populate common mappings
        let common = vec![
            ("BTC", "bitcoin"),
            ("ETH", "ethereum"),
            ("USDT", "tether"),
            ("USDC", "usd-coin"),
            ("BNB", "binance-coin"),
            ("XRP", "xrp"),
            ("ADA", "cardano"),
            ("SOL", "solana"),
            ("DOGE", "dogecoin"),
            ("DOT", "polkadot"),
            ("MATIC", "polygon"),
            ("LTC", "litecoin"),
            ("AVAX", "avalanche"),
            ("LINK", "chainlink"),
            ("UNI", "uniswap"),
            ("ATOM", "cosmos"),
            ("XLM", "stellar"),
            ("ALGO", "algorand"),
            ("NEAR", "near-protocol"),
            ("FTM", "fantom"),
            ("SHIB", "shiba-inu"),
            ("TRX", "tron"),
            ("DAI", "multi-collateral-dai"),
            ("AAVE", "aave"),
            ("CRO", "crypto-com-coin"),
            ("FIL", "filecoin"),
            ("ICP", "internet-computer"),
            ("ETC", "ethereum-classic"),
            ("HBAR", "hedera-hashgraph"),
            ("VET", "vechain"),
            ("MANA", "decentraland"),
            ("SAND", "the-sandbox"),
            ("XMR", "monero"),
            ("EOS", "eos"),
            ("THETA", "theta"),
            ("XTZ", "tezos"),
            ("EGLD", "elrond-erd-2"),
            ("AXS", "axie-infinity"),
            ("FLOW", "flow"),
            ("ZEC", "zcash"),
        ];
        for (sym, id) in common {
            symbol_map.insert(sym.to_string(), id.to_string());
        }

        let builder = Client::builder();
        #[cfg(not(target_arch = "wasm32"))]
        let builder = builder.timeout(Duration::from_secs(30));
        Self {
            client: builder.build().unwrap_or_else(|_| Client::new()),
            symbol_map: Mutex::new(symbol_map),
        }
    }

    /// Resolve a symbol like "BTC" to a CoinCap ID like "bitcoin".
    /// Checks the static map first.
    pub fn resolve_id(&self, symbol: &str) -> String {
        let upper = symbol.to_uppercase();
        let map = self.symbol_map.lock().unwrap_or_else(|e| e.into_inner());
        map.get(&upper)
            .cloned()
            .unwrap_or_else(|| symbol.to_lowercase())
    }

    /// Dynamically resolve a symbol by searching the CoinCap API.
    /// Caches the result for future lookups.
    async fn resolve_id_dynamic(&self, symbol: &str) -> Result<String, CoreError> {
        let upper = symbol.to_uppercase();

        // Check cache first
        {
            let map = self.symbol_map.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(id) = map.get(&upper) {
                return Ok(id.clone());
            }
        }

        // Search CoinCap API: /assets?search={symbol}&limit=5
        let url = format!("{BASE_URL}/assets?search={upper}&limit=5");
        let resp: AssetsSearchResponse = self
            .client
            .get(&url)
            .send()
            .await?
            .json()
            .await
            .map_err(|e| CoreError::Api {
                provider: "CoinCap".into(),
                message: format!("Failed to search for {upper}: {e}"),
            })?;

        // Find the asset whose symbol matches (case-insensitive)
        let matched = resp
            .data
            .iter()
            .find(|a| a.symbol.to_uppercase() == upper)
            .ok_or_else(|| CoreError::Api {
                provider: "CoinCap".into(),
                message: format!("No CoinCap asset found for symbol {upper}"),
            })?;

        let id = matched.id.clone();

        // Cache for next time
        {
            let mut map = self.symbol_map.lock().unwrap_or_else(|e| e.into_inner());
            map.insert(upper, id.clone());
        }

        Ok(id)
    }
}

impl Default for CoinCapProvider {
    fn default() -> Self {
        Self::new()
    }
}

// ── CoinCap API response types ──────────────────────────────────────

#[derive(Deserialize)]
struct AssetResponse {
    data: AssetData,
}

#[derive(Deserialize)]
struct AssetData {
    #[serde(rename = "priceUsd")]
    price_usd: Option<String>,
}

#[derive(Deserialize)]
struct HistoryResponse {
    data: Vec<HistoryPoint>,
}

#[derive(Deserialize)]
struct HistoryPoint {
    #[serde(rename = "priceUsd")]
    price_usd: String,
    time: i64, // unix timestamp in milliseconds
}

#[derive(Deserialize)]
struct AssetsSearchResponse {
    data: Vec<AssetSearchEntry>,
}

#[derive(Deserialize)]
struct AssetSearchEntry {
    id: String,
    symbol: String,
}

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
impl PriceProvider for CoinCapProvider {
    fn name(&self) -> &str {
        "CoinCap"
    }

    fn supported_asset_types(&self) -> Vec<AssetType> {
        vec![AssetType::Crypto]
    }

    async fn get_current_price(
        &self,
        symbol: &str,
        _currency: &str,
    ) -> Result<f64, CoreError> {
        let id = self.resolve_id_dynamic(symbol).await?;
        let url = format!("{BASE_URL}/assets/{id}");

        let resp: AssetResponse = self
            .client
            .get(&url)
            .send()
            .await?
            .json()
            .await
            .map_err(|e| CoreError::Api {
                provider: "CoinCap".into(),
                message: format!("Failed to parse response for {symbol}: {e}"),
            })?;

        let price_usd: f64 = resp
            .data
            .price_usd
            .ok_or_else(|| CoreError::Api {
                provider: "CoinCap".into(),
                message: format!("No price data for {symbol}"),
            })?
            .parse()
            .map_err(|e| CoreError::Api {
                provider: "CoinCap".into(),
                message: format!("Invalid price format for {symbol}: {e}"),
            })?;

        // CoinCap returns prices in USD. If target currency is not USD,
        // the caller (PriceService) will handle conversion via CurrencyService.
        Ok(price_usd)
    }

    async fn get_historical_price(
        &self,
        symbol: &str,
        currency: &str,
        date: NaiveDate,
    ) -> Result<f64, CoreError> {
        // CoinCap history API uses interval and start/end timestamps
        let id = self.resolve_id_dynamic(symbol).await?;
        let start = date
            .and_hms_opt(0, 0, 0)
            .unwrap()
            .and_utc()
            .timestamp_millis();
        let end = date
            .and_hms_opt(23, 59, 59)
            .unwrap()
            .and_utc()
            .timestamp_millis();

        let url = format!(
            "{BASE_URL}/assets/{id}/history?interval=d1&start={start}&end={end}"
        );

        let resp: HistoryResponse = self
            .client
            .get(&url)
            .send()
            .await?
            .json()
            .await
            .map_err(|e| CoreError::Api {
                provider: "CoinCap".into(),
                message: format!("Failed to parse history for {symbol}: {e}"),
            })?;

        let price_usd: f64 = resp
            .data
            .first()
            .ok_or_else(|| CoreError::PriceNotAvailable {
                symbol: symbol.to_string(),
                currency: currency.to_string(),
                date: date.to_string(),
            })?
            .price_usd
            .parse()
            .map_err(|e| CoreError::Api {
                provider: "CoinCap".into(),
                message: format!("Invalid price format: {e}"),
            })?;

        Ok(price_usd)
    }

    async fn get_price_range(
        &self,
        symbol: &str,
        _currency: &str,
        from: NaiveDate,
        to: NaiveDate,
    ) -> Result<Vec<PricePoint>, CoreError> {
        let id = self.resolve_id_dynamic(symbol).await?;
        let start = from
            .and_hms_opt(0, 0, 0)
            .unwrap()
            .and_utc()
            .timestamp_millis();
        let end = to
            .and_hms_opt(23, 59, 59)
            .unwrap()
            .and_utc()
            .timestamp_millis();

        let url = format!(
            "{BASE_URL}/assets/{id}/history?interval=d1&start={start}&end={end}"
        );

        let resp: HistoryResponse = self
            .client
            .get(&url)
            .send()
            .await?
            .json()
            .await
            .map_err(|e| CoreError::Api {
                provider: "CoinCap".into(),
                message: format!("Failed to parse history range for {symbol}: {e}"),
            })?;

        let points: Vec<PricePoint> = resp
            .data
            .iter()
            .filter_map(|p| {
                let price: f64 = p.price_usd.parse().ok()?;
                let dt = chrono::DateTime::from_timestamp_millis(p.time)?;
                Some(PricePoint {
                    date: dt.date_naive(),
                    price,
                })
            })
            .collect();

        Ok(points)
    }
}
