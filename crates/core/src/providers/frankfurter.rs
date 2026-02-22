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

const BASE_URL: &str = "https://api.frankfurter.dev/v1";

/// Frankfurter API provider for fiat currency exchange rates.
///
/// - **Free**: No API key, no rate limits, open-source.
/// - **Source**: European Central Bank (ECB) data.
/// - **Coverage**: ~30+ currencies (EUR, USD, PLN, GBP, JPY, etc.)
/// - **Endpoints**: `/latest`, `/{date}`, `/{start}..{end}`
///
/// Note: Frankfurter uses EUR as the base by default.
/// All rates are relative to the specified base currency.
pub struct FrankfurterProvider {
    client: Client,
}

impl FrankfurterProvider {
    pub fn new() -> Self {
        let builder = Client::builder();
        #[cfg(not(target_arch = "wasm32"))]
        let builder = builder.timeout(Duration::from_secs(30));
        Self {
            client: builder.build().unwrap_or_else(|_| Client::new()),
        }
    }
}

impl Default for FrankfurterProvider {
    fn default() -> Self {
        Self::new()
    }
}

// ── Frankfurter API response types ──────────────────────────────────

#[derive(Deserialize)]
struct RatesResponse {
    rates: HashMap<String, f64>,
}

#[derive(Deserialize)]
struct TimeSeriesResponse {
    rates: HashMap<String, HashMap<String, f64>>,
}

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
impl PriceProvider for FrankfurterProvider {
    fn name(&self) -> &str {
        "Frankfurter"
    }

    fn supported_asset_types(&self) -> Vec<AssetType> {
        vec![AssetType::Fiat]
    }

    async fn get_current_price(
        &self,
        symbol: &str,
        currency: &str,
    ) -> Result<f64, CoreError> {
        let base = symbol.to_uppercase();
        let target = currency.to_uppercase();

        // Same currency → rate is 1.0
        if base == target {
            return Ok(1.0);
        }

        let url = format!("{BASE_URL}/latest?base={base}&symbols={target}");

        let resp: RatesResponse = self
            .client
            .get(&url)
            .send()
            .await?
            .json()
            .await
            .map_err(|e| CoreError::Api {
                provider: "Frankfurter".into(),
                message: format!("Failed to parse response for {base}/{target}: {e}"),
            })?;

        resp.rates.get(&target).copied().ok_or_else(|| CoreError::Api {
            provider: "Frankfurter".into(),
            message: format!("No rate found for {base} → {target}"),
        })
    }

    async fn get_historical_price(
        &self,
        symbol: &str,
        currency: &str,
        date: NaiveDate,
    ) -> Result<f64, CoreError> {
        let base = symbol.to_uppercase();
        let target = currency.to_uppercase();

        if base == target {
            return Ok(1.0);
        }

        let date_str = date.format("%Y-%m-%d");
        let url = format!("{BASE_URL}/{date_str}?base={base}&symbols={target}");

        let resp: RatesResponse = self
            .client
            .get(&url)
            .send()
            .await?
            .json()
            .await
            .map_err(|e| CoreError::Api {
                provider: "Frankfurter".into(),
                message: format!("Failed to parse historical rate for {base}/{target} on {date}: {e}"),
            })?;

        resp.rates.get(&target).copied().ok_or_else(|| CoreError::PriceNotAvailable {
            symbol: symbol.to_string(),
            currency: currency.to_string(),
            date: date.to_string(),
        })
    }

    async fn get_price_range(
        &self,
        symbol: &str,
        currency: &str,
        from: NaiveDate,
        to: NaiveDate,
    ) -> Result<Vec<PricePoint>, CoreError> {
        let base = symbol.to_uppercase();
        let target = currency.to_uppercase();

        if base == target {
            // Build daily points all at 1.0
            let mut points = Vec::new();
            let mut d = from;
            while d <= to {
                points.push(PricePoint { date: d, price: 1.0 });
                match d.succ_opt() {
                    Some(next) => d = next,
                    None => break,
                }
            }
            return Ok(points);
        }

        let from_str = from.format("%Y-%m-%d");
        let to_str = to.format("%Y-%m-%d");
        let url = format!("{BASE_URL}/{from_str}..{to_str}?base={base}&symbols={target}");

        let resp: TimeSeriesResponse = self
            .client
            .get(&url)
            .send()
            .await?
            .json()
            .await
            .map_err(|e| CoreError::Api {
                provider: "Frankfurter".into(),
                message: format!("Failed to parse time series for {base}/{target}: {e}"),
            })?;

        let mut points: Vec<PricePoint> = resp
            .rates
            .iter()
            .filter_map(|(date_str, rates)| {
                let date = NaiveDate::parse_from_str(date_str, "%Y-%m-%d").ok()?;
                let price = rates.get(&target)?;
                Some(PricePoint { date, price: *price })
            })
            .collect();

        points.sort_by_key(|p| p.date);
        Ok(points)
    }
}
