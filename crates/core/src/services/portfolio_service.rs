use chrono::{NaiveDate, Utc};
use std::collections::HashMap;
use uuid::Uuid;

use crate::errors::CoreError;
use crate::models::asset::Asset;
use crate::models::event::{Event, EventType};
use crate::models::portfolio::Portfolio;

/// Manages portfolio events (buy/sell) and calculates current holdings.
///
/// Pure business logic — no I/O, no API calls. Easy to test.
pub struct PortfolioService;

impl PortfolioService {
    pub fn new() -> Self {
        Self
    }

    /// Add a new event to the portfolio.
    /// Validates the event before adding (e.g., can't sell more than you own).
    pub fn add_event(&self, portfolio: &mut Portfolio, event: Event) -> Result<(), CoreError> {
        self.validate_event(portfolio, &event)?;
        // Binary insert: find the correct position to maintain date-sorted order (O(log n))
        let pos = portfolio
            .events
            .binary_search_by_key(&event.date, |e| e.date)
            .unwrap_or_else(|pos| pos);
        portfolio.events.insert(pos, event);
        Ok(())
    }

    /// Remove an event by its UUID.
    /// Revalidates all subsequent sell events to ensure portfolio consistency.
    pub fn remove_event(&self, portfolio: &mut Portfolio, event_id: Uuid) -> Result<(), CoreError> {
        let idx = portfolio
            .events
            .iter()
            .position(|e| e.id == event_id)
            .ok_or_else(|| CoreError::EventNotFound(event_id.to_string()))?;

        let removed = portfolio.events.remove(idx);

        // Revalidate: check all sell events at or after the removed event's date
        // to ensure none would cause negative holdings.
        if removed.event_type == EventType::Buy {
            if let Err(e) = self.validate_portfolio_consistency(portfolio, removed.date) {
                // Rollback: re-insert at correct position
                Self::binary_insert(&mut portfolio.events, removed);
                return Err(e);
            }
        }

        Ok(())
    }

    /// Update an existing event. Validates the new state before committing.
    pub fn update_event(
        &self,
        portfolio: &mut Portfolio,
        event_id: Uuid,
        event_type: EventType,
        asset: Asset,
        amount: f64,
        date: NaiveDate,
    ) -> Result<(), CoreError> {
        let idx = portfolio
            .events
            .iter()
            .position(|e| e.id == event_id)
            .ok_or_else(|| CoreError::EventNotFound(event_id.to_string()))?;

        // Take the old event out, apply changes, validate, then commit
        let old_event = portfolio.events.remove(idx);

        let updated = Event {
            id: old_event.id,
            event_type,
            asset,
            amount,
            date,
            notes: old_event.notes.clone(),
        };

        // Validate the updated event against the portfolio (without the old event)
        if let Err(e) = self.validate_event(portfolio, &updated) {
            // Rollback: put the old event back
            Self::binary_insert(&mut portfolio.events, old_event);
            return Err(e);
        }

        Self::binary_insert(&mut portfolio.events, updated);

        // Revalidate consistency of all subsequent events
        let check_date = portfolio.events.iter().map(|e| e.date).min().unwrap_or(date);
        if let Err(e) = self.validate_portfolio_consistency(portfolio, check_date) {
            // Rollback: swap back to old event
            if let Some(new_idx) = portfolio.events.iter().position(|e| e.id == old_event.id) {
                portfolio.events.remove(new_idx);
            }
            Self::binary_insert(&mut portfolio.events, old_event);
            return Err(e);
        }

        Ok(())
    }

    /// Get all events sorted by date (newest first for display).
    pub fn get_events<'a>(&self, portfolio: &'a Portfolio) -> Vec<&'a Event> {
        let mut events: Vec<&Event> = portfolio.events.iter().collect();
        events.sort_by(|a, b| b.date.cmp(&a.date)); // newest first
        events
    }

    /// Calculate how much of each asset is held on a specific date.
    ///
    /// Iterates through all events up to `date`, summing buys and subtracting sells.
    /// Returns only assets with positive holdings (amount > 0).
    pub fn get_holdings(&self, portfolio: &Portfolio, date: NaiveDate) -> HashMap<Asset, f64> {
        let mut holdings: HashMap<Asset, f64> = HashMap::new();

        for event in &portfolio.events {
            if event.date > date {
                continue; // skip future events
            }

            let amount = holdings.entry(event.asset.clone()).or_insert(0.0);
            match event.event_type {
                EventType::Buy => *amount += event.amount,
                EventType::Sell => *amount -= event.amount,
            }
        }

        // Remove assets with zero or negative holdings
        holdings.retain(|_, amount| *amount > f64::EPSILON);
        holdings
    }

    /// Validate an event before adding it to the portfolio.
    ///
    /// Rules:
    /// - Amount must be positive
    /// - Can't sell more than you currently own at that date
    fn validate_event(&self, portfolio: &Portfolio, event: &Event) -> Result<(), CoreError> {
        if event.amount <= 0.0 {
            return Err(CoreError::ValidationError(
                "Event amount must be positive".into(),
            ));
        }

        // Warn about future dates (allow +1 day tolerance for timezone differences)
        let today = Utc::now().date_naive();
        if let Some(tomorrow) = today.succ_opt() {
            if event.date > tomorrow {
                return Err(CoreError::ValidationError(
                    format!("Event date {} is in the future — prices won't be available", event.date),
                ));
            }
        }

        // For sell events, check you have enough of the asset
        if event.event_type == EventType::Sell {
            let holdings = self.get_holdings(portfolio, event.date);
            let current_amount = holdings.get(&event.asset).copied().unwrap_or(0.0);

            if current_amount < event.amount {
                return Err(CoreError::ValidationError(format!(
                    "Cannot sell {} {} — you only hold {} on {}",
                    event.amount, event.asset.symbol, current_amount, event.date
                )));
            }
        }

        Ok(())
    }

    /// Validate that no sell event in the portfolio causes negative holdings
    /// from `from_date` onwards. Used after event removal or update.
    fn validate_portfolio_consistency(
        &self,
        portfolio: &Portfolio,
        from_date: NaiveDate,
    ) -> Result<(), CoreError> {
        // Re-simulate holdings day by day for sell events from from_date onwards
        let mut holdings: HashMap<Asset, f64> = HashMap::new();

        for event in &portfolio.events {
            let amount = holdings.entry(event.asset.clone()).or_insert(0.0);
            match event.event_type {
                EventType::Buy => *amount += event.amount,
                EventType::Sell => {
                    if event.date >= from_date && *amount < event.amount {
                        return Err(CoreError::ValidationError(format!(
                            "Removing/updating this event would make sell of {} {} on {} invalid \
                             (only {:.8} would be held)",
                            event.amount,
                            event.asset.symbol,
                            event.date,
                            *amount,
                        )));
                    }
                    *amount -= event.amount;
                }
            }
        }
        Ok(())
    }

    /// Binary insert into a date-sorted Vec<Event> in O(log n).
    fn binary_insert(events: &mut Vec<Event>, event: Event) {
        let pos = events
            .binary_search_by_key(&event.date, |e| e.date)
            .unwrap_or_else(|pos| pos);
        events.insert(pos, event);
    }

    /// Set or clear the notes on an existing event.
    pub fn set_notes(
        &self,
        portfolio: &mut Portfolio,
        event_id: Uuid,
        notes: Option<String>,
    ) -> Result<(), CoreError> {
        let event = portfolio
            .events
            .iter_mut()
            .find(|e| e.id == event_id)
            .ok_or_else(|| CoreError::EventNotFound(event_id.to_string()))?;
        event.notes = notes;
        Ok(())
    }
}

impl Default for PortfolioService {
    fn default() -> Self {
        Self::new()
    }
}
