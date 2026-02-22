use chrono::NaiveDate;
use savings_tracker_core::models::asset::{Asset, AssetType};
use savings_tracker_core::models::chart::{ChartDataPoint, ChartEvent};
use savings_tracker_core::models::event::{Event, EventType};
use savings_tracker_core::models::portfolio::Portfolio;
use savings_tracker_core::models::price::{PriceCache, PricePoint};
use savings_tracker_core::models::settings::Settings;
use std::collections::{HashMap, HashSet};

fn d(y: i32, m: u32, day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(y, m, day).unwrap()
}

// ═══════════════════════════════════════════════════════════════════
//  AssetType
// ═══════════════════════════════════════════════════════════════════

mod asset_type {
    use super::*;

    #[test]
    fn display_crypto() {
        assert_eq!(AssetType::Crypto.to_string(), "Crypto");
    }

    #[test]
    fn display_fiat() {
        assert_eq!(AssetType::Fiat.to_string(), "Fiat");
    }

    #[test]
    fn display_metal() {
        assert_eq!(AssetType::Metal.to_string(), "Metal");
    }

    #[test]
    fn display_stock() {
        assert_eq!(AssetType::Stock.to_string(), "Stock");
    }

    #[test]
    fn equality() {
        assert_eq!(AssetType::Crypto, AssetType::Crypto);
        assert_ne!(AssetType::Crypto, AssetType::Fiat);
        assert_ne!(AssetType::Metal, AssetType::Stock);
    }

    #[test]
    fn clone() {
        let a = AssetType::Crypto;
        let b = a.clone();
        assert_eq!(a, b);
    }

    #[test]
    fn serde_roundtrip_json() {
        for at in [AssetType::Crypto, AssetType::Fiat, AssetType::Metal, AssetType::Stock] {
            let json = serde_json::to_string(&at).unwrap();
            let back: AssetType = serde_json::from_str(&json).unwrap();
            assert_eq!(at, back);
        }
    }
}

// ═══════════════════════════════════════════════════════════════════
//  Asset
// ═══════════════════════════════════════════════════════════════════

mod asset {
    use super::*;

    // ── Asset::new ────────────────────────────────────────────────

    #[test]
    fn new_uppercases_lowercase_symbol() {
        let a = Asset::new("btc", "Bitcoin", AssetType::Crypto);
        assert_eq!(a.symbol, "BTC");
    }

    #[test]
    fn new_uppercases_mixed_case_symbol() {
        let a = Asset::new("eTh", "Ethereum", AssetType::Crypto);
        assert_eq!(a.symbol, "ETH");
    }

    #[test]
    fn new_preserves_already_uppercase() {
        let a = Asset::new("AAPL", "Apple", AssetType::Stock);
        assert_eq!(a.symbol, "AAPL");
    }

    #[test]
    fn new_preserves_name_case() {
        let a = Asset::new("btc", "Bitcoin", AssetType::Crypto);
        assert_eq!(a.name, "Bitcoin");
    }

    #[test]
    fn new_sets_asset_type() {
        let a = Asset::new("XAU", "Gold", AssetType::Metal);
        assert_eq!(a.asset_type, AssetType::Metal);
    }

    // ── Convenience constructors ──────────────────────────────────

    #[test]
    fn crypto_constructor() {
        let a = Asset::crypto("btc", "Bitcoin");
        assert_eq!(a.symbol, "BTC");
        assert_eq!(a.name, "Bitcoin");
        assert_eq!(a.asset_type, AssetType::Crypto);
    }

    #[test]
    fn fiat_constructor() {
        let a = Asset::fiat("usd", "US Dollar");
        assert_eq!(a.symbol, "USD");
        assert_eq!(a.name, "US Dollar");
        assert_eq!(a.asset_type, AssetType::Fiat);
    }

    #[test]
    fn metal_constructor() {
        let a = Asset::metal("xau", "Gold");
        assert_eq!(a.symbol, "XAU");
        assert_eq!(a.name, "Gold");
        assert_eq!(a.asset_type, AssetType::Metal);
    }

    #[test]
    fn stock_constructor() {
        let a = Asset::stock("aapl", "Apple Inc.");
        assert_eq!(a.symbol, "AAPL");
        assert_eq!(a.name, "Apple Inc.");
        assert_eq!(a.asset_type, AssetType::Stock);
    }

    // ── Equality & Hashing ────────────────────────────────────────

    #[test]
    fn equality_same_fields() {
        let a = Asset::crypto("BTC", "Bitcoin");
        let b = Asset::crypto("BTC", "Bitcoin");
        assert_eq!(a, b);
    }

    #[test]
    fn inequality_different_symbol() {
        let a = Asset::crypto("BTC", "Bitcoin");
        let b = Asset::crypto("ETH", "Ethereum");
        assert_ne!(a, b);
    }

    #[test]
    fn inequality_different_type_same_symbol() {
        let a = Asset::new("USD", "Dollar", AssetType::Fiat);
        let b = Asset::new("USD", "Dollar", AssetType::Crypto);
        assert_ne!(a, b);
    }

    #[test]
    fn equality_different_name_same_symbol_and_type() {
        // Asset equality is based on (symbol, asset_type) only, NOT name
        let a = Asset::crypto("BTC", "Bitcoin");
        let b = Asset::crypto("BTC", "BTC Token");
        assert_eq!(a, b);
    }

    #[test]
    fn hash_consistent_for_equal() {
        use std::hash::{Hash, Hasher};
        use std::collections::hash_map::DefaultHasher;
        let a = Asset::crypto("BTC", "Bitcoin");
        let b = Asset::crypto("BTC", "Bitcoin");
        let mut ha = DefaultHasher::new();
        let mut hb = DefaultHasher::new();
        a.hash(&mut ha);
        b.hash(&mut hb);
        assert_eq!(ha.finish(), hb.finish());
    }

    #[test]
    fn works_as_hashset_key() {
        let mut set = HashSet::new();
        set.insert(Asset::crypto("BTC", "Bitcoin"));
        set.insert(Asset::crypto("BTC", "Bitcoin")); // duplicate
        set.insert(Asset::crypto("ETH", "Ethereum"));
        assert_eq!(set.len(), 2);
    }

    #[test]
    fn works_as_hashmap_key() {
        let mut map = HashMap::new();
        map.insert(Asset::crypto("BTC", "Bitcoin"), 1.0);
        map.insert(Asset::crypto("ETH", "Ethereum"), 10.0);
        assert_eq!(map.get(&Asset::crypto("BTC", "Bitcoin")), Some(&1.0));
    }

    // ── Clone ─────────────────────────────────────────────────────

    #[test]
    fn clone_preserves_fields() {
        let a = Asset::crypto("BTC", "Bitcoin");
        let b = a.clone();
        assert_eq!(a, b);
    }

    // ── Serialization ─────────────────────────────────────────────

    #[test]
    fn serde_roundtrip_json() {
        let a = Asset::crypto("BTC", "Bitcoin");
        let json = serde_json::to_string(&a).unwrap();
        let back: Asset = serde_json::from_str(&json).unwrap();
        assert_eq!(a, back);
    }

    #[test]
    fn serde_roundtrip_bincode() {
        let a = Asset::stock("AAPL", "Apple Inc.");
        let bytes = bincode::serialize(&a).unwrap();
        let back: Asset = bincode::deserialize(&bytes).unwrap();
        assert_eq!(a, back);
    }

    // ── Edge cases ────────────────────────────────────────────────

    #[test]
    fn empty_symbol() {
        let a = Asset::crypto("", "No Symbol");
        assert_eq!(a.symbol, "");
    }

    #[test]
    fn empty_name() {
        let a = Asset::crypto("BTC", "");
        assert_eq!(a.name, "");
    }

    #[test]
    fn unicode_name() {
        let a = Asset::fiat("PLN", "Złoty");
        assert_eq!(a.name, "Złoty");
    }
}

// ═══════════════════════════════════════════════════════════════════
//  EventType
// ═══════════════════════════════════════════════════════════════════

mod event_type {
    use super::*;

    #[test]
    fn display_buy() {
        assert_eq!(EventType::Buy.to_string(), "Buy");
    }

    #[test]
    fn display_sell() {
        assert_eq!(EventType::Sell.to_string(), "Sell");
    }

    #[test]
    fn equality() {
        assert_eq!(EventType::Buy, EventType::Buy);
        assert_eq!(EventType::Sell, EventType::Sell);
        assert_ne!(EventType::Buy, EventType::Sell);
    }

    #[test]
    fn clone() {
        let a = EventType::Buy;
        let b = a.clone();
        assert_eq!(a, b);
    }

    #[test]
    fn serde_roundtrip() {
        for et in [EventType::Buy, EventType::Sell] {
            let json = serde_json::to_string(&et).unwrap();
            let back: EventType = serde_json::from_str(&json).unwrap();
            assert_eq!(et, back);
        }
    }
}

// ═══════════════════════════════════════════════════════════════════
//  Event
// ═══════════════════════════════════════════════════════════════════

mod event {
    use super::*;

    fn sample_date() -> NaiveDate {
        NaiveDate::from_ymd_opt(2025, 6, 15).unwrap()
    }

    #[test]
    fn new_generates_unique_ids() {
        let e1 = Event::new(EventType::Buy, Asset::crypto("BTC", "Bitcoin"), 1.0, sample_date());
        let e2 = Event::new(EventType::Buy, Asset::crypto("BTC", "Bitcoin"), 1.0, sample_date());
        assert_ne!(e1.id, e2.id);
    }

    #[test]
    fn positive_amount_stays_positive() {
        let e = Event::new(EventType::Buy, Asset::crypto("BTC", "Bitcoin"), 5.5, sample_date());
        assert_eq!(e.amount, 5.5);
    }

    #[test]
    fn negative_amount_preserved() {
        let e = Event::new(EventType::Buy, Asset::crypto("BTC", "Bitcoin"), -3.7, sample_date());
        assert_eq!(e.amount, -3.7);
    }

    #[test]
    fn zero_amount_stays_zero() {
        let e = Event::new(EventType::Buy, Asset::crypto("BTC", "Bitcoin"), 0.0, sample_date());
        assert_eq!(e.amount, 0.0);
    }

    #[test]
    fn preserves_event_type() {
        let buy = Event::new(EventType::Buy, Asset::crypto("BTC", "B"), 1.0, sample_date());
        let sell = Event::new(EventType::Sell, Asset::crypto("BTC", "B"), 1.0, sample_date());
        assert_eq!(buy.event_type, EventType::Buy);
        assert_eq!(sell.event_type, EventType::Sell);
    }

    #[test]
    fn preserves_asset() {
        let asset = Asset::stock("AAPL", "Apple");
        let e = Event::new(EventType::Buy, asset.clone(), 10.0, sample_date());
        assert_eq!(e.asset, asset);
    }

    #[test]
    fn preserves_date() {
        let dt = NaiveDate::from_ymd_opt(2024, 12, 25).unwrap();
        let e = Event::new(EventType::Buy, Asset::crypto("BTC", "Bitcoin"), 1.0, dt);
        assert_eq!(e.date, dt);
    }

    #[test]
    fn serde_roundtrip_bincode() {
        let e = Event::new(EventType::Buy, Asset::crypto("BTC", "Bitcoin"), 0.5, sample_date());
        let bytes = bincode::serialize(&e).unwrap();
        let back: Event = bincode::deserialize(&bytes).unwrap();
        assert_eq!(e.id, back.id);
        assert_eq!(e.event_type, back.event_type);
        assert_eq!(e.asset, back.asset);
        assert_eq!(e.amount, back.amount);
        assert_eq!(e.date, back.date);
    }

    #[test]
    fn serde_roundtrip_json() {
        let e = Event::new(EventType::Sell, Asset::fiat("USD", "Dollar"), 1000.0, sample_date());
        let json = serde_json::to_string(&e).unwrap();
        let back: Event = serde_json::from_str(&json).unwrap();
        assert_eq!(e.id, back.id);
        assert_eq!(e.amount, back.amount);
    }

    #[test]
    fn clone_preserves_all() {
        let e = Event::new(EventType::Buy, Asset::crypto("ETH", "Ethereum"), 10.0, sample_date());
        let c = e.clone();
        assert_eq!(e.id, c.id);
        assert_eq!(e.event_type, c.event_type);
        assert_eq!(e.amount, c.amount);
    }

    #[test]
    fn debug_format_contains_fields() {
        let e = Event::new(EventType::Buy, Asset::crypto("BTC", "Bitcoin"), 1.0, sample_date());
        let debug = format!("{:?}", e);
        assert!(debug.contains("Buy"));
        assert!(debug.contains("BTC"));
    }

    #[test]
    fn very_small_amount() {
        let e = Event::new(EventType::Buy, Asset::crypto("BTC", "Bitcoin"), 0.00000001, sample_date());
        assert!(e.amount > 0.0);
        assert!((e.amount - 0.00000001).abs() < f64::EPSILON);
    }

    #[test]
    fn very_large_amount() {
        let e = Event::new(EventType::Buy, Asset::fiat("USD", "Dollar"), 1_000_000_000.0, sample_date());
        assert_eq!(e.amount, 1_000_000_000.0);
    }
}

// ═══════════════════════════════════════════════════════════════════
//  PricePoint
// ═══════════════════════════════════════════════════════════════════

mod price_point {
    use super::*;

    #[test]
    fn equality() {
        let a = PricePoint { date: d(2025, 1, 15), price: 42000.0 };
        let b = PricePoint { date: d(2025, 1, 15), price: 42000.0 };
        assert_eq!(a, b);
    }

    #[test]
    fn inequality_different_price() {
        let a = PricePoint { date: d(2025, 1, 15), price: 42000.0 };
        let b = PricePoint { date: d(2025, 1, 15), price: 43000.0 };
        assert_ne!(a, b);
    }

    #[test]
    fn inequality_different_date() {
        let a = PricePoint { date: d(2025, 1, 15), price: 42000.0 };
        let b = PricePoint { date: d(2025, 1, 16), price: 42000.0 };
        assert_ne!(a, b);
    }

    #[test]
    fn clone() {
        let a = PricePoint { date: d(2025, 1, 15), price: 42000.0 };
        let b = a.clone();
        assert_eq!(a, b);
    }

    #[test]
    fn serde_roundtrip() {
        let p = PricePoint { date: d(2025, 1, 15), price: 42000.0 };
        let bytes = bincode::serialize(&p).unwrap();
        let back: PricePoint = bincode::deserialize(&bytes).unwrap();
        assert_eq!(p, back);
    }
}

// ═══════════════════════════════════════════════════════════════════
//  PriceCache
// ═══════════════════════════════════════════════════════════════════

mod price_cache {
    use super::*;

    // ── Construction ──────────────────────────────────────────────

    #[test]
    fn new_is_empty() {
        let cache = PriceCache::new();
        assert!(cache.entries.is_empty());
        assert!(cache.last_updated.is_empty());
    }

    #[test]
    fn default_is_empty() {
        let cache = PriceCache::default();
        assert!(cache.entries.is_empty());
    }

    // ── get_price / set_price ─────────────────────────────────────

    #[test]
    fn set_and_get_price() {
        let mut cache = PriceCache::new();
        cache.set_price("BTC", "USD", d(2025, 1, 15), 42000.0);
        assert_eq!(cache.get_price("BTC", "USD", d(2025, 1, 15)), Some(42000.0));
    }

    #[test]
    fn get_nonexistent_symbol() {
        let cache = PriceCache::new();
        assert_eq!(cache.get_price("BTC", "USD", d(2025, 1, 15)), None);
    }

    #[test]
    fn get_nonexistent_date() {
        let mut cache = PriceCache::new();
        cache.set_price("BTC", "USD", d(2025, 1, 15), 42000.0);
        assert_eq!(cache.get_price("BTC", "USD", d(2025, 1, 16)), None);
    }

    #[test]
    fn get_wrong_currency() {
        let mut cache = PriceCache::new();
        cache.set_price("BTC", "USD", d(2025, 1, 15), 42000.0);
        assert_eq!(cache.get_price("BTC", "EUR", d(2025, 1, 15)), None);
    }

    #[test]
    fn case_insensitive_get() {
        let mut cache = PriceCache::new();
        cache.set_price("BTC", "USD", d(2025, 1, 15), 42000.0);
        assert_eq!(cache.get_price("btc", "usd", d(2025, 1, 15)), Some(42000.0));
        assert_eq!(cache.get_price("Btc", "Usd", d(2025, 1, 15)), Some(42000.0));
    }

    #[test]
    fn case_insensitive_set_stores_uppercase() {
        let mut cache = PriceCache::new();
        cache.set_price("btc", "usd", d(2025, 1, 15), 42000.0);
        assert_eq!(cache.get_price("BTC", "USD", d(2025, 1, 15)), Some(42000.0));
    }

    #[test]
    fn update_existing_price() {
        let mut cache = PriceCache::new();
        cache.set_price("BTC", "USD", d(2025, 1, 15), 42000.0);
        cache.set_price("BTC", "USD", d(2025, 1, 15), 43000.0);
        assert_eq!(cache.get_price("BTC", "USD", d(2025, 1, 15)), Some(43000.0));
    }

    #[test]
    fn maintains_sort_order() {
        let mut cache = PriceCache::new();
        cache.set_price("BTC", "USD", d(2025, 1, 17), 41000.0);
        cache.set_price("BTC", "USD", d(2025, 1, 15), 42000.0);
        cache.set_price("BTC", "USD", d(2025, 1, 16), 43000.0);

        let key = ("BTC".to_string(), "USD".to_string());
        let entries = cache.entries.get(&key).unwrap();
        assert_eq!(entries[0].date, d(2025, 1, 15));
        assert_eq!(entries[1].date, d(2025, 1, 16));
        assert_eq!(entries[2].date, d(2025, 1, 17));
    }

    #[test]
    fn multiple_symbols() {
        let mut cache = PriceCache::new();
        cache.set_price("BTC", "USD", d(2025, 1, 15), 42000.0);
        cache.set_price("ETH", "USD", d(2025, 1, 15), 2500.0);
        assert_eq!(cache.get_price("BTC", "USD", d(2025, 1, 15)), Some(42000.0));
        assert_eq!(cache.get_price("ETH", "USD", d(2025, 1, 15)), Some(2500.0));
    }

    #[test]
    fn multiple_currencies() {
        let mut cache = PriceCache::new();
        cache.set_price("BTC", "USD", d(2025, 1, 15), 42000.0);
        cache.set_price("BTC", "EUR", d(2025, 1, 15), 38000.0);
        assert_eq!(cache.get_price("BTC", "USD", d(2025, 1, 15)), Some(42000.0));
        assert_eq!(cache.get_price("BTC", "EUR", d(2025, 1, 15)), Some(38000.0));
    }

    // ── set_prices (bulk) ─────────────────────────────────────────

    #[test]
    fn set_prices_bulk() {
        let mut cache = PriceCache::new();
        let points = vec![
            PricePoint { date: d(2025, 1, 15), price: 42000.0 },
            PricePoint { date: d(2025, 1, 16), price: 43000.0 },
            PricePoint { date: d(2025, 1, 17), price: 41000.0 },
        ];
        cache.set_prices("BTC", "USD", &points);
        assert_eq!(cache.get_price("BTC", "USD", d(2025, 1, 15)), Some(42000.0));
        assert_eq!(cache.get_price("BTC", "USD", d(2025, 1, 16)), Some(43000.0));
        assert_eq!(cache.get_price("BTC", "USD", d(2025, 1, 17)), Some(41000.0));
    }

    #[test]
    fn set_prices_empty_slice() {
        let mut cache = PriceCache::new();
        cache.set_prices("BTC", "USD", &[]);
        assert!(cache.entries.is_empty());
    }

    #[test]
    fn set_prices_overwrites_existing() {
        let mut cache = PriceCache::new();
        cache.set_price("BTC", "USD", d(2025, 1, 15), 42000.0);
        let points = vec![PricePoint { date: d(2025, 1, 15), price: 99000.0 }];
        cache.set_prices("BTC", "USD", &points);
        assert_eq!(cache.get_price("BTC", "USD", d(2025, 1, 15)), Some(99000.0));
    }

    // ── is_today_fresh / mark_updated_today ───────────────────────

    #[test]
    fn is_today_fresh_not_marked() {
        let cache = PriceCache::new();
        assert!(!cache.is_today_fresh("BTC", "USD", d(2025, 1, 15)));
    }

    #[test]
    fn is_today_fresh_after_marking() {
        let mut cache = PriceCache::new();
        let today = d(2025, 1, 15);
        cache.mark_updated_today("BTC", "USD", today);
        assert!(cache.is_today_fresh("BTC", "USD", today));
    }

    #[test]
    fn is_today_fresh_wrong_day() {
        let mut cache = PriceCache::new();
        cache.mark_updated_today("BTC", "USD", d(2025, 1, 15));
        assert!(!cache.is_today_fresh("BTC", "USD", d(2025, 1, 16)));
    }

    #[test]
    fn is_today_fresh_case_insensitive() {
        let mut cache = PriceCache::new();
        cache.mark_updated_today("btc", "usd", d(2025, 1, 15));
        assert!(cache.is_today_fresh("BTC", "USD", d(2025, 1, 15)));
    }

    #[test]
    fn is_today_fresh_different_symbol() {
        let mut cache = PriceCache::new();
        cache.mark_updated_today("BTC", "USD", d(2025, 1, 15));
        assert!(!cache.is_today_fresh("ETH", "USD", d(2025, 1, 15)));
    }

    #[test]
    fn mark_updated_today_overwrites() {
        let mut cache = PriceCache::new();
        cache.mark_updated_today("BTC", "USD", d(2025, 1, 15));
        cache.mark_updated_today("BTC", "USD", d(2025, 1, 16));
        assert!(!cache.is_today_fresh("BTC", "USD", d(2025, 1, 15)));
        assert!(cache.is_today_fresh("BTC", "USD", d(2025, 1, 16)));
    }

    // ── get_price_range ───────────────────────────────────────────

    #[test]
    fn get_price_range_normal() {
        let mut cache = PriceCache::new();
        cache.set_price("BTC", "USD", d(2025, 1, 15), 42000.0);
        cache.set_price("BTC", "USD", d(2025, 1, 16), 43000.0);
        cache.set_price("BTC", "USD", d(2025, 1, 17), 41000.0);
        let range = cache.get_price_range("BTC", "USD", d(2025, 1, 15), d(2025, 1, 17));
        assert_eq!(range.len(), 3);
        assert_eq!(range[0].price, 42000.0);
        assert_eq!(range[2].price, 41000.0);
    }

    #[test]
    fn get_price_range_empty_cache() {
        let cache = PriceCache::new();
        let range = cache.get_price_range("BTC", "USD", d(2025, 1, 15), d(2025, 1, 17));
        assert!(range.is_empty());
    }

    #[test]
    fn get_price_range_no_overlap() {
        let mut cache = PriceCache::new();
        cache.set_price("BTC", "USD", d(2025, 1, 10), 40000.0);
        let range = cache.get_price_range("BTC", "USD", d(2025, 1, 15), d(2025, 1, 17));
        assert!(range.is_empty());
    }

    #[test]
    fn get_price_range_partial_overlap() {
        let mut cache = PriceCache::new();
        cache.set_price("BTC", "USD", d(2025, 1, 14), 40000.0);
        cache.set_price("BTC", "USD", d(2025, 1, 15), 42000.0);
        cache.set_price("BTC", "USD", d(2025, 1, 16), 43000.0);
        cache.set_price("BTC", "USD", d(2025, 1, 18), 44000.0);
        let range = cache.get_price_range("BTC", "USD", d(2025, 1, 15), d(2025, 1, 17));
        assert_eq!(range.len(), 2);
    }

    #[test]
    fn get_price_range_single_day() {
        let mut cache = PriceCache::new();
        cache.set_price("BTC", "USD", d(2025, 1, 15), 42000.0);
        let range = cache.get_price_range("BTC", "USD", d(2025, 1, 15), d(2025, 1, 15));
        assert_eq!(range.len(), 1);
        assert_eq!(range[0].price, 42000.0);
    }

    #[test]
    fn get_price_range_case_insensitive() {
        let mut cache = PriceCache::new();
        cache.set_price("BTC", "USD", d(2025, 1, 15), 42000.0);
        let range = cache.get_price_range("btc", "usd", d(2025, 1, 15), d(2025, 1, 15));
        assert_eq!(range.len(), 1);
    }

    // ── Serialization roundtrip ───────────────────────────────────

    #[test]
    fn serde_roundtrip() {
        let mut cache = PriceCache::new();
        cache.set_price("BTC", "USD", d(2025, 1, 15), 42000.0);
        cache.set_price("ETH", "USD", d(2025, 1, 15), 2500.0);
        cache.mark_updated_today("BTC", "USD", d(2025, 1, 15));

        let bytes = bincode::serialize(&cache).unwrap();
        let back: PriceCache = bincode::deserialize(&bytes).unwrap();

        assert_eq!(back.get_price("BTC", "USD", d(2025, 1, 15)), Some(42000.0));
        assert_eq!(back.get_price("ETH", "USD", d(2025, 1, 15)), Some(2500.0));
        assert!(back.is_today_fresh("BTC", "USD", d(2025, 1, 15)));
    }
}

// ═══════════════════════════════════════════════════════════════════
//  Settings
// ═══════════════════════════════════════════════════════════════════

#[allow(clippy::field_reassign_with_default)]
mod settings {
    use super::*;

    #[test]
    fn default_currency_is_usd() {
        let s = Settings::default();
        assert_eq!(s.default_currency, "USD");
    }

    #[test]
    fn default_api_keys_empty() {
        let s = Settings::default();
        assert!(s.api_keys.is_empty());
    }

    #[test]
    fn custom_currency() {
        let mut s = Settings::default();
        s.default_currency = "PLN".to_string();
        assert_eq!(s.default_currency, "PLN");
    }

    #[test]
    fn add_api_key() {
        let mut s = Settings::default();
        s.api_keys.insert("metals_dev".to_string(), "key123".to_string());
        assert_eq!(s.api_keys.get("metals_dev"), Some(&"key123".to_string()));
    }

    #[test]
    fn multiple_api_keys() {
        let mut s = Settings::default();
        s.api_keys.insert("metals_dev".to_string(), "key1".to_string());
        s.api_keys.insert("alphavantage".to_string(), "key2".to_string());
        assert_eq!(s.api_keys.len(), 2);
    }

    #[test]
    fn serde_roundtrip_bincode() {
        let mut s = Settings::default();
        s.default_currency = "EUR".to_string();
        s.api_keys.insert("metals_dev".to_string(), "secret".to_string());
        let bytes = bincode::serialize(&s).unwrap();
        let back: Settings = bincode::deserialize(&bytes).unwrap();
        assert_eq!(back.default_currency, "EUR");
        assert_eq!(back.api_keys.get("metals_dev"), Some(&"secret".to_string()));
    }

    #[test]
    fn serde_roundtrip_json() {
        let mut s = Settings::default();
        s.default_currency = "GBP".to_string();
        let json = serde_json::to_string(&s).unwrap();
        let back: Settings = serde_json::from_str(&json).unwrap();
        assert_eq!(back.default_currency, "GBP");
    }

    #[test]
    fn clone_preserves_fields() {
        let mut s = Settings::default();
        s.default_currency = "PLN".to_string();
        let c = s.clone();
        assert_eq!(s.default_currency, c.default_currency);
    }
}

// ═══════════════════════════════════════════════════════════════════
//  ChartDataPoint / ChartEvent
// ═══════════════════════════════════════════════════════════════════

mod chart {
    use super::*;

    #[test]
    fn data_point_creation() {
        let point = ChartDataPoint {
            date: d(2025, 1, 15),
            portfolio_value: 10000.0,
            events: vec![],
        };
        assert_eq!(point.date, d(2025, 1, 15));
        assert_eq!(point.portfolio_value, 10000.0);
        assert!(point.events.is_empty());
    }

    #[test]
    fn data_point_with_events() {
        let event = ChartEvent {
            event_type: EventType::Buy,
            asset_symbol: "BTC".to_string(),
            amount: 0.5,
            value_in_default_currency: 21000.0,
        };
        let point = ChartDataPoint {
            date: d(2025, 1, 15),
            portfolio_value: 21000.0,
            events: vec![event],
        };
        assert_eq!(point.events.len(), 1);
        assert_eq!(point.events[0].asset_symbol, "BTC");
    }

    #[test]
    fn chart_event_buy() {
        let e = ChartEvent {
            event_type: EventType::Buy,
            asset_symbol: "ETH".to_string(),
            amount: 10.0,
            value_in_default_currency: 25000.0,
        };
        assert_eq!(e.event_type, EventType::Buy);
        assert_eq!(e.amount, 10.0);
    }

    #[test]
    fn chart_event_sell() {
        let e = ChartEvent {
            event_type: EventType::Sell,
            asset_symbol: "BTC".to_string(),
            amount: 0.1,
            value_in_default_currency: 4200.0,
        };
        assert_eq!(e.event_type, EventType::Sell);
    }

    #[test]
    fn data_point_clone() {
        let point = ChartDataPoint {
            date: d(2025, 1, 15),
            portfolio_value: 5000.0,
            events: vec![ChartEvent {
                event_type: EventType::Buy,
                asset_symbol: "BTC".to_string(),
                amount: 0.1,
                value_in_default_currency: 4200.0,
            }],
        };
        let c = point.clone();
        assert_eq!(c.portfolio_value, point.portfolio_value);
        assert_eq!(c.events.len(), 1);
    }

    #[test]
    fn data_point_serde_roundtrip() {
        let point = ChartDataPoint {
            date: d(2025, 1, 15),
            portfolio_value: 10000.0,
            events: vec![ChartEvent {
                event_type: EventType::Buy,
                asset_symbol: "BTC".to_string(),
                amount: 0.5,
                value_in_default_currency: 21000.0,
            }],
        };
        let json = serde_json::to_string(&point).unwrap();
        let back: ChartDataPoint = serde_json::from_str(&json).unwrap();
        assert_eq!(back.portfolio_value, 10000.0);
        assert_eq!(back.events[0].asset_symbol, "BTC");
    }
}

// ═══════════════════════════════════════════════════════════════════
//  Portfolio
// ═══════════════════════════════════════════════════════════════════

mod portfolio {
    use super::*;

    #[test]
    fn default_has_empty_events() {
        let p = Portfolio::default();
        assert!(p.events.is_empty());
    }

    #[test]
    fn default_settings_usd() {
        let p = Portfolio::default();
        assert_eq!(p.settings.default_currency, "USD");
    }

    #[test]
    fn default_cache_empty() {
        let p = Portfolio::default();
        assert!(p.price_cache.entries.is_empty());
    }

    #[test]
    fn clone_preserves_events() {
        let mut p = Portfolio::default();
        p.events.push(Event::new(
            EventType::Buy,
            Asset::crypto("BTC", "Bitcoin"),
            1.0,
            NaiveDate::from_ymd_opt(2025, 1, 15).unwrap(),
        ));
        let c = p.clone();
        assert_eq!(c.events.len(), 1);
        assert_eq!(c.events[0].asset.symbol, "BTC");
    }

    #[test]
    fn serde_roundtrip() {
        let mut p = Portfolio::default();
        p.settings.default_currency = "PLN".to_string();
        p.events.push(Event::new(
            EventType::Buy,
            Asset::crypto("ETH", "Ethereum"),
            10.0,
            NaiveDate::from_ymd_opt(2025, 2, 1).unwrap(),
        ));
        p.price_cache.set_price(
            "ETH", "USD",
            NaiveDate::from_ymd_opt(2025, 2, 1).unwrap(),
            2500.0,
        );

        let bytes = bincode::serialize(&p).unwrap();
        let back: Portfolio = bincode::deserialize(&bytes).unwrap();
        assert_eq!(back.events.len(), 1);
        assert_eq!(back.settings.default_currency, "PLN");
        assert_eq!(
            back.price_cache.get_price("ETH", "USD", NaiveDate::from_ymd_opt(2025, 2, 1).unwrap()),
            Some(2500.0),
        );
    }
}

// ═══════════════════════════════════════════════════════════════════
// PriceCache prune edge cases (T9)
// ═══════════════════════════════════════════════════════════════════

mod price_cache_prune_edge_cases {
    use savings_tracker_core::models::price::PriceCache;
    use chrono::NaiveDate;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    #[test]
    fn prune_exact_date_match() {
        let mut cache = PriceCache::new();
        cache.set_price("BTC", "USD", d(2025, 1, 10), 40000.0);
        cache.set_price("BTC", "USD", d(2025, 1, 15), 42000.0);
        cache.set_price("BTC", "USD", d(2025, 1, 20), 44000.0);

        // Prune before 2025-01-15 → removes Jan 10, keeps Jan 15 and Jan 20
        let removed = cache.prune_before(d(2025, 1, 15));
        assert_eq!(removed, 1);
        assert_eq!(cache.get_price("BTC", "USD", d(2025, 1, 10)), None);
        assert_eq!(cache.get_price("BTC", "USD", d(2025, 1, 15)), Some(42000.0));
    }

    #[test]
    fn prune_all_entries() {
        let mut cache = PriceCache::new();
        cache.set_price("BTC", "USD", d(2025, 1, 1), 40000.0);
        cache.set_price("BTC", "USD", d(2025, 1, 2), 41000.0);

        let removed = cache.prune_before(d(2026, 1, 1));
        assert_eq!(removed, 2);
        assert_eq!(cache.total_entries(), 0);
        assert_eq!(cache.asset_count(), 0);
    }

    #[test]
    fn prune_with_nothing_to_remove() {
        let mut cache = PriceCache::new();
        cache.set_price("BTC", "USD", d(2025, 6, 1), 50000.0);

        let removed = cache.prune_before(d(2025, 1, 1));
        assert_eq!(removed, 0);
        assert_eq!(cache.total_entries(), 1);
    }

    #[test]
    fn prune_multiple_pairs() {
        let mut cache = PriceCache::new();
        cache.set_price("BTC", "USD", d(2025, 1, 1), 40000.0);
        cache.set_price("BTC", "USD", d(2025, 6, 1), 50000.0);
        cache.set_price("ETH", "USD", d(2025, 1, 1), 2500.0);
        cache.set_price("ETH", "USD", d(2025, 6, 1), 3000.0);

        let removed = cache.prune_before(d(2025, 3, 1));
        assert_eq!(removed, 2); // one from each pair
        assert_eq!(cache.total_entries(), 2);
    }

    #[test]
    fn prune_cleans_stale_last_updated() {
        let mut cache = PriceCache::new();
        cache.set_price("BTC", "USD", d(2025, 1, 1), 40000.0);
        cache.mark_updated_today("BTC", "USD", d(2025, 1, 1));

        // Prune all → should also clean last_updated
        cache.prune_before(d(2026, 1, 1));
        assert_eq!(cache.asset_count(), 0);
        assert!(cache.last_updated.is_empty());
    }

    #[test]
    fn prune_empty_cache() {
        let mut cache = PriceCache::new();
        let removed = cache.prune_before(d(2025, 1, 1));
        assert_eq!(removed, 0);
    }
}

// ═══════════════════════════════════════════════════════════════════
// Portfolio with trash — serde roundtrip
// ═══════════════════════════════════════════════════════════════════

mod portfolio_trash_serde {
    use savings_tracker_core::models::asset::Asset;
    use savings_tracker_core::models::event::{Event, EventType};
    use savings_tracker_core::models::portfolio::Portfolio;
    use chrono::NaiveDate;

    #[test]
    fn serde_roundtrip_with_trash() {
        let mut p = Portfolio::default();
        let event = Event::new(
            EventType::Buy,
            Asset::crypto("BTC", "Bitcoin"),
            1.0,
            NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
        );
        p.trash.push(event);

        let bytes = bincode::serialize(&p).unwrap();
        let back: Portfolio = bincode::deserialize(&bytes).unwrap();
        assert_eq!(back.trash.len(), 1);
        assert_eq!(back.trash[0].asset.symbol, "BTC");
    }

    #[test]
    fn serde_roundtrip_without_trash_backward_compat() {
        // Old Portfolio without trash field → should default to empty Vec
        let p = Portfolio::default();
        let bytes = bincode::serialize(&p).unwrap();
        let back: Portfolio = bincode::deserialize(&bytes).unwrap();
        assert!(back.trash.is_empty());
    }
}
