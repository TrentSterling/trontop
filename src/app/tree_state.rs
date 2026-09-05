//! Ephemeral tree presentation state, never authority for native process actions.
use crate::model::ProcessRow;
use std::collections::{HashMap, HashSet};

#[derive(Clone, Copy)]
struct ObservedStart {
    native: Option<u64>,
    unix_seconds: Option<u64>,
}

impl ObservedStart {
    fn from_row(row: &ProcessRow) -> Self {
        Self {
            native: row.identity().map(|identity| identity.created_at_100ns),
            unix_seconds: (row.started_at_unix > 0).then_some(row.started_at_unix),
        }
    }

    fn continued(self, next: Self) -> Option<Self> {
        let changed = |old: Option<u64>, new: Option<u64>| matches!((old, new), (Some(old), Some(new)) if old != new);
        if changed(self.native, next.native) || changed(self.unix_seconds, next.unix_seconds) {
            return None;
        }
        // Missing access cannot erase a previously observed identity. Unknown-only
        // continuity is best effort for layout, not proof of process identity.
        Some(Self {
            native: next.native.or(self.native),
            unix_seconds: next.unix_seconds.or(self.unix_seconds),
        })
    }
}

#[derive(Default)]
pub(super) struct Expansion {
    pids: HashSet<u32>,
    starts: HashMap<u32, ObservedStart>,
}

impl Expansion {
    pub(super) fn pids(&self) -> &HashSet<u32> {
        &self.pids
    }

    pub(super) fn expand(&mut self, row: &ProcessRow) {
        self.pids.insert(row.pid);
        self.starts.insert(row.pid, ObservedStart::from_row(row));
    }

    pub(super) fn toggle(&mut self, row: &ProcessRow) {
        if self.pids.remove(&row.pid) {
            self.starts.remove(&row.pid);
        } else {
            self.expand(row);
        }
    }

    pub(super) fn retain_live(&mut self, processes: &[ProcessRow]) {
        if self.pids.is_empty() {
            return;
        }
        let mut retained = HashMap::with_capacity(self.starts.len().min(processes.len()));
        for row in processes {
            if let Some(old) = self.starts.get(&row.pid) {
                if let Some(start) = old.continued(ObservedStart::from_row(row)) {
                    retained.insert(row.pid, start);
                } else {
                    // If a malformed snapshot duplicates a PID, its last row wins,
                    // consistent with the display forest's canonical observation.
                    retained.remove(&row.pid);
                }
            }
        }
        self.pids.retain(|pid| retained.contains_key(pid));
        self.starts = retained;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(pid: u32, native: Option<u64>, unix_seconds: u64) -> ProcessRow {
        ProcessRow {
            pid,
            started_at_unix: unix_seconds,
            control: crate::model::ProcessControlInfo {
                created_at_100ns: native,
                ..Default::default()
            },
            ..Default::default()
        }
    }

    #[test]
    fn known_identity_survives_temporary_missing_access_but_not_reuse() {
        let mut state = Expansion::default();
        state.expand(&row(1, Some(100), 10));
        state.retain_live(&[row(1, None, 10)]);
        assert!(state.pids.contains(&1));
        assert_eq!(state.starts[&1].native, Some(100));
        state.retain_live(&[row(1, Some(101), 10)]);
        assert!(state.pids.is_empty());
        assert!(state.starts.is_empty());
    }

    #[test]
    fn seconds_fallback_is_enriched_and_contradictory_times_expire() {
        let mut state = Expansion::default();
        state.expand(&row(1, None, 10));
        state.retain_live(&[row(1, Some(100), 10)]);
        assert_eq!(state.starts[&1].native, Some(100));
        state.retain_live(&[row(1, None, 11)]);
        assert!(state.pids.is_empty());
        state.expand(&row(1, Some(100), 10));
        state.retain_live(&[row(1, Some(100), 11)]);
        assert!(state.pids.is_empty());
    }

    #[test]
    fn continuous_unknown_rows_keep_layout_but_disappearance_clears_state() {
        let mut state = Expansion::default();
        state.expand(&row(1, Some(0), 0));
        state.retain_live(&[row(1, None, 0)]);
        assert!(state.pids.contains(&1));
        state.retain_live(&[]);
        state.retain_live(&[row(1, None, 0)]);
        assert!(state.pids.is_empty());
        assert!(state.starts.is_empty());
    }

    #[test]
    fn toggles_removals_and_duplicate_last_observations_keep_caches_consistent() {
        let mut state = Expansion::default();
        let original = row(1, Some(100), 10);
        state.toggle(&original);
        assert!(state.pids.contains(&1));
        state.toggle(&original);
        assert!(state.pids.is_empty() && state.starts.is_empty());
        state.expand(&original);
        state.retain_live(&[original.clone(), row(1, Some(200), 20)]);
        assert!(state.pids.is_empty());
        state.expand(&original);
        state.retain_live(&[row(1, Some(200), 20), original]);
        assert!(state.pids.contains(&1));
        for pid in 2..1000 {
            let current = row(pid, Some(pid as u64), pid as u64);
            state.expand(&current);
            state.retain_live(&[current]);
            assert_eq!(state.pids.len(), 1);
            assert_eq!(state.starts.len(), 1);
        }
    }
}
