use serde::{Deserialize, Serialize};

use super::asset::Asset;

/// Summary of the entire portfolio at a point in time.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortfolioSummary {
    /// Date this summary was computed for
    pub as_of_date: chrono::NaiveDate,

    /// Currency used for all monetary values
    pub currency: String,

    /// Total number of events in the portfolio
    pub total_events: usize,

    /// Date of the earliest event, if any
    pub inception_date: Option<chrono::NaiveDate>,

    /// Total portfolio value in the default display currency
    pub total_value: f64,

    /// Total amount invested (sum of all buy events' values in display currency)
    pub total_invested: f64,

    /// Total returned from sells (sum of all sell events' values in display currency)
    pub total_returned: f64,

    /// Absolute gain/loss: total_value + total_returned - total_invested
    pub total_gain_loss: f64,

    /// Percentage return: (total_gain_loss / total_invested) * 100
    pub total_return_pct: f64,

    /// Per-asset breakdown
    pub holdings: Vec<HoldingSummary>,
}

/// Summary of a single held asset.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HoldingSummary {
    /// The asset
    pub asset: Asset,

    /// Amount held
    pub amount: f64,

    /// Current value in the display currency
    pub current_value: f64,

    /// Total invested in this asset (sum of buy amounts × price at buy date)
    pub total_invested: f64,

    /// Average cost per unit (total_invested / total_units_bought)
    pub cost_basis_per_unit: f64,

    /// Absolute gain/loss for this asset
    pub gain_loss: f64,

    /// Percentage return for this asset
    pub return_pct: f64,

    /// Allocation percentage (this asset's value / total portfolio value × 100)
    pub allocation_pct: f64,
}
