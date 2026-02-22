# Savings Tracker

A portable, encrypted savings and investment tracking library written in Rust. Designed as the core engine for native (macOS/Windows/Linux), mobile, and web (WASM) frontends.

## Features

- **Multi-asset tracking** — Crypto, stocks, fiat currencies, and precious metals
- **Encrypted storage** — AES-256-GCM with Argon2id key derivation (`.svtk` file format)
- **Live & historical prices** — 5 API providers with automatic fallback and 30s timeouts
- **Portfolio analytics** — Total value, gain/loss, allocation %, per-asset breakdown with cost basis
- **Chart generation** — Day-by-day portfolio and per-asset value charts (up to 10 years)
- **Currency conversion** — Automatic cross-currency conversion (e.g. BTC → PLN)
- **Offline support** — Prices are cached locally inside the encrypted file
- **Search & sort** — Full-text event search, 6 sort orders, type/asset filtering
- **Bulk operations** — Atomic add/remove of multiple events
- **Trash & undo** — Soft-delete with single-level undo
- **Export/Import** — JSON and CSV export, JSON import
- **WASM-ready** — Runs in the browser via WebAssembly

## Supported Asset Types

| Type | Examples | Price Provider |
|------|----------|---------------|
| Crypto | BTC, ETH, SOL | CoinCap (free, no key) |
| Fiat | USD, EUR, PLN | Frankfurter / ECB (free, no key) |
| Metal | XAU, XAG, XPT | metals.dev (API key required) |
| Stock | AAPL, MSFT | Yahoo Finance (native) / Alpha Vantage (API key) |

## Architecture

```
savings-tracker-core/
├── models/          # Asset, Event, Portfolio, Settings, PriceCache, Analytics
├── services/        # PortfolioService, PriceService, ChartService, CurrencyService, AnalyticsService
├── providers/       # CoinCap, Frankfurter, metals.dev, Alpha Vantage, Yahoo Finance
├── storage/         # AES-256-GCM encryption, Argon2id KDF, SVTK binary format
└── lib.rs           # SavingsTracker — single entry-point facade
```

All business logic is in pure services with no I/O. API calls are isolated in the providers layer. The `SavingsTracker` struct is the only public entry point.

## Quick Start

```rust
use savings_tracker_core::SavingsTracker;
use savings_tracker_core::models::asset::Asset;
use savings_tracker_core::models::event::EventType;
use chrono::NaiveDate;

// Create a new portfolio
let mut tracker = SavingsTracker::create_new();

// Add events
tracker.add_event(
    EventType::Buy,
    Asset::crypto("BTC", "Bitcoin"),
    0.5,
    NaiveDate::from_ymd_opt(2025, 1, 15).unwrap(),
).unwrap();

// Save to encrypted bytes (for WASM / Tauri)
let bytes = tracker.save_to_bytes("my-password").unwrap();

// Load back
let tracker = SavingsTracker::load_from_bytes(&bytes, "my-password").unwrap();
```

### File I/O (native only)

```rust
// Save to disk
tracker.save_to_file("portfolio.svtk", "my-password").unwrap();

// Load from disk
let tracker = SavingsTracker::load_from_file("portfolio.svtk", "my-password").unwrap();
```

### Async Operations (prices, charts, analytics)

```rust
// Get portfolio value in default currency
let value = tracker.get_portfolio_value(today).await?;

// Get price of a single asset
let btc = Asset::crypto("BTC", "Bitcoin");
let price = tracker.get_asset_price(&btc, today).await?;

// Generate portfolio chart
let chart = tracker.generate_portfolio_chart(from, to).await?;

// Get full analytics summary
let summary = tracker.get_portfolio_summary(today).await?;
println!("Total value: {}", summary.total_value);
println!("Return: {:.1}%", summary.total_return_pct);
```

## API Reference

See [CORE_API.md](CORE_API.md) for the full API reference with method signatures, return types, error handling, and detailed usage examples.

## File Format (`.svtk`)

```
[SVTK: 4B] [version: 2B] [KDF params: 12B] [salt: 16B] [nonce: 12B] [ct_len: 8B] [ciphertext]
```

- **Encryption**: AES-256-GCM
- **Key derivation**: Argon2id (64 MB memory, 3 iterations, 4 parallelism)
- **Serialization**: bincode (compact binary)
- **Magic bytes**: `SVTK`

## Building

```bash
cargo build
```

## Testing

```bash
cargo test
```

486 tests across 6 test suites covering models, services, providers, storage, and integration scenarios.

```bash
cargo clippy --all-targets   # 0 warnings
```

## License

MIT
