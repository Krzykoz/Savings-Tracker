pub mod registry;
pub mod traits;

// API provider implementations
pub mod alphavantage;
pub mod coincap;
pub mod frankfurter;
pub mod metals_dev;
#[cfg(not(target_arch = "wasm32"))]
pub mod yahoo_finance;
