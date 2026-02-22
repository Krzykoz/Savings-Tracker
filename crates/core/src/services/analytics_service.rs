use chrono::NaiveDate;

use crate::errors::CoreError;
use crate::models::analytics::{HoldingSummary, PortfolioSummary};
use crate::models::event::EventType;
use crate::models::portfolio::Portfolio;
use crate::models::price::PriceCache;
use crate::services::currency_service::CurrencyService;
use crate::services::portfolio_service::PortfolioService;
use crate::services::price_service::PriceService;

/// Computes portfolio analytics: gain/loss, returns, allocation breakdown.
///
/// All calculations use market prices from APIs (current or cached).
/// Cost basis is determined by the market price on the event date.
pub struct AnalyticsService {
    portfolio_service: PortfolioService,
    currency_service: CurrencyService,
}

impl AnalyticsService {
    pub fn new() -> Self {
        Self {
            portfolio_service: PortfolioService::new(),
            currency_service: CurrencyService::new(),
        }
    }

    /// Generate a full portfolio summary at a given date.
    ///
    /// Computes:
    /// - Total current value
    /// - Total invested (sum of buy event values at their dates)
    /// - Total returned (sum of sell event values at their dates)
    /// - Gain/loss and % return (overall and per-asset)
    /// - Allocation percentages
    pub async fn get_portfolio_summary(
        &self,
        portfolio: &Portfolio,
        price_service: &PriceService,
        price_cache: &mut PriceCache,
        date: NaiveDate,
        currency: &str,
    ) -> Result<PortfolioSummary, CoreError> {
        let holdings = self.portfolio_service.get_holdings(portfolio, date);

        // 1. Calculate current value per asset
        let mut holding_summaries = Vec::new();
        let mut total_value = 0.0;

        for (asset, amount) in &holdings {
            let current_value = self
                .currency_service
                .convert_asset_to_currency(
                    price_service,
                    price_cache,
                    asset,
                    *amount,
                    currency,
                    date,
                )
                .await?;

            total_value += current_value;

            holding_summaries.push(HoldingSummary {
                asset: asset.clone(),
                amount: *amount,
                current_value,
                total_invested: 0.0,      // filled below
                cost_basis_per_unit: 0.0,  // filled below
                gain_loss: 0.0,           // filled below
                return_pct: 0.0,          // filled below
                allocation_pct: 0.0,      // filled below
            });
        }

        // 2. Calculate total invested and returned from events
        let mut total_invested = 0.0;
        let mut total_returned = 0.0;

        // Track per-asset invested and returned amounts, and total units bought
        let mut asset_invested: std::collections::HashMap<
            crate::models::asset::Asset,
            f64,
        > = std::collections::HashMap::new();
        let mut asset_returned: std::collections::HashMap<
            crate::models::asset::Asset,
            f64,
        > = std::collections::HashMap::new();
        let mut asset_units_bought: std::collections::HashMap<
            crate::models::asset::Asset,
            f64,
        > = std::collections::HashMap::new();

        for event in &portfolio.events {
            if event.date > date {
                continue;
            }

            let event_value = self
                .currency_service
                .convert_asset_to_currency(
                    price_service,
                    price_cache,
                    &event.asset,
                    event.amount,
                    currency,
                    event.date,
                )
                .await?;

            match event.event_type {
                EventType::Buy => {
                    total_invested += event_value;
                    *asset_invested.entry(event.asset.clone()).or_insert(0.0) += event_value;
                    *asset_units_bought.entry(event.asset.clone()).or_insert(0.0) += event.amount;
                }
                EventType::Sell => {
                    total_returned += event_value;
                    *asset_returned.entry(event.asset.clone()).or_insert(0.0) += event_value;
                }
            }
        }

        // 3. Fill in per-asset details
        for holding in &mut holding_summaries {
            let invested = asset_invested.get(&holding.asset).copied().unwrap_or(0.0);
            let returned = asset_returned.get(&holding.asset).copied().unwrap_or(0.0);
            let units_bought = asset_units_bought.get(&holding.asset).copied().unwrap_or(0.0);
            holding.total_invested = invested;
            holding.cost_basis_per_unit = if units_bought > 0.0 {
                invested / units_bought
            } else {
                0.0
            };
            // I5: gain/loss = current_value + sell_proceeds - total_invested
            holding.gain_loss = holding.current_value + returned - invested;
            holding.return_pct = if invested > 0.0 {
                (holding.gain_loss / invested) * 100.0
            } else {
                0.0
            };
            holding.allocation_pct = if total_value > 0.0 {
                (holding.current_value / total_value) * 100.0
            } else {
                0.0
            };
        }

        // Sort by allocation (largest first)
        holding_summaries.sort_by(|a, b| {
            b.allocation_pct
                .partial_cmp(&a.allocation_pct)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // 4. Overall gain/loss
        let total_gain_loss = total_value + total_returned - total_invested;
        let total_return_pct = if total_invested > 0.0 {
            (total_gain_loss / total_invested) * 100.0
        } else {
            0.0
        };

        // G3: Compute context fields
        let inception_date = portfolio.events.iter().map(|e| e.date).min();

        Ok(PortfolioSummary {
            as_of_date: date,
            currency: currency.to_string(),
            total_events: portfolio.events.len(),
            inception_date,
            total_value,
            total_invested,
            total_returned,
            total_gain_loss,
            total_return_pct,
            holdings: holding_summaries,
        })
    }
}

impl Default for AnalyticsService {
    fn default() -> Self {
        Self::new()
    }
}
