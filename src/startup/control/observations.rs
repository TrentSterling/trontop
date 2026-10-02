use super::{Key, Outcome, Receipt, Request};
use crate::model::StartupRow;
use crate::startup::{Entry, Snapshot, SourceSnapshot};
use std::{
    borrow::Cow,
    collections::BTreeMap,
    time::{Duration, Instant},
};

const LIMIT: usize = 64;
const BYTE_LIMIT: usize = 2 * 1024 * 1024;
#[derive(Default)]
pub struct Observations {
    records: BTreeMap<Key, Record>,
    evicted_fence: Option<Instant>,
}
struct Record {
    fence: Instant,
    receipt: Option<Receipt>,
    uncertain: bool,
}
impl Record {
    fn bytes(&self) -> usize {
        self.receipt.as_ref().map_or(0, |r| {
            r.target.text_bytes() + r.before.text_bytes() + r.after.text_bytes()
        })
    }
}
impl Observations {
    pub fn apply(&mut self, outcome: &Outcome) {
        let key = Key::of(&outcome.request.target);
        match &outcome.result {
            Ok(receipt) => {
                let mut receipt = receipt.clone();
                if let Some(control) = &mut receipt.target.control {
                    control.approval = receipt.after.clone();
                }
                self.records.insert(
                    key,
                    Record {
                        fence: receipt.observed_at,
                        receipt: Some(receipt),
                        uncertain: false,
                    },
                );
            }
            Err(error) if error.uncertain => {
                let record = self.records.entry(key).or_insert(Record {
                    fence: Instant::now(),
                    receipt: None,
                    uncertain: true,
                });
                record.fence = Instant::now();
                record.uncertain = true;
            }
            Err(_) => {}
        }
        while self.records.len() > LIMIT
            || self
                .records
                .iter()
                .map(|(key, record)| key.name.len() + record.bytes())
                .sum::<usize>()
                > BYTE_LIMIT
        {
            let Some(key) = self
                .records
                .iter()
                .min_by_key(|(_, record)| record.fence)
                .map(|(key, _)| key.clone())
            else {
                break;
            };
            if let Some(record) = self.records.remove(&key) {
                self.evicted_fence = Some(
                    self.evicted_fence
                        .map_or(record.fence, |at| at.max(record.fence)),
                );
            }
        }
    }
    pub fn reconcile(&mut self, snapshot: &Snapshot) {
        self.records.retain(|key, record| {
            let Some(source) = snapshot
                .sources
                .iter()
                .find(|source| source.source == key.source)
            else {
                return true;
            };
            if let Some(entry) = source
                .entries
                .iter()
                .find(|entry| Key::of(&entry.row) == *key)
            {
                if entry.observed_at <= record.fence {
                    return true;
                }
                record.uncertain = false;
                record
                    .receipt
                    .as_ref()
                    .is_some_and(|receipt| undo_matches(receipt, &entry.row))
            } else {
                source.last_complete.is_none_or(|at| at <= record.fence)
            }
        });
    }
    pub fn view<'a>(
        &'a self,
        source: &SourceSnapshot,
        entry: &'a Entry,
        now: Instant,
    ) -> (Cow<'a, StartupRow>, &'static str, bool) {
        let normal = source.row_state(entry, now);
        if let Some(record) = self.records.get(&Key::of(&entry.row))
            && entry.observed_at <= record.fence
        {
            if record.uncertain {
                return (Cow::Borrowed(&entry.row), "Refresh required", false);
            }
            if let Some(receipt) = &record.receipt
                && same_registration(&receipt.target, &entry.row)
            {
                return (
                    Cow::Borrowed(&receipt.target),
                    "Command read",
                    now.saturating_duration_since(receipt.observed_at) <= Duration::from_secs(75),
                );
            }
            return (Cow::Borrowed(&entry.row), "Pre-command", false);
        }
        if self.evicted_fence.is_some_and(|at| entry.observed_at <= at) {
            return (Cow::Borrowed(&entry.row), "Refresh required", false);
        }
        (
            Cow::Borrowed(&entry.row),
            normal,
            matches!(normal, "Live" | "Observed"),
        )
    }
    pub fn undo(&self, row: &StartupRow) -> Option<Request> {
        self.undo_receipt(row).map(Receipt::undo)
    }
    pub fn observed_at(&self, row: &StartupRow, inventory_at: Instant) -> Instant {
        self.records
            .get(&Key::of(row))
            .and_then(|record| record.receipt.as_ref())
            .filter(|receipt| receipt.observed_at >= inventory_at && undo_matches(receipt, row))
            .map_or(inventory_at, |receipt| receipt.observed_at)
    }
    pub fn next_state_change(&self, snapshot: &Snapshot, now: Instant) -> Option<Instant> {
        snapshot
            .rows()
            .filter_map(|(_, entry)| {
                let record = self.records.get(&Key::of(&entry.row))?;
                let receipt = record.receipt.as_ref()?;
                if record.uncertain
                    || entry.observed_at > record.fence
                    || !same_registration(&receipt.target, &entry.row)
                {
                    return None;
                }
                let at = receipt.observed_at + Duration::from_secs(75);
                (at > now).then_some(at)
            })
            .min()
    }
    pub fn can_undo(&self, row: &StartupRow) -> bool {
        self.undo_receipt(row).is_some()
    }
    fn undo_receipt(&self, row: &StartupRow) -> Option<&Receipt> {
        let record = self.records.get(&Key::of(row))?;
        if record.uncertain {
            return None;
        }
        record
            .receipt
            .as_ref()
            .filter(|receipt| undo_matches(receipt, row))
    }
}
fn same_registration(a: &StartupRow, b: &StartupRow) -> bool {
    Key::of(a) == Key::of(b)
        && a.command == b.command
        && a.control
            .as_ref()
            .zip(b.control.as_ref())
            .is_some_and(|(a, b)| a.registration == b.registration && a.registration.is_some())
}
fn undo_matches(receipt: &Receipt, row: &StartupRow) -> bool {
    same_registration(&receipt.target, row)
        && row
            .control
            .as_ref()
            .is_some_and(|control| control.approval == receipt.after)
}

#[cfg(test)]
mod tests;
