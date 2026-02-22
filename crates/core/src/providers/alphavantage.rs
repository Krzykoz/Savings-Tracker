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

const BASE_URL: &str = "https://www.alphavantage.co/query";

/// Alpha Vantage API provider for stock/equity prices.
///
/// - **Free tier**: 25 requests/day (across ALL endpoints).
/// - **Requires**: API key (set via settings as "alphavantage").
/// - **Coverage**: 100k+ global equity symbols.
/// - **Strategy**: Cache aggressively (24h), fetch daily data only.
///
/// Note: Returns prices in the stock's native currency (typically USD).
/// Cross-currency conversion handled by CurrencyService.
pub struct AlphaVantageProvider {
    client: Client,
    api_key: String,
}

impl AlphaVantageProvider {
    pub fn new(api_key: String) -> Self {
        let builder = Client::builder();
        #[cfg(not(target_arch = "wasm32"))]
        let builder = builder.timeout(Duration::from_secs(30));
        Self {
            client: builder.build().unwrap_or_else(|_| Client::new()),
            api_key,
        }
    }
}

// ── Alpha Vantage API response types ────────────────────────────────

#[derive(Deserialize)]
struct GlobalQuoteResponse {
    #[serde(rename = "Global Quote")]
    global_quote: Option<GlobalQuote>,
}

#[derive(Deserialize)]
struct GlobalQuote {
    #[serde(rename = "05. price")]
    price: Option<String>,
}

#[derive(Deserialize)]
struct TimeSeriesResponse {
    #[serde(rename = "Time Series (Daily)")]
    time_series: Option<HashMap<String, DailyData>>,
}

#[derive(Deserialize)]
struct DailyData {
    #[serde(rename = "4. close")]
    close: String,
}

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
impl PriceProvider for AlphaVantageProvider {
    fn name(&self) -> &str {
        "Alpha Vantage"
    }

    fn supported_asset_types(&self) -> Vec<AssetType> {
        vec![AssetType::Stock]
    }

    async fn get_current_price(
        &self,
        symbol: &str,
        _currency: &str,
    ) -> Result<f64, CoreError> {
        let resp: GlobalQuoteResponse = self
            .client
            .get(BASE_URL)
            .query(&[
                ("function", "GLOBAL_QUOTE"),
                ("symbol", &symbol.to_uppercase()),
                ("apikey", &self.api_key),
            ])
            .send()
            .await?
            .json()
            .await
            .map_err(|e| CoreError::Api {
                provider: "Alpha Vantage".into(),
                message: format!("Failed to parse quote for {symbol}: {e}"),
            })?;

        let price_str = resp
            .global_quote
            .and_then(|q| q.price)
            .ok_or_else(|| CoreError::Api {
                provider: "Alpha Vantage".into(),
                message: format!("No quote data for {symbol}. API limit may be exceeded."),
            })?;

        price_str.parse().map_err(|e| CoreError::Api {
            provider: "Alpha Vantage".into(),
            message: format!("Invalid price format for {symbol}: {e}"),
        })
    }

    async fn get_historical_price(
        &self,
        symbol: &str,
        _currency: &str,
        date: NaiveDate,
    ) -> Result<f64, CoreError> {
        // Fetch daily time series and find the specific date
        let time_series = self.fetch_daily_series(symbol).await?;

        let date_str = date.format("%Y-%m-%d").to_string();
        time_series
            .get(&date_str)
            .and_then(|d| d.close.parse().ok())
            .ok_or_else(|| CoreError::PriceNotAvailable {
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
        let time_series = self.fetch_daily_series(symbol).await?;

        let mut points: Vec<PricePoint> = time_series
            .iter()
            .filter_map(|(date_str, data)| {
                let date = NaiveDate::parse_from_str(date_str, "%Y-%m-%d").ok()?;
                if date >= from && date <= to {
                    let price: f64 = data.close.parse().ok()?;
                    Some(PricePoint { date, price })
                } else {
                    None
                }
            })
            .collect();

        points.sort_by_key(|p| p.date);
        Ok(points)
    }
}

impl AlphaVantageProvider {
    /// Fetch the daily time series for a stock symbol.
    /// Returns compact data (last 100 trading days).
    async fn fetch_daily_series(
        &self,
        symbol: &str,
    ) -> Result<HashMap<String, DailyData>, CoreError> {
        let resp: TimeSeriesResponse = self
            .client
            .get(BASE_URL)
            .query(&[
                ("function", "TIME_SERIES_DAILY"),
                ("symbol", &symbol.to_uppercase()),
                ("outputsize", "compact"),
                ("apikey", &self.api_key),
            ])
            .send()
            .await?
            .json()
            .await
            .map_err(|e| CoreError::Api {
                provider: "Alpha Vantage".into(),
                message: format!("Failed to parse time series for {symbol}: {e}"),
            })?;

        resp.time_series.ok_or_else(|| CoreError::Api {
            provider: "Alpha Vantage".into(),
            message: format!("No time series data for {symbol}. API limit may be exceeded."),
        })
    }
}
