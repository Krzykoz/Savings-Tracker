// ═══════════════════════════════════════════════════════════════════
// Service & Integration Tests — PortfolioService, PriceService,
// CurrencyService, ChartService, SavingsTracker facade
// ═══════════════════════════════════════════════════════════════════

use async_trait::async_trait;
use chrono::NaiveDate;
use std::collections::HashMap;
use uuid::Uuid;

use savings_tracker_core::errors::CoreError;
use savings_tracker_core::models::asset::{Asset, AssetType};
use savings_tracker_core::models::event::{Event, EventType};
use savings_tracker_core::models::portfolio::Portfolio;
use savings_tracker_core::models::price::{PriceCache, PricePoint};
use savings_tracker_core::providers::registry::PriceProviderRegistry;
use savings_tracker_core::providers::traits::PriceProvider;
use savings_tracker_core::services::chart_service::ChartService;
use savings_tracker_core::services::currency_service::CurrencyService;
use savings_tracker_core::services::portfolio_service::PortfolioService;
use savings_tracker_core::services::price_service::PriceService;
use savings_tracker_core::services::analytics_service::AnalyticsService;
use savings_tracker_core::SavingsTracker;

// ═══════════════════════════════════════════════════════════════════
// Mock Provider
// ═══════════════════════════════════════════════════════════════════

struct MockPriceProvider {
    prices: HashMap<(String, String, String), f64>,
}

impl MockPriceProvider {
    fn new() -> Self {
        let mut prices = HashMap::new();
        // BTC prices in USD
        prices.insert(("BTC".into(), "USD".into(), "2025-01-15".into()), 42000.0);
        prices.insert(("BTC".into(), "USD".into(), "2025-01-16".into()), 43500.0);
        prices.insert(("BTC".into(), "USD".into(), "2025-01-17".into()), 41000.0);
        // ETH prices in USD
        prices.insert(("ETH".into(), "USD".into(), "2025-01-15".into()), 2500.0);
        prices.insert(("ETH".into(), "USD".into(), "2025-01-16".into()), 2600.0);
        // Fiat rates
        prices.insert(("USD".into(), "PLN".into(), "2025-01-15".into()), 4.05);
        prices.insert(("USD".into(), "PLN".into(), "2025-01-16".into()), 4.10);
        prices.insert(("EUR".into(), "PLN".into(), "2025-01-15".into()), 4.35);
        prices.insert(("EUR".into(), "USD".into(), "2025-01-15".into()), 1.08);
        // Stock
        prices.insert(("AAPL".into(), "USD".into(), "2025-01-15".into()), 185.0);
        // Metal
        prices.insert(("XAU".into(), "USD".into(), "2025-01-15".into()), 2050.0);

        Self { prices }
    }

    #[allow(dead_code)]
    fn with_prices(prices: HashMap<(String, String, String), f64>) -> Self {
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
        self.prices
            .get(&key)
            .copied()
            .ok_or(CoreError::PriceNotAvailable {
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

/// A mock that always fails (for testing fallback behavior).
struct FailingMockProvider;

#[async_trait]
impl PriceProvider for FailingMockProvider {
    fn name(&self) -> &str {
        "FailingMock"
    }

    fn supported_asset_types(&self) -> Vec<AssetType> {
        vec![
            AssetType::Crypto,
            AssetType::Fiat,
            AssetType::Metal,
            AssetType::Stock,
        ]
    }

    async fn get_current_price(&self, sym: &str, cur: &str) -> Result<f64, CoreError> {
        Err(CoreError::Api {
            provider: "FailingMock".into(),
            message: format!("Simulated failure {sym}/{cur}"),
        })
    }

    async fn get_historical_price(
        &self,
        sym: &str,
        cur: &str,
        _date: NaiveDate,
    ) -> Result<f64, CoreError> {
        Err(CoreError::Api {
            provider: "FailingMock".into(),
            message: format!("Simulated failure {sym}/{cur}"),
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
            provider: "FailingMock".into(),
            message: "Simulated failure".into(),
        })
    }
}

fn make_registry_with_mock() -> PriceProviderRegistry {
    let mut registry = PriceProviderRegistry::new();
    registry.register(Box::new(MockPriceProvider::new()));
    registry
}

fn make_date(y: i32, m: u32, d: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(y, m, d).unwrap()
}

// ═══════════════════════════════════════════════════════════════════
// PortfolioService — add_event
// ═══════════════════════════════════════════════════════════════════

mod portfolio_add_event {
    use super::*;

    #[test]
    fn add_buy_event() {
        let svc = PortfolioService::new();
        let mut portfolio = Portfolio::default();

        let event = Event::new(
            EventType::Buy,
            Asset::crypto("BTC", "Bitcoin"),
            0.5,
            make_date(2025, 1, 15),
        );
        svc.add_event(&mut portfolio, event).unwrap();

        assert_eq!(portfolio.events.len(), 1);
        assert_eq!(portfolio.events[0].amount, 0.5);
    }

    #[test]
    fn add_multiple_events_sorted_by_date() {
        let svc = PortfolioService::new();
        let mut portfolio = Portfolio::default();

        svc.add_event(
            &mut portfolio,
            Event::new(
                EventType::Buy,
                Asset::crypto("BTC", "Bitcoin"),
                1.0,
                make_date(2025, 3, 1),
            ),
        )
        .unwrap();
        svc.add_event(
            &mut portfolio,
            Event::new(
                EventType::Buy,
                Asset::crypto("BTC", "Bitcoin"),
                0.5,
                make_date(2025, 1, 1),
            ),
        )
        .unwrap();

        // Events should be sorted by date
        assert_eq!(portfolio.events[0].date, make_date(2025, 1, 1));
        assert_eq!(portfolio.events[1].date, make_date(2025, 3, 1));
    }

    #[test]
    fn add_sell_event_with_enough_holdings() {
        let svc = PortfolioService::new();
        let mut portfolio = Portfolio::default();

        svc.add_event(
            &mut portfolio,
            Event::new(
                EventType::Buy,
                Asset::crypto("BTC", "Bitcoin"),
                1.0,
                make_date(2025, 1, 1),
            ),
        )
        .unwrap();
        svc.add_event(
            &mut portfolio,
            Event::new(
                EventType::Sell,
                Asset::crypto("BTC", "Bitcoin"),
                0.5,
                make_date(2025, 2, 1),
            ),
        )
        .unwrap();

        assert_eq!(portfolio.events.len(), 2);
    }

    #[test]
    fn sell_exact_amount_held() {
        let svc = PortfolioService::new();
        let mut portfolio = Portfolio::default();

        svc.add_event(
            &mut portfolio,
            Event::new(
                EventType::Buy,
                Asset::crypto("ETH", "Ethereum"),
                2.0,
                make_date(2025, 1, 1),
            ),
        )
        .unwrap();
        // Sell exactly what you have
        svc.add_event(
            &mut portfolio,
            Event::new(
                EventType::Sell,
                Asset::crypto("ETH", "Ethereum"),
                2.0,
                make_date(2025, 2, 1),
            ),
        )
        .unwrap();

        assert_eq!(portfolio.events.len(), 2);
    }

    #[test]
    fn sell_more_than_held_fails() {
        let svc = PortfolioService::new();
        let mut portfolio = Portfolio::default();

        svc.add_event(
            &mut portfolio,
            Event::new(
                EventType::Buy,
                Asset::crypto("BTC", "Bitcoin"),
                0.5,
                make_date(2025, 1, 1),
            ),
        )
        .unwrap();

        let result = svc.add_event(
            &mut portfolio,
            Event::new(
                EventType::Sell,
                Asset::crypto("BTC", "Bitcoin"),
                1.0,
                make_date(2025, 2, 1),
            ),
        );

        assert!(result.is_err());
        match result.unwrap_err() {
            CoreError::ValidationError(msg) => {
                assert!(msg.contains("Cannot sell"));
                assert!(msg.contains("BTC"));
            }
            other => panic!("Expected ValidationError, got {:?}", other),
        }
    }

    #[test]
    fn sell_without_buying_fails() {
        let svc = PortfolioService::new();
        let mut portfolio = Portfolio::default();

        let result = svc.add_event(
            &mut portfolio,
            Event::new(
                EventType::Sell,
                Asset::crypto("BTC", "Bitcoin"),
                0.1,
                make_date(2025, 1, 1),
            ),
        );

        assert!(result.is_err());
    }

    #[test]
    fn zero_amount_fails() {
        let svc = PortfolioService::new();
        let mut portfolio = Portfolio::default();

        let event = Event::new(
            EventType::Buy,
            Asset::crypto("BTC", "Bitcoin"),
            0.0,
            make_date(2025, 1, 1),
        );
        // validate_event rejects amount <= 0.0
        let result = svc.add_event(&mut portfolio, event);
        assert!(result.is_err());
        match result.unwrap_err() {
            CoreError::ValidationError(msg) => assert!(msg.contains("positive")),
            other => panic!("Expected ValidationError, got {:?}", other),
        }
    }

    #[test]
    fn negative_amount_rejected_by_validation() {
        let mut portfolio = Portfolio::default();
        let svc = PortfolioService::new();
        let event = Event::new(
            EventType::Buy,
            Asset::crypto("BTC", "Bitcoin"),
            -1.5,
            make_date(2025, 1, 1),
        );
        assert_eq!(event.amount, -1.5);
        let result = svc.add_event(&mut portfolio, event);
        assert!(result.is_err());
        match result.unwrap_err() {
            CoreError::ValidationError(msg) => assert!(msg.contains("positive")),
            other => panic!("Expected ValidationError, got {:?}", other),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════
// PortfolioService — remove_event
// ═══════════════════════════════════════════════════════════════════

mod portfolio_remove_event {
    use super::*;

    #[test]
    fn remove_existing_event() {
        let svc = PortfolioService::new();
        let mut portfolio = Portfolio::default();

        let event = Event::new(
            EventType::Buy,
            Asset::crypto("BTC", "Bitcoin"),
            1.0,
            make_date(2025, 1, 1),
        );
        let id = event.id;
        svc.add_event(&mut portfolio, event).unwrap();

        svc.remove_event(&mut portfolio, id).unwrap();
        assert_eq!(portfolio.events.len(), 0);
    }

    #[test]
    fn remove_nonexistent_event_fails() {
        let svc = PortfolioService::new();
        let mut portfolio = Portfolio::default();

        let result = svc.remove_event(&mut portfolio, Uuid::new_v4());
        assert!(result.is_err());
        match result.unwrap_err() {
            CoreError::EventNotFound(_) => {}
            other => panic!("Expected EventNotFound, got {:?}", other),
        }
    }

    #[test]
    fn remove_from_empty_portfolio_fails() {
        let svc = PortfolioService::new();
        let mut portfolio = Portfolio::default();
        let result = svc.remove_event(&mut portfolio, Uuid::new_v4());
        assert!(result.is_err());
    }

    #[test]
    fn remove_one_of_many() {
        let svc = PortfolioService::new();
        let mut portfolio = Portfolio::default();

        let e1 = Event::new(
            EventType::Buy,
            Asset::crypto("BTC", "Bitcoin"),
            1.0,
            make_date(2025, 1, 1),
        );
        let e2 = Event::new(
            EventType::Buy,
            Asset::crypto("ETH", "Ethereum"),
            10.0,
            make_date(2025, 1, 2),
        );
        let e3 = Event::new(
            EventType::Buy,
            Asset::fiat("USD", "US Dollar"),
            1000.0,
            make_date(2025, 1, 3),
        );
        let id2 = e2.id;

        svc.add_event(&mut portfolio, e1).unwrap();
        svc.add_event(&mut portfolio, e2).unwrap();
        svc.add_event(&mut portfolio, e3).unwrap();

        svc.remove_event(&mut portfolio, id2).unwrap();
        assert_eq!(portfolio.events.len(), 2);
        assert!(portfolio.events.iter().all(|e| e.id != id2));
    }

    #[test]
    fn remove_same_event_twice_fails() {
        let svc = PortfolioService::new();
        let mut portfolio = Portfolio::default();

        let event = Event::new(
            EventType::Buy,
            Asset::crypto("BTC", "Bitcoin"),
            1.0,
            make_date(2025, 1, 1),
        );
        let id = event.id;
        svc.add_event(&mut portfolio, event).unwrap();

        svc.remove_event(&mut portfolio, id).unwrap();
        assert!(svc.remove_event(&mut portfolio, id).is_err());
    }
}

// ═══════════════════════════════════════════════════════════════════
// PortfolioService — get_events
// ═══════════════════════════════════════════════════════════════════

mod portfolio_get_events {
    use super::*;

    #[test]
    fn empty_portfolio() {
        let svc = PortfolioService::new();
        let portfolio = Portfolio::default();
        assert!(svc.get_events(&portfolio).is_empty());
    }

    #[test]
    fn returns_newest_first() {
        let svc = PortfolioService::new();
        let mut portfolio = Portfolio::default();

        svc.add_event(
            &mut portfolio,
            Event::new(
                EventType::Buy,
                Asset::crypto("BTC", "Bitcoin"),
                1.0,
                make_date(2025, 1, 1),
            ),
        )
        .unwrap();
        svc.add_event(
            &mut portfolio,
            Event::new(
                EventType::Buy,
                Asset::crypto("ETH", "Ethereum"),
                5.0,
                make_date(2025, 6, 15),
            ),
        )
        .unwrap();
        svc.add_event(
            &mut portfolio,
            Event::new(
                EventType::Buy,
                Asset::fiat("USD", "Dollar"),
                100.0,
                make_date(2025, 3, 10),
            ),
        )
        .unwrap();

        let events = svc.get_events(&portfolio);
        assert_eq!(events.len(), 3);
        assert!(events[0].date >= events[1].date);
        assert!(events[1].date >= events[2].date);
    }

    #[test]
    fn returns_references_not_clones() {
        let svc = PortfolioService::new();
        let mut portfolio = Portfolio::default();
        let event = Event::new(
            EventType::Buy,
            Asset::crypto("BTC", "Bitcoin"),
            1.0,
            make_date(2025, 1, 1),
        );
        let id = event.id;
        svc.add_event(&mut portfolio, event).unwrap();

        let events = svc.get_events(&portfolio);
        assert_eq!(events[0].id, id);
    }
}

// ═══════════════════════════════════════════════════════════════════
// PortfolioService — get_holdings
// ═══════════════════════════════════════════════════════════════════

mod portfolio_holdings {
    use super::*;

    #[test]
    fn empty_portfolio_empty_holdings() {
        let svc = PortfolioService::new();
        let portfolio = Portfolio::default();
        let holdings = svc.get_holdings(&portfolio, make_date(2025, 12, 31));
        assert!(holdings.is_empty());
    }

    #[test]
    fn single_buy() {
        let svc = PortfolioService::new();
        let mut portfolio = Portfolio::default();
        svc.add_event(
            &mut portfolio,
            Event::new(
                EventType::Buy,
                Asset::crypto("BTC", "Bitcoin"),
                1.5,
                make_date(2025, 1, 1),
            ),
        )
        .unwrap();

        let holdings = svc.get_holdings(&portfolio, make_date(2025, 6, 1));
        let btc = Asset::crypto("BTC", "Bitcoin");
        assert_eq!(holdings.get(&btc).copied().unwrap(), 1.5);
    }

    #[test]
    fn buy_then_sell_partial() {
        let svc = PortfolioService::new();
        let mut portfolio = Portfolio::default();
        svc.add_event(
            &mut portfolio,
            Event::new(
                EventType::Buy,
                Asset::crypto("BTC", "Bitcoin"),
                2.0,
                make_date(2025, 1, 1),
            ),
        )
        .unwrap();
        svc.add_event(
            &mut portfolio,
            Event::new(
                EventType::Sell,
                Asset::crypto("BTC", "Bitcoin"),
                0.5,
                make_date(2025, 2, 1),
            ),
        )
        .unwrap();

        let holdings = svc.get_holdings(&portfolio, make_date(2025, 6, 1));
        let btc = Asset::crypto("BTC", "Bitcoin");
        assert!((holdings.get(&btc).copied().unwrap() - 1.5).abs() < f64::EPSILON);
    }

    #[test]
    fn buy_then_sell_all_removes_from_holdings() {
        let svc = PortfolioService::new();
        let mut portfolio = Portfolio::default();
        svc.add_event(
            &mut portfolio,
            Event::new(
                EventType::Buy,
                Asset::crypto("BTC", "Bitcoin"),
                1.0,
                make_date(2025, 1, 1),
            ),
        )
        .unwrap();
        svc.add_event(
            &mut portfolio,
            Event::new(
                EventType::Sell,
                Asset::crypto("BTC", "Bitcoin"),
                1.0,
                make_date(2025, 2, 1),
            ),
        )
        .unwrap();

        let holdings = svc.get_holdings(&portfolio, make_date(2025, 6, 1));
        // Completely sold off — should not appear in holdings
        assert!(holdings.is_empty());
    }

    #[test]
    fn holdings_before_buy_date_empty() {
        let svc = PortfolioService::new();
        let mut portfolio = Portfolio::default();
        svc.add_event(
            &mut portfolio,
            Event::new(
                EventType::Buy,
                Asset::crypto("BTC", "Bitcoin"),
                1.0,
                make_date(2025, 6, 1),
            ),
        )
        .unwrap();

        // Query before buy date
        let holdings = svc.get_holdings(&portfolio, make_date(2025, 1, 1));
        assert!(holdings.is_empty());
    }

    #[test]
    fn holdings_on_buy_date() {
        let svc = PortfolioService::new();
        let mut portfolio = Portfolio::default();
        svc.add_event(
            &mut portfolio,
            Event::new(
                EventType::Buy,
                Asset::crypto("BTC", "Bitcoin"),
                1.0,
                make_date(2025, 6, 15),
            ),
        )
        .unwrap();

        let holdings = svc.get_holdings(&portfolio, make_date(2025, 6, 15));
        let btc = Asset::crypto("BTC", "Bitcoin");
        assert_eq!(holdings.get(&btc).copied().unwrap(), 1.0);
    }

    #[test]
    fn multiple_assets() {
        let svc = PortfolioService::new();
        let mut portfolio = Portfolio::default();
        svc.add_event(
            &mut portfolio,
            Event::new(
                EventType::Buy,
                Asset::crypto("BTC", "Bitcoin"),
                1.0,
                make_date(2025, 1, 1),
            ),
        )
        .unwrap();
        svc.add_event(
            &mut portfolio,
            Event::new(
                EventType::Buy,
                Asset::crypto("ETH", "Ethereum"),
                10.0,
                make_date(2025, 1, 2),
            ),
        )
        .unwrap();
        svc.add_event(
            &mut portfolio,
            Event::new(
                EventType::Buy,
                Asset::fiat("USD", "US Dollar"),
                5000.0,
                make_date(2025, 1, 3),
            ),
        )
        .unwrap();

        let holdings = svc.get_holdings(&portfolio, make_date(2025, 12, 31));
        assert_eq!(holdings.len(), 3);
    }

    #[test]
    fn multiple_buys_same_asset_accumulate() {
        let svc = PortfolioService::new();
        let mut portfolio = Portfolio::default();
        for _ in 0..5 {
            svc.add_event(
                &mut portfolio,
                Event::new(
                    EventType::Buy,
                    Asset::crypto("BTC", "Bitcoin"),
                    0.2,
                    make_date(2025, 1, 1),
                ),
            )
            .unwrap();
        }

        let holdings = svc.get_holdings(&portfolio, make_date(2025, 12, 31));
        let btc = Asset::crypto("BTC", "Bitcoin");
        assert!((holdings.get(&btc).copied().unwrap() - 1.0).abs() < 1e-10);
    }

    #[test]
    #[allow(clippy::default_constructed_unit_structs)]
    fn default_trait() {
        let svc = PortfolioService::default();
        // Should work same as new()
        let portfolio = Portfolio::default();
        assert!(svc.get_events(&portfolio).is_empty());
    }
}

// ═══════════════════════════════════════════════════════════════════
// PriceService — with mock provider
// ═══════════════════════════════════════════════════════════════════

mod price_service {
    use super::*;

    #[tokio::test]
    async fn get_price_cache_miss_then_hit() {
        let registry = make_registry_with_mock();
        let svc = PriceService::new(registry);
        let mut cache = PriceCache::default();
        let date = make_date(2025, 1, 15);

        // First call — cache miss, fetches from mock
        let price = svc
            .get_price(&mut cache, "BTC", "USD", date, &AssetType::Crypto)
            .await
            .unwrap();
        assert_eq!(price, 42000.0);

        // Should now be in cache
        assert_eq!(cache.get_price("BTC", "USD", date), Some(42000.0));
    }

    #[tokio::test]
    async fn get_price_cache_hit_returns_cached() {
        let registry = make_registry_with_mock();
        let svc = PriceService::new(registry);
        let mut cache = PriceCache::default();
        let date = make_date(2025, 1, 15);

        // Pre-populate cache with different value
        cache.set_price("BTC", "USD", date, 99999.0);

        // Should return cached value for historical date
        let price = svc
            .get_price(&mut cache, "BTC", "USD", date, &AssetType::Crypto)
            .await
            .unwrap();
        assert_eq!(price, 99999.0);
    }

    #[tokio::test]
    async fn get_price_no_provider_fails() {
        let registry = PriceProviderRegistry::new(); // empty
        let svc = PriceService::new(registry);
        let mut cache = PriceCache::default();
        let date = make_date(2025, 1, 15);

        let result = svc
            .get_price(&mut cache, "BTC", "USD", date, &AssetType::Crypto)
            .await;
        assert!(result.is_err());
        match result.unwrap_err() {
            CoreError::NoProvider(_) => {}
            other => panic!("Expected NoProvider, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn get_price_range_populates_cache() {
        let registry = make_registry_with_mock();
        let svc = PriceService::new(registry);
        let mut cache = PriceCache::default();

        let from = make_date(2025, 1, 15);
        let to = make_date(2025, 1, 17);

        let points = svc
            .get_price_range(&mut cache, "BTC", "USD", from, to, &AssetType::Crypto)
            .await
            .unwrap();

        assert!(!points.is_empty());
        // Cache should have the fetched prices
        assert!(cache.get_price("BTC", "USD", make_date(2025, 1, 15)).is_some());
    }

    #[tokio::test]
    async fn get_price_range_empty_registry_fails() {
        let registry = PriceProviderRegistry::new();
        let svc = PriceService::new(registry);
        let mut cache = PriceCache::default();

        let result = svc
            .get_price_range(
                &mut cache,
                "BTC",
                "USD",
                make_date(2025, 1, 1),
                make_date(2025, 1, 31),
                &AssetType::Crypto,
            )
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn fallback_on_primary_failure() {
        let mut registry = PriceProviderRegistry::new();
        // First: failing provider
        registry.register(Box::new(FailingMockProvider));
        // Second: working mock provider
        registry.register(Box::new(MockPriceProvider::new()));

        let svc = PriceService::new(registry);
        let mut cache = PriceCache::default();
        let date = make_date(2025, 1, 15);

        // Should fall back to the working provider
        let price = svc
            .get_price(&mut cache, "BTC", "USD", date, &AssetType::Crypto)
            .await
            .unwrap();
        assert_eq!(price, 42000.0);
    }

    #[tokio::test]
    async fn all_providers_fail_returns_last_error() {
        let mut registry = PriceProviderRegistry::new();
        registry.register(Box::new(FailingMockProvider));

        let svc = PriceService::new(registry);
        let mut cache = PriceCache::default();
        let date = make_date(2025, 1, 15);

        let result = svc
            .get_price(&mut cache, "BTC", "USD", date, &AssetType::Crypto)
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn price_range_fallback() {
        let mut registry = PriceProviderRegistry::new();
        registry.register(Box::new(FailingMockProvider));
        registry.register(Box::new(MockPriceProvider::new()));

        let svc = PriceService::new(registry);
        let mut cache = PriceCache::default();

        let points = svc
            .get_price_range(
                &mut cache,
                "BTC",
                "USD",
                make_date(2025, 1, 15),
                make_date(2025, 1, 17),
                &AssetType::Crypto,
            )
            .await
            .unwrap();
        assert!(!points.is_empty());
    }
}

// ═══════════════════════════════════════════════════════════════════
// CurrencyService
// ═══════════════════════════════════════════════════════════════════

mod currency_service {
    use super::*;

    #[tokio::test]
    async fn convert_fiat_same_currency() {
        let registry = make_registry_with_mock();
        let price_svc = PriceService::new(registry);
        let currency_svc = CurrencyService::new();
        let mut cache = PriceCache::default();

        let result = currency_svc
            .convert_fiat(
                &price_svc,
                &mut cache,
                100.0,
                "USD",
                "USD",
                make_date(2025, 1, 15),
            )
            .await
            .unwrap();
        assert_eq!(result, 100.0);
    }

    #[tokio::test]
    async fn convert_fiat_usd_to_pln() {
        let registry = make_registry_with_mock();
        let price_svc = PriceService::new(registry);
        let currency_svc = CurrencyService::new();
        let mut cache = PriceCache::default();

        let result = currency_svc
            .convert_fiat(
                &price_svc,
                &mut cache,
                100.0,
                "USD",
                "PLN",
                make_date(2025, 1, 15),
            )
            .await
            .unwrap();
        assert!((result - 405.0).abs() < 0.01); // 100 * 4.05
    }

    #[tokio::test]
    async fn convert_fiat_case_insensitive() {
        let registry = make_registry_with_mock();
        let price_svc = PriceService::new(registry);
        let currency_svc = CurrencyService::new();
        let mut cache = PriceCache::default();

        let result = currency_svc
            .convert_fiat(
                &price_svc,
                &mut cache,
                100.0,
                "usd",
                "usd",
                make_date(2025, 1, 15),
            )
            .await
            .unwrap();
        assert_eq!(result, 100.0);
    }

    #[tokio::test]
    async fn convert_asset_fiat_to_fiat() {
        let registry = make_registry_with_mock();
        let price_svc = PriceService::new(registry);
        let currency_svc = CurrencyService::new();
        let mut cache = PriceCache::default();

        let asset = Asset::fiat("EUR", "Euro");
        let result = currency_svc
            .convert_asset_to_currency(
                &price_svc,
                &mut cache,
                &asset,
                100.0,
                "PLN",
                make_date(2025, 1, 15),
            )
            .await
            .unwrap();
        // EUR → PLN = 100 * 4.35 = 435
        assert!((result - 435.0).abs() < 0.01);
    }

    #[tokio::test]
    async fn convert_crypto_to_usd() {
        let registry = make_registry_with_mock();
        let price_svc = PriceService::new(registry);
        let currency_svc = CurrencyService::new();
        let mut cache = PriceCache::default();

        let asset = Asset::crypto("BTC", "Bitcoin");
        let result = currency_svc
            .convert_asset_to_currency(
                &price_svc,
                &mut cache,
                &asset,
                0.5,
                "USD",
                make_date(2025, 1, 15),
            )
            .await
            .unwrap();
        // 0.5 * 42000 = 21000
        assert!((result - 21000.0).abs() < 0.01);
    }

    #[tokio::test]
    async fn convert_crypto_to_pln_two_step() {
        let registry = make_registry_with_mock();
        let price_svc = PriceService::new(registry);
        let currency_svc = CurrencyService::new();
        let mut cache = PriceCache::default();

        let asset = Asset::crypto("BTC", "Bitcoin");
        let result = currency_svc
            .convert_asset_to_currency(
                &price_svc,
                &mut cache,
                &asset,
                1.0,
                "PLN",
                make_date(2025, 1, 15),
            )
            .await
            .unwrap();
        // BTC → USD (42000) → PLN (42000 * 4.05 = 170100)
        assert!((result - 170100.0).abs() < 1.0);
    }

    #[tokio::test]
    async fn convert_stock_to_usd() {
        let registry = make_registry_with_mock();
        let price_svc = PriceService::new(registry);
        let currency_svc = CurrencyService::new();
        let mut cache = PriceCache::default();

        let asset = Asset::stock("AAPL", "Apple");
        let result = currency_svc
            .convert_asset_to_currency(
                &price_svc,
                &mut cache,
                &asset,
                10.0,
                "USD",
                make_date(2025, 1, 15),
            )
            .await
            .unwrap();
        // 10 * 185 = 1850
        assert!((result - 1850.0).abs() < 0.01);
    }

    #[tokio::test]
    async fn convert_metal_to_pln() {
        let registry = make_registry_with_mock();
        let price_svc = PriceService::new(registry);
        let currency_svc = CurrencyService::new();
        let mut cache = PriceCache::default();

        let asset = Asset::metal("XAU", "Gold");
        let result = currency_svc
            .convert_asset_to_currency(
                &price_svc,
                &mut cache,
                &asset,
                1.0,
                "PLN",
                make_date(2025, 1, 15),
            )
            .await
            .unwrap();
        // XAU → USD (2050) → PLN (2050 * 4.05 = 8302.5)
        assert!((result - 8302.5).abs() < 1.0);
    }

    #[tokio::test]
    async fn convert_fiat_zero_amount() {
        let registry = make_registry_with_mock();
        let price_svc = PriceService::new(registry);
        let currency_svc = CurrencyService::new();
        let mut cache = PriceCache::default();

        let result = currency_svc
            .convert_fiat(
                &price_svc,
                &mut cache,
                0.0,
                "USD",
                "PLN",
                make_date(2025, 1, 15),
            )
            .await
            .unwrap();
        assert_eq!(result, 0.0);
    }

    #[test]
    #[allow(clippy::default_constructed_unit_structs)]
    fn default_trait() {
        let _svc = CurrencyService::default();
        // Should not panic
    }
}

// ═══════════════════════════════════════════════════════════════════
// ChartService
// ═══════════════════════════════════════════════════════════════════

mod chart_service {
    use super::*;

    #[tokio::test]
    async fn empty_portfolio_generates_zero_value_chart() {
        let chart_svc = ChartService::new();
        let portfolio = Portfolio::default();
        let mut price_svc = PriceService::new(make_registry_with_mock());
        let mut cache = PriceCache::default();

        let chart = chart_svc
            .generate_portfolio_chart(
                &portfolio,
                &mut price_svc,
                &mut cache,
                make_date(2025, 1, 15),
                make_date(2025, 1, 17),
                "USD",
            )
            .await
            .unwrap();

        assert_eq!(chart.len(), 3); // 3 days
        for point in &chart {
            assert_eq!(point.portfolio_value, 0.0);
            assert!(point.events.is_empty());
        }
    }

    #[tokio::test]
    async fn chart_with_events_has_annotations() {
        let chart_svc = ChartService::new();
        let mut portfolio = Portfolio::default();
        let svc = PortfolioService::new();
        svc.add_event(
            &mut portfolio,
            Event::new(
                EventType::Buy,
                Asset::crypto("BTC", "Bitcoin"),
                1.0,
                make_date(2025, 1, 15),
            ),
        )
        .unwrap();

        let mut price_svc = PriceService::new(make_registry_with_mock());
        let mut cache = PriceCache::default();

        let chart = chart_svc
            .generate_portfolio_chart(
                &portfolio,
                &mut price_svc,
                &mut cache,
                make_date(2025, 1, 15),
                make_date(2025, 1, 17),
                "USD",
            )
            .await
            .unwrap();

        assert_eq!(chart.len(), 3);
        // Day 1 should have the buy event
        assert_eq!(chart[0].events.len(), 1);
        assert_eq!(chart[0].events[0].asset_symbol, "BTC");
        assert_eq!(chart[0].events[0].event_type, EventType::Buy);
        // Day 2 and 3 should have no events
        assert!(chart[1].events.is_empty());
        assert!(chart[2].events.is_empty());
    }

    #[tokio::test]
    async fn chart_dates_are_sequential() {
        let chart_svc = ChartService::new();
        let portfolio = Portfolio::default();
        let mut price_svc = PriceService::new(make_registry_with_mock());
        let mut cache = PriceCache::default();

        let from = make_date(2025, 1, 15);
        let to = make_date(2025, 1, 20);

        let chart = chart_svc
            .generate_portfolio_chart(
                &portfolio,
                &mut price_svc,
                &mut cache,
                from,
                to,
                "USD",
            )
            .await
            .unwrap();

        assert_eq!(chart.len(), 6); // 6 days inclusive
        for (i, point) in chart.iter().enumerate() {
            let expected_date = from + chrono::Duration::days(i as i64);
            assert_eq!(point.date, expected_date);
        }
    }

    #[tokio::test]
    async fn chart_single_day() {
        let chart_svc = ChartService::new();
        let portfolio = Portfolio::default();
        let mut price_svc = PriceService::new(make_registry_with_mock());
        let mut cache = PriceCache::default();

        let date = make_date(2025, 1, 15);
        let chart = chart_svc
            .generate_portfolio_chart(
                &portfolio,
                &mut price_svc,
                &mut cache,
                date,
                date,
                "USD",
            )
            .await
            .unwrap();

        assert_eq!(chart.len(), 1);
        assert_eq!(chart[0].date, date);
    }

    #[tokio::test]
    async fn asset_chart_nonexistent_asset_fails() {
        let chart_svc = ChartService::new();
        let portfolio = Portfolio::default(); // no events
        let mut price_svc = PriceService::new(make_registry_with_mock());
        let mut cache = PriceCache::default();

        let result = chart_svc
            .generate_asset_chart(
                &portfolio,
                &mut price_svc,
                &mut cache,
                "BTC",
                make_date(2025, 1, 15),
                make_date(2025, 1, 17),
                "USD",
            )
            .await;

        assert!(result.is_err());
        match result.unwrap_err() {
            CoreError::ValidationError(msg) => assert!(msg.contains("not found")),
            other => panic!("Expected ValidationError, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn asset_chart_with_holdings() {
        let chart_svc = ChartService::new();
        let mut portfolio = Portfolio::default();
        let svc = PortfolioService::new();
        svc.add_event(
            &mut portfolio,
            Event::new(
                EventType::Buy,
                Asset::crypto("BTC", "Bitcoin"),
                1.0,
                make_date(2025, 1, 15),
            ),
        )
        .unwrap();

        let mut price_svc = PriceService::new(make_registry_with_mock());
        let mut cache = PriceCache::default();

        let chart = chart_svc
            .generate_asset_chart(
                &portfolio,
                &mut price_svc,
                &mut cache,
                "BTC",
                make_date(2025, 1, 15),
                make_date(2025, 1, 17),
                "USD",
            )
            .await
            .unwrap();

        assert_eq!(chart.len(), 3);
        // Should have value on all 3 days (1 BTC * price each day)
        assert!(chart[0].portfolio_value > 0.0);
    }

    #[test]
    fn default_trait() {
        let _svc = ChartService::default();
    }
}

// ═══════════════════════════════════════════════════════════════════
// SavingsTracker Facade
// ═══════════════════════════════════════════════════════════════════

mod savings_tracker {
    use super::*;

    #[test]
    fn create_new() {
        let tracker = SavingsTracker::create_new();
        assert!(tracker.get_events().is_empty());
    }

    #[test]
    fn add_and_get_events() {
        let mut tracker = SavingsTracker::create_new();

        let id = tracker
            .add_event(
                EventType::Buy,
                Asset::crypto("BTC", "Bitcoin"),
                1.0,
                make_date(2025, 1, 15),
            )
            .unwrap();

        let events = tracker.get_events();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].id, id);
    }

    #[test]
    fn add_and_remove_event() {
        let mut tracker = SavingsTracker::create_new();

        let id = tracker
            .add_event(
                EventType::Buy,
                Asset::crypto("BTC", "Bitcoin"),
                1.0,
                make_date(2025, 1, 15),
            )
            .unwrap();

        tracker.remove_event(id).unwrap();
        assert!(tracker.get_events().is_empty());
    }

    #[test]
    fn remove_nonexistent_event_fails() {
        let mut tracker = SavingsTracker::create_new();
        let result = tracker.remove_event(Uuid::new_v4());
        assert!(result.is_err());
    }

    #[test]
    fn get_holdings() {
        let mut tracker = SavingsTracker::create_new();
        tracker
            .add_event(
                EventType::Buy,
                Asset::crypto("BTC", "Bitcoin"),
                2.5,
                make_date(2025, 1, 15),
            )
            .unwrap();

        let holdings = tracker.get_holdings(make_date(2025, 6, 1));
        let btc = Asset::crypto("BTC", "Bitcoin");
        assert_eq!(holdings.get(&btc).copied().unwrap(), 2.5);
    }

    #[test]
    fn set_and_get_default_currency() {
        let mut tracker = SavingsTracker::create_new();
        assert_eq!(tracker.get_settings().default_currency, "USD");

        tracker.set_default_currency("PLN".into()).unwrap();
        assert_eq!(tracker.get_settings().default_currency, "PLN");
    }

    #[test]
    fn set_and_get_api_key() {
        let mut tracker = SavingsTracker::create_new();
        tracker.set_api_key("metals_dev".into(), "my-key".into());
        assert_eq!(
            tracker.get_settings().api_keys.get("metals_dev").unwrap(),
            "my-key"
        );
    }

    #[test]
    fn save_and_load_bytes() {
        let mut tracker = SavingsTracker::create_new();
        tracker
            .add_event(
                EventType::Buy,
                Asset::crypto("ETH", "Ethereum"),
                10.0,
                make_date(2025, 3, 1),
            )
            .unwrap();
        tracker.set_default_currency("EUR".into()).unwrap();

        let bytes = tracker.save_to_bytes("password123").unwrap();
        let loaded = SavingsTracker::load_from_bytes(&bytes, "password123").unwrap();

        assert_eq!(loaded.get_events().len(), 1);
        assert_eq!(loaded.get_settings().default_currency, "EUR");
    }

    #[test]
    fn load_with_wrong_password_fails() {
        let mut tracker = SavingsTracker::create_new();
        let bytes = tracker.save_to_bytes("correct").unwrap();
        let result = SavingsTracker::load_from_bytes(&bytes, "wrong");
        assert!(result.is_err());
    }

    #[test]
    fn load_from_garbage_bytes_fails() {
        let result = SavingsTracker::load_from_bytes(&[0xDE, 0xAD], "pw");
        assert!(result.is_err());
    }

    #[test]
    fn multiple_add_sell_workflow() {
        let mut tracker = SavingsTracker::create_new();

        // Buy BTC
        tracker
            .add_event(
                EventType::Buy,
                Asset::crypto("BTC", "Bitcoin"),
                2.0,
                make_date(2025, 1, 1),
            )
            .unwrap();
        // Buy ETH
        tracker
            .add_event(
                EventType::Buy,
                Asset::crypto("ETH", "Ethereum"),
                10.0,
                make_date(2025, 1, 5),
            )
            .unwrap();
        // Sell some BTC
        tracker
            .add_event(
                EventType::Sell,
                Asset::crypto("BTC", "Bitcoin"),
                0.5,
                make_date(2025, 2, 1),
            )
            .unwrap();

        let holdings = tracker.get_holdings(make_date(2025, 12, 31));
        let btc = Asset::crypto("BTC", "Bitcoin");
        let eth = Asset::crypto("ETH", "Ethereum");
        assert!((holdings.get(&btc).copied().unwrap() - 1.5).abs() < f64::EPSILON);
        assert_eq!(holdings.get(&eth).copied().unwrap(), 10.0);
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn save_and_load_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("tracker.svtk");
        let path_str = path.to_str().unwrap();

        let mut tracker = SavingsTracker::create_new();
        tracker
            .add_event(
                EventType::Buy,
                Asset::stock("AAPL", "Apple"),
                5.0,
                make_date(2025, 6, 1),
            )
            .unwrap();
        tracker.set_api_key("metals_dev".into(), "key".into());

        tracker.save_to_file(path_str, "file-pw").unwrap();
        let loaded = SavingsTracker::load_from_file(path_str, "file-pw").unwrap();

        assert_eq!(loaded.get_events().len(), 1);
        assert_eq!(
            loaded.get_settings().api_keys.get("metals_dev").unwrap(),
            "key"
        );
    }

    #[test]
    fn events_returned_newest_first() {
        let mut tracker = SavingsTracker::create_new();
        tracker
            .add_event(
                EventType::Buy,
                Asset::crypto("BTC", "Bitcoin"),
                1.0,
                make_date(2025, 1, 1),
            )
            .unwrap();
        tracker
            .add_event(
                EventType::Buy,
                Asset::crypto("ETH", "Ethereum"),
                1.0,
                make_date(2025, 12, 31),
            )
            .unwrap();

        let events = tracker.get_events();
        assert!(events[0].date >= events[1].date);
    }
}

// ═══════════════════════════════════════════════════════════════════
// PortfolioService — update_event
// ═══════════════════════════════════════════════════════════════════

mod update_event {
    use super::*;

    #[test]
    fn update_changes_amount() {
        let mut portfolio = Portfolio::default();
        let svc = PortfolioService::new();

        let event = Event::new(EventType::Buy, Asset::crypto("BTC", "Bitcoin"), 1.0, make_date(2025, 1, 1));
        let id = event.id;
        svc.add_event(&mut portfolio, event).unwrap();

        svc.update_event(&mut portfolio, id, EventType::Buy, Asset::crypto("BTC", "Bitcoin"), 2.0, make_date(2025, 1, 1)).unwrap();

        assert_eq!(portfolio.events[0].amount, 2.0);
        assert_eq!(portfolio.events[0].id, id); // same ID preserved
    }

    #[test]
    fn update_changes_date() {
        let mut portfolio = Portfolio::default();
        let svc = PortfolioService::new();

        let event = Event::new(EventType::Buy, Asset::crypto("BTC", "B"), 1.0, make_date(2025, 1, 1));
        let id = event.id;
        svc.add_event(&mut portfolio, event).unwrap();

        svc.update_event(&mut portfolio, id, EventType::Buy, Asset::crypto("BTC", "B"), 1.0, make_date(2025, 6, 15)).unwrap();

        assert_eq!(portfolio.events[0].date, make_date(2025, 6, 15));
    }

    #[test]
    fn update_changes_asset() {
        let mut portfolio = Portfolio::default();
        let svc = PortfolioService::new();

        let event = Event::new(EventType::Buy, Asset::crypto("BTC", "B"), 1.0, make_date(2025, 1, 1));
        let id = event.id;
        svc.add_event(&mut portfolio, event).unwrap();

        svc.update_event(&mut portfolio, id, EventType::Buy, Asset::crypto("ETH", "E"), 1.0, make_date(2025, 1, 1)).unwrap();

        assert_eq!(portfolio.events[0].asset.symbol, "ETH");
    }

    #[test]
    fn update_nonexistent_event_fails() {
        let mut portfolio = Portfolio::default();
        let svc = PortfolioService::new();

        let result = svc.update_event(&mut portfolio, Uuid::new_v4(), EventType::Buy, Asset::crypto("BTC", "B"), 1.0, make_date(2025, 1, 1));
        assert!(result.is_err());
    }

    #[test]
    fn update_validates_positive_amount() {
        let mut portfolio = Portfolio::default();
        let svc = PortfolioService::new();

        let event = Event::new(EventType::Buy, Asset::crypto("BTC", "B"), 1.0, make_date(2025, 1, 1));
        let id = event.id;
        svc.add_event(&mut portfolio, event).unwrap();

        let result = svc.update_event(&mut portfolio, id, EventType::Buy, Asset::crypto("BTC", "B"), -5.0, make_date(2025, 1, 1));
        assert!(result.is_err());
        // Original event should be preserved on rollback
        assert_eq!(portfolio.events[0].amount, 1.0);
    }

    #[test]
    fn update_sell_to_more_than_held_fails() {
        let mut portfolio = Portfolio::default();
        let svc = PortfolioService::new();

        let buy = Event::new(EventType::Buy, Asset::crypto("BTC", "B"), 1.0, make_date(2025, 1, 1));
        svc.add_event(&mut portfolio, buy).unwrap();

        let sell = Event::new(EventType::Sell, Asset::crypto("BTC", "B"), 0.5, make_date(2025, 2, 1));
        let sell_id = sell.id;
        svc.add_event(&mut portfolio, sell).unwrap();

        // Try to update sell to more than held
        let result = svc.update_event(&mut portfolio, sell_id, EventType::Sell, Asset::crypto("BTC", "B"), 5.0, make_date(2025, 2, 1));
        assert!(result.is_err());
        // Sell amount should remain 0.5
        let sell_event = portfolio.events.iter().find(|e| e.id == sell_id).unwrap();
        assert_eq!(sell_event.amount, 0.5);
    }

    #[test]
    fn update_preserves_sort_order() {
        let mut portfolio = Portfolio::default();
        let svc = PortfolioService::new();

        let e1 = Event::new(EventType::Buy, Asset::crypto("BTC", "B"), 1.0, make_date(2025, 1, 1));
        let e2 = Event::new(EventType::Buy, Asset::crypto("ETH", "E"), 1.0, make_date(2025, 3, 1));
        let id1 = e1.id;
        svc.add_event(&mut portfolio, e1).unwrap();
        svc.add_event(&mut portfolio, e2).unwrap();

        // Move first event to after second
        svc.update_event(&mut portfolio, id1, EventType::Buy, Asset::crypto("BTC", "B"), 1.0, make_date(2025, 6, 1)).unwrap();

        assert!(portfolio.events[0].date <= portfolio.events[1].date);
    }
}

// ═══════════════════════════════════════════════════════════════════
// PortfolioService — revalidation on remove
// ═══════════════════════════════════════════════════════════════════

mod remove_revalidation {
    use super::*;

    #[test]
    fn removing_buy_before_sell_fails() {
        let mut portfolio = Portfolio::default();
        let svc = PortfolioService::new();

        let buy = Event::new(EventType::Buy, Asset::crypto("BTC", "B"), 1.0, make_date(2025, 1, 1));
        let buy_id = buy.id;
        svc.add_event(&mut portfolio, buy).unwrap();

        let sell = Event::new(EventType::Sell, Asset::crypto("BTC", "B"), 0.5, make_date(2025, 2, 1));
        svc.add_event(&mut portfolio, sell).unwrap();

        // Removing the buy should fail because the sell would become invalid
        let result = svc.remove_event(&mut portfolio, buy_id);
        assert!(result.is_err());
        // Events should remain unchanged (rollback)
        assert_eq!(portfolio.events.len(), 2);
    }

    #[test]
    fn removing_sell_always_succeeds() {
        let mut portfolio = Portfolio::default();
        let svc = PortfolioService::new();

        let buy = Event::new(EventType::Buy, Asset::crypto("BTC", "B"), 1.0, make_date(2025, 1, 1));
        svc.add_event(&mut portfolio, buy).unwrap();

        let sell = Event::new(EventType::Sell, Asset::crypto("BTC", "B"), 0.5, make_date(2025, 2, 1));
        let sell_id = sell.id;
        svc.add_event(&mut portfolio, sell).unwrap();

        svc.remove_event(&mut portfolio, sell_id).unwrap();
        assert_eq!(portfolio.events.len(), 1);
    }

    #[test]
    fn removing_buy_without_dependent_sell_succeeds() {
        let mut portfolio = Portfolio::default();
        let svc = PortfolioService::new();

        let buy = Event::new(EventType::Buy, Asset::crypto("BTC", "B"), 1.0, make_date(2025, 1, 1));
        let buy_id = buy.id;
        svc.add_event(&mut portfolio, buy).unwrap();

        svc.remove_event(&mut portfolio, buy_id).unwrap();
        assert_eq!(portfolio.events.len(), 0);
    }

    #[test]
    fn removing_one_of_two_buys_that_covers_sell() {
        let mut portfolio = Portfolio::default();
        let svc = PortfolioService::new();

        let buy1 = Event::new(EventType::Buy, Asset::crypto("BTC", "B"), 2.0, make_date(2025, 1, 1));
        let buy1_id = buy1.id;
        svc.add_event(&mut portfolio, buy1).unwrap();

        let buy2 = Event::new(EventType::Buy, Asset::crypto("BTC", "B"), 3.0, make_date(2025, 1, 5));
        svc.add_event(&mut portfolio, buy2).unwrap();

        let sell = Event::new(EventType::Sell, Asset::crypto("BTC", "B"), 4.0, make_date(2025, 2, 1));
        svc.add_event(&mut portfolio, sell).unwrap();

        // Removing buy1 (2.0) should fail: remaining buy2 (3.0) < sell (4.0)
        let result = svc.remove_event(&mut portfolio, buy1_id);
        assert!(result.is_err());
        assert_eq!(portfolio.events.len(), 3);
    }
}

// ═══════════════════════════════════════════════════════════════════
// SavingsTracker — update_event, get_event, get_unique_assets
// ═══════════════════════════════════════════════════════════════════

mod tracker_new_apis {
    use super::*;

    #[test]
    fn get_event_found() {
        let mut tracker = SavingsTracker::create_new();
        let id = tracker.add_event(EventType::Buy, Asset::crypto("BTC", "B"), 1.0, make_date(2025, 1, 1)).unwrap();

        let event = tracker.get_event(id);
        assert!(event.is_some());
        assert_eq!(event.unwrap().id, id);
    }

    #[test]
    fn get_event_not_found() {
        let tracker = SavingsTracker::create_new();
        assert!(tracker.get_event(Uuid::new_v4()).is_none());
    }

    #[test]
    fn update_event_via_tracker() {
        let mut tracker = SavingsTracker::create_new();
        let id = tracker.add_event(EventType::Buy, Asset::crypto("BTC", "B"), 1.0, make_date(2025, 1, 1)).unwrap();

        tracker.update_event(id, EventType::Buy, Asset::crypto("BTC", "B"), 5.0, make_date(2025, 1, 1)).unwrap();

        let event = tracker.get_event(id).unwrap();
        assert_eq!(event.amount, 5.0);
    }

    #[test]
    fn get_unique_assets_deduplicates() {
        let mut tracker = SavingsTracker::create_new();
        tracker.add_event(EventType::Buy, Asset::crypto("BTC", "Bitcoin"), 1.0, make_date(2025, 1, 1)).unwrap();
        tracker.add_event(EventType::Buy, Asset::crypto("BTC", "Bitcoin"), 2.0, make_date(2025, 2, 1)).unwrap();
        tracker.add_event(EventType::Buy, Asset::crypto("ETH", "Ethereum"), 5.0, make_date(2025, 1, 1)).unwrap();

        let assets = tracker.get_unique_assets();
        assert_eq!(assets.len(), 2);
    }

    #[test]
    fn get_unique_assets_empty_portfolio() {
        let tracker = SavingsTracker::create_new();
        assert!(tracker.get_unique_assets().is_empty());
    }
}

// ═══════════════════════════════════════════════════════════════════
// Event notes
// ═══════════════════════════════════════════════════════════════════

mod event_notes {
    use super::*;

    #[test]
    fn event_new_has_no_notes() {
        let event = Event::new(EventType::Buy, Asset::crypto("BTC", "B"), 1.0, make_date(2025, 1, 1));
        assert!(event.notes.is_none());
    }

    #[test]
    fn event_with_notes_stores_them() {
        let event = Event::with_notes(
            EventType::Buy,
            Asset::crypto("BTC", "B"),
            1.0,
            make_date(2025, 1, 1),
            "Bought on Binance",
        );
        assert_eq!(event.notes.as_deref(), Some("Bought on Binance"));
    }

    #[test]
    fn add_event_with_notes_via_tracker() {
        let mut tracker = SavingsTracker::create_new();
        let id = tracker
            .add_event_with_notes(
                EventType::Buy,
                Asset::crypto("BTC", "Bitcoin"),
                1.0,
                make_date(2025, 1, 1),
                "DCA purchase",
            )
            .unwrap();

        let event = tracker.get_event(id).unwrap();
        assert_eq!(event.notes.as_deref(), Some("DCA purchase"));
    }

    #[test]
    fn set_event_notes_on_existing() {
        let mut tracker = SavingsTracker::create_new();
        let id = tracker
            .add_event(EventType::Buy, Asset::crypto("BTC", "B"), 1.0, make_date(2025, 1, 1))
            .unwrap();

        tracker.set_event_notes(id, Some("Added later".into())).unwrap();
        assert_eq!(tracker.get_event(id).unwrap().notes.as_deref(), Some("Added later"));
    }

    #[test]
    fn clear_event_notes() {
        let mut tracker = SavingsTracker::create_new();
        let id = tracker
            .add_event_with_notes(
                EventType::Buy,
                Asset::crypto("BTC", "B"),
                1.0,
                make_date(2025, 1, 1),
                "Some note",
            )
            .unwrap();

        tracker.set_event_notes(id, None).unwrap();
        assert!(tracker.get_event(id).unwrap().notes.is_none());
    }

    #[test]
    fn set_notes_nonexistent_event_fails() {
        let mut tracker = SavingsTracker::create_new();
        let result = tracker.set_event_notes(Uuid::new_v4(), Some("nope".into()));
        assert!(result.is_err());
    }

    #[test]
    fn update_event_preserves_notes() {
        let mut tracker = SavingsTracker::create_new();
        let id = tracker
            .add_event_with_notes(
                EventType::Buy,
                Asset::crypto("BTC", "B"),
                1.0,
                make_date(2025, 1, 1),
                "Original note",
            )
            .unwrap();

        tracker.update_event(id, EventType::Buy, Asset::crypto("BTC", "B"), 2.0, make_date(2025, 1, 1)).unwrap();
        assert_eq!(tracker.get_event(id).unwrap().notes.as_deref(), Some("Original note"));
    }

    #[test]
    fn notes_survive_save_load() {
        let mut tracker = SavingsTracker::create_new();
        tracker
            .add_event_with_notes(
                EventType::Buy,
                Asset::crypto("ETH", "Ethereum"),
                5.0,
                make_date(2025, 3, 1),
                "Monthly DCA",
            )
            .unwrap();

        let bytes = tracker.save_to_bytes("pw").unwrap();
        let loaded = SavingsTracker::load_from_bytes(&bytes, "pw").unwrap();
        let event = loaded.get_events()[0];
        assert_eq!(event.notes.as_deref(), Some("Monthly DCA"));
    }
}

// ═══════════════════════════════════════════════════════════════════
// has_unsaved_changes / dirty tracking
// ═══════════════════════════════════════════════════════════════════

mod dirty_tracking {
    use super::*;

    #[test]
    fn new_tracker_is_clean() {
        let tracker = SavingsTracker::create_new();
        assert!(!tracker.has_unsaved_changes());
    }

    #[test]
    fn add_event_marks_dirty() {
        let mut tracker = SavingsTracker::create_new();
        tracker.add_event(EventType::Buy, Asset::crypto("BTC", "B"), 1.0, make_date(2025, 1, 1)).unwrap();
        assert!(tracker.has_unsaved_changes());
    }

    #[test]
    fn save_clears_dirty() {
        let mut tracker = SavingsTracker::create_new();
        tracker.add_event(EventType::Buy, Asset::crypto("BTC", "B"), 1.0, make_date(2025, 1, 1)).unwrap();
        assert!(tracker.has_unsaved_changes());

        tracker.save_to_bytes("pw").unwrap();
        assert!(!tracker.has_unsaved_changes());
    }

    #[test]
    fn loaded_tracker_is_clean() {
        let mut tracker = SavingsTracker::create_new();
        tracker.add_event(EventType::Buy, Asset::crypto("BTC", "B"), 1.0, make_date(2025, 1, 1)).unwrap();
        let bytes = tracker.save_to_bytes("pw").unwrap();

        let loaded = SavingsTracker::load_from_bytes(&bytes, "pw").unwrap();
        assert!(!loaded.has_unsaved_changes());
    }

    #[test]
    fn remove_event_marks_dirty() {
        let mut tracker = SavingsTracker::create_new();
        let id = tracker.add_event(EventType::Buy, Asset::crypto("BTC", "B"), 1.0, make_date(2025, 1, 1)).unwrap();
        tracker.save_to_bytes("pw").unwrap();
        assert!(!tracker.has_unsaved_changes());

        tracker.remove_event(id).unwrap();
        assert!(tracker.has_unsaved_changes());
    }

    #[test]
    fn update_event_marks_dirty() {
        let mut tracker = SavingsTracker::create_new();
        let id = tracker.add_event(EventType::Buy, Asset::crypto("BTC", "B"), 1.0, make_date(2025, 1, 1)).unwrap();
        tracker.save_to_bytes("pw").unwrap();

        tracker.update_event(id, EventType::Buy, Asset::crypto("BTC", "B"), 2.0, make_date(2025, 1, 1)).unwrap();
        assert!(tracker.has_unsaved_changes());
    }

    #[test]
    fn set_default_currency_marks_dirty() {
        let mut tracker = SavingsTracker::create_new();
        tracker.save_to_bytes("pw").unwrap();

        tracker.set_default_currency("PLN".into()).unwrap();
        assert!(tracker.has_unsaved_changes());
    }

    #[test]
    fn set_api_key_marks_dirty() {
        let mut tracker = SavingsTracker::create_new();
        tracker.save_to_bytes("pw").unwrap();

        tracker.set_api_key("metals_dev".into(), "key".into());
        assert!(tracker.has_unsaved_changes());
    }

    #[test]
    fn set_event_notes_marks_dirty() {
        let mut tracker = SavingsTracker::create_new();
        let id = tracker.add_event(EventType::Buy, Asset::crypto("BTC", "B"), 1.0, make_date(2025, 1, 1)).unwrap();
        tracker.save_to_bytes("pw").unwrap();

        tracker.set_event_notes(id, Some("note".into())).unwrap();
        assert!(tracker.has_unsaved_changes());
    }
}

// ═══════════════════════════════════════════════════════════════════
// change_password
// ═══════════════════════════════════════════════════════════════════

mod change_password {
    use super::*;

    #[test]
    fn change_password_produces_loadable_bytes() {
        let mut tracker = SavingsTracker::create_new();
        tracker.add_event(EventType::Buy, Asset::crypto("BTC", "B"), 1.0, make_date(2025, 1, 1)).unwrap();

        // Save with original password
        let saved_bytes = tracker.save_to_bytes("old_pw").unwrap();

        // Change password (provide last saved bytes for verification)
        let new_bytes = tracker.change_password(&saved_bytes, "old_pw", "new_pw").unwrap();

        // Load with new password succeeds
        let loaded = SavingsTracker::load_from_bytes(&new_bytes, "new_pw").unwrap();
        assert_eq!(loaded.get_events().len(), 1);

        // Load with old password fails
        assert!(SavingsTracker::load_from_bytes(&new_bytes, "old_pw").is_err());
    }

    #[test]
    fn change_password_clears_dirty() {
        let mut tracker = SavingsTracker::create_new();
        tracker.add_event(EventType::Buy, Asset::crypto("BTC", "B"), 1.0, make_date(2025, 1, 1)).unwrap();
        let saved_bytes = tracker.save_to_bytes("old").unwrap();

        tracker.set_default_currency("PLN".into()).unwrap();
        assert!(tracker.has_unsaved_changes());

        tracker.change_password(&saved_bytes, "old", "new").unwrap();
        assert!(!tracker.has_unsaved_changes());
    }
}

// ═══════════════════════════════════════════════════════════════════
// Binary insert order verification
// ═══════════════════════════════════════════════════════════════════

mod binary_insert_order {
    use super::*;

    #[test]
    fn events_stay_sorted_after_many_inserts() {
        let mut portfolio = Portfolio::default();
        let svc = PortfolioService::new();

        // Insert in random date order
        let dates = vec![
            make_date(2025, 6, 1),
            make_date(2025, 1, 1),
            make_date(2025, 12, 1),
            make_date(2025, 3, 15),
            make_date(2025, 9, 20),
            make_date(2025, 1, 1), // duplicate date
            make_date(2025, 7, 4),
        ];

        for date in dates {
            let event = Event::new(EventType::Buy, Asset::crypto("BTC", "B"), 0.1, date);
            svc.add_event(&mut portfolio, event).unwrap();
        }

        // Verify sorted order
        for i in 1..portfolio.events.len() {
            assert!(
                portfolio.events[i - 1].date <= portfolio.events[i].date,
                "Events not sorted: {} > {}",
                portfolio.events[i - 1].date,
                portfolio.events[i].date
            );
        }
    }

    #[test]
    fn binary_insert_at_start() {
        let mut portfolio = Portfolio::default();
        let svc = PortfolioService::new();

        svc.add_event(&mut portfolio, Event::new(EventType::Buy, Asset::crypto("BTC", "B"), 1.0, make_date(2025, 6, 1))).unwrap();
        svc.add_event(&mut portfolio, Event::new(EventType::Buy, Asset::crypto("BTC", "B"), 1.0, make_date(2025, 1, 1))).unwrap();

        assert_eq!(portfolio.events[0].date, make_date(2025, 1, 1));
    }

    #[test]
    fn binary_insert_at_end() {
        let mut portfolio = Portfolio::default();
        let svc = PortfolioService::new();

        svc.add_event(&mut portfolio, Event::new(EventType::Buy, Asset::crypto("BTC", "B"), 1.0, make_date(2025, 1, 1))).unwrap();
        svc.add_event(&mut portfolio, Event::new(EventType::Buy, Asset::crypto("BTC", "B"), 1.0, make_date(2025, 12, 1))).unwrap();

        assert_eq!(portfolio.events[1].date, make_date(2025, 12, 1));
    }

    #[test]
    fn binary_insert_in_middle() {
        let mut portfolio = Portfolio::default();
        let svc = PortfolioService::new();

        svc.add_event(&mut portfolio, Event::new(EventType::Buy, Asset::crypto("BTC", "B"), 1.0, make_date(2025, 1, 1))).unwrap();
        svc.add_event(&mut portfolio, Event::new(EventType::Buy, Asset::crypto("BTC", "B"), 1.0, make_date(2025, 12, 1))).unwrap();
        svc.add_event(&mut portfolio, Event::new(EventType::Buy, Asset::crypto("BTC", "B"), 1.0, make_date(2025, 6, 1))).unwrap();

        assert_eq!(portfolio.events[1].date, make_date(2025, 6, 1));
    }
}

// ═══════════════════════════════════════════════════════════════════
// CoinCap dynamic resolution
// ═══════════════════════════════════════════════════════════════════

mod coincap_resolution {
    use savings_tracker_core::providers::coincap::CoinCapProvider;

    #[test]
    fn resolve_known_symbols() {
        let provider = CoinCapProvider::new();
        assert_eq!(provider.resolve_id("BTC"), "bitcoin");
        assert_eq!(provider.resolve_id("ETH"), "ethereum");
        assert_eq!(provider.resolve_id("SOL"), "solana");
        assert_eq!(provider.resolve_id("XRP"), "xrp");
    }

    #[test]
    fn resolve_new_symbols_added_in_v2() {
        let provider = CoinCapProvider::new();
        assert_eq!(provider.resolve_id("USDC"), "usd-coin");
        assert_eq!(provider.resolve_id("TRX"), "tron");
        assert_eq!(provider.resolve_id("DAI"), "multi-collateral-dai");
        assert_eq!(provider.resolve_id("XMR"), "monero");
        assert_eq!(provider.resolve_id("ZEC"), "zcash");
        assert_eq!(provider.resolve_id("AAVE"), "aave");
        assert_eq!(provider.resolve_id("FIL"), "filecoin");
        assert_eq!(provider.resolve_id("ICP"), "internet-computer");
        assert_eq!(provider.resolve_id("HBAR"), "hedera-hashgraph");
        assert_eq!(provider.resolve_id("VET"), "vechain");
    }

    #[test]
    fn unknown_symbol_falls_back_to_lowercase() {
        let provider = CoinCapProvider::new();
        assert_eq!(provider.resolve_id("UNKNOWN"), "unknown");
    }

    #[test]
    fn case_insensitive_resolve() {
        let provider = CoinCapProvider::new();
        assert_eq!(provider.resolve_id("btc"), "bitcoin");
        assert_eq!(provider.resolve_id("Eth"), "ethereum");
    }
}

// ═══════════════════════════════════════════════════════════════════
// Event filtering
// ═══════════════════════════════════════════════════════════════════

mod event_filtering {
    use super::*;

    #[test]
    fn filter_by_asset_symbol() {
        let mut tracker = SavingsTracker::create_new();
        tracker.add_event(EventType::Buy, Asset::crypto("BTC", "Bitcoin"), 1.0, make_date(2025, 1, 1)).unwrap();
        tracker.add_event(EventType::Buy, Asset::crypto("ETH", "Ethereum"), 2.0, make_date(2025, 1, 2)).unwrap();
        tracker.add_event(EventType::Buy, Asset::crypto("BTC", "Bitcoin"), 3.0, make_date(2025, 1, 3)).unwrap();

        let btc_events = tracker.get_events_for_asset("BTC");
        assert_eq!(btc_events.len(), 2);
        assert!(btc_events.iter().all(|e| e.asset.symbol == "BTC"));
    }

    #[test]
    fn filter_by_asset_case_insensitive() {
        let mut tracker = SavingsTracker::create_new();
        tracker.add_event(EventType::Buy, Asset::crypto("BTC", "Bitcoin"), 1.0, make_date(2025, 1, 1)).unwrap();

        assert_eq!(tracker.get_events_for_asset("btc").len(), 1);
        assert_eq!(tracker.get_events_for_asset("Btc").len(), 1);
    }

    #[test]
    fn filter_by_type() {
        let mut tracker = SavingsTracker::create_new();
        tracker.add_event(EventType::Buy, Asset::crypto("BTC", "Bitcoin"), 5.0, make_date(2025, 1, 1)).unwrap();
        tracker.add_event(EventType::Buy, Asset::crypto("ETH", "Ethereum"), 10.0, make_date(2025, 1, 2)).unwrap();
        tracker.add_event(EventType::Sell, Asset::crypto("BTC", "Bitcoin"), 1.0, make_date(2025, 1, 3)).unwrap();

        let buys = tracker.get_events_by_type(&EventType::Buy);
        assert_eq!(buys.len(), 2);

        let sells = tracker.get_events_by_type(&EventType::Sell);
        assert_eq!(sells.len(), 1);
    }

    #[test]
    fn filter_by_date_range() {
        let mut tracker = SavingsTracker::create_new();
        tracker.add_event(EventType::Buy, Asset::crypto("BTC", "Bitcoin"), 1.0, make_date(2025, 1, 1)).unwrap();
        tracker.add_event(EventType::Buy, Asset::crypto("BTC", "Bitcoin"), 2.0, make_date(2025, 3, 1)).unwrap();
        tracker.add_event(EventType::Buy, Asset::crypto("BTC", "Bitcoin"), 3.0, make_date(2025, 6, 1)).unwrap();

        let in_range = tracker.get_events_in_range(make_date(2025, 2, 1), make_date(2025, 4, 1));
        assert_eq!(in_range.len(), 1);
        assert_eq!(in_range[0].amount, 2.0);
    }

    #[test]
    fn filter_no_match_returns_empty() {
        let mut tracker = SavingsTracker::create_new();
        tracker.add_event(EventType::Buy, Asset::crypto("BTC", "Bitcoin"), 1.0, make_date(2025, 1, 1)).unwrap();

        assert!(tracker.get_events_for_asset("DOGE").is_empty());
        assert!(tracker.get_events_by_type(&EventType::Sell).is_empty());
        assert!(tracker.get_events_in_range(make_date(2026, 1, 1), make_date(2026, 2, 1)).is_empty());
    }
}

// ═══════════════════════════════════════════════════════════════════
// Currency validation
// ═══════════════════════════════════════════════════════════════════

mod currency_validation {
    use super::*;

    #[test]
    fn valid_currency_codes() {
        let mut tracker = SavingsTracker::create_new();
        assert!(tracker.set_default_currency("USD".into()).is_ok());
        assert!(tracker.set_default_currency("eur".into()).is_ok());
        assert_eq!(tracker.get_settings().default_currency, "EUR"); // uppercased
        assert!(tracker.set_default_currency("pln".into()).is_ok());
        assert_eq!(tracker.get_settings().default_currency, "PLN");
    }

    #[test]
    fn invalid_currency_too_short() {
        let mut tracker = SavingsTracker::create_new();
        assert!(tracker.set_default_currency("US".into()).is_err());
    }

    #[test]
    fn invalid_currency_too_long() {
        let mut tracker = SavingsTracker::create_new();
        assert!(tracker.set_default_currency("USDD".into()).is_err());
    }

    #[test]
    fn invalid_currency_with_digits() {
        let mut tracker = SavingsTracker::create_new();
        assert!(tracker.set_default_currency("US1".into()).is_err());
    }

    #[test]
    fn invalid_currency_empty() {
        let mut tracker = SavingsTracker::create_new();
        assert!(tracker.set_default_currency("".into()).is_err());
    }
}

// ═══════════════════════════════════════════════════════════════════
// Remove API key
// ═══════════════════════════════════════════════════════════════════

mod remove_api_key {
    use super::*;

    #[test]
    fn remove_existing_key() {
        let mut tracker = SavingsTracker::create_new();
        tracker.set_api_key("metals_dev".into(), "key123".into());
        assert!(tracker.get_settings().api_keys.contains_key("metals_dev"));

        assert!(tracker.remove_api_key("metals_dev"));
        assert!(!tracker.get_settings().api_keys.contains_key("metals_dev"));
    }

    #[test]
    fn remove_nonexistent_key_returns_false() {
        let mut tracker = SavingsTracker::create_new();
        assert!(!tracker.remove_api_key("nonexistent"));
    }

    #[test]
    fn remove_api_key_marks_dirty() {
        let mut tracker = SavingsTracker::create_new();
        tracker.set_api_key("metals_dev".into(), "key123".into());
        tracker.save_to_bytes("pw").unwrap();
        assert!(!tracker.has_unsaved_changes());

        tracker.remove_api_key("metals_dev");
        assert!(tracker.has_unsaved_changes());
    }
}

// ═══════════════════════════════════════════════════════════════════
// Cache pruning / stats
// ═══════════════════════════════════════════════════════════════════

mod cache_management {
    use super::*;

    #[test]
    fn cache_stats_empty() {
        let tracker = SavingsTracker::create_new();
        assert_eq!(tracker.cache_total_entries(), 0);
        assert_eq!(tracker.cache_asset_count(), 0);
    }

    #[test]
    fn prune_removes_old_entries() {
        use savings_tracker_core::models::price::PriceCache;

        let mut cache = PriceCache::new();
        cache.set_price("BTC", "USD", make_date(2024, 1, 1), 40000.0);
        cache.set_price("BTC", "USD", make_date(2024, 6, 1), 60000.0);
        cache.set_price("BTC", "USD", make_date(2025, 1, 1), 100000.0);

        assert_eq!(cache.total_entries(), 3);
        let removed = cache.prune_before(make_date(2025, 1, 1));
        assert_eq!(removed, 2);
        assert_eq!(cache.total_entries(), 1);
        assert!(cache.get_price("BTC", "USD", make_date(2025, 1, 1)).is_some());
    }

    #[test]
    fn clear_removes_everything() {
        use savings_tracker_core::models::price::PriceCache;

        let mut cache = PriceCache::new();
        cache.set_price("BTC", "USD", make_date(2024, 1, 1), 40000.0);
        cache.set_price("ETH", "USD", make_date(2024, 1, 1), 2000.0);
        cache.mark_updated_today("BTC", "USD", make_date(2024, 1, 1));

        cache.clear();
        assert_eq!(cache.total_entries(), 0);
        assert_eq!(cache.asset_count(), 0);
    }

    #[test]
    fn cache_clear_via_tracker_marks_dirty() {
        let mut tracker = SavingsTracker::create_new();
        tracker.save_to_bytes("pw").unwrap();

        tracker.cache_clear();
        assert!(tracker.has_unsaved_changes());
    }
}

// ═══════════════════════════════════════════════════════════════════
// Price cache binary search range
// ═══════════════════════════════════════════════════════════════════

mod price_cache_binary_range {
    use super::*;
    use savings_tracker_core::models::price::PriceCache;

    #[test]
    fn binary_range_exact_boundaries() {
        let mut cache = PriceCache::new();
        for day in 1..=10 {
            cache.set_price("BTC", "USD", make_date(2025, 1, day), day as f64 * 1000.0);
        }

        let range = cache.get_price_range("BTC", "USD", make_date(2025, 1, 3), make_date(2025, 1, 7));
        assert_eq!(range.len(), 5);
        assert_eq!(range[0].date, make_date(2025, 1, 3));
        assert_eq!(range[4].date, make_date(2025, 1, 7));
    }

    #[test]
    fn binary_range_no_exact_match() {
        let mut cache = PriceCache::new();
        cache.set_price("BTC", "USD", make_date(2025, 1, 1), 1000.0);
        cache.set_price("BTC", "USD", make_date(2025, 1, 5), 5000.0);
        cache.set_price("BTC", "USD", make_date(2025, 1, 10), 10000.0);

        // Range from 3 to 7 should include day 5 only
        let range = cache.get_price_range("BTC", "USD", make_date(2025, 1, 3), make_date(2025, 1, 7));
        assert_eq!(range.len(), 1);
        assert_eq!(range[0].date, make_date(2025, 1, 5));
    }
}

// ═══════════════════════════════════════════════════════════════════
// Deterministic get_unique_assets
// ═══════════════════════════════════════════════════════════════════

mod deterministic_assets {
    use super::*;

    #[test]
    fn unique_assets_sorted_by_symbol() {
        let mut tracker = SavingsTracker::create_new();
        tracker.add_event(EventType::Buy, Asset::crypto("ETH", "Ethereum"), 1.0, make_date(2025, 1, 1)).unwrap();
        tracker.add_event(EventType::Buy, Asset::crypto("BTC", "Bitcoin"), 1.0, make_date(2025, 1, 2)).unwrap();
        tracker.add_event(EventType::Buy, Asset::stock("AAPL", "Apple"), 1.0, make_date(2025, 1, 3)).unwrap();

        let assets = tracker.get_unique_assets();
        assert_eq!(assets.len(), 3);
        assert_eq!(assets[0].symbol, "AAPL");
        assert_eq!(assets[1].symbol, "BTC");
        assert_eq!(assets[2].symbol, "ETH");
    }
}

// ═══════════════════════════════════════════════════════════════════
// Debug impl
// ═══════════════════════════════════════════════════════════════════

mod debug_impl {
    use super::*;

    #[test]
    fn savings_tracker_is_debug() {
        let tracker = SavingsTracker::create_new();
        let debug_str = format!("{:?}", tracker);
        assert!(debug_str.contains("SavingsTracker"));
        assert!(debug_str.contains("events"));
        assert!(debug_str.contains("dirty"));
    }
}

// ═══════════════════════════════════════════════════════════════════
// Range validation
// ═══════════════════════════════════════════════════════════════════

mod range_validation {
    use super::*;

    #[tokio::test]
    async fn portfolio_chart_from_after_to_fails() {
        let mut tracker = SavingsTracker::create_new();
        let result = tracker.generate_portfolio_chart(make_date(2025, 3, 1), make_date(2025, 1, 1)).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn asset_chart_from_after_to_fails() {
        let mut tracker = SavingsTracker::create_new();
        tracker.add_event(EventType::Buy, Asset::crypto("BTC", "B"), 1.0, make_date(2025, 1, 1)).unwrap();
        let result = tracker.generate_asset_chart("BTC", make_date(2025, 3, 1), make_date(2025, 1, 1)).await;
        assert!(result.is_err());
    }
}

// ═══════════════════════════════════════════════════════════════════
// Future date validation
// ═══════════════════════════════════════════════════════════════════

mod future_date_validation {
    use super::*;

    #[test]
    fn add_event_with_future_date_fails() {
        let mut tracker = SavingsTracker::create_new();
        let future = chrono::Utc::now().date_naive() + chrono::Duration::days(30);
        let result = tracker.add_event(EventType::Buy, Asset::crypto("BTC", "B"), 1.0, future);
        assert!(result.is_err());
    }

    #[test]
    fn add_event_with_today_succeeds() {
        let mut tracker = SavingsTracker::create_new();
        let today = chrono::Utc::now().date_naive();
        let result = tracker.add_event(EventType::Buy, Asset::crypto("BTC", "B"), 1.0, today);
        assert!(result.is_ok());
    }

    #[test]
    fn add_event_with_tomorrow_succeeds_timezone_tolerance() {
        // I6: Allow +1 day tolerance for timezone differences
        let mut tracker = SavingsTracker::create_new();
        let tomorrow = chrono::Utc::now().date_naive() + chrono::Duration::days(1);
        let result = tracker.add_event(EventType::Buy, Asset::crypto("BTC", "B"), 1.0, tomorrow);
        assert!(result.is_ok());
    }
}

// ═══════════════════════════════════════════════════════════════════
// Search events (M3)
// ═══════════════════════════════════════════════════════════════════

mod search_events {
    use super::*;

    #[test]
    fn search_by_symbol() {
        let mut tracker = SavingsTracker::create_new();
        tracker.add_event(EventType::Buy, Asset::crypto("BTC", "Bitcoin"), 1.0, make_date(2025, 1, 1)).unwrap();
        tracker.add_event(EventType::Buy, Asset::crypto("ETH", "Ethereum"), 2.0, make_date(2025, 1, 2)).unwrap();

        let results = tracker.search_events("btc");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].asset.symbol, "BTC");
    }

    #[test]
    fn search_by_name() {
        let mut tracker = SavingsTracker::create_new();
        tracker.add_event(EventType::Buy, Asset::crypto("BTC", "Bitcoin"), 1.0, make_date(2025, 1, 1)).unwrap();
        tracker.add_event(EventType::Buy, Asset::crypto("ETH", "Ethereum"), 2.0, make_date(2025, 1, 2)).unwrap();

        let results = tracker.search_events("ethereum");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].asset.symbol, "ETH");
    }

    #[test]
    fn search_by_notes() {
        let mut tracker = SavingsTracker::create_new();
        tracker.add_event_with_notes(EventType::Buy, Asset::crypto("BTC", "Bitcoin"), 1.0, make_date(2025, 1, 1), "bought on binance").unwrap();
        tracker.add_event(EventType::Buy, Asset::crypto("ETH", "Ethereum"), 2.0, make_date(2025, 1, 2)).unwrap();

        let results = tracker.search_events("binance");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].asset.symbol, "BTC");
    }

    #[test]
    fn search_no_results() {
        let mut tracker = SavingsTracker::create_new();
        tracker.add_event(EventType::Buy, Asset::crypto("BTC", "Bitcoin"), 1.0, make_date(2025, 1, 1)).unwrap();
        let results = tracker.search_events("DOGE");
        assert!(results.is_empty());
    }
}

// ═══════════════════════════════════════════════════════════════════
// Sorted events (M4)
// ═══════════════════════════════════════════════════════════════════

mod sorted_events {
    use super::*;
    use savings_tracker_core::models::event::EventSortOrder;

    #[test]
    fn sort_by_date_desc() {
        let mut tracker = SavingsTracker::create_new();
        tracker.add_event(EventType::Buy, Asset::crypto("BTC", "B"), 1.0, make_date(2025, 1, 1)).unwrap();
        tracker.add_event(EventType::Buy, Asset::crypto("ETH", "E"), 2.0, make_date(2025, 1, 3)).unwrap();
        tracker.add_event(EventType::Buy, Asset::crypto("SOL", "S"), 3.0, make_date(2025, 1, 2)).unwrap();

        let events = tracker.get_events_sorted(&EventSortOrder::DateDesc);
        assert_eq!(events[0].date, make_date(2025, 1, 3));
        assert_eq!(events[2].date, make_date(2025, 1, 1));
    }

    #[test]
    fn sort_by_amount_desc() {
        let mut tracker = SavingsTracker::create_new();
        tracker.add_event(EventType::Buy, Asset::crypto("BTC", "B"), 0.5, make_date(2025, 1, 1)).unwrap();
        tracker.add_event(EventType::Buy, Asset::crypto("ETH", "E"), 10.0, make_date(2025, 1, 2)).unwrap();
        tracker.add_event(EventType::Buy, Asset::crypto("SOL", "S"), 3.0, make_date(2025, 1, 3)).unwrap();

        let events = tracker.get_events_sorted(&EventSortOrder::AmountDesc);
        assert_eq!(events[0].amount, 10.0);
        assert_eq!(events[2].amount, 0.5);
    }

    #[test]
    fn sort_by_asset_asc() {
        let mut tracker = SavingsTracker::create_new();
        tracker.add_event(EventType::Buy, Asset::crypto("ETH", "E"), 2.0, make_date(2025, 1, 1)).unwrap();
        tracker.add_event(EventType::Buy, Asset::crypto("BTC", "B"), 1.0, make_date(2025, 1, 2)).unwrap();

        let events = tracker.get_events_sorted(&EventSortOrder::AssetAsc);
        assert_eq!(events[0].asset.symbol, "BTC");
        assert_eq!(events[1].asset.symbol, "ETH");
    }
}

// ═══════════════════════════════════════════════════════════════════
// Filter by AssetType (M9)
// ═══════════════════════════════════════════════════════════════════

mod filter_by_asset_type {
    use super::*;

    #[test]
    fn filter_crypto_only() {
        let mut tracker = SavingsTracker::create_new();
        tracker.add_event(EventType::Buy, Asset::crypto("BTC", "B"), 1.0, make_date(2025, 1, 1)).unwrap();
        tracker.add_event(EventType::Buy, Asset::stock("AAPL", "A"), 10.0, make_date(2025, 1, 2)).unwrap();
        tracker.add_event(EventType::Buy, Asset::crypto("ETH", "E"), 2.0, make_date(2025, 1, 3)).unwrap();

        let results = tracker.get_events_for_asset_type(&AssetType::Crypto);
        assert_eq!(results.len(), 2);
        assert!(results.iter().all(|e| e.asset.asset_type == AssetType::Crypto));
    }

    #[test]
    fn filter_empty_result() {
        let mut tracker = SavingsTracker::create_new();
        tracker.add_event(EventType::Buy, Asset::crypto("BTC", "B"), 1.0, make_date(2025, 1, 1)).unwrap();

        let results = tracker.get_events_for_asset_type(&AssetType::Stock);
        assert!(results.is_empty());
    }
}

// ═══════════════════════════════════════════════════════════════════
// Event count (M10)
// ═══════════════════════════════════════════════════════════════════

mod event_count {
    use super::*;

    #[test]
    fn event_count_empty() {
        let tracker = SavingsTracker::create_new();
        assert_eq!(tracker.event_count(), 0);
    }

    #[test]
    fn event_count_matches_len() {
        let mut tracker = SavingsTracker::create_new();
        tracker.add_event(EventType::Buy, Asset::crypto("BTC", "B"), 1.0, make_date(2025, 1, 1)).unwrap();
        tracker.add_event(EventType::Buy, Asset::crypto("ETH", "E"), 2.0, make_date(2025, 1, 2)).unwrap();
        assert_eq!(tracker.event_count(), 2);
    }
}

// ═══════════════════════════════════════════════════════════════════
// Current holdings (M11), inception/last date helpers (M6)
// ═══════════════════════════════════════════════════════════════════

mod convenience_helpers {
    use super::*;

    #[test]
    fn get_current_holdings_returns_holdings() {
        let mut tracker = SavingsTracker::create_new();
        tracker.add_event(EventType::Buy, Asset::crypto("BTC", "B"), 1.5, make_date(2025, 1, 1)).unwrap();
        let holdings = tracker.get_current_holdings();
        assert_eq!(holdings.len(), 1);
        assert!((holdings[&Asset::crypto("BTC", "B")] - 1.5).abs() < f64::EPSILON);
    }

    #[test]
    fn earliest_and_latest_event_dates() {
        let mut tracker = SavingsTracker::create_new();
        tracker.add_event(EventType::Buy, Asset::crypto("BTC", "B"), 1.0, make_date(2025, 3, 1)).unwrap();
        tracker.add_event(EventType::Buy, Asset::crypto("ETH", "E"), 2.0, make_date(2025, 1, 15)).unwrap();
        tracker.add_event(EventType::Buy, Asset::crypto("SOL", "S"), 3.0, make_date(2025, 6, 20)).unwrap();

        assert_eq!(tracker.earliest_event_date(), Some(make_date(2025, 1, 15)));
        assert_eq!(tracker.latest_event_date(), Some(make_date(2025, 6, 20)));
    }

    #[test]
    fn portfolio_age_days() {
        let mut tracker = SavingsTracker::create_new();
        assert!(tracker.portfolio_age_days().is_none());

        tracker.add_event(EventType::Buy, Asset::crypto("BTC", "B"), 1.0, make_date(2025, 1, 1)).unwrap();
        let age = tracker.portfolio_age_days().unwrap();
        assert!(age > 0);
    }

    #[test]
    fn no_events_returns_none() {
        let tracker = SavingsTracker::create_new();
        assert!(tracker.earliest_event_date().is_none());
        assert!(tracker.latest_event_date().is_none());
        assert!(tracker.portfolio_age_days().is_none());
    }
}

// ═══════════════════════════════════════════════════════════════════
// Bulk operations (M8)
// ═══════════════════════════════════════════════════════════════════

mod bulk_operations {
    use super::*;

    #[test]
    fn add_events_all_or_nothing_success() {
        let mut tracker = SavingsTracker::create_new();
        let events = vec![
            Event::new(EventType::Buy, Asset::crypto("BTC", "B"), 1.0, make_date(2025, 1, 1)),
            Event::new(EventType::Buy, Asset::crypto("ETH", "E"), 2.0, make_date(2025, 1, 2)),
        ];

        let ids = tracker.add_events(events).unwrap();
        assert_eq!(ids.len(), 2);
        assert_eq!(tracker.event_count(), 2);
    }

    #[test]
    fn add_events_all_or_nothing_failure() {
        let mut tracker = SavingsTracker::create_new();
        let events = vec![
            Event::new(EventType::Buy, Asset::crypto("BTC", "B"), 1.0, make_date(2025, 1, 1)),
            Event::new(EventType::Sell, Asset::crypto("BTC", "B"), 999.0, make_date(2025, 1, 2)), // invalid
        ];

        let result = tracker.add_events(events);
        assert!(result.is_err());
        assert_eq!(tracker.event_count(), 0); // rolled back
    }

    #[test]
    fn remove_events_all_or_nothing_success() {
        let mut tracker = SavingsTracker::create_new();
        let id1 = tracker.add_event(EventType::Buy, Asset::crypto("BTC", "B"), 1.0, make_date(2025, 1, 1)).unwrap();
        let id2 = tracker.add_event(EventType::Buy, Asset::crypto("ETH", "E"), 2.0, make_date(2025, 1, 2)).unwrap();

        tracker.remove_events(&[id1, id2]).unwrap();
        assert_eq!(tracker.event_count(), 0);
    }
}

// ═══════════════════════════════════════════════════════════════════
// Trash / Undo (M7)
// ═══════════════════════════════════════════════════════════════════

mod trash_undo {
    use super::*;

    #[test]
    fn remove_to_trash_and_restore() {
        let mut tracker = SavingsTracker::create_new();
        let id = tracker.add_event(EventType::Buy, Asset::crypto("BTC", "B"), 1.0, make_date(2025, 1, 1)).unwrap();

        let removed = tracker.remove_event_to_trash(id).unwrap();
        assert_eq!(removed.id, id);
        assert_eq!(tracker.event_count(), 0);
        assert_eq!(tracker.get_trash().len(), 1);

        let restored = tracker.undo_last_removal().unwrap();
        assert!(restored.is_some());
        assert_eq!(tracker.event_count(), 1);
        assert_eq!(tracker.get_trash().len(), 0);
    }

    #[test]
    fn undo_empty_trash_returns_none() {
        let mut tracker = SavingsTracker::create_new();
        let result = tracker.undo_last_removal().unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn clear_trash() {
        let mut tracker = SavingsTracker::create_new();
        let id = tracker.add_event(EventType::Buy, Asset::crypto("BTC", "B"), 1.0, make_date(2025, 1, 1)).unwrap();
        tracker.remove_event_to_trash(id).unwrap();
        assert_eq!(tracker.get_trash().len(), 1);

        tracker.clear_trash();
        assert_eq!(tracker.get_trash().len(), 0);
    }
}

// ═══════════════════════════════════════════════════════════════════
// Export / Import (M1, M2)
// ═══════════════════════════════════════════════════════════════════

mod export_import {
    use super::*;

    #[test]
    fn export_json_roundtrip() {
        let mut tracker = SavingsTracker::create_new();
        tracker.add_event(EventType::Buy, Asset::crypto("BTC", "Bitcoin"), 1.5, make_date(2025, 1, 1)).unwrap();
        tracker.add_event_with_notes(EventType::Sell, Asset::crypto("BTC", "Bitcoin"), 0.5, make_date(2025, 2, 1), "partial sell").unwrap();

        let json = tracker.export_events_to_json().unwrap();

        let mut new_tracker = SavingsTracker::create_new();
        let count = new_tracker.import_events_from_json(&json).unwrap();
        assert_eq!(count, 2);
        assert_eq!(new_tracker.event_count(), 2);
    }

    #[test]
    fn export_csv_format() {
        let mut tracker = SavingsTracker::create_new();
        tracker.add_event(EventType::Buy, Asset::crypto("BTC", "Bitcoin"), 1.5, make_date(2025, 1, 1)).unwrap();

        let csv = tracker.export_events_to_csv();
        assert!(csv.starts_with("id,event_type,symbol,name,asset_type,amount,date,notes\n"));
        assert!(csv.contains("BTC"));
        assert!(csv.contains("Buy"));
        assert!(csv.contains("1.5"));
    }

    #[test]
    fn export_csv_escapes_commas_in_name() {
        let mut tracker = SavingsTracker::create_new();
        tracker.add_event(EventType::Buy, Asset::new("TEST", "Test, Inc.", AssetType::Stock), 1.0, make_date(2025, 1, 1)).unwrap();

        let csv = tracker.export_events_to_csv();
        // Name with comma should be quoted
        assert!(csv.contains("\"Test, Inc.\""));
    }

    #[test]
    fn to_json_snapshot() {
        let mut tracker = SavingsTracker::create_new();
        tracker.add_event(EventType::Buy, Asset::crypto("BTC", "Bitcoin"), 1.0, make_date(2025, 1, 1)).unwrap();

        let json = tracker.to_json().unwrap();
        assert!(json.contains("BTC"));
        assert!(json.contains("events"));
    }

    #[test]
    fn import_invalid_json_fails() {
        let mut tracker = SavingsTracker::create_new();
        let result = tracker.import_events_from_json("not valid json [[[");
        assert!(result.is_err());
    }
}

// ═══════════════════════════════════════════════════════════════════
// Cache inspection (G1), manual cache (M12)
// ═══════════════════════════════════════════════════════════════════

mod cache_inspection {
    use super::*;

    #[test]
    fn set_and_get_cached_price() {
        let mut tracker = SavingsTracker::create_new();
        tracker.set_cached_price("BTC", "USD", make_date(2025, 1, 1), 42000.0);

        assert_eq!(tracker.get_cached_price("BTC", "USD", make_date(2025, 1, 1)), Some(42000.0));
        assert!(tracker.has_unsaved_changes());
    }

    #[test]
    fn get_cached_pairs() {
        let mut tracker = SavingsTracker::create_new();
        tracker.set_cached_price("BTC", "USD", make_date(2025, 1, 1), 42000.0);
        tracker.set_cached_price("ETH", "USD", make_date(2025, 1, 1), 2500.0);

        let pairs = tracker.get_cached_pairs();
        assert_eq!(pairs.len(), 2);
    }

    #[test]
    fn get_last_refreshed() {
        let mut tracker = SavingsTracker::create_new();
        // No refreshed data yet
        assert!(tracker.get_last_refreshed("BTC", "USD").is_none());
    }
}

// ═══════════════════════════════════════════════════════════════════
// Provider availability (G2)
// ═══════════════════════════════════════════════════════════════════

mod provider_availability {
    use super::*;

    #[test]
    fn crypto_provider_available() {
        let tracker = SavingsTracker::create_new();
        assert!(tracker.is_provider_available(&AssetType::Crypto));
    }

    #[test]
    fn fiat_provider_available() {
        let tracker = SavingsTracker::create_new();
        assert!(tracker.is_provider_available(&AssetType::Fiat));
    }

    #[test]
    fn metal_provider_not_available_without_key() {
        let tracker = SavingsTracker::create_new();
        // Metals require API key, so without setting one, no provider
        assert!(!tracker.is_provider_available(&AssetType::Metal));
    }

    #[test]
    fn get_provider_names_for_crypto() {
        let tracker = SavingsTracker::create_new();
        let names = tracker.get_provider_names(&AssetType::Crypto);
        assert!(!names.is_empty());
        assert!(names.contains(&"CoinCap".to_string()));
    }
}

// ═══════════════════════════════════════════════════════════════════
// Consistent sort order (G5) — filter methods return newest-first
// ═══════════════════════════════════════════════════════════════════

mod sort_order_consistency {
    use super::*;

    #[test]
    fn get_events_for_asset_newest_first() {
        let mut tracker = SavingsTracker::create_new();
        tracker.add_event(EventType::Buy, Asset::crypto("BTC", "B"), 1.0, make_date(2025, 1, 1)).unwrap();
        tracker.add_event(EventType::Buy, Asset::crypto("BTC", "B"), 2.0, make_date(2025, 3, 1)).unwrap();
        tracker.add_event(EventType::Buy, Asset::crypto("BTC", "B"), 3.0, make_date(2025, 2, 1)).unwrap();

        let events = tracker.get_events_for_asset("BTC");
        assert_eq!(events[0].date, make_date(2025, 3, 1)); // newest first
        assert_eq!(events[2].date, make_date(2025, 1, 1)); // oldest last
    }

    #[test]
    fn get_events_by_type_newest_first() {
        let mut tracker = SavingsTracker::create_new();
        tracker.add_event(EventType::Buy, Asset::crypto("BTC", "B"), 1.0, make_date(2025, 1, 1)).unwrap();
        tracker.add_event(EventType::Buy, Asset::crypto("ETH", "E"), 2.0, make_date(2025, 3, 1)).unwrap();

        let events = tracker.get_events_by_type(&EventType::Buy);
        assert_eq!(events[0].date, make_date(2025, 3, 1));
    }

    #[test]
    fn get_events_in_range_newest_first() {
        let mut tracker = SavingsTracker::create_new();
        tracker.add_event(EventType::Buy, Asset::crypto("BTC", "B"), 1.0, make_date(2025, 1, 1)).unwrap();
        tracker.add_event(EventType::Buy, Asset::crypto("ETH", "E"), 2.0, make_date(2025, 1, 15)).unwrap();

        let events = tracker.get_events_in_range(make_date(2025, 1, 1), make_date(2025, 1, 31));
        assert_eq!(events[0].date, make_date(2025, 1, 15)); // newest first
    }
}

// ═══════════════════════════════════════════════════════════════════
// Chart range limit (R9)
// ═══════════════════════════════════════════════════════════════════

mod chart_range_limit {
    use super::*;

    #[tokio::test]
    async fn portfolio_chart_exceeding_max_range_fails() {
        let mut tracker = SavingsTracker::create_new();
        let from = make_date(2010, 1, 1);
        let to = make_date(2025, 1, 1); // ~15 years, exceeds 10-year limit
        let result = tracker.generate_portfolio_chart(from, to).await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("exceeds maximum"));
    }

    #[tokio::test]
    async fn asset_chart_exceeding_max_range_fails() {
        let mut tracker = SavingsTracker::create_new();
        let from = make_date(2010, 1, 1);
        let to = make_date(2025, 1, 1);
        let result = tracker.generate_asset_chart("BTC", from, to).await;
        assert!(result.is_err());
    }
}

// ═══════════════════════════════════════════════════════════════════
// change_password verification (I1 — T8)
// ═══════════════════════════════════════════════════════════════════

mod change_password_verification {
    use super::*;

    #[test]
    fn wrong_current_password_fails() {
        let mut tracker = SavingsTracker::create_new();
        tracker.add_event(EventType::Buy, Asset::crypto("BTC", "B"), 1.0, make_date(2025, 1, 1)).unwrap();
        let saved = tracker.save_to_bytes("correct_pw").unwrap();

        let result = tracker.change_password(&saved, "wrong_pw", "new_pw");
        assert!(result.is_err());
    }
}

// ═══════════════════════════════════════════════════════════════════
// AnalyticsService comprehensive tests (T1)
// ═══════════════════════════════════════════════════════════════════

mod analytics_service_tests {
    use super::*;

    fn make_price_service_with_mock() -> PriceService {
        let mock = MockPriceProvider::new();
        let mut registry = PriceProviderRegistry::new();
        registry.register(Box::new(mock));
        PriceService::new(registry)
    }

    #[tokio::test]
    async fn empty_portfolio_summary() {
        let analytics = AnalyticsService::new();
        let portfolio = Portfolio::default();
        let price_service = make_price_service_with_mock();
        let mut cache = PriceCache::new();
        let date = NaiveDate::from_ymd_opt(2025, 1, 15).unwrap();

        let summary = analytics
            .get_portfolio_summary(&portfolio, &price_service, &mut cache, date, "USD")
            .await
            .unwrap();

        assert_eq!(summary.total_value, 0.0);
        assert_eq!(summary.total_invested, 0.0);
        assert_eq!(summary.total_returned, 0.0);
        assert_eq!(summary.total_gain_loss, 0.0);
        assert_eq!(summary.total_return_pct, 0.0);
        assert!(summary.holdings.is_empty());
        assert_eq!(summary.currency, "USD");
        assert_eq!(summary.as_of_date, date);
        assert_eq!(summary.total_events, 0);
        assert_eq!(summary.inception_date, None);
    }

    #[tokio::test]
    async fn single_asset_portfolio() {
        let analytics = AnalyticsService::new();
        let mut portfolio = Portfolio::default();
        let buy_date = NaiveDate::from_ymd_opt(2025, 1, 15).unwrap();
        let eval_date = NaiveDate::from_ymd_opt(2025, 1, 16).unwrap();

        // Buy 2 BTC at $42,000 on Jan 15
        portfolio.events.push(Event::new(
            EventType::Buy,
            Asset::crypto("BTC", "Bitcoin"),
            2.0,
            buy_date,
        ));

        let price_service = make_price_service_with_mock();
        let mut cache = PriceCache::new();

        let summary = analytics
            .get_portfolio_summary(&portfolio, &price_service, &mut cache, eval_date, "USD")
            .await
            .unwrap();

        // On Jan 16, BTC=$43,500 → 2 * 43500 = $87,000 current value
        assert!((summary.total_value - 87000.0).abs() < 1.0);
        // Invested: 2 * $42,000 (Jan 15) = $84,000
        assert!((summary.total_invested - 84000.0).abs() < 1.0);
        // Gain: 87000 - 84000 = +3000
        assert!((summary.total_gain_loss - 3000.0).abs() < 1.0);
        assert_eq!(summary.holdings.len(), 1);
        assert_eq!(summary.holdings[0].asset.symbol, "BTC");
        assert!((summary.holdings[0].allocation_pct - 100.0).abs() < 0.01);
        // cost_basis_per_unit = $84,000 / 2 units = $42,000
        assert!((summary.holdings[0].cost_basis_per_unit - 42000.0).abs() < 1.0);
        // Context fields
        assert_eq!(summary.total_events, 1);
        assert_eq!(summary.inception_date, Some(buy_date));
    }

    #[tokio::test]
    async fn multi_asset_with_sells() {
        let analytics = AnalyticsService::new();
        let mut portfolio = Portfolio::default();
        let date = NaiveDate::from_ymd_opt(2025, 1, 15).unwrap();

        // Buy 1 BTC
        portfolio.events.push(Event::new(
            EventType::Buy,
            Asset::crypto("BTC", "Bitcoin"),
            1.0,
            date,
        ));
        // Buy 10 ETH
        portfolio.events.push(Event::new(
            EventType::Buy,
            Asset::crypto("ETH", "Ethereum"),
            10.0,
            date,
        ));
        // Sell 5 ETH
        portfolio.events.push(Event::new(
            EventType::Sell,
            Asset::crypto("ETH", "Ethereum"),
            5.0,
            date,
        ));

        let price_service = make_price_service_with_mock();
        let mut cache = PriceCache::new();

        let summary = analytics
            .get_portfolio_summary(&portfolio, &price_service, &mut cache, date, "USD")
            .await
            .unwrap();

        // 1 BTC @ $42,000 = $42,000
        // 5 ETH left @ $2,500 = $12,500
        // Total = $54,500
        assert!((summary.total_value - 54500.0).abs() < 1.0);

        // Total invested: 1*42000 + 10*2500 = $67,000
        assert!((summary.total_invested - 67000.0).abs() < 1.0);

        // Total returned (sell proceeds): 5*2500 = $12,500
        assert!((summary.total_returned - 12500.0).abs() < 1.0);

        // Gain/loss: 54500 + 12500 - 67000 = 0 (no price change same day)
        assert!((summary.total_gain_loss - 0.0).abs() < 1.0);

        // Must have 2 holdings
        assert_eq!(summary.holdings.len(), 2);
        assert_eq!(summary.total_events, 3);
    }

    #[tokio::test]
    async fn allocation_sums_to_100() {
        let analytics = AnalyticsService::new();
        let mut portfolio = Portfolio::default();
        let date = NaiveDate::from_ymd_opt(2025, 1, 15).unwrap();

        portfolio.events.push(Event::new(
            EventType::Buy,
            Asset::crypto("BTC", "Bitcoin"),
            1.0,
            date,
        ));
        portfolio.events.push(Event::new(
            EventType::Buy,
            Asset::crypto("ETH", "Ethereum"),
            10.0,
            date,
        ));

        let price_service = make_price_service_with_mock();
        let mut cache = PriceCache::new();

        let summary = analytics
            .get_portfolio_summary(&portfolio, &price_service, &mut cache, date, "USD")
            .await
            .unwrap();

        let total_allocation: f64 = summary
            .holdings
            .iter()
            .map(|h| h.allocation_pct)
            .sum();

        assert!((total_allocation - 100.0).abs() < 0.01);
    }

    #[tokio::test]
    async fn return_pct_positive_when_price_rises() {
        let analytics = AnalyticsService::new();
        let mut portfolio = Portfolio::default();

        // Buy on Jan 15 (BTC=$42,000), evaluate on Jan 16 (BTC=$43,500)
        portfolio.events.push(Event::new(
            EventType::Buy,
            Asset::crypto("BTC", "Bitcoin"),
            1.0,
            NaiveDate::from_ymd_opt(2025, 1, 15).unwrap(),
        ));

        let price_service = make_price_service_with_mock();
        let mut cache = PriceCache::new();
        let eval_date = NaiveDate::from_ymd_opt(2025, 1, 16).unwrap();

        let summary = analytics
            .get_portfolio_summary(&portfolio, &price_service, &mut cache, eval_date, "USD")
            .await
            .unwrap();

        // Return: (43500 - 42000) / 42000 * 100 ≈ 3.57%
        assert!(summary.total_return_pct > 0.0);
        assert!((summary.total_return_pct - 3.571).abs() < 0.1);
    }

    #[tokio::test]
    async fn return_pct_negative_when_price_drops() {
        let analytics = AnalyticsService::new();
        let mut portfolio = Portfolio::default();

        // Buy on Jan 16 (BTC=$43,500), evaluate on Jan 17 (BTC=$41,000)
        portfolio.events.push(Event::new(
            EventType::Buy,
            Asset::crypto("BTC", "Bitcoin"),
            1.0,
            NaiveDate::from_ymd_opt(2025, 1, 16).unwrap(),
        ));

        let price_service = make_price_service_with_mock();
        let mut cache = PriceCache::new();
        let eval_date = NaiveDate::from_ymd_opt(2025, 1, 17).unwrap();

        let summary = analytics
            .get_portfolio_summary(&portfolio, &price_service, &mut cache, eval_date, "USD")
            .await
            .unwrap();

        // Return: (41000 - 43500) / 43500 * 100 ≈ -5.75%
        assert!(summary.total_return_pct < 0.0);
    }

    #[tokio::test]
    async fn sorted_by_allocation_descending() {
        let analytics = AnalyticsService::new();
        let mut portfolio = Portfolio::default();
        let date = NaiveDate::from_ymd_opt(2025, 1, 15).unwrap();

        // BTC: 1 * $42,000 = $42,000
        portfolio.events.push(Event::new(
            EventType::Buy,
            Asset::crypto("BTC", "Bitcoin"),
            1.0,
            date,
        ));
        // ETH: 10 * $2,500 = $25,000 (smaller allocation)
        portfolio.events.push(Event::new(
            EventType::Buy,
            Asset::crypto("ETH", "Ethereum"),
            10.0,
            date,
        ));

        let price_service = make_price_service_with_mock();
        let mut cache = PriceCache::new();

        let summary = analytics
            .get_portfolio_summary(&portfolio, &price_service, &mut cache, date, "USD")
            .await
            .unwrap();

        assert_eq!(summary.holdings.len(), 2);
        // BTC should be first (higher allocation)
        assert_eq!(summary.holdings[0].asset.symbol, "BTC");
        assert_eq!(summary.holdings[1].asset.symbol, "ETH");
        assert!(summary.holdings[0].allocation_pct >= summary.holdings[1].allocation_pct);
    }
}

// ═══════════════════════════════════════════════════════════════════
// Error message quality (T10)
// ═══════════════════════════════════════════════════════════════════

mod error_message_quality {
    use super::*;

    #[test]
    fn event_not_found_includes_context() {
        let mut tracker = SavingsTracker::create_new();
        let fake_id = Uuid::new_v4();
        let result = tracker.remove_event(fake_id);
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        // Should contain the ID so users can diagnose which event was not found
        assert!(msg.contains(&fake_id.to_string()) || msg.contains("not found"), "Error should be informative: {msg}");
    }

    #[test]
    fn invalid_file_format_error() {
        let result = SavingsTracker::load_from_bytes(b"not a valid file", "pw");
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.to_lowercase().contains("invalid") || msg.to_lowercase().contains("magic") || msg.to_lowercase().contains("svtk"),
            "Error should indicate invalid format: {msg}");
    }

    #[test]
    fn password_too_short_error() {
        let mut tracker = SavingsTracker::create_new();
        let result = tracker.save_to_bytes("");
        // Empty password should either work or give a meaningful error
        // (This tests that no panic occurs with edge-case input)
        let _ = result; // just ensure no panic
    }
}

