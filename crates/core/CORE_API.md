# Core API Reference

Complete API reference for `savings-tracker-core`. All public methods are accessed through the [`SavingsTracker`](#savingstracker) struct — the single entry point for the library.

---

## Table of Contents

- [SavingsTracker](#savingstracker)
- [Construction & Storage](#construction--storage)
- [Event Management](#event-management)
- [Bulk Operations](#bulk-operations)
- [Trash & Undo](#trash--undo)
- [Event Filtering](#event-filtering)
- [Event Search & Sort](#event-search--sort)
- [Holdings & Portfolio Value](#holdings--portfolio-value)
- [Charts](#charts)
- [Analytics](#analytics)
- [Prices](#prices)
- [Cache Management](#cache-management)
- [Cache Inspection](#cache-inspection)
- [Provider Availability](#provider-availability)
- [Export & Import](#export--import)
- [Settings & API Keys](#settings--api-keys)
- [Password & Dirty State](#password--dirty-state)
- [Models](#models)
  - [Asset](#asset)
  - [AssetType](#assettype)
  - [Event](#event)
  - [EventType](#eventtype)
  - [ChartDataPoint](#chartdatapoint)
  - [ChartEvent](#chartevent)
  - [EventSortOrder](#eventsortorder)
  - [PortfolioSummary](#portfoliosummary)
  - [HoldingSummary](#holdingsummary)
  - [Settings](#settings)
  - [PriceCache](#pricecache)
- [Error Handling](#error-handling)
- [Platform Notes](#platform-notes)

---

## SavingsTracker

```rust
pub struct SavingsTracker { /* private fields */ }
```

The main facade for the library. Holds portfolio state, all services, and a dirty-tracking flag. Implements `Debug`.

```rust
// Debug output example:
// SavingsTracker { events: 12, settings: Settings { default_currency: "PLN", .. }, cached_prices: 847, dirty: false }
```

---

## Construction & Storage

### `SavingsTracker::create_new()`

```rust
pub fn create_new() -> Self
```

Create a new empty portfolio with default settings (USD currency, no API keys).

```rust
let tracker = SavingsTracker::create_new();
```

---

### `SavingsTracker::load_from_bytes()`

```rust
pub fn load_from_bytes(encrypted: &[u8], password: &str) -> Result<Self, CoreError>
```

Load a portfolio from encrypted bytes. Use this for WASM or Tauri where the frontend handles file I/O.

| Error | When |
|-------|------|
| `CoreError::Decryption` | Wrong password or corrupted data |
| `CoreError::InvalidFileFormat` | Not a valid `.svtk` file |
| `CoreError::UnsupportedVersion` | File version newer than library supports |

```rust
let tracker = SavingsTracker::load_from_bytes(&bytes, "my-password")?;
```

---

### `save_to_bytes()`

```rust
pub fn save_to_bytes(&mut self, password: &str) -> Result<Vec<u8>, CoreError>
```

Encrypt and serialize the portfolio to bytes. Clears the `dirty` flag on success. The returned bytes can be written to a file or stored by the frontend.

```rust
let bytes = tracker.save_to_bytes("my-password")?;
// Frontend writes `bytes` to disk / IndexedDB / etc.
```

---

### `load_from_file()` — native only

```rust
#[cfg(not(target_arch = "wasm32"))]
pub fn load_from_file(path: &str, password: &str) -> Result<Self, CoreError>
```

Load from an encrypted `.svtk` file on disk. Not available on WASM.

---

### `save_to_file()` — native only

```rust
#[cfg(not(target_arch = "wasm32"))]
pub fn save_to_file(&mut self, path: &str, password: &str) -> Result<(), CoreError>
```

Save to an encrypted `.svtk` file on disk. Clears the `dirty` flag. Not available on WASM.

---

## Event Management

### `add_event()`

```rust
pub fn add_event(
    &mut self,
    event_type: EventType,
    asset: Asset,
    amount: f64,
    date: NaiveDate,
) -> Result<Uuid, CoreError>
```

Add a buy or sell event. Returns the generated event UUID.

**Validation rules:**
- `amount` must be positive
- `date` must not be in the future
- For `Sell` events: you must hold enough of the asset at that date

```rust
let id = tracker.add_event(
    EventType::Buy,
    Asset::crypto("BTC", "Bitcoin"),
    0.5,
    NaiveDate::from_ymd_opt(2025, 1, 15).unwrap(),
)?;
```

---

### `add_event_with_notes()`

```rust
pub fn add_event_with_notes(
    &mut self,
    event_type: EventType,
    asset: Asset,
    amount: f64,
    date: NaiveDate,
    notes: impl Into<String>,
) -> Result<Uuid, CoreError>
```

Same as `add_event()` but with a text note attached (e.g., exchange name, memo).

```rust
let id = tracker.add_event_with_notes(
    EventType::Buy,
    Asset::stock("AAPL", "Apple"),
    10.0,
    date,
    "Bought on earnings dip",
)?;
```

---

### `remove_event()`

```rust
pub fn remove_event(&mut self, event_id: Uuid) -> Result<(), CoreError>
```

Remove an event by its UUID. If the event is a `Buy`, the library validates that removing it won't make any subsequent `Sell` events invalid (negative holdings). If it would, the removal is rejected and the portfolio is unchanged.

| Error | When |
|-------|------|
| `CoreError::EventNotFound` | No event with that ID |
| `CoreError::ValidationError` | Removal would create inconsistent sells |

---

### `update_event()`

```rust
pub fn update_event(
    &mut self,
    event_id: Uuid,
    event_type: EventType,
    asset: Asset,
    amount: f64,
    date: NaiveDate,
) -> Result<(), CoreError>
```

Update an existing event. Validates the new state before committing. On validation failure, the original event is restored (atomic rollback). Notes are preserved across updates.

---

### `set_event_notes()`

```rust
pub fn set_event_notes(
    &mut self,
    event_id: Uuid,
    notes: Option<String>,
) -> Result<(), CoreError>
```

Set or clear the notes on an existing event. Pass `None` to clear.

---

### `get_event()`

```rust
pub fn get_event(&self, event_id: Uuid) -> Option<&Event>
```

Look up a single event by its UUID. Returns `None` if not found.

---

### `get_events()`

```rust
pub fn get_events(&self) -> Vec<&Event>
```

Get all events, sorted newest-first (for display).

---

## Bulk Operations

### `add_events()`

```rust
pub fn add_events(&mut self, events: Vec<Event>) -> Result<Vec<Uuid>, CoreError>
```

Add multiple events atomically (all-or-nothing). If any event fails validation, the entire batch is rejected and the portfolio is unchanged. Returns the generated UUIDs.

```rust
let events = vec![
    Event::new(EventType::Buy, Asset::crypto("BTC", "Bitcoin"), 1.0, date),
    Event::new(EventType::Buy, Asset::crypto("ETH", "Ethereum"), 10.0, date),
];
let ids = tracker.add_events(events)?;
```

---

### `remove_events()`

```rust
pub fn remove_events(&mut self, event_ids: &[Uuid]) -> Result<(), CoreError>
```

Remove multiple events atomically. If any ID is invalid or removal would break sell consistency, the entire operation is rejected.

---

## Trash & Undo

Soft-delete support with single-level undo. Trashed events are stored in the portfolio and survive save/load cycles.

### `remove_event_to_trash()`

```rust
pub fn remove_event_to_trash(&mut self, event_id: Uuid) -> Result<Event, CoreError>
```

Move an event to the trash instead of permanently deleting it. Returns a clone of the trashed event. Same consistency checks as `remove_event()`.

---

### `undo_last_removal()`

```rust
pub fn undo_last_removal(&mut self) -> Result<Uuid, CoreError>
```

Restore the most recently trashed event. Returns the event's UUID. Fails if the trash is empty or restoring would create inconsistencies.

---

### `get_trash()`

```rust
pub fn get_trash(&self) -> &[Event]
```

View all events currently in the trash.

---

### `clear_trash()`

```rust
pub fn clear_trash(&mut self)
```

Permanently delete all trashed events. Marks the tracker as dirty.

---

## Event Filtering

### `get_events_for_asset()`

```rust
pub fn get_events_for_asset(&self, asset_symbol: &str) -> Vec<&Event>
```

Filter events by asset symbol. Case-insensitive (`"btc"` matches `"BTC"`). Returns events newest-first.

```rust
let btc_events = tracker.get_events_for_asset("BTC");
```

---

### `get_events_by_type()`

```rust
pub fn get_events_by_type(&self, event_type: &EventType) -> Vec<&Event>
```

Filter events by type (Buy or Sell).

```rust
let sells = tracker.get_events_by_type(&EventType::Sell);
```

---

### `get_events_in_range()`

```rust
pub fn get_events_in_range(&self, from: NaiveDate, to: NaiveDate) -> Vec<&Event>
```

Get all events within a date range (inclusive on both ends).

```rust
let q1_events = tracker.get_events_in_range(
    NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
    NaiveDate::from_ymd_opt(2025, 3, 31).unwrap(),
);
```

---

## Event Search & Sort

### `search_events()`

```rust
pub fn search_events(&self, query: &str) -> Vec<&Event>
```

Case-insensitive full-text search across event symbol, asset name, and notes. Returns matching events newest-first.

```rust
let results = tracker.search_events("bitcoin");
```

---

### `get_events_sorted()`

```rust
pub fn get_events_sorted(&self, order: &EventSortOrder) -> Vec<&Event>
```

Get all events sorted by the given criteria. See [`EventSortOrder`](#eventsortorder).

```rust
use savings_tracker_core::models::event::EventSortOrder;
let by_amount = tracker.get_events_sorted(&EventSortOrder::AmountDesc);
```

---

### `get_events_for_asset_type()`

```rust
pub fn get_events_for_asset_type(&self, asset_type: &AssetType) -> Vec<&Event>
```

Filter events by asset type (Crypto, Fiat, Metal, Stock). Newest-first.

---

### `event_count()`

```rust
pub fn event_count(&self) -> usize
```

Number of events in the portfolio. O(1).

---

### `earliest_event_date()` / `latest_event_date()`

```rust
pub fn earliest_event_date(&self) -> Option<NaiveDate>
pub fn latest_event_date(&self) -> Option<NaiveDate>
```

Returns the date of the earliest/latest event, or `None` if the portfolio is empty.

---

### `portfolio_age_days()`

```rust
pub fn portfolio_age_days(&self) -> Option<i64>
```

Number of days between the earliest event and today. Returns `None` if the portfolio is empty.

---

## Holdings & Portfolio Value

### `get_holdings()`

```rust
pub fn get_holdings(&self, date: NaiveDate) -> HashMap<Asset, f64>
```

Calculate how much of each asset is held on the given date. Iterates all events up to `date`, summing buys and subtracting sells. Only returns assets with positive amounts (>ε).

```rust
let holdings = tracker.get_holdings(today);
for (asset, amount) in &holdings {
    println!("{}: {}", asset.symbol, amount);
}
```

---

### `get_current_holdings()`

```rust
pub fn get_current_holdings(&self) -> HashMap<Asset, f64>
```

Convenience method — equivalent to `get_holdings(today)`. Returns current holdings as of today.

---

### `get_portfolio_value()` — async

```rust
pub async fn get_portfolio_value(&mut self, date: NaiveDate) -> Result<f64, CoreError>
```

Get the total portfolio value in the default currency. Fetches prices from APIs (or cache) and converts all holdings.

---

### `get_unique_assets()`

```rust
pub fn get_unique_assets(&self) -> Vec<&Asset>
```

Get all distinct assets that appear in portfolio events. Returns deterministic order sorted alphabetically by symbol.

---

## Charts

### `generate_portfolio_chart()` — async

```rust
pub async fn generate_portfolio_chart(
    &mut self,
    from: NaiveDate,
    to: NaiveDate,
) -> Result<Vec<ChartDataPoint>, CoreError>
```

Generate day-by-day portfolio value data for chart rendering. Each data point contains the date, total portfolio value in the default currency, and any buy/sell events that occurred on that day.

**Validation:** `from` must not be after `to`. Maximum range: 3,650 days (10 years).

**Performance:** Uses incremental holdings computation — O(days + events) instead of O(days × events). Carries forward the last known value on weekends/holidays when no price data is available.

```rust
let chart = tracker.generate_portfolio_chart(from, to).await?;
for point in &chart {
    println!("{}: ${:.2} ({} events)", point.date, point.portfolio_value, point.events.len());
}
```

---

### `generate_asset_chart()` — async

```rust
pub async fn generate_asset_chart(
    &mut self,
    asset_symbol: &str,
    from: NaiveDate,
    to: NaiveDate,
) -> Result<Vec<ChartDataPoint>, CoreError>
```

Generate a chart for a single asset's value over time. Same format as the portfolio chart.

| Error | When |
|-------|------|
| `CoreError::ValidationError` | `from > to`, or asset not found in portfolio |

---

## Analytics

### `get_portfolio_summary()` — async

```rust
pub async fn get_portfolio_summary(
    &mut self,
    date: NaiveDate,
) -> Result<PortfolioSummary, CoreError>
```

Generate a full portfolio breakdown at a given date. Returns total value, total invested, total returned (from sells), overall gain/loss, return %, and a per-asset breakdown sorted by allocation.

```rust
let summary = tracker.get_portfolio_summary(today).await?;
println!("Portfolio value: ${:.2}", summary.total_value);
println!("Total invested:  ${:.2}", summary.total_invested);
println!("Return:          {:.1}%", summary.total_return_pct);

for h in &summary.holdings {
    println!("  {} — {:.1}% allocation, {:.1}% return",
        h.asset.symbol, h.allocation_pct, h.return_pct);
}
```

---

## Prices

### `get_asset_price()` — async

```rust
pub async fn get_asset_price(
    &mut self,
    asset: &Asset,
    date: NaiveDate,
) -> Result<f64, CoreError>
```

Get the price of one unit of an asset in the default currency on the given date. Checks cache first, falls back to API providers.

```rust
let btc = Asset::crypto("BTC", "Bitcoin");
let price = tracker.get_asset_price(&btc, today).await?;
println!("BTC price: ${:.2}", price);
```

---

### `refresh_prices()` — async

```rust
pub async fn refresh_prices(&mut self) -> Result<(), CoreError>
```

Force-refresh today's prices for all currently held assets from the APIs. Updates the internal cache.

---

## Cache Management

Price data is cached inside the encrypted portfolio file for offline access. Historical prices (past dates) are fetched once and never re-fetched.

### `cache_total_entries()`

```rust
pub fn cache_total_entries(&self) -> usize
```

Total number of cached price data points across all assets.

---

### `cache_asset_count()`

```rust
pub fn cache_asset_count(&self) -> usize
```

Number of distinct (symbol, currency) pairs in the cache.

---

### `cache_prune_before()`

```rust
pub fn cache_prune_before(&mut self, before: NaiveDate) -> usize
```

Remove all cached price points with dates strictly before `before`. Returns the number of entries removed. Marks the tracker as dirty if anything was pruned.

```rust
let removed = tracker.cache_prune_before(
    NaiveDate::from_ymd_opt(2024, 1, 1).unwrap()
);
println!("Pruned {removed} old price entries");
```

---

### `cache_clear()`

```rust
pub fn cache_clear(&mut self)
```

Clear all cached price data. Marks the tracker as dirty.

---

## Cache Inspection

### `get_cached_price()`

```rust
pub fn get_cached_price(&self, symbol: &str, currency: &str, date: NaiveDate) -> Option<f64>
```

Look up a single cached price. Returns `None` if not in cache.

---

### `get_cached_pairs()`

```rust
pub fn get_cached_pairs(&self) -> Vec<(String, String)>
```

Get all (symbol, currency) pairs that have cached data.

---

### `get_last_refreshed()`

```rust
pub fn get_last_refreshed(&self, symbol: &str, currency: &str) -> Option<NaiveDate>
```

When was a specific pair last refreshed? Used to detect stale data.

---

### `set_cached_price()`

```rust
pub fn set_cached_price(&mut self, symbol: &str, currency: &str, date: NaiveDate, price: f64)
```

Manually insert a price into the cache. Marks the tracker as dirty. Useful for offline data entry or custom price corrections.

---

## Provider Availability

### `is_provider_available()`

```rust
pub fn is_provider_available(&self, asset_type: &AssetType) -> bool
```

Check if at least one price provider is registered for the given asset type.

---

### `get_provider_names()`

```rust
pub fn get_provider_names(&self, asset_type: &AssetType) -> Vec<String>
```

Get the names of all registered providers for an asset type.

```rust
let providers = tracker.get_provider_names(&AssetType::Stock);
// ["YahooFinance", "AlphaVantage"]
```

---

## Export & Import

### `export_events_to_json()`

```rust
pub fn export_events_to_json(&self) -> Result<String, CoreError>
```

Export all events as a JSON array. Useful for backup, migration, or interop.

---

### `export_events_to_csv()`

```rust
pub fn export_events_to_csv(&self) -> String
```

Export all events as CSV (with header row). Properly escapes commas and newlines in notes.

Format: `id,type,symbol,name,asset_type,amount,date,notes`

---

### `import_events_from_json()`

```rust
pub fn import_events_from_json(&mut self, json: &str) -> Result<usize, CoreError>
```

Import events from a JSON array. New UUIDs are generated for each imported event. Events are validated and added to the existing portfolio. Returns the number of events imported.

---

### `to_json()`

```rust
pub fn to_json(&self) -> Result<String, CoreError>
```

Serialize the entire portfolio (events, settings, price cache, trash) to unencrypted JSON. Useful for debugging or frontend state transfer.

---

## Settings & API Keys

### `set_default_currency()`

```rust
pub fn set_default_currency(&mut self, currency: String) -> Result<(), CoreError>
```

Set the display currency for all values (portfolio, charts, analytics). Must be exactly 3 ASCII letters. Automatically uppercased.

| Error | When |
|-------|------|
| `CoreError::ValidationError` | Not 3 letters, contains digits, empty, etc. |

```rust
tracker.set_default_currency("PLN".into())?;
```

---

### `get_settings()`

```rust
pub fn get_settings(&self) -> &Settings
```

Get the current settings (default currency, API keys).

---

### `set_api_key()`

```rust
pub fn set_api_key(&mut self, provider: String, key: String)
```

Set an API key for a provider. Immediately rebuilds the provider registry so the key takes effect.

| Provider name | Service |
|---------------|---------|
| `"metals_dev"` | metals.dev — precious metals |
| `"alphavantage"` | Alpha Vantage — stocks (fallback) |

```rust
tracker.set_api_key("metals_dev".into(), "your-api-key".into());
```

---

### `remove_api_key()`

```rust
pub fn remove_api_key(&mut self, provider: &str) -> bool
```

Remove an API key. Returns `true` if the key existed and was removed. Rebuilds the provider registry.

```rust
let was_set = tracker.remove_api_key("metals_dev");
```

---

## Password & Dirty State

### `change_password()`

```rust
pub fn change_password(
    &mut self,
    last_saved_bytes: &[u8],
    current_password: &str,
    new_password: &str,
) -> Result<Vec<u8>, CoreError>
```

Re-encrypt the portfolio with a new password. First verifies the current password against the provided `last_saved_bytes` (returns `CoreError::Decryption` if wrong). Returns the new encrypted bytes for the caller to write to storage. Clears the dirty flag.

```rust
// Frontend keeps `saved_bytes` from last save/load
let new_bytes = tracker.change_password(&saved_bytes, "old-pw", "new-pw")?;
std::fs::write("portfolio.svtk", &new_bytes)?;
```

---

### `has_unsaved_changes()`

```rust
pub fn has_unsaved_changes(&self) -> bool
```

Returns `true` if the portfolio has been modified since the last save or load. Any mutation (add/remove/update event, change settings, change API key, prune cache) sets this to `true`. Saving or changing password clears it.

---

## Models

### Asset

```rust
pub struct Asset {
    pub symbol: String,    // Uppercased ticker (e.g. "BTC", "AAPL")
    pub name: String,      // Human-readable (e.g. "Bitcoin", "Apple Inc.")
    pub asset_type: AssetType,
}
```

Equality and hashing are based on `(symbol, asset_type)` only — `name` is ignored.

**Convenience constructors:**

```rust
Asset::crypto("BTC", "Bitcoin")
Asset::fiat("USD", "US Dollar")
Asset::metal("XAU", "Gold")
Asset::stock("AAPL", "Apple Inc.")
```

---

### AssetType

```rust
pub enum AssetType {
    Crypto,  // CoinCap API
    Fiat,    // Frankfurter API
    Metal,   // metals.dev API
    Stock,   // Yahoo Finance (native) / Alpha Vantage (WASM/fallback)
}
```

Determines which price provider is used for fetching market data.

---

### Event

```rust
pub struct Event {
    pub id: Uuid,
    pub event_type: EventType,
    pub asset: Asset,
    pub amount: f64,         // Always positive
    pub date: NaiveDate,
    pub notes: Option<String>,
}
```

A single buy or sell transaction. Price is NOT stored on the event — it's fetched from APIs based on the date and cached.

---

### EventType

```rust
pub enum EventType {
    Buy,
    Sell,
}
```

---

### ChartDataPoint

```rust
pub struct ChartDataPoint {
    pub date: NaiveDate,
    pub portfolio_value: f64,     // Total value in default currency
    pub events: Vec<ChartEvent>,  // Buy/sell events on this date
}
```

Returned by `generate_portfolio_chart()` and `generate_asset_chart()`. One per day in the requested range.

---

### ChartEvent

```rust
pub struct ChartEvent {
    pub event_type: EventType,
    pub asset_symbol: String,
    pub amount: f64,
    pub value_in_default_currency: f64,
}
```

An event annotation on a chart data point. Tells the frontend what was bought/sold and its value on that day.

---

### EventSortOrder

```rust
pub enum EventSortOrder {
    DateDesc,    // Newest first (default)
    DateAsc,     // Oldest first
    AmountDesc,  // Largest amount first
    AmountAsc,   // Smallest amount first
    AssetAsc,    // Alphabetical by symbol A→Z
    AssetDesc,   // Alphabetical by symbol Z→A
}
```

Used with `get_events_sorted()`.

---

### PortfolioSummary

```rust
pub struct PortfolioSummary {
    pub as_of_date: NaiveDate,      // Date this was computed for
    pub currency: String,           // Currency of all monetary values
    pub total_events: usize,        // Number of events in portfolio
    pub inception_date: Option<NaiveDate>, // Earliest event date
    pub total_value: f64,           // Current portfolio value
    pub total_invested: f64,        // Sum of buys (at buy-date prices)
    pub total_returned: f64,        // Sum of sells (at sell-date prices)
    pub total_gain_loss: f64,       // total_value + total_returned - total_invested
    pub total_return_pct: f64,      // (total_gain_loss / total_invested) × 100
    pub holdings: Vec<HoldingSummary>,
}
```

---

### HoldingSummary

```rust
pub struct HoldingSummary {
    pub asset: Asset,
    pub amount: f64,
    pub current_value: f64,
    pub total_invested: f64,
    pub cost_basis_per_unit: f64,  // total_invested / total_units_bought
    pub gain_loss: f64,            // current_value + sell_proceeds - total_invested
    pub return_pct: f64,
    pub allocation_pct: f64,       // (current_value / total_value) × 100
}
```

Sorted by `allocation_pct` (largest first). `gain_loss` now includes sell proceeds for partially-sold positions.

---

### Settings

```rust
pub struct Settings {
    pub default_currency: String,              // e.g. "USD", "PLN"
    pub api_keys: HashMap<String, String>,     // provider → key
}
```

Default: `{ default_currency: "USD", api_keys: {} }`

---

### PriceCache

```rust
pub struct PriceCache {
    pub entries: HashMap<(String, String), Vec<PricePoint>>,
    pub last_updated: HashMap<(String, String), NaiveDate>,
}
```

Internal cache stored inside the encrypted portfolio. Historical prices are immutable once cached. Today's price is refreshed once per session.

**Public methods on `PriceCache`:**

| Method | Description |
|--------|-------------|
| `get_price(symbol, currency, date)` | Cached price lookup (binary search) |
| `set_price(symbol, currency, date, price)` | Insert/update a price point |
| `set_prices(symbol, currency, &[PricePoint])` | Bulk insert |
| `get_price_range(symbol, currency, from, to)` | Range query (binary search) |
| `is_today_fresh(symbol, currency, today)` | Was today's price already fetched? |
| `total_entries()` | Total cached data points |
| `asset_count()` | Distinct (symbol, currency) pairs |
| `prune_before(date)` | Remove entries older than date |
| `clear()` | Remove everything |

---

## Error Handling

All fallible methods return `Result<T, CoreError>`. The error type is a single enum:

```rust
pub enum CoreError {
    // Storage
    InvalidFileFormat(String),
    UnsupportedVersion(u16),
    Encryption(String),
    Decryption,
    Serialization(String),
    Deserialization(String),
    FileIO(String),

    // Network / API
    Api { provider: String, message: String },
    Network(String),
    NoProvider(String),

    // Business logic
    ValidationError(String),
    EventNotFound(String),
    PriceNotAvailable { symbol: String, currency: String, date: String },
}
```

`CoreError` implements `std::error::Error`, `Debug`, `Display`, `Send`, and `Sync`.

**Automatic conversions (`From` impls):**
- `std::io::Error` → `FileIO`
- `bincode::Error` → `Serialization`
- `serde_json::Error` → `Deserialization`
- `reqwest::Error` → `Network` (with query-string sanitization to prevent API key leakage)
- `aes_gcm::Error` → `Decryption`

---

## Platform Notes

### WASM

- `load_from_file()` and `save_to_file()` are **not available** — use the `_bytes` variants
- Yahoo Finance provider is excluded (uses native-only connectors) — Alpha Vantage serves as the stock price provider
- `async_trait` uses `?Send` futures on WASM (reqwest + wasm-bindgen-futures produce non-Send futures)
- `uuid` and `getrandom` use the `js` feature for browser randomness

### Native (macOS / Windows / Linux)

- Full API available including file I/O
- Yahoo Finance is the primary stock provider (free, no API key)
- Alpha Vantage acts as a fallback (requires API key)

### Price Provider Fallback

When fetching a price, providers are tried in registration order. If the primary fails (network error, rate limit, etc.), the next provider for that asset type is tried automatically.

| Asset Type | Primary | Fallback | Key Required |
|------------|---------|----------|-------------|
| Crypto | CoinCap | — | No |
| Fiat | Frankfurter | — | No |
| Metal | metals.dev | — | Yes |
| Stock | Yahoo Finance (native) | Alpha Vantage | AV: Yes |
