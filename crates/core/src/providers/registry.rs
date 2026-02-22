use std::collections::HashMap;

use crate::models::asset::AssetType;

use super::alphavantage::AlphaVantageProvider;
use super::coincap::CoinCapProvider;
use super::frankfurter::FrankfurterProvider;
use super::metals_dev::MetalsDevProvider;
#[cfg(not(target_arch = "wasm32"))]
use super::yahoo_finance::YahooFinanceProvider;
use super::traits::PriceProvider;

/// Registry of all available price providers.
///
/// Routes requests to the correct provider based on `AssetType`.
/// New providers can be added without modifying existing code (Open/Closed Principle).
pub struct PriceProviderRegistry {
    providers: Vec<Box<dyn PriceProvider>>,
}

impl PriceProviderRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self {
            providers: Vec::new(),
        }
    }

    /// Create a registry with all default providers pre-configured.
    pub fn new_with_defaults(api_keys: &HashMap<String, String>) -> Self {
        let mut registry = Self::new();

        // CoinCap — crypto, no API key needed
        registry.register(Box::new(CoinCapProvider::new()));

        // Frankfurter — forex, no API key needed
        registry.register(Box::new(FrankfurterProvider::new()));

        // metals.dev — precious metals, requires API key
        if let Some(key) = api_keys.get("metals_dev") {
            registry.register(Box::new(MetalsDevProvider::new(key.clone())));
        }

        // Yahoo Finance — stocks, NO API key needed (primary)
        // Not available on WASM (uses native reqwest/tokio connectors)
        #[cfg(not(target_arch = "wasm32"))]
        {
            if let Ok(yahoo) = YahooFinanceProvider::new() {
                registry.register(Box::new(yahoo));
            }
        }

        // Alpha Vantage — stocks, requires API key (fallback)
        if let Some(key) = api_keys.get("alphavantage") {
            registry.register(Box::new(AlphaVantageProvider::new(key.clone())));
        }

        registry
    }

    /// Register a new price provider.
    pub fn register(&mut self, provider: Box<dyn PriceProvider>) {
        self.providers.push(provider);
    }

    /// Find the first provider that supports the given asset type.
    pub fn get_provider_for(&self, asset_type: &AssetType) -> Option<&dyn PriceProvider> {
        self.providers
            .iter()
            .find(|p| p.supported_asset_types().contains(asset_type))
            .map(|p| p.as_ref())
    }

    /// Return ALL providers that support the given asset type, ordered by registration priority.
    /// Used for fallback: if the first provider fails, try the next one.
    pub fn get_providers_for(&self, asset_type: &AssetType) -> Vec<&dyn PriceProvider> {
        self.providers
            .iter()
            .filter(|p| p.supported_asset_types().contains(asset_type))
            .map(|p| p.as_ref())
            .collect()
    }
}

impl Default for PriceProviderRegistry {
    fn default() -> Self {
        Self::new()
    }
}
