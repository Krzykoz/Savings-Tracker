use async_trait::async_trait;
use chrono::{Datelike, NaiveDate};
use time::OffsetDateTime;

use crate::errors::CoreError;
use crate::models::asset::AssetType;
use crate::models::price::PricePoint;
use super::traits::PriceProvider;

/// Yahoo Finance API provider for stock/equity prices.
///
/// - **Free**: No API key required.
/// - **No strict rate limits** (unofficial public API).
/// - **Coverage**: Global equities, ETFs, indices, mutual funds.
/// - **Data**: Real-time quotes + full historical OHLCV.
///
/// Uses the `yahoo_finance_api` crate which wraps Yahoo Finance's
/// public endpoints. Prices are returned in the stock's native currency
/// (typically USD). Cross-currency conversion handled by CurrencyService.
///
/// **Note**: Not WASM-compatible (uses native reqwest/tokio). For WASM
/// targets, Alpha Vantage via direct reqwest calls can be used as fallback.
pub struct YahooFinanceProvider {
    connector: yahoo_finance_api::YahooConnector,
}

impl YahooFinanceProvider {
    pub fn new() -> Result<Self, CoreError> {
        let connector = yahoo_finance_api::YahooConnector::new()
            .map_err(|e| CoreError::Api {
                provider: "Yahoo Finance".into(),
                message: format!("Failed to create connector: {e}"),
            })?;
        Ok(Self { connector })
    }

    /// Convert a `chrono::NaiveDate` to `time::OffsetDateTime` (midnight UTC).
    fn to_offset_datetime(date: NaiveDate) -> Result<OffsetDateTime, CoreError> {
        let month: time::Month = match date.month() {
            1 => time::Month::January,
            2 => time::Month::February,
            3 => time::Month::March,
            4 => time::Month::April,
            5 => time::Month::May,
            6 => time::Month::June,
            7 => time::Month::July,
            8 => time::Month::August,
            9 => time::Month::September,
            10 => time::Month::October,
            11 => time::Month::November,
            12 => time::Month::December,
            _ => unreachable!(),
        };

        let odt = time::Date::from_calendar_date(date.year(), month, date.day() as u8)
            .map_err(|e| CoreError::Api {
                provider: "Yahoo Finance".into(),
                message: format!("Invalid date {date}: {e}"),
            })?
            .with_hms(0, 0, 0)
            .map_err(|e| CoreError::Api {
                provider: "Yahoo Finance".into(),
                message: format!("Invalid time for {date}: {e}"),
            })?
            .assume_utc();
        Ok(odt)
    }

    /// Convert a unix timestamp (seconds) to `chrono::NaiveDate`.
    fn timestamp_to_naive_date(ts: i64) -> Option<NaiveDate> {
        chrono::DateTime::from_timestamp(ts, 0).map(|dt| dt.date_naive())
    }
}



#[async_trait]
impl PriceProvider for YahooFinanceProvider {
    fn name(&self) -> &str {
        "Yahoo Finance"
    }

    fn supported_asset_types(&self) -> Vec<AssetType> {
        vec![AssetType::Stock]
    }

    async fn get_current_price(
        &self,
        symbol: &str,
        _currency: &str,
    ) -> Result<f64, CoreError> {
        let resp = self
            .connector
            .get_latest_quotes(symbol, "1d")
            .await
            .map_err(|e| CoreError::Api {
                provider: "Yahoo Finance".into(),
                message: format!("Failed to fetch latest quote for {symbol}: {e}"),
            })?;

        let quote = resp.last_quote().map_err(|e| CoreError::Api {
            provider: "Yahoo Finance".into(),
            message: format!("No quote data for {symbol}: {e}"),
        })?;

        Ok(quote.close)
    }

    async fn get_historical_price(
        &self,
        symbol: &str,
        _currency: &str,
        date: NaiveDate,
    ) -> Result<f64, CoreError> {
        let start = Self::to_offset_datetime(date)?;
        // Fetch a 3-day window to handle weekends/holidays
        let end_date = date + chrono::Duration::days(3);
        let end = Self::to_offset_datetime(end_date)?;

        let resp = self
            .connector
            .get_quote_history(symbol, start, end)
            .await
            .map_err(|e| CoreError::Api {
                provider: "Yahoo Finance".into(),
                message: format!("Failed to fetch history for {symbol} on {date}: {e}"),
            })?;

        let quotes = resp.quotes().map_err(|e| CoreError::Api {
            provider: "Yahoo Finance".into(),
            message: format!("Failed to parse quotes for {symbol}: {e}"),
        })?;

        // Find the closest quote to the requested date
        let target_ts = date
            .and_hms_opt(0, 0, 0)
            .unwrap()
            .and_utc()
            .timestamp() as u64;

        // Try exact date first, then closest
        let quote = quotes
            .iter()
            .min_by_key(|q| (q.timestamp - target_ts as i64).unsigned_abs())
            .ok_or_else(|| CoreError::PriceNotAvailable {
                symbol: symbol.to_string(),
                currency: "USD".to_string(),
                date: date.to_string(),
            })?;

        Ok(quote.close)
    }

    async fn get_price_range(
        &self,
        symbol: &str,
        _currency: &str,
        from: NaiveDate,
        to: NaiveDate,
    ) -> Result<Vec<PricePoint>, CoreError> {
        let start = Self::to_offset_datetime(from)?;
        let end = Self::to_offset_datetime(to + chrono::Duration::days(1))?; // inclusive end

        let resp = self
            .connector
            .get_quote_history(symbol, start, end)
            .await
            .map_err(|e| CoreError::Api {
                provider: "Yahoo Finance".into(),
                message: format!("Failed to fetch history range for {symbol}: {e}"),
            })?;

        let quotes = resp.quotes().map_err(|e| CoreError::Api {
            provider: "Yahoo Finance".into(),
            message: format!("Failed to parse quotes for {symbol}: {e}"),
        })?;

        let points: Vec<PricePoint> = quotes
            .iter()
            .filter_map(|q| {
                let date = Self::timestamp_to_naive_date(q.timestamp)?;
                if date >= from && date <= to {
                    Some(PricePoint {
                        date,
                        price: q.close,
                    })
                } else {
                    None
                }
            })
            .collect();

        Ok(points)
    }
}
