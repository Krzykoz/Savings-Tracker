use async_trait::async_trait;
use chrono::NaiveDate;
use reqwest::Client;
use serde::Deserialize;
use std::collections::HashMap;
#[cfg(not(target_arch = "wasm32"))]
use std::time::Duration;

use crate::errors::CoreError;
use crate::models::asset::AssetType;
use crate::models::price::PricePoint;
use super::traits::PriceProvider;

const BASE_URL: &str = "https://api.metals.dev/v1";

/// metals.dev API provider for precious metals prices.
///
/// - **Free tier**: 100 requests/month (no credit card required).
/// - **Requires**: API key (set via settings as "metals_dev").
/// - **Coverage**: Gold (XAU), Silver (XAG), Platinum (XPT), Palladium (XPD), etc.
/// - **Strategy**: Cache aggressively (24h+), only fetch when truly needed.
///
/// Note: metals.dev returns prices in USD. Cross-currency conversion
/// is handled by CurrencyService using Frankfurter.
pub struct MetalsDevProvider {
    client: Client,
    api_key: String,
    /// Map from our symbol (XAU) to metals.dev metal name (gold)
    symbol_map: HashMap<String, String>,
}

impl MetalsDevProvider {
    pub fn new(api_key: String) -> Self {
        let mut symbol_map = HashMap::new();
        symbol_map.insert("XAU".to_string(), "gold".to_string());
        symbol_map.insert("XAG".to_string(), "silver".to_string());
        symbol_map.insert("XPT".to_string(), "platinum".to_string());
        symbol_map.insert("XPD".to_string(), "palladium".to_string());

        let builder = Client::builder();
        #[cfg(not(target_arch = "wasm32"))]
        let builder = builder.timeout(Duration::from_secs(30));
        Self {
            client: builder.build().unwrap_or_else(|_| Client::new()),
            api_key,
            symbol_map,
        }
    }

    pub fn resolve_metal_name(&self, symbol: &str) -> Result<String, CoreError> {
        let upper = symbol.to_uppercase();
        self.symbol_map
            .get(&upper)
            .cloned()
            .ok_or_else(|| CoreError::Api {
                provider: "metals.dev".into(),
                message: format!("Unknown metal symbol: {symbol}. Supported: XAU, XAG, XPT, XPD"),
            })
    }
}

// ── metals.dev API response types ───────────────────────────────────

#[derive(Deserialize)]
struct LatestResponse {
    metals: HashMap<String, f64>,
}

// Note: metals.dev timeseries responses are parsed dynamically via serde_json::Value
// because the response structure varies by metal name key.

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
impl PriceProvider for MetalsDevProvider {
    fn name(&self) -> &str {
        "metals.dev"
    }

    fn supported_asset_types(&self) -> Vec<AssetType> {
        vec![AssetType::Metal]
    }

    async fn get_current_price(
        &self,
        symbol: &str,
        _currency: &str,
    ) -> Result<f64, CoreError> {
        let metal_name = self.resolve_metal_name(symbol)?;
        let url = format!("{BASE_URL}/latest");

        let resp: LatestResponse = self
            .client
            .get(&url)
            .query(&[("api_key", &self.api_key), ("currency", &"USD".to_string())])
            .send()
            .await?
            .json()
            .await
            .map_err(|e| CoreError::Api {
                provider: "metals.dev".into(),
                message: format!("Failed to parse latest prices: {e}"),
            })?;

        resp.metals
            .get(&metal_name)
            .copied()
            .ok_or_else(|| CoreError::PriceNotAvailable {
                symbol: symbol.to_string(),
                currency: "USD".to_string(),
                date: "latest".to_string(),
            })
    }

    async fn get_historical_price(
        &self,
        symbol: &str,
        _currency: &str,
        date: NaiveDate,
    ) -> Result<f64, CoreError> {
        // metals.dev has a /timeseries endpoint for historical data
        let metal_name = self.resolve_metal_name(symbol)?;
        let date_str = date.format("%Y-%m-%d").to_string();
        let url = format!("{BASE_URL}/timeseries");

        let resp_text = self
            .client
            .get(&url)
            .query(&[
                ("api_key", self.api_key.as_str()),
                ("currency", "USD"),
                ("metal", &metal_name),
                ("start_date", &date_str),
                ("end_date", &date_str),
            ])
            .send()
            .await?
            .text()
            .await?;

        // metals.dev timeseries returns: { "metal_name": [{"date": "...", "price": ...}] }
        let parsed: serde_json::Value = serde_json::from_str(&resp_text)
            .map_err(|e| CoreError::Api {
                provider: "metals.dev".into(),
                message: format!("Failed to parse timeseries response: {e}"),
            })?;

        // Try to extract the price from the response
        if let Some(arr) = parsed.get(&metal_name).and_then(|v| v.as_array()) {
            if let Some(point) = arr.first() {
                if let Some(price) = point.get("price").and_then(|v| v.as_f64()) {
                    return Ok(price);
                }
            }
        }

        Err(CoreError::PriceNotAvailable {
            symbol: symbol.to_string(),
            currency: "USD".to_string(),
            date: date.to_string(),
        })
    }

    async fn get_price_range(
        &self,
        symbol: &str,
        _currency: &str,
        from: NaiveDate,
        to: NaiveDate,
    ) -> Result<Vec<PricePoint>, CoreError> {
        let metal_name = self.resolve_metal_name(symbol)?;
        let from_str = from.format("%Y-%m-%d").to_string();
        let to_str = to.format("%Y-%m-%d").to_string();

        let url = format!("{BASE_URL}/timeseries");

        let resp_text = self
            .client
            .get(&url)
            .query(&[
                ("api_key", self.api_key.as_str()),
                ("currency", "USD"),
                ("metal", &metal_name),
                ("start_date", &from_str),
                ("end_date", &to_str),
            ])
            .send()
            .await?
            .text()
            .await?;

        let parsed: serde_json::Value = serde_json::from_str(&resp_text)
            .map_err(|e| CoreError::Api {
                provider: "metals.dev".into(),
                message: format!("Failed to parse timeseries: {e}"),
            })?;

        let mut points = Vec::new();

        if let Some(arr) = parsed.get(&metal_name).and_then(|v| v.as_array()) {
            for item in arr {
                if let (Some(date_str), Some(price)) = (
                    item.get("date").and_then(|v| v.as_str()),
                    item.get("price").and_then(|v| v.as_f64()),
                ) {
                    if let Ok(date) = NaiveDate::parse_from_str(date_str, "%Y-%m-%d") {
                        points.push(PricePoint { date, price });
                    }
                }
            }
        }

        points.sort_by_key(|p| p.date);
        Ok(points)
    }
}
