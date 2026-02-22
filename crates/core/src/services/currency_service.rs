use chrono::NaiveDate;

use crate::errors::CoreError;
use crate::models::asset::{Asset, AssetType};
use crate::models::price::PriceCache;
use super::price_service::PriceService;

/// Handles currency conversion between any two currencies or asset → currency.
///
/// Uses PriceService + Frankfurter (for fiat) to convert:
/// - Fiat ↔ Fiat (e.g., EUR → PLN)
/// - Crypto → Fiat (e.g., BTC in USD → value in PLN)
/// - Metal → Fiat (e.g., XAU in USD → value in PLN)
/// - Stock → Fiat (e.g., AAPL in USD → value in PLN)
///
/// Most API providers return prices in USD. If the target currency
/// is not USD, we do a two-step conversion: Asset → USD → target currency.
pub struct CurrencyService;

impl CurrencyService {
    pub fn new() -> Self {
        Self
    }

    /// Convert an amount of a fiat currency to another fiat currency.
    /// E.g., convert(1000.0, "USD", "PLN", date) → ~4100.0
    pub async fn convert_fiat(
        &self,
        price_service: &PriceService,
        cache: &mut PriceCache,
        amount: f64,
        from_currency: &str,
        to_currency: &str,
        date: NaiveDate,
    ) -> Result<f64, CoreError> {
        let from = from_currency.to_uppercase();
        let to = to_currency.to_uppercase();

        if from == to {
            return Ok(amount);
        }

        // Get the exchange rate from → to using Frankfurter
        let rate = price_service
            .get_price(cache, &from, &to, date, &AssetType::Fiat)
            .await?;

        Ok(amount * rate)
    }

    /// Convert an asset holding to a target fiat currency.
    ///
    /// For non-fiat assets (crypto, metal, stock), providers return prices in USD.
    /// We first get the USD price, then convert USD → target currency if needed.
    ///
    /// For fiat assets, we directly use the Frankfurter exchange rate.
    pub async fn convert_asset_to_currency(
        &self,
        price_service: &PriceService,
        cache: &mut PriceCache,
        asset: &Asset,
        amount: f64,
        target_currency: &str,
        date: NaiveDate,
    ) -> Result<f64, CoreError> {
        let target = target_currency.to_uppercase();

        match asset.asset_type {
            AssetType::Fiat => {
                // Fiat → Fiat: direct conversion
                self.convert_fiat(price_service, cache, amount, &asset.symbol, &target, date)
                    .await
            }

            AssetType::Crypto | AssetType::Metal | AssetType::Stock => {
                // Step 1: Get asset price in USD from the respective provider
                let price_usd = price_service
                    .get_price(cache, &asset.symbol, "USD", date, &asset.asset_type)
                    .await?;

                let value_usd = amount * price_usd;

                // Step 2: If target is USD, we're done
                if target == "USD" {
                    return Ok(value_usd);
                }

                // Step 3: Convert USD → target currency
                self.convert_fiat(price_service, cache, value_usd, "USD", &target, date)
                    .await
            }
        }
    }
}

impl Default for CurrencyService {
    fn default() -> Self {
        Self::new()
    }
}
