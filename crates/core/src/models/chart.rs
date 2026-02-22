use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

use super::event::EventType;

/// A single data point for portfolio chart rendering.
///
/// The core generates these — the frontend just renders them.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChartDataPoint {
    /// The date for this data point
    pub date: NaiveDate,

    /// Total portfolio value in the default display currency at this date
    pub portfolio_value: f64,

    /// Any buy/sell events that happened on this date
    pub events: Vec<ChartEvent>,
}

/// An event annotation on a chart data point.
///
/// Tells the frontend "on this date, the user bought/sold X amount of Y,
/// worth Z in the display currency".
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChartEvent {
    /// Buy or Sell
    pub event_type: EventType,

    /// Asset symbol (e.g., "BTC", "USD")
    pub asset_symbol: String,

    /// Amount of the asset
    pub amount: f64,

    /// Value of this event in the default display currency
    pub value_in_default_currency: f64,
}
