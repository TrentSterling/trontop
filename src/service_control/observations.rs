//! Retained observations are keyed by SCM service name, not by the selected row.
use super::{Event, Status};
use std::collections::HashMap;
use std::time::{Duration, Instant};

const LIMIT: usize = 256;
const MAX_AGE: Duration = Duration::from_secs(75);

#[derive(Default)]
struct Record {
    command_at: Option<Instant>,
    observed: Option<(Instant, Status)>,
}

#[derive(Default)]
pub struct Observations {
    records: HashMap<String, Record>,
    overflow_at: Option<Instant>,
}

impl Observations {
    pub fn can_track(&self, name: &str) -> bool {
        self.overflow_at.is_none()
            && (self.records.contains_key(&name.to_lowercase()) || self.records.len() < LIMIT)
    }

    pub fn record(&mut self, event: &Event) {
        let Some(at) = event
            .command_at
            .into_iter()
            .chain(event.observed.map(|(at, _)| at))
            .max()
        else {
            return;
        };
        let name = event.name.to_lowercase(); // SCM names are case-insensitive.
        if self.records.len() >= LIMIT && !self.records.contains_key(&name) {
            // Defensive fail-closed path. The UI reserves capacity before dispatch.
            self.overflow_at = Some(self.overflow_at.map_or(at, |old| old.max(at)));
            return;
        }
        let record = self.records.entry(name).or_default();
        record.command_at = record.command_at.into_iter().chain(event.command_at).max();
        if let Some(observed) = event.observed
            && record.observed.is_none_or(|(old, _)| observed.0 >= old)
        {
            record.observed = Some(observed);
        }
    }

    pub fn reconcile(&mut self, inventory_at: Option<Instant>) {
        let Some(inventory_at) = inventory_at else {
            return;
        };
        self.records.retain(|_, record| {
            record
                .command_at
                .into_iter()
                .chain(record.observed.map(|(at, _)| at))
                .any(|at| at >= inventory_at)
        });
        if self.overflow_at.is_some_and(|at| inventory_at > at) {
            self.overflow_at = None;
        }
    }

    pub fn status(
        &self,
        name: &str,
        inventory_at: Option<Instant>,
        now: Instant,
    ) -> Option<Status> {
        let record = self.records.get(&name.to_lowercase())?;
        let (at, status) = record.observed?;
        (record.command_at.is_none_or(|command| at >= command)
            && inventory_at.is_none_or(|inventory| at >= inventory)
            && now.saturating_duration_since(at) <= MAX_AGE)
            .then_some(status)
    }

    pub fn uncertain(&self, name: &str, inventory_at: Option<Instant>) -> bool {
        if self
            .overflow_at
            .is_some_and(|at| inventory_at.is_none_or(|inventory| inventory <= at))
        {
            return true;
        }
        self.records
            .get(&name.to_lowercase())
            .is_some_and(|record| {
                record.command_at.is_some_and(|command| {
                    inventory_at.is_none_or(|inventory| inventory <= command)
                        && record.observed.is_none_or(|(at, _)| at < command)
                })
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::service_control::{Action, State};

    fn event(name: &str, at: Instant, observed: bool) -> Event {
        Event {
            name: name.into(),
            action: Action::Stop,
            phase: "Fixture",
            command_at: Some(at),
            observed: observed.then_some((
                at,
                Status {
                    state: State::Stopped,
                    ..Default::default()
                },
            )),
            done: true,
            error: None,
        }
    }

    #[test]
    fn separate_services_keep_results_and_uncertainty_across_selection_and_time() {
        let at = Instant::now();
        let mut records = Observations::default();
        records.record(&event("Alpha", at, true));
        records.record(&event("Beta", at, false));
        records.record(&event("Gamma", at, true));
        assert_eq!(
            records.status("ALPHA", None, at).unwrap().state,
            State::Stopped
        );
        assert!(records.uncertain("beta", None));
        assert!(!records.uncertain("gamma", None));
        assert!(
            records
                .status("alpha", None, at + MAX_AGE + Duration::from_secs(1))
                .is_none()
        );
        // Age alone must never erase an unresolved command, even with a full cache.
        assert!(records.uncertain("beta", None));
        records.reconcile(Some(at - Duration::from_millis(1)));
        assert!(records.uncertain("beta", Some(at - Duration::from_millis(1))));
        records.reconcile(Some(at + Duration::from_millis(1)));
        assert!(records.records.is_empty());
    }

    #[test]
    fn a_new_preflight_resolves_prior_uncertainty_but_queued_events_do_not() {
        let at = Instant::now();
        let mut records = Observations::default();
        records.record(&event("Alpha", at, false));
        let mut next = event("ALPHA", at, false);
        next.command_at = None;
        records.record(&next);
        assert!(records.uncertain("alpha", None));
        next.observed = Some((at + Duration::from_millis(1), Status::default()));
        records.record(&next);
        assert!(!records.uncertain("alpha", None));
        next.command_at = Some(at + Duration::from_millis(2));
        records.record(&next);
        assert!(records.uncertain("alpha", None));
        records.record(&event("Alpha", at, true)); // A delayed event cannot erase a newer barrier.
        assert!(records.uncertain("alpha", None));
    }

    #[test]
    fn capacity_never_evicts_unknown_outcomes_and_overflow_fails_closed() {
        let at = Instant::now();
        let mut records = Observations::default();
        for i in 0..LIMIT {
            records.record(&event(&format!("service{i}"), at, false));
        }
        assert!(!records.can_track("another"));
        assert!(records.can_track("service0"));
        records.record(&event("another", at, false));
        assert_eq!(records.records.len(), LIMIT);
        assert!(records.uncertain("any", None));
        records.reconcile(Some(at));
        assert!(!records.can_track("service0"));
        records.reconcile(Some(at + Duration::from_millis(1)));
        assert!(records.can_track("another"));
        assert!(!records.uncertain("any", None));
    }
}
