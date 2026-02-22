use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A single price data point (date → price).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PricePoint {
    pub date: NaiveDate,
    pub price: f64,
}

/// Cache key: (asset_symbol, target_currency) e.g., ("BTC", "USD")
pub type PriceCacheKey = (String, String);

/// Local cache of historical and current price data.
///
/// Stored inside the encrypted portfolio file so that:
/// - Historical prices (date < today) are fetched ONCE and never re-fetched.
/// - The app works fully offline with cached data.
/// - Today's price can be refreshed when online.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PriceCache {
    /// Historical price entries: (symbol, currency) → sorted Vec of PricePoints
    pub entries: HashMap<PriceCacheKey, Vec<PricePoint>>,

    /// Tracks when we last refreshed "today's" price for each (symbol, currency).
    /// Used to avoid redundant API calls within the same day.
    pub last_updated: HashMap<PriceCacheKey, NaiveDate>,
}

impl PriceCache {
    pub fn new() -> Self {
        Self::default()
    }

    /// Get a cached price for a specific (symbol, currency, date).
    /// Returns None if not cached. Uses binary search (O(log n)).
    pub fn get_price(&self, symbol: &str, currency: &str, date: NaiveDate) -> Option<f64> {
        let key = (symbol.to_uppercase(), currency.to_uppercase());
        let entries = self.entries.get(&key)?;
        entries
            .binary_search_by_key(&date, |p| p.date)
            .ok()
            .map(|idx| entries[idx].price)
    }

    /// Insert or update a price point in the cache.
    /// Maintains sorted order by date using binary search (O(log n) insertion).
    pub fn set_price(&mut self, symbol: &str, currency: &str, date: NaiveDate, price: f64) {
        let key = (symbol.to_uppercase(), currency.to_uppercase());
        let entries = self.entries.entry(key.clone()).or_default();

        // Binary search for existing entry or insertion point
        match entries.binary_search_by_key(&date, |p| p.date) {
            Ok(idx) => {
                // Update existing entry at this date
                entries[idx].price = price;
            }
            Err(idx) => {
                // Insert at sorted position
                entries.insert(idx, PricePoint { date, price });
            }
        }
    }

    /// Insert multiple price points at once (e.g., from a historical range API call).
    pub fn set_prices(&mut self, symbol: &str, currency: &str, points: &[PricePoint]) {
        for point in points {
            self.set_price(symbol, currency, point.date, point.price);
        }
    }

    /// Check if today's price was already fetched today (avoid redundant API calls).
    pub fn is_today_fresh(&self, symbol: &str, currency: &str, today: NaiveDate) -> bool {
        let key = (symbol.to_uppercase(), currency.to_uppercase());
        self.last_updated.get(&key).is_some_and(|&d| d == today)
    }

    /// Mark that we've refreshed the current price for this asset today.
    pub fn mark_updated_today(&mut self, symbol: &str, currency: &str, today: NaiveDate) {
        let key = (symbol.to_uppercase(), currency.to_uppercase());
        self.last_updated.insert(key, today);
    }

    /// Get the total number of cached price points across all assets.
    pub fn total_entries(&self) -> usize {
        self.entries.values().map(|v| v.len()).sum()
    }

    /// Get the number of distinct (symbol, currency) pairs cached.
    pub fn asset_count(&self) -> usize {
        self.entries.len()
    }

    /// Remove all cached price points older than `before` date.
    /// Returns the number of entries removed.
    pub fn prune_before(&mut self, before: NaiveDate) -> usize {
        let mut removed = 0;
        for entries in self.entries.values_mut() {
            let old_len = entries.len();
            // Binary search for the first entry >= before
            let split = entries
                .binary_search_by_key(&before, |p| p.date)
                .unwrap_or_else(|pos| pos);
            if split > 0 {
                entries.drain(..split);
                removed += old_len - entries.len();
            }
        }
        // Remove empty entries
        self.entries.retain(|_, v| !v.is_empty());
        // I4: Also prune last_updated entries for removed/empty asset pairs
        // or stale entries older than the prune date
        self.last_updated.retain(|key, updated| {
            self.entries.contains_key(key) && *updated >= before
        });
        removed
    }

    /// Clear all cached data.
    pub fn clear(&mut self) {
        self.entries.clear();
        self.last_updated.clear();
    }

    /// Get all cached price points for a (symbol, currency) pair in a date range.
    /// Uses binary search to efficiently find the range boundaries.
    pub fn get_price_range(
        &self,
        symbol: &str,
        currency: &str,
        from: NaiveDate,
        to: NaiveDate,
    ) -> Vec<PricePoint> {
        let key = (symbol.to_uppercase(), currency.to_uppercase());
        self.entries
            .get(&key)
            .map(|entries| {
                // Binary search for start index (first entry >= from)
                let start = entries
                    .binary_search_by_key(&from, |p| p.date)
                    .unwrap_or_else(|pos| pos);
                // Binary search for end index (first entry > to)
                let end = entries
                    .binary_search_by_key(&to, |p| p.date)
                    .map(|pos| pos + 1) // include the exact match
                    .unwrap_or_else(|pos| pos);
                entries[start..end].to_vec()
            })
            .unwrap_or_default()
    }
}
