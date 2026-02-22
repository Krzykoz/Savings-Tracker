pub mod errors;
pub mod models;
pub mod providers;
pub mod services;
pub mod storage;

use chrono::NaiveDate;
use models::{
    analytics::PortfolioSummary,
    asset::{Asset, AssetType},
    chart::ChartDataPoint,
    event::{Event, EventSortOrder, EventType},
    portfolio::Portfolio,
    settings::Settings,
};
use providers::registry::PriceProviderRegistry;
use services::{
    analytics_service::AnalyticsService,
    chart_service::ChartService, currency_service::CurrencyService,
    portfolio_service::PortfolioService, price_service::PriceService,
};
use std::collections::HashMap;
use storage::manager::StorageManager;

use errors::CoreError;

/// Maximum chart date range in days (10 years).
const MAX_CHART_RANGE_DAYS: i64 = 3650;

/// Main entry point for the Savings Tracker core library.
/// Holds the portfolio state and all services needed to operate on it.
#[must_use]
pub struct SavingsTracker {
    portfolio: Portfolio,
    portfolio_service: PortfolioService,
    price_service: PriceService,
    chart_service: ChartService,
    currency_service: CurrencyService,
    analytics_service: AnalyticsService,
    /// Tracks whether any mutation has occurred since the last save/load.
    dirty: bool,
}

impl std::fmt::Debug for SavingsTracker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SavingsTracker")
            .field("events", &self.portfolio.events.len())
            .field("settings", &self.portfolio.settings)
            .field("cached_prices", &self.portfolio.price_cache.total_entries())
            .field("dirty", &self.dirty)
            .finish()
    }
}

impl SavingsTracker {
    /// Create a brand new empty portfolio with default settings.
    pub fn create_new() -> Self {
        let portfolio = Portfolio::default();
        Self::build(portfolio)
    }

    /// Load an existing portfolio from encrypted bytes (password required).
    /// Use this for WASM / Tauri where the frontend handles file I/O.
    pub fn load_from_bytes(encrypted: &[u8], password: &str) -> Result<Self, CoreError> {
        let portfolio = StorageManager::load_from_bytes(encrypted, password)?;
        Ok(Self::build(portfolio))
    }

    /// Save the current portfolio to encrypted bytes.
    /// Returns raw bytes that the frontend can write to a file.
    /// Clears the unsaved-changes flag on success.
    pub fn save_to_bytes(&mut self, password: &str) -> Result<Vec<u8>, CoreError> {
        let bytes = StorageManager::save_to_bytes(&self.portfolio, password)?;
        self.dirty = false;
        Ok(bytes)
    }

    /// Load from an encrypted file on disk (native only, not WASM).
    #[cfg(not(target_arch = "wasm32"))]
    pub fn load_from_file(path: &str, password: &str) -> Result<Self, CoreError> {
        let portfolio = StorageManager::load_from_file(path, password)?;
        Ok(Self::build(portfolio))
    }

    /// Save to an encrypted file on disk (native only, not WASM).
    /// Clears the unsaved-changes flag on success.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn save_to_file(&mut self, path: &str, password: &str) -> Result<(), CoreError> {
        StorageManager::save_to_file(&self.portfolio, path, password)?;
        self.dirty = false;
        Ok(())
    }

    // ── Event Management ────────────────────────────────────────────

    /// Add a buy/sell event to the portfolio.
    pub fn add_event(
        &mut self,
        event_type: EventType,
        asset: Asset,
        amount: f64,
        date: NaiveDate,
    ) -> Result<uuid::Uuid, CoreError> {
        let event = Event::new(event_type, asset, amount, date);
        let id = event.id;
        self.portfolio_service
            .add_event(&mut self.portfolio, event)?;
        self.dirty = true;
        Ok(id)
    }

    /// Add a buy/sell event with notes attached.
    pub fn add_event_with_notes(
        &mut self,
        event_type: EventType,
        asset: Asset,
        amount: f64,
        date: NaiveDate,
        notes: impl Into<String>,
    ) -> Result<uuid::Uuid, CoreError> {
        let event = Event::with_notes(event_type, asset, amount, date, notes);
        let id = event.id;
        self.portfolio_service
            .add_event(&mut self.portfolio, event)?;
        self.dirty = true;
        Ok(id)
    }

    /// Remove an event by its ID.
    /// Validates that removal doesn't create inconsistent sell events.
    pub fn remove_event(&mut self, event_id: uuid::Uuid) -> Result<(), CoreError> {
        self.portfolio_service
            .remove_event(&mut self.portfolio, event_id)?;
        self.dirty = true;
        Ok(())
    }

    /// Update an existing event by its ID.
    /// Validates the updated event before committing.
    pub fn update_event(
        &mut self,
        event_id: uuid::Uuid,
        event_type: EventType,
        asset: Asset,
        amount: f64,
        date: NaiveDate,
    ) -> Result<(), CoreError> {
        self.portfolio_service.update_event(
            &mut self.portfolio,
            event_id,
            event_type,
            asset,
            amount,
            date,
        )?;
        self.dirty = true;
        Ok(())
    }

    /// Set or clear notes on an existing event.
    pub fn set_event_notes(
        &mut self,
        event_id: uuid::Uuid,
        notes: Option<String>,
    ) -> Result<(), CoreError> {
        self.portfolio_service
            .set_notes(&mut self.portfolio, event_id, notes)?;
        self.dirty = true;
        Ok(())
    }

    /// Get a single event by its ID.
    #[must_use]
    pub fn get_event(&self, event_id: uuid::Uuid) -> Option<&Event> {
        self.portfolio.events.iter().find(|e| e.id == event_id)
    }

    /// Get all events, ordered by date.
    #[must_use]
    pub fn get_events(&self) -> Vec<&Event> {
        self.portfolio_service.get_events(&self.portfolio)
    }

    /// Get events filtered by asset symbol (case-insensitive).
    /// Returns newest-first, consistent with `get_events()`.
    #[must_use]
    pub fn get_events_for_asset(&self, asset_symbol: &str) -> Vec<&Event> {
        let upper = asset_symbol.to_uppercase();
        let mut events: Vec<&Event> = self.portfolio
            .events
            .iter()
            .filter(|e| e.asset.symbol == upper)
            .collect();
        events.reverse(); // internal storage is oldest-first; reverse for newest-first
        events
    }

    /// Get events filtered by event type (Buy or Sell).
    /// Returns newest-first, consistent with `get_events()`.
    #[must_use]
    pub fn get_events_by_type(&self, event_type: &EventType) -> Vec<&Event> {
        let mut events: Vec<&Event> = self.portfolio
            .events
            .iter()
            .filter(|e| &e.event_type == event_type)
            .collect();
        events.reverse();
        events
    }

    /// Get events within a date range (inclusive).
    /// Returns newest-first, consistent with `get_events()`.
    #[must_use]
    pub fn get_events_in_range(&self, from: NaiveDate, to: NaiveDate) -> Vec<&Event> {
        let mut events: Vec<&Event> = self.portfolio
            .events
            .iter()
            .filter(|e| e.date >= from && e.date <= to)
            .collect();
        events.reverse();
        events
    }

    // ── Holdings & Value ────────────────────────────────────────────

    /// Calculate current holdings (how much of each asset you own) at a given date.
    #[must_use]
    pub fn get_holdings(&self, date: NaiveDate) -> HashMap<Asset, f64> {
        self.portfolio_service
            .get_holdings(&self.portfolio, date)
    }

    /// Get the total portfolio value in the default currency.
    /// Requires price data (online or cached).
    pub async fn get_portfolio_value(
        &mut self,
        date: NaiveDate,
    ) -> Result<f64, CoreError> {
        let holdings = self.get_holdings(date);
        let default_currency = self.portfolio.settings.default_currency.clone();
        let mut total = 0.0;

        for (asset, amount) in &holdings {
            let value = self
                .currency_service
                .convert_asset_to_currency(
                    &self.price_service,
                    &mut self.portfolio.price_cache,
                    asset,
                    *amount,
                    &default_currency,
                    date,
                )
                .await?;
            total += value;
        }

        Ok(total)
    }

    // ── Charts ──────────────────────────────────────────────────────

    /// Generate chart data for the whole portfolio over a date range.
    pub async fn generate_portfolio_chart(
        &mut self,
        from: NaiveDate,
        to: NaiveDate,
    ) -> Result<Vec<ChartDataPoint>, CoreError> {
        if from > to {
            return Err(CoreError::ValidationError(
                format!("'from' date ({from}) must not be after 'to' date ({to})"),
            ));
        }
        let range_days = (to - from).num_days();
        if range_days > MAX_CHART_RANGE_DAYS {
            return Err(CoreError::ValidationError(
                format!("Chart range of {range_days} days exceeds maximum of {MAX_CHART_RANGE_DAYS} days (10 years)"),
            ));
        }

        let currency = self.portfolio.settings.default_currency.clone();

        // Temporarily take price_cache out of portfolio to satisfy the borrow checker:
        // generate_portfolio_chart needs &Portfolio (immutable) and &mut PriceCache (mutable).
        // Since PriceCache lives inside Portfolio, we can't borrow both simultaneously.
        let mut price_cache = std::mem::take(&mut self.portfolio.price_cache);

        let result = self
            .chart_service
            .generate_portfolio_chart(
                &self.portfolio,
                &mut self.price_service,
                &mut price_cache,
                from,
                to,
                &currency,
            )
            .await;

        // Put the (now updated) cache back
        self.portfolio.price_cache = price_cache;

        result
    }

    /// Generate chart data for a single asset over a date range.
    pub async fn generate_asset_chart(
        &mut self,
        asset_symbol: &str,
        from: NaiveDate,
        to: NaiveDate,
    ) -> Result<Vec<ChartDataPoint>, CoreError> {
        if from > to {
            return Err(CoreError::ValidationError(
                format!("'from' date ({from}) must not be after 'to' date ({to})"),
            ));
        }
        let range_days = (to - from).num_days();
        if range_days > MAX_CHART_RANGE_DAYS {
            return Err(CoreError::ValidationError(
                format!("Chart range of {range_days} days exceeds maximum of {MAX_CHART_RANGE_DAYS} days (10 years)"),
            ));
        }

        let currency = self.portfolio.settings.default_currency.clone();

        let mut price_cache = std::mem::take(&mut self.portfolio.price_cache);

        let result = self
            .chart_service
            .generate_asset_chart(
                &self.portfolio,
                &mut self.price_service,
                &mut price_cache,
                asset_symbol,
                from,
                to,
                &currency,
            )
            .await;

        self.portfolio.price_cache = price_cache;

        result
    }

    // ── Analytics ───────────────────────────────────────────────────

    /// Get a full portfolio summary with gain/loss, returns, and allocation breakdown.
    pub async fn get_portfolio_summary(
        &mut self,
        date: NaiveDate,
    ) -> Result<PortfolioSummary, CoreError> {
        let currency = self.portfolio.settings.default_currency.clone();

        let mut price_cache = std::mem::take(&mut self.portfolio.price_cache);

        let result = self
            .analytics_service
            .get_portfolio_summary(
                &self.portfolio,
                &self.price_service,
                &mut price_cache,
                date,
                &currency,
            )
            .await;

        self.portfolio.price_cache = price_cache;

        result
    }

    /// Get a list of all unique assets that appear in portfolio events.
    /// Returns deterministic order (sorted by symbol).
    #[must_use]
    pub fn get_unique_assets(&self) -> Vec<&Asset> {
        let mut seen = std::collections::HashSet::new();
        let mut assets: Vec<&Asset> = self
            .portfolio
            .events
            .iter()
            .filter_map(|e| {
                if seen.insert((&e.asset.symbol, &e.asset.asset_type)) {
                    Some(&e.asset)
                } else {
                    None
                }
            })
            .collect();
        assets.sort_by(|a, b| a.symbol.cmp(&b.symbol));
        assets
    }

    // ── Prices ──────────────────────────────────────────────────────

    /// Get the price of a specific asset in the default currency on a given date.
    /// Uses cache first, falls back to API providers.
    pub async fn get_asset_price(
        &mut self,
        asset: &Asset,
        date: NaiveDate,
    ) -> Result<f64, CoreError> {
        let currency = self.portfolio.settings.default_currency.clone();
        self.currency_service
            .convert_asset_to_currency(
                &self.price_service,
                &mut self.portfolio.price_cache,
                asset,
                1.0,
                &currency,
                date,
            )
            .await
    }

    /// Refresh current prices for all held assets from APIs.
    pub async fn refresh_prices(&mut self) -> Result<(), CoreError> {
        let today = chrono::Utc::now().date_naive();
        let holdings = self.get_holdings(today);
        let currency = self.portfolio.settings.default_currency.clone();

        for asset in holdings.keys() {
            self.price_service
                .get_price(
                    &mut self.portfolio.price_cache,
                    &asset.symbol,
                    &currency,
                    today,
                    &asset.asset_type,
                )
                .await?;
        }

        Ok(())
    }

    // ── Cache Management ────────────────────────────────────────────

    /// Get the total number of cached price points.
    #[must_use]
    pub fn cache_total_entries(&self) -> usize {
        self.portfolio.price_cache.total_entries()
    }

    /// Get the number of distinct asset/currency pairs cached.
    #[must_use]
    pub fn cache_asset_count(&self) -> usize {
        self.portfolio.price_cache.asset_count()
    }

    /// Remove all cached price points older than `before` date.
    /// Returns the number of entries removed.
    pub fn cache_prune_before(&mut self, before: NaiveDate) -> usize {
        let removed = self.portfolio.price_cache.prune_before(before);
        if removed > 0 {
            self.dirty = true;
        }
        removed
    }

    /// Clear all cached price data.
    pub fn cache_clear(&mut self) {
        self.portfolio.price_cache.clear();
        self.dirty = true;
    }

    // ── Settings ────────────────────────────────────────────────────

    /// Set the default display currency (e.g., "PLN", "USD", "EUR").
    /// Currency code must be a 3-letter alphabetic string.
    pub fn set_default_currency(&mut self, currency: String) -> Result<(), CoreError> {
        let trimmed = currency.trim().to_uppercase();
        if trimmed.len() != 3 || !trimmed.chars().all(|c| c.is_ascii_alphabetic()) {
            return Err(CoreError::ValidationError(
                format!("Invalid currency code '{currency}': must be exactly 3 ASCII letters (e.g., USD, EUR, PLN)"),
            ));
        }
        self.portfolio.settings.default_currency = trimmed;
        self.dirty = true;
        Ok(())
    }

    /// Get current settings.
    #[must_use]
    pub fn get_settings(&self) -> &Settings {
        &self.portfolio.settings
    }

    /// Set an API key for a provider (e.g., "metals_dev", "alphavantage").
    /// Rebuilds the provider registry so the new key takes effect immediately.
    pub fn set_api_key(&mut self, provider: String, key: String) {
        self.portfolio
            .settings
            .api_keys
            .insert(provider, key);

        // Rebuild registry with updated API keys
        let registry = PriceProviderRegistry::new_with_defaults(&self.portfolio.settings.api_keys);
        self.price_service = PriceService::new(registry);
        self.dirty = true;
    }

    /// Remove an API key for a provider.
    /// Rebuilds the provider registry so the removal takes effect immediately.
    pub fn remove_api_key(&mut self, provider: &str) -> bool {
        let removed = self.portfolio.settings.api_keys.remove(provider).is_some();
        if removed {
            let registry =
                PriceProviderRegistry::new_with_defaults(&self.portfolio.settings.api_keys);
            self.price_service = PriceService::new(registry);
            self.dirty = true;
        }
        removed
    }

    // ── Password & Dirty State ──────────────────────────────────────

    /// Re-encrypt the portfolio with a new password.
    /// Returns the encrypted bytes. The caller should write them to storage.
    ///
    /// `last_saved_bytes` must be the most recently saved encrypted bytes
    /// for this portfolio. The current password is verified by decrypting them.
    /// If verification fails, returns `CoreError::Decryption`.
    pub fn change_password(
        &mut self,
        last_saved_bytes: &[u8],
        current_password: &str,
        new_password: &str,
    ) -> Result<Vec<u8>, CoreError> {
        // Verify the current password against the actual saved data.
        // This ensures the caller truly knows the old password.
        StorageManager::load_from_bytes(last_saved_bytes, current_password)?;

        // Re-encrypt with the new password
        let new_bytes = StorageManager::save_to_bytes(&self.portfolio, new_password)?;
        self.dirty = false;
        Ok(new_bytes)
    }

    /// Returns `true` if the portfolio has been modified since the last save or load.
    #[must_use]
    pub fn has_unsaved_changes(&self) -> bool {
        self.dirty
    }

    // ── Search & Sorting ────────────────────────────────────────────

    /// Search events by matching query against symbol, name, and notes (case-insensitive).
    #[must_use]
    pub fn search_events(&self, query: &str) -> Vec<&Event> {
        let q = query.to_lowercase();
        self.portfolio
            .events
            .iter()
            .filter(|e| {
                e.asset.symbol.to_lowercase().contains(&q)
                    || e.asset.name.to_lowercase().contains(&q)
                    || e.notes.as_deref().unwrap_or("").to_lowercase().contains(&q)
            })
            .collect()
    }

    /// Get events sorted by a specific order.
    #[must_use]
    pub fn get_events_sorted(&self, order: &EventSortOrder) -> Vec<&Event> {
        let mut events: Vec<&Event> = self.portfolio.events.iter().collect();
        match order {
            EventSortOrder::DateDesc => events.sort_by(|a, b| b.date.cmp(&a.date)),
            EventSortOrder::DateAsc => events.sort_by(|a, b| a.date.cmp(&b.date)),
            EventSortOrder::AmountDesc => events.sort_by(|a, b| b.amount.partial_cmp(&a.amount).unwrap_or(std::cmp::Ordering::Equal)),
            EventSortOrder::AmountAsc => events.sort_by(|a, b| a.amount.partial_cmp(&b.amount).unwrap_or(std::cmp::Ordering::Equal)),
            EventSortOrder::AssetAsc => events.sort_by(|a, b| a.asset.symbol.cmp(&b.asset.symbol)),
            EventSortOrder::AssetDesc => events.sort_by(|a, b| b.asset.symbol.cmp(&a.asset.symbol)),
        }
        events
    }

    /// Get events filtered by asset type (e.g., show all Crypto events).
    #[must_use]
    pub fn get_events_for_asset_type(&self, asset_type: &AssetType) -> Vec<&Event> {
        self.portfolio
            .events
            .iter()
            .filter(|e| &e.asset.asset_type == asset_type)
            .collect()
    }

    /// Get the total number of events without materializing a sorted vector.
    #[must_use]
    pub fn event_count(&self) -> usize {
        self.portfolio.events.len()
    }

    // ── Convenience Helpers ─────────────────────────────────────────

    /// Get current holdings (as of today).
    #[must_use]
    pub fn get_current_holdings(&self) -> HashMap<Asset, f64> {
        let today = chrono::Utc::now().date_naive();
        self.portfolio_service.get_holdings(&self.portfolio, today)
    }

    /// Get the date of the earliest event in the portfolio.
    #[must_use]
    pub fn earliest_event_date(&self) -> Option<NaiveDate> {
        self.portfolio.events.first().map(|e| e.date)
    }

    /// Get the date of the most recent event in the portfolio.
    #[must_use]
    pub fn latest_event_date(&self) -> Option<NaiveDate> {
        self.portfolio.events.last().map(|e| e.date)
    }

    /// Get the number of days since the first event (portfolio age).
    #[must_use]
    pub fn portfolio_age_days(&self) -> Option<i64> {
        self.earliest_event_date()
            .map(|d| (chrono::Utc::now().date_naive() - d).num_days())
    }

    // ── Bulk Operations ─────────────────────────────────────────────

    /// Add multiple events at once. All events are validated first;
    /// if any event fails validation, none are added (all-or-nothing).
    /// Returns the IDs of all added events.
    pub fn add_events(&mut self, events: Vec<Event>) -> Result<Vec<uuid::Uuid>, CoreError> {
        // Phase 1: Validate all events against a temporary portfolio state
        let mut temp_portfolio = self.portfolio.clone();
        let mut ids = Vec::with_capacity(events.len());

        for event in &events {
            self.portfolio_service.add_event(&mut temp_portfolio, event.clone())?;
            ids.push(event.id);
        }

        // Phase 2: All valid — apply to real portfolio
        self.portfolio = temp_portfolio;
        self.dirty = true;
        Ok(ids)
    }

    /// Remove multiple events at once. All removals are validated first;
    /// if any removal fails, none are removed (all-or-nothing).
    pub fn remove_events(&mut self, event_ids: &[uuid::Uuid]) -> Result<(), CoreError> {
        let mut temp_portfolio = self.portfolio.clone();

        for id in event_ids {
            self.portfolio_service.remove_event(&mut temp_portfolio, *id)?;
        }

        self.portfolio = temp_portfolio;
        self.dirty = true;
        Ok(())
    }

    // ── Undo (Trash) ────────────────────────────────────────────────

    /// Remove an event and keep it in the trash for potential undo.
    /// Returns the removed event.
    pub fn remove_event_to_trash(&mut self, event_id: uuid::Uuid) -> Result<Event, CoreError> {
        let event = self.portfolio.events.iter().find(|e| e.id == event_id)
            .cloned()
            .ok_or_else(|| CoreError::EventNotFound(event_id.to_string()))?;

        self.portfolio_service.remove_event(&mut self.portfolio, event_id)?;
        self.portfolio.trash.push(event.clone());
        self.dirty = true;
        Ok(event)
    }

    /// Restore the most recently trashed event back into the portfolio.
    /// Returns the restored event, or `None` if trash is empty.
    pub fn undo_last_removal(&mut self) -> Result<Option<Event>, CoreError> {
        let event = match self.portfolio.trash.pop() {
            Some(e) => e,
            None => return Ok(None),
        };

        self.portfolio_service.add_event(&mut self.portfolio, event.clone())?;
        self.dirty = true;
        Ok(Some(event))
    }

    /// Get events currently in the trash.
    #[must_use]
    pub fn get_trash(&self) -> &[Event] {
        &self.portfolio.trash
    }

    /// Clear all trashed events permanently.
    pub fn clear_trash(&mut self) {
        if !self.portfolio.trash.is_empty() {
            self.portfolio.trash.clear();
            self.dirty = true;
        }
    }

    // ── Export / Import ─────────────────────────────────────────────

    /// Export all events as a JSON string.
    pub fn export_events_to_json(&self) -> Result<String, CoreError> {
        serde_json::to_string_pretty(&self.portfolio.events)
            .map_err(|e| CoreError::Serialization(format!("Failed to serialize events to JSON: {e}")))
    }

    /// Export all events as a CSV string.
    /// Columns: id, event_type, symbol, name, asset_type, amount, date, notes
    #[must_use]
    pub fn export_events_to_csv(&self) -> String {
        let mut csv = String::from("id,event_type,symbol,name,asset_type,amount,date,notes\n");
        for event in &self.portfolio.events {
            let notes = event.notes.as_deref().unwrap_or("");
            // Escape CSV: quote fields containing commas, quotes, or newlines
            let escaped_notes = if notes.contains(',') || notes.contains('"') || notes.contains('\n') {
                format!("\"{}\"", notes.replace('"', "\"\""))
            } else {
                notes.to_string()
            };
            let escaped_name = if event.asset.name.contains(',') || event.asset.name.contains('"') {
                format!("\"{}\"", event.asset.name.replace('"', "\"\""))
            } else {
                event.asset.name.clone()
            };
            csv.push_str(&format!(
                "{},{},{},{},{},{},{},{}\n",
                event.id,
                event.event_type,
                event.asset.symbol,
                escaped_name,
                event.asset.asset_type,
                event.amount,
                event.date,
                escaped_notes,
            ));
        }
        csv
    }

    /// Import events from a JSON string. Validates each event.
    /// Returns the number of events imported.
    pub fn import_events_from_json(&mut self, json: &str) -> Result<usize, CoreError> {
        let events: Vec<Event> = serde_json::from_str(json)?;
        let count = events.len();
        self.add_events(events)?;
        Ok(count)
    }

    /// Export the full portfolio summary as JSON (unencrypted snapshot for debugging/display).
    pub fn to_json(&self) -> Result<String, CoreError> {
        serde_json::to_string_pretty(&self.portfolio)
            .map_err(|e| CoreError::Serialization(format!("Failed to serialize portfolio: {e}")))
    }

    // ── Cache Inspection ────────────────────────────────────────────

    /// Get a specific cached price.
    #[must_use]
    pub fn get_cached_price(&self, symbol: &str, currency: &str, date: NaiveDate) -> Option<f64> {
        self.portfolio.price_cache.get_price(symbol, currency, date)
    }

    /// Get all cached (symbol, currency) pairs.
    #[must_use]
    pub fn get_cached_pairs(&self) -> Vec<(String, String)> {
        self.portfolio.price_cache.entries.keys().cloned().collect()
    }

    /// Get the date when a (symbol, currency) pair was last refreshed.
    #[must_use]
    pub fn get_last_refreshed(&self, symbol: &str, currency: &str) -> Option<NaiveDate> {
        let key = (symbol.to_uppercase(), currency.to_uppercase());
        self.portfolio.price_cache.last_updated.get(&key).copied()
    }

    /// Manually insert a price into the cache (useful for testing, offline, or historical import).
    pub fn set_cached_price(&mut self, symbol: &str, currency: &str, date: NaiveDate, price: f64) {
        self.portfolio.price_cache.set_price(symbol, currency, date, price);
        self.dirty = true;
    }

    // ── Provider Availability ───────────────────────────────────────

    /// Check if at least one price provider is available for a given asset type.
    #[must_use]
    pub fn is_provider_available(&self, asset_type: &AssetType) -> bool {
        self.price_service.has_provider_for(asset_type)
    }

    /// Get the names of available providers for a given asset type.
    #[must_use]
    pub fn get_provider_names(&self, asset_type: &AssetType) -> Vec<String> {
        self.price_service.get_provider_names(asset_type)
    }

    // ── Internal ────────────────────────────────────────────────────

    fn build(portfolio: Portfolio) -> Self {
        let api_keys = portfolio.settings.api_keys.clone();
        let registry = PriceProviderRegistry::new_with_defaults(&api_keys);
        let price_service = PriceService::new(registry);
        let portfolio_service = PortfolioService::new();
        let chart_service = ChartService::new();
        let currency_service = CurrencyService::new();
        let analytics_service = AnalyticsService::new();

        Self {
            portfolio,
            portfolio_service,
            price_service,
            chart_service,
            currency_service,
            analytics_service,
            dirty: false,
        }
    }
}
