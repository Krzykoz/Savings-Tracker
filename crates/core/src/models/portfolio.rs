use serde::{Deserialize, Serialize};

use super::event::Event;
use super::price::PriceCache;
use super::settings::Settings;

/// The main data container. Everything in here gets serialized,
/// encrypted, and saved to the portable .svtk file.
///
/// Contains: events (buy/sell history), user settings, and the price cache
/// (so historical prices are available offline).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Portfolio {
    /// All buy/sell events in the portfolio
    pub events: Vec<Event>,

    /// User settings (default currency, API keys, etc.)
    pub settings: Settings,

    /// Cached price data — historical prices saved for offline access.
    /// Once a historical price is fetched, it's stored here permanently.
    pub price_cache: PriceCache,

    /// Events that have been removed but can be restored (undo support).
    #[serde(default)]
    pub trash: Vec<Event>,
}

impl Default for Portfolio {
    fn default() -> Self {
        Self {
            events: Vec::new(),
            settings: Settings::default(),
            price_cache: PriceCache::new(),
            trash: Vec::new(),
        }
    }
}
