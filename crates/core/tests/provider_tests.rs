// ═══════════════════════════════════════════════════════════════════
// Provider Tests — Registry, CoinCap, Frankfurter, MetalsDev logic
// ═══════════════════════════════════════════════════════════════════

use async_trait::async_trait;
use chrono::NaiveDate;
use std::collections::HashMap;

use savings_tracker_core::errors::CoreError;
use savings_tracker_core::models::asset::AssetType;
use savings_tracker_core::models::price::PricePoint;
use savings_tracker_core::providers::coincap::CoinCapProvider;
use savings_tracker_core::providers::frankfurter::FrankfurterProvider;
use savings_tracker_core::providers::metals_dev::MetalsDevProvider;
use savings_tracker_core::providers::registry::PriceProviderRegistry;
use savings_tracker_core::providers::traits::PriceProvider;
use savings_tracker_core::providers::yahoo_finance::YahooFinanceProvider;

// ═══════════════════════════════════════════════════════════════════
// Test Helpers — Mock Providers
// ═══════════════════════════════════════════════════════════════════

/// A mock provider that supports only the specified asset types.
struct MockProvider {
    name: String,
    types: Vec<AssetType>,
}

impl MockProvider {
    fn new(name: &str, types: Vec<AssetType>) -> Self {
        Self {
            name: name.to_string(),
            types,
        }
    }
}

#[async_trait]
impl PriceProvider for MockProvider {
    fn name(&self) -> &str {
        &self.name
    }

    fn supported_asset_types(&self) -> Vec<AssetType> {
        self.types.clone()
    }

    async fn get_current_price(&self, _sym: &str, _cur: &str) -> Result<f64, CoreError> {
        Ok(100.0)
    }

    async fn get_historical_price(
        &self,
        _sym: &str,
        _cur: &str,
        _date: NaiveDate,
    ) -> Result<f64, CoreError> {
        Ok(99.0)
    }

    async fn get_price_range(
        &self,
        _sym: &str,
        _cur: &str,
        _from: NaiveDate,
        _to: NaiveDate,
    ) -> Result<Vec<PricePoint>, CoreError> {
        Ok(vec![])
    }
}

/// A mock provider that always fails.
#[allow(dead_code)]
struct FailingProvider {
    name: String,
    types: Vec<AssetType>,
}

impl FailingProvider {
    #[allow(dead_code)]
    fn new(name: &str, types: Vec<AssetType>) -> Self {
        Self {
            name: name.to_string(),
            types,
        }
    }
}

#[async_trait]
impl PriceProvider for FailingProvider {
    fn name(&self) -> &str {
        &self.name
    }

    fn supported_asset_types(&self) -> Vec<AssetType> {
        self.types.clone()
    }

    async fn get_current_price(&self, sym: &str, cur: &str) -> Result<f64, CoreError> {
        Err(CoreError::Api {
            provider: self.name.clone(),
            message: format!("Failed for {sym}/{cur}"),
        })
    }

    async fn get_historical_price(
        &self,
        sym: &str,
        cur: &str,
        _date: NaiveDate,
    ) -> Result<f64, CoreError> {
        Err(CoreError::Api {
            provider: self.name.clone(),
            message: format!("Failed historical for {sym}/{cur}"),
        })
    }

    async fn get_price_range(
        &self,
        _sym: &str,
        _cur: &str,
        _from: NaiveDate,
        _to: NaiveDate,
    ) -> Result<Vec<PricePoint>, CoreError> {
        Err(CoreError::Api {
            provider: self.name.clone(),
            message: "Failed range".into(),
        })
    }
}

// ═══════════════════════════════════════════════════════════════════
// PriceProviderRegistry — Construction
// ═══════════════════════════════════════════════════════════════════

mod registry_construction {
    use super::*;

    #[test]
    fn new_creates_empty_registry() {
        let registry = PriceProviderRegistry::new();
        assert!(registry.get_provider_for(&AssetType::Crypto).is_none());
        assert!(registry.get_provider_for(&AssetType::Fiat).is_none());
        assert!(registry.get_provider_for(&AssetType::Metal).is_none());
        assert!(registry.get_provider_for(&AssetType::Stock).is_none());
    }

    #[test]
    fn default_creates_empty_registry() {
        let registry = PriceProviderRegistry::default();
        assert!(registry.get_provider_for(&AssetType::Crypto).is_none());
    }

    #[test]
    fn register_single_provider() {
        let mut registry = PriceProviderRegistry::new();
        registry.register(Box::new(MockProvider::new(
            "MockCrypto",
            vec![AssetType::Crypto],
        )));
        assert!(registry.get_provider_for(&AssetType::Crypto).is_some());
        assert!(registry.get_provider_for(&AssetType::Fiat).is_none());
    }

    #[test]
    fn register_multiple_providers() {
        let mut registry = PriceProviderRegistry::new();
        registry.register(Box::new(MockProvider::new(
            "CryptoProvider",
            vec![AssetType::Crypto],
        )));
        registry.register(Box::new(MockProvider::new(
            "FiatProvider",
            vec![AssetType::Fiat],
        )));

        assert!(registry.get_provider_for(&AssetType::Crypto).is_some());
        assert!(registry.get_provider_for(&AssetType::Fiat).is_some());
        assert!(registry.get_provider_for(&AssetType::Metal).is_none());
    }

    #[test]
    fn register_multi_type_provider() {
        let mut registry = PriceProviderRegistry::new();
        registry.register(Box::new(MockProvider::new(
            "AllInOne",
            vec![
                AssetType::Crypto,
                AssetType::Fiat,
                AssetType::Metal,
                AssetType::Stock,
            ],
        )));

        assert!(registry.get_provider_for(&AssetType::Crypto).is_some());
        assert!(registry.get_provider_for(&AssetType::Fiat).is_some());
        assert!(registry.get_provider_for(&AssetType::Metal).is_some());
        assert!(registry.get_provider_for(&AssetType::Stock).is_some());
    }
}

// ═══════════════════════════════════════════════════════════════════
// PriceProviderRegistry — get_provider_for
// ═══════════════════════════════════════════════════════════════════

mod registry_get_provider {
    use super::*;

    #[test]
    fn returns_first_matching_provider() {
        let mut registry = PriceProviderRegistry::new();
        registry.register(Box::new(MockProvider::new(
            "First",
            vec![AssetType::Crypto],
        )));
        registry.register(Box::new(MockProvider::new(
            "Second",
            vec![AssetType::Crypto],
        )));

        let provider = registry.get_provider_for(&AssetType::Crypto).unwrap();
        assert_eq!(provider.name(), "First");
    }

    #[test]
    fn returns_none_for_unregistered_type() {
        let registry = PriceProviderRegistry::new();
        assert!(registry.get_provider_for(&AssetType::Stock).is_none());
    }

    #[test]
    fn skips_non_matching_providers() {
        let mut registry = PriceProviderRegistry::new();
        registry.register(Box::new(MockProvider::new(
            "CryptoOnly",
            vec![AssetType::Crypto],
        )));
        registry.register(Box::new(MockProvider::new(
            "StockOnly",
            vec![AssetType::Stock],
        )));

        let provider = registry.get_provider_for(&AssetType::Stock).unwrap();
        assert_eq!(provider.name(), "StockOnly");
    }
}

// ═══════════════════════════════════════════════════════════════════
// PriceProviderRegistry — get_providers_for (fallback support)
// ═══════════════════════════════════════════════════════════════════

mod registry_get_providers {
    use super::*;

    #[test]
    fn returns_all_matching_providers() {
        let mut registry = PriceProviderRegistry::new();
        registry.register(Box::new(MockProvider::new(
            "Primary",
            vec![AssetType::Stock],
        )));
        registry.register(Box::new(MockProvider::new(
            "Fallback",
            vec![AssetType::Stock],
        )));
        registry.register(Box::new(MockProvider::new(
            "Unrelated",
            vec![AssetType::Crypto],
        )));

        let providers = registry.get_providers_for(&AssetType::Stock);
        assert_eq!(providers.len(), 2);
        assert_eq!(providers[0].name(), "Primary");
        assert_eq!(providers[1].name(), "Fallback");
    }

    #[test]
    fn returns_empty_for_no_match() {
        let registry = PriceProviderRegistry::new();
        let providers = registry.get_providers_for(&AssetType::Metal);
        assert!(providers.is_empty());
    }

    #[test]
    fn preserves_registration_order() {
        let mut registry = PriceProviderRegistry::new();
        registry.register(Box::new(MockProvider::new("A", vec![AssetType::Fiat])));
        registry.register(Box::new(MockProvider::new("B", vec![AssetType::Fiat])));
        registry.register(Box::new(MockProvider::new("C", vec![AssetType::Fiat])));

        let providers = registry.get_providers_for(&AssetType::Fiat);
        assert_eq!(providers.len(), 3);
        assert_eq!(providers[0].name(), "A");
        assert_eq!(providers[1].name(), "B");
        assert_eq!(providers[2].name(), "C");
    }

    #[test]
    fn single_provider_returns_vec_of_one() {
        let mut registry = PriceProviderRegistry::new();
        registry.register(Box::new(MockProvider::new(
            "Only",
            vec![AssetType::Metal],
        )));

        let providers = registry.get_providers_for(&AssetType::Metal);
        assert_eq!(providers.len(), 1);
        assert_eq!(providers[0].name(), "Only");
    }
}

// ═══════════════════════════════════════════════════════════════════
// PriceProviderRegistry — new_with_defaults
// ═══════════════════════════════════════════════════════════════════

mod registry_defaults {
    use super::*;

    #[test]
    fn without_api_keys_has_crypto_fiat_stock() {
        let keys = HashMap::new();
        let registry = PriceProviderRegistry::new_with_defaults(&keys);

        // CoinCap for crypto
        assert!(registry.get_provider_for(&AssetType::Crypto).is_some());
        // Frankfurter for fiat
        assert!(registry.get_provider_for(&AssetType::Fiat).is_some());
        // Yahoo Finance for stocks (no key needed)
        assert!(registry.get_provider_for(&AssetType::Stock).is_some());
        // No metals provider without API key
        assert!(registry.get_provider_for(&AssetType::Metal).is_none());
    }

    #[test]
    fn with_metals_key_has_metal_provider() {
        let mut keys = HashMap::new();
        keys.insert("metals_dev".to_string(), "test-key".to_string());
        let registry = PriceProviderRegistry::new_with_defaults(&keys);

        assert!(registry.get_provider_for(&AssetType::Metal).is_some());
    }

    #[test]
    fn with_alphavantage_key_has_stock_fallback() {
        let mut keys = HashMap::new();
        keys.insert("alphavantage".to_string(), "av-key".to_string());
        let registry = PriceProviderRegistry::new_with_defaults(&keys);

        // Should have Yahoo Finance (primary) + Alpha Vantage (fallback)
        let stock_providers = registry.get_providers_for(&AssetType::Stock);
        assert_eq!(stock_providers.len(), 2);
    }

    #[test]
    fn without_alphavantage_key_only_yahoo() {
        let keys = HashMap::new();
        let registry = PriceProviderRegistry::new_with_defaults(&keys);

        let stock_providers = registry.get_providers_for(&AssetType::Stock);
        assert_eq!(stock_providers.len(), 1);
        assert_eq!(stock_providers[0].name(), "Yahoo Finance");
    }

    #[test]
    fn with_all_keys() {
        let mut keys = HashMap::new();
        keys.insert("metals_dev".to_string(), "m-key".to_string());
        keys.insert("alphavantage".to_string(), "a-key".to_string());
        let registry = PriceProviderRegistry::new_with_defaults(&keys);

        assert!(registry.get_provider_for(&AssetType::Crypto).is_some());
        assert!(registry.get_provider_for(&AssetType::Fiat).is_some());
        assert!(registry.get_provider_for(&AssetType::Metal).is_some());
        assert!(registry.get_provider_for(&AssetType::Stock).is_some());
    }

    #[test]
    fn irrelevant_keys_ignored() {
        let mut keys = HashMap::new();
        keys.insert("unknown_provider".to_string(), "whatever".to_string());
        let registry = PriceProviderRegistry::new_with_defaults(&keys);

        // Without metals_dev or alphavantage, no metal or extra stock provider
        assert!(registry.get_provider_for(&AssetType::Metal).is_none());
        let stock_providers = registry.get_providers_for(&AssetType::Stock);
        assert_eq!(stock_providers.len(), 1);
    }

    #[test]
    fn provider_names_correct() {
        let keys = HashMap::new();
        let registry = PriceProviderRegistry::new_with_defaults(&keys);

        let crypto = registry.get_provider_for(&AssetType::Crypto).unwrap();
        assert_eq!(crypto.name(), "CoinCap");

        let fiat = registry.get_provider_for(&AssetType::Fiat).unwrap();
        assert_eq!(fiat.name(), "Frankfurter");

        let stock = registry.get_provider_for(&AssetType::Stock).unwrap();
        assert_eq!(stock.name(), "Yahoo Finance");
    }
}

// ═══════════════════════════════════════════════════════════════════
// CoinCapProvider — resolve_id and trait impl
// ═══════════════════════════════════════════════════════════════════

mod coincap {
    use super::*;

    #[test]
    fn name() {
        let provider = CoinCapProvider::new();
        assert_eq!(provider.name(), "CoinCap");
    }

    #[test]
    fn supported_types() {
        let provider = CoinCapProvider::new();
        let types = provider.supported_asset_types();
        assert_eq!(types, vec![AssetType::Crypto]);
    }

    #[test]
    fn default_trait() {
        let provider = CoinCapProvider::default();
        assert_eq!(provider.name(), "CoinCap");
    }

    #[test]
    fn resolve_id_btc() {
        let provider = CoinCapProvider::new();
        assert_eq!(provider.resolve_id("BTC"), "bitcoin");
    }

    #[test]
    fn resolve_id_eth() {
        let provider = CoinCapProvider::new();
        assert_eq!(provider.resolve_id("ETH"), "ethereum");
    }

    #[test]
    fn resolve_id_lowercase_input() {
        let provider = CoinCapProvider::new();
        assert_eq!(provider.resolve_id("btc"), "bitcoin");
    }

    #[test]
    fn resolve_id_mixed_case_input() {
        let provider = CoinCapProvider::new();
        assert_eq!(provider.resolve_id("Btc"), "bitcoin");
    }

    #[test]
    fn resolve_id_unknown_falls_back_to_lowercase() {
        let provider = CoinCapProvider::new();
        assert_eq!(provider.resolve_id("UNKNOWN"), "unknown");
    }

    #[test]
    fn resolve_id_all_common_symbols() {
        let provider = CoinCapProvider::new();
        let expected = vec![
            ("BTC", "bitcoin"),
            ("ETH", "ethereum"),
            ("USDT", "tether"),
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
        ];
        for (sym, id) in expected {
            assert_eq!(
                provider.resolve_id(sym),
                id,
                "Failed for symbol: {}",
                sym
            );
        }
    }
}

// ═══════════════════════════════════════════════════════════════════
// FrankfurterProvider
// ═══════════════════════════════════════════════════════════════════

mod frankfurter {
    use super::*;

    #[test]
    fn name() {
        let provider = FrankfurterProvider::new();
        assert_eq!(provider.name(), "Frankfurter");
    }

    #[test]
    fn supported_types() {
        let provider = FrankfurterProvider::new();
        let types = provider.supported_asset_types();
        assert_eq!(types, vec![AssetType::Fiat]);
    }

    #[test]
    fn default_trait() {
        let provider = FrankfurterProvider::default();
        assert_eq!(provider.name(), "Frankfurter");
    }
}

// ═══════════════════════════════════════════════════════════════════
// MetalsDevProvider — resolve_metal_name
// ═══════════════════════════════════════════════════════════════════

mod metals_dev {
    use super::*;

    fn make_provider() -> MetalsDevProvider {
        MetalsDevProvider::new("test-key".to_string())
    }

    #[test]
    fn name() {
        let provider = make_provider();
        assert_eq!(provider.name(), "metals.dev");
    }

    #[test]
    fn supported_types() {
        let provider = make_provider();
        let types = provider.supported_asset_types();
        assert_eq!(types, vec![AssetType::Metal]);
    }

    #[test]
    fn resolve_xau() {
        let provider = make_provider();
        assert_eq!(provider.resolve_metal_name("XAU").unwrap(), "gold");
    }

    #[test]
    fn resolve_xag() {
        let provider = make_provider();
        assert_eq!(provider.resolve_metal_name("XAG").unwrap(), "silver");
    }

    #[test]
    fn resolve_xpt() {
        let provider = make_provider();
        assert_eq!(provider.resolve_metal_name("XPT").unwrap(), "platinum");
    }

    #[test]
    fn resolve_xpd() {
        let provider = make_provider();
        assert_eq!(provider.resolve_metal_name("XPD").unwrap(), "palladium");
    }

    #[test]
    fn resolve_lowercase_input() {
        let provider = make_provider();
        assert_eq!(provider.resolve_metal_name("xau").unwrap(), "gold");
    }

    #[test]
    fn resolve_mixed_case_input() {
        let provider = make_provider();
        assert_eq!(provider.resolve_metal_name("Xau").unwrap(), "gold");
    }

    #[test]
    fn resolve_unknown_symbol_fails() {
        let provider = make_provider();
        let result = provider.resolve_metal_name("XYZ");
        assert!(result.is_err());
        match result.unwrap_err() {
            CoreError::Api { provider, message } => {
                assert_eq!(provider, "metals.dev");
                assert!(message.contains("Unknown metal symbol"));
                assert!(message.contains("XYZ"));
            }
            other => panic!("Expected Api error, got {:?}", other),
        }
    }

    #[test]
    fn resolve_empty_symbol_fails() {
        let provider = make_provider();
        assert!(provider.resolve_metal_name("").is_err());
    }
}

// ═══════════════════════════════════════════════════════════════════
// YahooFinanceProvider
// ═══════════════════════════════════════════════════════════════════

mod yahoo_finance {
    use super::*;

    #[test]
    fn name() {
        let provider = YahooFinanceProvider::new().unwrap();
        assert_eq!(provider.name(), "Yahoo Finance");
    }

    #[test]
    fn supported_types() {
        let provider = YahooFinanceProvider::new().unwrap();
        let types = provider.supported_asset_types();
        assert_eq!(types, vec![AssetType::Stock]);
    }
}

// ═══════════════════════════════════════════════════════════════════
// Provider trait compliance
// ═══════════════════════════════════════════════════════════════════

mod trait_compliance {
    use super::*;

    /// Verify all providers implement Send + Sync (required by async-trait).
    #[test]
    fn providers_are_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}

        assert_send_sync::<CoinCapProvider>();
        assert_send_sync::<FrankfurterProvider>();
        assert_send_sync::<MetalsDevProvider>();
        assert_send_sync::<YahooFinanceProvider>();
    }

    /// Verify providers can be stored as trait objects in the registry.
    #[test]
    fn providers_as_trait_objects() {
        let mut registry = PriceProviderRegistry::new();
        registry.register(Box::new(CoinCapProvider::new()));
        registry.register(Box::new(FrankfurterProvider::new()));
        registry.register(Box::new(MetalsDevProvider::new("k".into())));
        registry.register(Box::new(YahooFinanceProvider::new().unwrap()));

        assert!(registry.get_provider_for(&AssetType::Crypto).is_some());
        assert!(registry.get_provider_for(&AssetType::Fiat).is_some());
        assert!(registry.get_provider_for(&AssetType::Metal).is_some());
        assert!(registry.get_provider_for(&AssetType::Stock).is_some());
    }
}
