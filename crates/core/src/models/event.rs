use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::asset::Asset;

/// Type of portfolio event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EventType {
    /// Buying / acquiring an asset
    Buy,
    /// Selling / disposing of an asset
    Sell,
}

impl std::fmt::Display for EventType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EventType::Buy => write!(f, "Buy"),
            EventType::Sell => write!(f, "Sell"),
        }
    }
}

/// Sort order for event listings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EventSortOrder {
    /// Newest date first (default for display)
    DateDesc,
    /// Oldest date first
    DateAsc,
    /// Largest amount first
    AmountDesc,
    /// Smallest amount first
    AmountAsc,
    /// Alphabetical by asset symbol
    AssetAsc,
    /// Reverse alphabetical by asset symbol
    AssetDesc,
}

/// A single buy/sell event in the portfolio.
///
/// **Important**: Events do NOT store price. Price is fetched from APIs
/// based on the event date, and cached locally for offline access.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Event {
    /// Unique identifier
    pub id: Uuid,

    /// Buy or Sell
    pub event_type: EventType,

    /// The asset involved
    pub asset: Asset,

    /// Amount of the asset (always positive)
    pub amount: f64,

    /// Date of the event (no time component — daily granularity)
    pub date: NaiveDate,

    /// Optional free-text notes (e.g., reason, exchange, memo)
    #[serde(default)]
    pub notes: Option<String>,
}

impl Event {
    pub fn new(event_type: EventType, asset: Asset, amount: f64, date: NaiveDate) -> Self {
        Self {
            id: Uuid::new_v4(),
            event_type,
            asset,
            amount,
            date,
            notes: None,
        }
    }

    /// Create an event with notes attached.
    pub fn with_notes(
        event_type: EventType,
        asset: Asset,
        amount: f64,
        date: NaiveDate,
        notes: impl Into<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            event_type,
            asset,
            amount,
            date,
            notes: Some(notes.into()),
        }
    }
}
