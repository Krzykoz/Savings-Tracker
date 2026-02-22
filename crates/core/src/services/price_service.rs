use chrono::NaiveDate;

use crate::errors::CoreError;
use crate::models::asset::AssetType;
use crate::models::price::{PriceCache, PricePoint};
use crate::providers::registry::PriceProviderRegistry;

/// Fetches asset prices from API providers with intelligent caching.
///
/// Cache strategy:
/// - **Historical dates (< today)**: fetch once, cache forever. Past prices don't change.
/// - **Today's date**: fetch once per session/day, refresh on explicit `refresh_prices()`.
/// - All cached prices are stored in `PriceCache` → saved in the encrypted file → offline access.
///
/// **Note on precision**: All prices are stored as `f64`, which has ~15-17 significant
/// decimal digits. For most financial use cases this is sufficient, but repeated
/// arithmetic operations may accumulate small floating-point errors.
pub struct PriceService {
    registry: PriceProviderRegistry,
}

impl PriceService {
    pub fn new(registry: PriceProviderRegistry) -> Self {
        Self { registry }
    }

    /// Check if at least one provider is available for a given asset type.
    pub fn has_provider_for(&self, asset_type: &AssetType) -> bool {
        self.registry.get_provider_for(asset_type).is_some()
    }

    /// Get the names of all providers available for a given asset type.
    pub fn get_provider_names(&self, asset_type: &AssetType) -> Vec<String> {
        self.registry
            .get_providers_for(asset_type)
            .iter()
            .map(|p| p.name().to_string())
            .collect()
    }

    /// Get the price of an asset in a given currency on a specific date.
    ///
    /// 1. Check cache → return if found (for historical dates, always use cache).
    /// 2. If not cached: fetch from API → store in cache → return.
    /// 3. For today's date: re-fetch if not already fetched today.
    pub async fn get_price(
        &self,
        cache: &mut PriceCache,
        symbol: &str,
        currency: &str,
        date: NaiveDate,
        asset_type: &AssetType,
    ) -> Result<f64, CoreError> {
        let today = chrono::Utc::now().date_naive();

        // Check cache first
        if let Some(price) = cache.get_price(symbol, currency, date) {
            // For historical dates, always trust cache
            if date < today {
                return Ok(price);
            }
            // For today, check if already refreshed today
            if cache.is_today_fresh(symbol, currency, today) {
                return Ok(price);
            }
        }

        // Cache miss — fetch from API
        let price = self.fetch_price(symbol, currency, date, asset_type).await?;

        // Store in cache
        cache.set_price(symbol, currency, date, price);
        if date == today {
            cache.mark_updated_today(symbol, currency, today);
        }

        Ok(price)
    }

    /// Fetch a range of prices (for chart generation).
    /// Uses cache for dates that are already cached, fetches missing ones from API.
    pub async fn get_price_range(
        &self,
        cache: &mut PriceCache,
        symbol: &str,
        currency: &str,
        from: NaiveDate,
        to: NaiveDate,
        asset_type: &AssetType,
    ) -> Result<Vec<PricePoint>, CoreError> {
        // Check what we already have cached
        let cached = cache.get_price_range(symbol, currency, from, to);

        // Use cache if we have data spanning the requested range boundaries
        // (checking first/last dates is more reliable than counting points,
        // since weekends/holidays produce fewer points than calendar days)
        if cached.len() >= 2 {
            let first = cached.first().unwrap().date;
            let last = cached.last().unwrap().date;
            // If cached data covers the range boundaries (within 3 days tolerance for
            // weekends/holidays at both ends), trust the cache
            if (first - from).num_days().abs() <= 3 && (to - last).num_days().abs() <= 3 {
                return Ok(cached);
            }
        }

        // Fetch the full range from API (with fallback)
        let providers = self.registry.get_providers_for(asset_type);
        if providers.is_empty() {
            return Err(CoreError::NoProvider(asset_type.to_string()));
        }

        let mut last_error = None;
        for provider in &providers {
            match provider.get_price_range(symbol, currency, from, to).await {
                Ok(points) => {
                    cache.set_prices(symbol, currency, &points);
                    return Ok(points);
                }
                Err(e) => {
                    last_error = Some(e);
                    // Try next provider
                }
            }
        }

        Err(last_error.unwrap_or_else(|| CoreError::NoProvider(asset_type.to_string())))
    }

    /// Internal: fetch a single price from API providers with automatic fallback.
    ///
    /// Tries providers in registration order. If the primary fails (API down,
    /// rate limited, etc.), automatically falls back to the next provider.
    /// Validates that returned prices are finite and non-negative.
    async fn fetch_price(
        &self,
        symbol: &str,
        currency: &str,
        date: NaiveDate,
        asset_type: &AssetType,
    ) -> Result<f64, CoreError> {
        let providers = self.registry.get_providers_for(asset_type);
        if providers.is_empty() {
            return Err(CoreError::NoProvider(asset_type.to_string()));
        }

        let today = chrono::Utc::now().date_naive();
        let mut last_error = None;

        for provider in &providers {
            let result = if date >= today {
                provider.get_current_price(symbol, currency).await
            } else {
                provider.get_historical_price(symbol, currency, date).await
            };

            match result {
                Ok(price) => {
                    // R4: Validate price is finite and non-negative
                    if !price.is_finite() || price < 0.0 {
                        last_error = Some(CoreError::Api {
                            provider: provider.name().to_string(),
                            message: format!(
                                "Invalid price returned for {symbol}: {price} (must be finite and non-negative)"
                            ),
                        });
                        continue;
                    }
                    return Ok(price);
                }
                Err(e) => {
                    last_error = Some(e);
                    // Try next provider
                }
            }
        }

        Err(last_error.unwrap_or_else(|| CoreError::NoProvider(asset_type.to_string())))
    }
}
