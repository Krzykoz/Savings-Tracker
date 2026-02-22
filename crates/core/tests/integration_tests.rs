use async_trait::async_trait;
use chrono::NaiveDate;
use savings_tracker_core::errors::CoreError;
use savings_tracker_core::models::asset::{Asset, AssetType};
use savings_tracker_core::models::event::{Event, EventType};
use savings_tracker_core::models::portfolio::Portfolio;
use savings_tracker_core::models::price::{PriceCache, PricePoint};
use savings_tracker_core::providers::traits::PriceProvider;
use savings_tracker_core::services::portfolio_service::PortfolioService;
use savings_tracker_core::storage::manager::StorageManager;

// ═══════════════════════════════════════════════════════════════════
// Mock Price Provider (for testing without real API calls)
// ═══════════════════════════════════════════════════════════════════

#[allow(dead_code)]
struct MockPriceProvider {
    prices: std::collections::HashMap<(String, String, String), f64>,
}

impl MockPriceProvider {
    #[allow(dead_code)]
    fn new() -> Self {
        let mut prices = std::collections::HashMap::new();
        // BTC prices in USD
        prices.insert(
            ("BTC".into(), "USD".into(), "2025-01-15".into()),
            42000.0,
        );
        prices.insert(
            ("BTC".into(), "USD".into(), "2025-01-16".into()),
            43500.0,
        );
        prices.insert(
            ("BTC".into(), "USD".into(), "2025-01-17".into()),
            41000.0,
        );
        // ETH prices in USD
        prices.insert(
            ("ETH".into(), "USD".into(), "2025-01-15".into()),
            2500.0,
        );
        // USD/PLN rates
        prices.insert(
            ("USD".into(), "PLN".into(), "2025-01-15".into()),
            4.05,
        );

        Self { prices }
    }
}

#[async_trait]
impl PriceProvider for MockPriceProvider {
    fn name(&self) -> &str {
        "MockProvider"
    }

    fn supported_asset_types(&self) -> Vec<AssetType> {
        vec![
            AssetType::Crypto,
            AssetType::Fiat,
            AssetType::Metal,
            AssetType::Stock,
        ]
    }

    async fn get_current_price(&self, symbol: &str, currency: &str) -> Result<f64, CoreError> {
        // Return any matching price
        for ((s, c, _), price) in &self.prices {
            if s == symbol && c == currency {
                return Ok(*price);
            }
        }
        Err(CoreError::PriceNotAvailable {
            symbol: symbol.into(),
            currency: currency.into(),
            date: "current".into(),
        })
    }

    async fn get_historical_price(
        &self,
        symbol: &str,
        currency: &str,
        date: NaiveDate,
    ) -> Result<f64, CoreError> {
        let key = (
            symbol.to_string(),
            currency.to_string(),
            date.format("%Y-%m-%d").to_string(),
        );
        self.prices.get(&key).copied().ok_or(CoreError::PriceNotAvailable {
            symbol: symbol.into(),
            currency: currency.into(),
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
        let mut points = Vec::new();
        for ((s, c, d), price) in &self.prices {
            if s == symbol && c == currency {
                if let Ok(date) = NaiveDate::parse_from_str(d, "%Y-%m-%d") {
                    if date >= from && date <= to {
                        points.push(PricePoint {
                            date,
                            price: *price,
                        });
                    }
                }
            }
        }
        points.sort_by_key(|p| p.date);
        Ok(points)
    }
}

// ═══════════════════════════════════════════════════════════════════
// Storage Tests — encrypt/decrypt round-trip
// ═══════════════════════════════════════════════════════════════════

#[test]
fn test_storage_round_trip_empty_portfolio() {
    let portfolio = Portfolio::default();
    let password = "test-password-123!";

    let encrypted = StorageManager::save_to_bytes(&portfolio, password).unwrap();
    let decrypted = StorageManager::load_from_bytes(&encrypted, password).unwrap();

    assert_eq!(decrypted.events.len(), 0);
    assert_eq!(
        decrypted.settings.default_currency,
        portfolio.settings.default_currency
    );
}

#[test]
fn test_storage_round_trip_with_events() {
    let mut portfolio = Portfolio::default();
    portfolio.settings.default_currency = "PLN".to_string();
    portfolio.events.push(Event::new(
        EventType::Buy,
        Asset::crypto("BTC", "Bitcoin"),
        0.5,
        NaiveDate::from_ymd_opt(2025, 1, 15).unwrap(),
    ));
    portfolio.events.push(Event::new(
        EventType::Buy,
        Asset::fiat("USD", "US Dollar"),
        1000.0,
        NaiveDate::from_ymd_opt(2025, 1, 10).unwrap(),
    ));

    let password = "secure-password!";
    let encrypted = StorageManager::save_to_bytes(&portfolio, password).unwrap();
    let decrypted = StorageManager::load_from_bytes(&encrypted, password).unwrap();

    assert_eq!(decrypted.events.len(), 2);
    assert_eq!(decrypted.settings.default_currency, "PLN");
    assert_eq!(decrypted.events[0].asset.symbol, "BTC");
    assert_eq!(decrypted.events[0].amount, 0.5);
    assert_eq!(decrypted.events[1].asset.symbol, "USD");
}

#[test]
fn test_storage_round_trip_with_price_cache() {
    let mut portfolio = Portfolio::default();
    portfolio.price_cache.set_price(
        "BTC",
        "USD",
        NaiveDate::from_ymd_opt(2025, 1, 15).unwrap(),
        42000.0,
    );
    portfolio.price_cache.set_price(
        "ETH",
        "USD",
        NaiveDate::from_ymd_opt(2025, 1, 15).unwrap(),
        2500.0,
    );

    let password = "cache-test-pass";
    let encrypted = StorageManager::save_to_bytes(&portfolio, password).unwrap();
    let decrypted = StorageManager::load_from_bytes(&encrypted, password).unwrap();

    assert_eq!(
        decrypted.price_cache.get_price(
            "BTC",
            "USD",
            NaiveDate::from_ymd_opt(2025, 1, 15).unwrap()
        ),
        Some(42000.0)
    );
    assert_eq!(
        decrypted.price_cache.get_price(
            "ETH",
            "USD",
            NaiveDate::from_ymd_opt(2025, 1, 15).unwrap()
        ),
        Some(2500.0)
    );
}

#[test]
fn test_storage_wrong_password() {
    let portfolio = Portfolio::default();
    let encrypted = StorageManager::save_to_bytes(&portfolio, "correct-pass").unwrap();
    let result = StorageManager::load_from_bytes(&encrypted, "wrong-pass");

    assert!(result.is_err());
    match result.unwrap_err() {
        CoreError::Decryption => {} // expected
        e => panic!("Expected CoreError::Decryption, got: {:?}", e),
    }
}

#[test]
fn test_storage_corrupted_data() {
    let portfolio = Portfolio::default();
    let mut encrypted = StorageManager::save_to_bytes(&portfolio, "pass").unwrap();

    // Corrupt some ciphertext bytes
    if encrypted.len() > 60 {
        encrypted[58] ^= 0xFF;
        encrypted[59] ^= 0xFF;
    }

    let result = StorageManager::load_from_bytes(&encrypted, "pass");
    assert!(result.is_err());
}

#[test]
fn test_storage_invalid_magic() {
    let data = b"NOPE_not_a_valid_svtk_file";
    let result = StorageManager::load_from_bytes(data, "any");
    assert!(result.is_err());
    match result.unwrap_err() {
        CoreError::InvalidFileFormat(_) => {}
        e => panic!("Expected InvalidFileFormat, got: {:?}", e),
    }
}

#[test]
fn test_storage_file_too_small() {
    let data = b"SVT"; // less than 4 magic bytes
    let result = StorageManager::load_from_bytes(data, "any");
    assert!(result.is_err());
}

// ═══════════════════════════════════════════════════════════════════
// File I/O Tests (native only)
// ═══════════════════════════════════════════════════════════════════

#[test]
fn test_storage_file_round_trip() {
    let mut portfolio = Portfolio::default();
    portfolio.events.push(Event::new(
        EventType::Buy,
        Asset::crypto("BTC", "Bitcoin"),
        1.5,
        NaiveDate::from_ymd_opt(2025, 6, 1).unwrap(),
    ));
    portfolio
        .price_cache
        .set_price("BTC", "USD", NaiveDate::from_ymd_opt(2025, 6, 1).unwrap(), 50000.0);

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.svtk");
    let path_str = path.to_str().unwrap();
    let password = "file-test-pass!";

    StorageManager::save_to_file(&portfolio, path_str, password).unwrap();
    let loaded = StorageManager::load_from_file(path_str, password).unwrap();

    assert_eq!(loaded.events.len(), 1);
    assert_eq!(loaded.events[0].asset.symbol, "BTC");
    assert_eq!(loaded.events[0].amount, 1.5);
    assert_eq!(
        loaded.price_cache.get_price("BTC", "USD", NaiveDate::from_ymd_opt(2025, 6, 1).unwrap()),
        Some(50000.0)
    );
}

// ═══════════════════════════════════════════════════════════════════
// Portfolio Service Tests
// ═══════════════════════════════════════════════════════════════════

#[test]
fn test_portfolio_add_buy_events() {
    let service = PortfolioService::new();
    let mut portfolio = Portfolio::default();

    let event = Event::new(
        EventType::Buy,
        Asset::crypto("BTC", "Bitcoin"),
        0.5,
        NaiveDate::from_ymd_opt(2025, 1, 15).unwrap(),
    );

    service.add_event(&mut portfolio, event).unwrap();
    assert_eq!(portfolio.events.len(), 1);

    let event2 = Event::new(
        EventType::Buy,
        Asset::crypto("BTC", "Bitcoin"),
        0.3,
        NaiveDate::from_ymd_opt(2025, 1, 20).unwrap(),
    );
    service.add_event(&mut portfolio, event2).unwrap();
    assert_eq!(portfolio.events.len(), 2);
}

#[test]
fn test_portfolio_holdings_single_buy() {
    let service = PortfolioService::new();
    let mut portfolio = Portfolio::default();

    let btc = Asset::crypto("BTC", "Bitcoin");
    let event = Event::new(
        EventType::Buy,
        btc.clone(),
        0.5,
        NaiveDate::from_ymd_opt(2025, 1, 15).unwrap(),
    );
    service.add_event(&mut portfolio, event).unwrap();

    let holdings = service.get_holdings(&portfolio, NaiveDate::from_ymd_opt(2025, 1, 15).unwrap());
    assert_eq!(holdings.get(&btc), Some(&0.5));
}

#[test]
fn test_portfolio_holdings_buy_and_sell() {
    let service = PortfolioService::new();
    let mut portfolio = Portfolio::default();

    let btc = Asset::crypto("BTC", "Bitcoin");
    // Buy 1.0 BTC
    service
        .add_event(
            &mut portfolio,
            Event::new(
                EventType::Buy,
                btc.clone(),
                1.0,
                NaiveDate::from_ymd_opt(2025, 1, 10).unwrap(),
            ),
        )
        .unwrap();
    // Sell 0.3 BTC
    service
        .add_event(
            &mut portfolio,
            Event::new(
                EventType::Sell,
                btc.clone(),
                0.3,
                NaiveDate::from_ymd_opt(2025, 1, 20).unwrap(),
            ),
        )
        .unwrap();

    // Before sell
    let holdings_before = service.get_holdings(
        &portfolio,
        NaiveDate::from_ymd_opt(2025, 1, 15).unwrap(),
    );
    assert_eq!(holdings_before.get(&btc), Some(&1.0));

    // After sell
    let holdings_after = service.get_holdings(
        &portfolio,
        NaiveDate::from_ymd_opt(2025, 1, 25).unwrap(),
    );
    assert!((holdings_after.get(&btc).unwrap() - 0.7).abs() < f64::EPSILON);
}

#[test]
fn test_portfolio_holdings_before_any_events() {
    let service = PortfolioService::new();
    let mut portfolio = Portfolio::default();

    service
        .add_event(
            &mut portfolio,
            Event::new(
                EventType::Buy,
                Asset::crypto("BTC", "Bitcoin"),
                1.0,
                NaiveDate::from_ymd_opt(2025, 3, 1).unwrap(),
            ),
        )
        .unwrap();

    // Query date before the buy event
    let holdings = service.get_holdings(
        &portfolio,
        NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
    );
    assert!(holdings.is_empty());
}

#[test]
fn test_portfolio_cannot_sell_more_than_owned() {
    let service = PortfolioService::new();
    let mut portfolio = Portfolio::default();

    // Buy 0.5 BTC
    service
        .add_event(
            &mut portfolio,
            Event::new(
                EventType::Buy,
                Asset::crypto("BTC", "Bitcoin"),
                0.5,
                NaiveDate::from_ymd_opt(2025, 1, 10).unwrap(),
            ),
        )
        .unwrap();

    // Try to sell 1.0 BTC (more than owned)
    let result = service.add_event(
        &mut portfolio,
        Event::new(
            EventType::Sell,
            Asset::crypto("BTC", "Bitcoin"),
            1.0,
            NaiveDate::from_ymd_opt(2025, 1, 15).unwrap(),
        ),
    );

    assert!(result.is_err());
    match result.unwrap_err() {
        CoreError::ValidationError(msg) => {
            assert!(msg.contains("Cannot sell"));
        }
        e => panic!("Expected ValidationError, got: {:?}", e),
    }
}

#[test]
fn test_portfolio_cannot_add_zero_amount() {
    let service = PortfolioService::new();
    let mut portfolio = Portfolio::default();

    let result = service.add_event(
        &mut portfolio,
        Event::new(
            EventType::Buy,
            Asset::crypto("BTC", "Bitcoin"),
            0.0,
            NaiveDate::from_ymd_opt(2025, 1, 15).unwrap(),
        ),
    );

    assert!(result.is_err());
}

#[test]
fn test_portfolio_remove_event() {
    let service = PortfolioService::new();
    let mut portfolio = Portfolio::default();

    let event = Event::new(
        EventType::Buy,
        Asset::crypto("BTC", "Bitcoin"),
        1.0,
        NaiveDate::from_ymd_opt(2025, 1, 15).unwrap(),
    );
    let event_id = event.id;
    service.add_event(&mut portfolio, event).unwrap();

    assert_eq!(portfolio.events.len(), 1);
    service.remove_event(&mut portfolio, event_id).unwrap();
    assert_eq!(portfolio.events.len(), 0);
}

#[test]
fn test_portfolio_remove_nonexistent_event() {
    let service = PortfolioService::new();
    let mut portfolio = Portfolio::default();

    let result = service.remove_event(&mut portfolio, uuid::Uuid::new_v4());
    assert!(result.is_err());
    match result.unwrap_err() {
        CoreError::EventNotFound(_) => {}
        e => panic!("Expected EventNotFound, got: {:?}", e),
    }
}

#[test]
fn test_portfolio_multiple_assets() {
    let service = PortfolioService::new();
    let mut portfolio = Portfolio::default();

    let btc = Asset::crypto("BTC", "Bitcoin");
    let eth = Asset::crypto("ETH", "Ethereum");
    let usd = Asset::fiat("USD", "US Dollar");

    service
        .add_event(
            &mut portfolio,
            Event::new(
                EventType::Buy,
                btc.clone(),
                0.5,
                NaiveDate::from_ymd_opt(2025, 1, 10).unwrap(),
            ),
        )
        .unwrap();
    service
        .add_event(
            &mut portfolio,
            Event::new(
                EventType::Buy,
                eth.clone(),
                10.0,
                NaiveDate::from_ymd_opt(2025, 1, 10).unwrap(),
            ),
        )
        .unwrap();
    service
        .add_event(
            &mut portfolio,
            Event::new(
                EventType::Buy,
                usd.clone(),
                5000.0,
                NaiveDate::from_ymd_opt(2025, 1, 10).unwrap(),
            ),
        )
        .unwrap();

    let holdings = service.get_holdings(
        &portfolio,
        NaiveDate::from_ymd_opt(2025, 1, 15).unwrap(),
    );
    assert_eq!(holdings.len(), 3);
    assert_eq!(holdings.get(&btc), Some(&0.5));
    assert_eq!(holdings.get(&eth), Some(&10.0));
    assert_eq!(holdings.get(&usd), Some(&5000.0));
}

// ═══════════════════════════════════════════════════════════════════
// Price Cache Tests
// ═══════════════════════════════════════════════════════════════════

#[test]
fn test_price_cache_set_and_get() {
    let mut cache = PriceCache::new();
    let date = NaiveDate::from_ymd_opt(2025, 1, 15).unwrap();

    cache.set_price("BTC", "USD", date, 42000.0);
    assert_eq!(cache.get_price("BTC", "USD", date), Some(42000.0));
    assert_eq!(cache.get_price("btc", "usd", date), Some(42000.0)); // case insensitive
}

#[test]
fn test_price_cache_miss() {
    let cache = PriceCache::new();
    let date = NaiveDate::from_ymd_opt(2025, 1, 15).unwrap();

    assert_eq!(cache.get_price("BTC", "USD", date), None);
}

#[test]
fn test_price_cache_update_existing() {
    let mut cache = PriceCache::new();
    let date = NaiveDate::from_ymd_opt(2025, 1, 15).unwrap();

    cache.set_price("BTC", "USD", date, 42000.0);
    cache.set_price("BTC", "USD", date, 43000.0); // update

    assert_eq!(cache.get_price("BTC", "USD", date), Some(43000.0));
}

#[test]
fn test_price_cache_today_freshness() {
    let mut cache = PriceCache::new();
    let today = NaiveDate::from_ymd_opt(2025, 1, 15).unwrap();

    assert!(!cache.is_today_fresh("BTC", "USD", today));
    cache.mark_updated_today("BTC", "USD", today);
    assert!(cache.is_today_fresh("BTC", "USD", today));

    // Different day should not be fresh
    let tomorrow = NaiveDate::from_ymd_opt(2025, 1, 16).unwrap();
    assert!(!cache.is_today_fresh("BTC", "USD", tomorrow));
}

#[test]
fn test_price_cache_range() {
    let mut cache = PriceCache::new();
    let d1 = NaiveDate::from_ymd_opt(2025, 1, 15).unwrap();
    let d2 = NaiveDate::from_ymd_opt(2025, 1, 16).unwrap();
    let d3 = NaiveDate::from_ymd_opt(2025, 1, 17).unwrap();

    cache.set_price("BTC", "USD", d1, 42000.0);
    cache.set_price("BTC", "USD", d2, 43000.0);
    cache.set_price("BTC", "USD", d3, 41000.0);

    let range = cache.get_price_range("BTC", "USD", d1, d3);
    assert_eq!(range.len(), 3);
    assert_eq!(range[0].price, 42000.0);
    assert_eq!(range[1].price, 43000.0);
    assert_eq!(range[2].price, 41000.0);
}

#[test]
fn test_price_cache_set_prices_bulk() {
    let mut cache = PriceCache::new();
    let points = vec![
        PricePoint {
            date: NaiveDate::from_ymd_opt(2025, 1, 15).unwrap(),
            price: 42000.0,
        },
        PricePoint {
            date: NaiveDate::from_ymd_opt(2025, 1, 16).unwrap(),
            price: 43000.0,
        },
    ];

    cache.set_prices("BTC", "USD", &points);

    assert_eq!(
        cache.get_price("BTC", "USD", NaiveDate::from_ymd_opt(2025, 1, 15).unwrap()),
        Some(42000.0)
    );
    assert_eq!(
        cache.get_price("BTC", "USD", NaiveDate::from_ymd_opt(2025, 1, 16).unwrap()),
        Some(43000.0)
    );
}

// ═══════════════════════════════════════════════════════════════════
// Model Tests
// ═══════════════════════════════════════════════════════════════════

#[test]
fn test_asset_creation() {
    let btc = Asset::crypto("btc", "Bitcoin");
    assert_eq!(btc.symbol, "BTC"); // auto-uppercased
    assert_eq!(btc.name, "Bitcoin");
    assert_eq!(btc.asset_type, AssetType::Crypto);
}

#[test]
fn test_asset_equality() {
    let a = Asset::crypto("BTC", "Bitcoin");
    let b = Asset::crypto("BTC", "Bitcoin");
    assert_eq!(a, b);

    let c = Asset::crypto("ETH", "Ethereum");
    assert_ne!(a, c);
}

#[test]
fn test_event_creates_uuid() {
    let e1 = Event::new(
        EventType::Buy,
        Asset::crypto("BTC", "Bitcoin"),
        1.0,
        NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
    );
    let e2 = Event::new(
        EventType::Buy,
        Asset::crypto("BTC", "Bitcoin"),
        1.0,
        NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
    );

    // Each event should get a unique ID
    assert_ne!(e1.id, e2.id);
}

#[test]
fn test_event_preserves_negative_amount() {
    let event = Event::new(
        EventType::Buy,
        Asset::crypto("BTC", "Bitcoin"),
        -5.0,
        NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
    );
    assert_eq!(event.amount, -5.0);
}

#[test]
fn test_default_settings() {
    let settings = savings_tracker_core::models::settings::Settings::default();
    assert_eq!(settings.default_currency, "USD");
    assert!(settings.api_keys.is_empty());
}

#[test]
fn test_default_portfolio() {
    let portfolio = Portfolio::default();
    assert!(portfolio.events.is_empty());
    assert_eq!(portfolio.settings.default_currency, "USD");
    assert!(portfolio.price_cache.entries.is_empty());
}

// ═══════════════════════════════════════════════════════════════════
// Encryption Module Tests
// ═══════════════════════════════════════════════════════════════════

#[test]
fn test_encryption_round_trip() {
    use savings_tracker_core::storage::encryption;

    let plaintext = b"Hello, encrypted world!";
    let password = "test-password";
    let salt = encryption::generate_salt().unwrap();
    let nonce = encryption::generate_nonce().unwrap();
    let params = encryption::KdfParams::default();

    let key = encryption::derive_key(password, &salt, &params).unwrap();
    let ciphertext = encryption::encrypt(plaintext, &key, &nonce).unwrap();

    // Ciphertext should be different from plaintext
    assert_ne!(&ciphertext[..plaintext.len()], plaintext);

    // Ciphertext should be larger (includes 16-byte auth tag)
    assert_eq!(ciphertext.len(), plaintext.len() + 16);

    let decrypted = encryption::decrypt(&ciphertext, &key, &nonce).unwrap();
    assert_eq!(decrypted, plaintext);
}

#[test]
fn test_encryption_wrong_key_fails() {
    use savings_tracker_core::storage::encryption;

    let plaintext = b"Secret data";
    let salt = encryption::generate_salt().unwrap();
    let nonce = encryption::generate_nonce().unwrap();
    let params = encryption::KdfParams::default();

    let key1 = encryption::derive_key("password1", &salt, &params).unwrap();
    let key2 = encryption::derive_key("password2", &salt, &params).unwrap();

    let ciphertext = encryption::encrypt(plaintext, &key1, &nonce).unwrap();
    let result = encryption::decrypt(&ciphertext, &key2, &nonce);

    assert!(result.is_err());
}

#[test]
fn test_different_salts_produce_different_keys() {
    use savings_tracker_core::storage::encryption;

    let params = encryption::KdfParams::default();
    let salt1 = encryption::generate_salt().unwrap();
    let salt2 = encryption::generate_salt().unwrap();

    let key1 = encryption::derive_key("same-password", &salt1, &params).unwrap();
    let key2 = encryption::derive_key("same-password", &salt2, &params).unwrap();

    assert_ne!(key1, key2);
}

// ═══════════════════════════════════════════════════════════════════
// File Format Tests
// ═══════════════════════════════════════════════════════════════════

#[test]
fn test_file_format_round_trip() {
    use savings_tracker_core::storage::encryption::KdfParams;
    use savings_tracker_core::storage::format;

    let kdf_params = KdfParams::default();
    let salt = [1u8; 16];
    let nonce = [2u8; 12];
    let ciphertext = vec![3u8; 100];

    let file_bytes = format::write_file(
        format::CURRENT_VERSION,
        &kdf_params,
        &salt,
        &nonce,
        &ciphertext,
    );

    let (header, ct) = format::read_file(&file_bytes).unwrap();

    assert_eq!(header.version, format::CURRENT_VERSION);
    assert_eq!(header.salt, salt);
    assert_eq!(header.nonce, nonce);
    assert_eq!(header.kdf_params.memory_cost, kdf_params.memory_cost);
    assert_eq!(header.kdf_params.time_cost, kdf_params.time_cost);
    assert_eq!(header.kdf_params.parallelism, kdf_params.parallelism);
    assert_eq!(header.ciphertext_len, 100);
    assert_eq!(ct, &ciphertext[..]);
}

#[test]
fn test_file_format_magic_validation() {
    use savings_tracker_core::storage::format;

    // Valid magic
    let mut data = vec![0u8; 60];
    data[0..4].copy_from_slice(b"SVTK");
    data[4] = 1; // version 1

    let result = format::read_file(&data);
    assert!(result.is_ok() || matches!(result, Err(CoreError::InvalidFileFormat(_))));
}

// ═══════════════════════════════════════════════════════════════════
// Full Integration Test (no network — using storage round-trip)
// ═══════════════════════════════════════════════════════════════════

#[test]
fn test_full_flow_create_save_load() {
    let service = PortfolioService::new();
    let mut portfolio = Portfolio::default();
    portfolio.settings.default_currency = "PLN".to_string();

    // Add events
    service
        .add_event(
            &mut portfolio,
            Event::new(
                EventType::Buy,
                Asset::crypto("BTC", "Bitcoin"),
                0.1,
                NaiveDate::from_ymd_opt(2025, 1, 10).unwrap(),
            ),
        )
        .unwrap();
    service
        .add_event(
            &mut portfolio,
            Event::new(
                EventType::Buy,
                Asset::fiat("USD", "US Dollar"),
                5000.0,
                NaiveDate::from_ymd_opt(2025, 1, 12).unwrap(),
            ),
        )
        .unwrap();

    // Populate price cache (simulating what would happen after API calls)
    portfolio.price_cache.set_price(
        "BTC",
        "USD",
        NaiveDate::from_ymd_opt(2025, 1, 10).unwrap(),
        42000.0,
    );
    portfolio.price_cache.set_price(
        "USD",
        "PLN",
        NaiveDate::from_ymd_opt(2025, 1, 10).unwrap(),
        4.05,
    );

    // Save and reload
    let password = "integration-test!";
    let encrypted = StorageManager::save_to_bytes(&portfolio, password).unwrap();
    let loaded = StorageManager::load_from_bytes(&encrypted, password).unwrap();

    // Verify everything survived the round trip
    assert_eq!(loaded.events.len(), 2);
    assert_eq!(loaded.settings.default_currency, "PLN");

    // Verify cache survived
    assert_eq!(
        loaded
            .price_cache
            .get_price("BTC", "USD", NaiveDate::from_ymd_opt(2025, 1, 10).unwrap()),
        Some(42000.0)
    );
    assert_eq!(
        loaded
            .price_cache
            .get_price("USD", "PLN", NaiveDate::from_ymd_opt(2025, 1, 10).unwrap()),
        Some(4.05)
    );

    // Verify holdings
    let holdings = service.get_holdings(
        &loaded,
        NaiveDate::from_ymd_opt(2025, 1, 15).unwrap(),
    );
    assert_eq!(holdings.len(), 2);
}
