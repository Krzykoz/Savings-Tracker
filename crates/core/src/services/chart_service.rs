use chrono::NaiveDate;

use crate::errors::CoreError;
use crate::models::asset::Asset;
use crate::models::chart::{ChartDataPoint, ChartEvent};
use crate::models::event::Event;
use crate::models::portfolio::Portfolio;
use crate::models::price::PriceCache;
use crate::services::currency_service::CurrencyService;
use crate::services::portfolio_service::PortfolioService;
use crate::services::price_service::PriceService;

/// Generates chart-ready data sets from portfolio data.
///
/// The core computes all the numbers — the frontend only renders.
/// Chart data includes:
/// - Daily portfolio value in the default currency
/// - Buy/sell event annotations (what was bought/sold and its value)
pub struct ChartService {
    portfolio_service: PortfolioService,
    currency_service: CurrencyService,
}

impl ChartService {
    pub fn new() -> Self {
        Self {
            portfolio_service: PortfolioService::new(),
            currency_service: CurrencyService::new(),
        }
    }

    /// Generate a full portfolio chart over a date range.
    ///
    /// For each day from `from` to `to`:
    /// 1. Maintain incremental holdings (apply events as we advance)
    /// 2. Get prices for all held assets
    /// 3. Convert everything to `currency` and sum up
    /// 4. Annotate any buy/sell events that happened on that date
    ///
    /// Uses incremental computation: O(days + events) instead of O(days × events).
    /// Returns Vec<ChartDataPoint> ready for frontend rendering.
    pub async fn generate_portfolio_chart(
        &self,
        portfolio: &Portfolio,
        price_service: &mut PriceService,
        price_cache: &mut PriceCache,
        from: NaiveDate,
        to: NaiveDate,
        currency: &str,
    ) -> Result<Vec<ChartDataPoint>, CoreError> {
        let mut chart_data = Vec::new();
        let mut current_date = from;
        let mut last_known_value = 0.0;

        // Pre-compute holdings at the start date (includes all events before `from`)
        let mut holdings: std::collections::HashMap<Asset, f64> =
            self.portfolio_service.get_holdings(portfolio, from);

        // Index events by date for O(1) lookup per day
        let mut events_by_date: std::collections::HashMap<NaiveDate, Vec<&Event>> =
            std::collections::HashMap::new();
        for event in &portfolio.events {
            if event.date >= from && event.date <= to {
                events_by_date.entry(event.date).or_default().push(event);
            }
        }

        // We already have holdings for `from`, but events ON `from` are already included
        // via get_holdings. For subsequent days, we apply events incrementally.
        let mut is_first_day = true;

        while current_date <= to {
            // Apply events for this date (skip on first day — already in initial holdings)
            if !is_first_day {
                if let Some(day_events) = events_by_date.get(&current_date) {
                    for event in day_events {
                        let amount = holdings.entry(event.asset.clone()).or_insert(0.0);
                        match event.event_type {
                            crate::models::event::EventType::Buy => *amount += event.amount,
                            crate::models::event::EventType::Sell => *amount -= event.amount,
                        }
                    }
                    // Clean up zero/negative holdings
                    holdings.retain(|_, amount| *amount > f64::EPSILON);
                }
            }
            is_first_day = false;

            // Calculate total portfolio value
            let mut portfolio_value = 0.0;
            let mut any_price_found = false;

            for (asset, amount) in &holdings {
                match self
                    .currency_service
                    .convert_asset_to_currency(
                        price_service,
                        price_cache,
                        asset,
                        *amount,
                        currency,
                        current_date,
                    )
                    .await
                {
                    Ok(value) => {
                        portfolio_value += value;
                        any_price_found = true;
                    }
                    Err(_) => {
                        continue;
                    }
                }
            }

            // Carry forward last known value on days with no price data (weekends/holidays)
            if !holdings.is_empty() && !any_price_found {
                portfolio_value = last_known_value;
            } else {
                last_known_value = portfolio_value;
            }

            // Collect events that happened on this date and compute their values
            let mut chart_events = Vec::new();
            if let Some(day_events) = events_by_date.get(&current_date) {
                for event in day_events {
                    let value = self
                        .currency_service
                        .convert_asset_to_currency(
                            price_service,
                            price_cache,
                            &event.asset,
                            event.amount,
                            currency,
                            current_date,
                        )
                        .await
                        .unwrap_or(0.0);

                    chart_events.push(ChartEvent {
                        event_type: event.event_type.clone(),
                        asset_symbol: event.asset.symbol.clone(),
                        amount: event.amount,
                        value_in_default_currency: value,
                    });
                }
            }

            chart_data.push(ChartDataPoint {
                date: current_date,
                portfolio_value,
                events: chart_events,
            });

            // Move to next day
            current_date = match current_date.succ_opt() {
                Some(next) => next,
                None => break,
            };
        }

        Ok(chart_data)
    }

    /// Generate a chart for a single asset's price history with events overlaid.
    ///
    /// Uses incremental holdings computation (O(days + events)) like `generate_portfolio_chart`.
    #[allow(clippy::too_many_arguments)]
    pub async fn generate_asset_chart(
        &self,
        portfolio: &Portfolio,
        price_service: &mut PriceService,
        price_cache: &mut PriceCache,
        asset_symbol: &str,
        from: NaiveDate,
        to: NaiveDate,
        currency: &str,
    ) -> Result<Vec<ChartDataPoint>, CoreError> {
        let mut chart_data = Vec::new();
        let mut current_date = from;
        let mut last_known_value = 0.0;
        let upper_symbol = asset_symbol.to_uppercase();

        // Find the asset in portfolio events
        let asset = portfolio
            .events
            .iter()
            .find(|e| e.asset.symbol == upper_symbol)
            .map(|e| e.asset.clone())
            .ok_or_else(|| CoreError::ValidationError(
                format!("Asset {asset_symbol} not found in portfolio events"),
            ))?;

        // Pre-compute holdings of this asset at the start date
        let initial_holdings = self.portfolio_service.get_holdings(portfolio, from);
        let mut amount_held = initial_holdings.get(&asset).copied().unwrap_or(0.0);

        // Index events for this asset by date for O(1) lookup
        let mut events_by_date: std::collections::HashMap<NaiveDate, Vec<&Event>> =
            std::collections::HashMap::new();
        for event in &portfolio.events {
            if event.asset.symbol == upper_symbol && event.date >= from && event.date <= to {
                events_by_date.entry(event.date).or_default().push(event);
            }
        }

        let mut is_first_day = true;

        while current_date <= to {
            // Apply events for this date incrementally (skip first day — already in initial holdings)
            if !is_first_day {
                if let Some(day_events) = events_by_date.get(&current_date) {
                    for event in day_events {
                        match event.event_type {
                            crate::models::event::EventType::Buy => amount_held += event.amount,
                            crate::models::event::EventType::Sell => amount_held -= event.amount,
                        }
                    }
                    if amount_held < f64::EPSILON {
                        amount_held = 0.0;
                    }
                }
            }
            is_first_day = false;

            // Calculate value, carry forward on weekends/holidays
            let portfolio_value = if amount_held > 0.0 {
                match self.currency_service
                    .convert_asset_to_currency(
                        price_service,
                        price_cache,
                        &asset,
                        amount_held,
                        currency,
                        current_date,
                    )
                    .await
                {
                    Ok(value) => {
                        last_known_value = value;
                        value
                    }
                    Err(_) => last_known_value,
                }
            } else {
                last_known_value = 0.0;
                0.0
            };

            // Events for this asset on this date — calculate values
            let mut events_with_values = Vec::new();
            if let Some(day_events) = events_by_date.get(&current_date) {
                for event in day_events {
                    let value = self
                        .currency_service
                        .convert_asset_to_currency(
                            price_service,
                            price_cache,
                            &event.asset,
                            event.amount,
                            currency,
                            current_date,
                        )
                        .await
                        .unwrap_or(0.0);

                    events_with_values.push(ChartEvent {
                        event_type: event.event_type.clone(),
                        asset_symbol: event.asset.symbol.clone(),
                        amount: event.amount,
                        value_in_default_currency: value,
                    });
                }
            }

            chart_data.push(ChartDataPoint {
                date: current_date,
                portfolio_value,
                events: events_with_values,
            });

            current_date = match current_date.succ_opt() {
                Some(next) => next,
                None => break,
            };
        }

        Ok(chart_data)
    }
}

impl Default for ChartService {
    fn default() -> Self {
        Self::new()
    }
}
