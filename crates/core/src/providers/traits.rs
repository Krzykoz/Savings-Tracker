use async_trait::async_trait;
use chrono::NaiveDate;

use crate::errors::CoreError;
use crate::models::asset::AssetType;
use crate::models::price::PricePoint;

/// Trait abstraction for all price data providers (SOLID: Dependency Inversion).
///
/// Each API provider (CoinCap, Frankfurter, metals.dev, Alpha Vantage)
/// implements this trait. If an API stops working or changes, we replace
/// only that one implementation — the rest of the codebase is untouched.
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
pub trait PriceProvider: Send + Sync {
    /// Human-readable name of this provider (for logs/errors).
    fn name(&self) -> &str;

    /// Which asset types this provider can handle.
    fn supported_asset_types(&self) -> Vec<AssetType>;

    /// Get the current (latest) price of an asset in a given currency.
    async fn get_current_price(
        &self,
        symbol: &str,
        currency: &str,
    ) -> Result<f64, CoreError>;

    /// Get the historical price of an asset on a specific date.
    async fn get_historical_price(
        &self,
        symbol: &str,
        currency: &str,
        date: NaiveDate,
    ) -> Result<f64, CoreError>;

    /// Get price data for a date range (for chart generation).
    /// Returns a Vec of PricePoints sorted by date.
    async fn get_price_range(
        &self,
        symbol: &str,
        currency: &str,
        from: NaiveDate,
        to: NaiveDate,
    ) -> Result<Vec<PricePoint>, CoreError>;
}
