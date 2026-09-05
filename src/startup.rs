//! Per-source startup snapshots. Failed enumeration cannot prove an entry was removed.
use crate::model::StartupRow;
use std::collections::BTreeMap;
use std::time::{Duration, Instant};

pub const ENTRY_LIMIT: usize = 4096;
pub const TEXT_LIMIT: usize = 2 * 1024 * 1024;
const STALE_AFTER: Duration = Duration::from_secs(75);

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Ord, PartialOrd)]
pub enum Source {
    #[default]
    UserRun,
    MachineRun,
    MachineRun32,
    UserFolder,
    MachineFolder,
}

impl Source {
    pub const ALL: [Self; 5] = [
        Self::UserRun,
        Self::MachineRun,
        Self::MachineRun32,
        Self::UserFolder,
        Self::MachineFolder,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Self::UserRun => "Current user Run key",
            Self::MachineRun => "Machine Run key",
            Self::MachineRun32 => "32-bit machine Run key",
            Self::UserFolder => "Current user Startup folder",
            Self::MachineFolder => "Machine Startup folder",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReadState {
    Readable,
    Missing,
    Failed,
}

pub struct Read {
    pub source: Source,
    pub rows: Vec<StartupRow>,
    pub state: ReadState,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum State {
    Starting,
    Live,
    Empty,
    Absent,
    Partial,
    Cached,
    Unavailable,
}
impl State {
    pub fn label(self) -> &'static str {
        match self {
            Self::Starting => "Starting",
            Self::Live => "Live",
            Self::Empty => "Empty",
            Self::Absent => "Absent",
            Self::Partial => "Partial",
            Self::Cached => "Cached",
            Self::Unavailable => "Unavailable",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Entry {
    pub row: StartupRow,
    pub observed_at: Instant,
    pub observed_in_attempt: bool,
}

#[derive(Clone, Debug)]
pub struct SourceSnapshot {
    pub source: Source,
    pub last_attempt: Option<Instant>,
    pub last_complete: Option<Instant>,
    pub result: Option<ReadState>,
    pub entries: Vec<Entry>,
    pub retention_limited: bool,
}

impl SourceSnapshot {
    fn new(source: Source) -> Self {
        Self {
            source,
            last_attempt: None,
            last_complete: None,
            result: None,
            entries: Vec::new(),
            retention_limited: false,
        }
    }

    pub fn state(&self, now: Instant) -> State {
        let Some(result) = self.result else {
            return State::Starting;
        };
        if self
            .last_attempt
            .is_none_or(|at| now.saturating_duration_since(at) > STALE_AFTER)
        {
            return if self.last_complete.is_some() || !self.entries.is_empty() {
                State::Cached
            } else {
                State::Unavailable
            };
        }
        match result {
            ReadState::Readable if self.entries.is_empty() => State::Empty,
            ReadState::Readable => State::Live,
            ReadState::Missing => State::Absent,
            ReadState::Failed if self.entries.iter().any(|e| e.observed_in_attempt) => {
                State::Partial
            }
            ReadState::Failed if self.last_complete.is_some() || !self.entries.is_empty() => {
                State::Cached
            }
            ReadState::Failed => State::Unavailable,
        }
    }

    pub fn row_state(&self, entry: &Entry, now: Instant) -> &'static str {
        if self
            .last_attempt
            .is_none_or(|at| now.saturating_duration_since(at) > STALE_AFTER)
        {
            return "Cached";
        }
        match self.result {
            Some(ReadState::Readable) => "Live",
            Some(ReadState::Failed) if entry.observed_in_attempt => "Observed",
            _ => "Cached",
        }
    }

    fn apply(&mut self, mut read: Read, at: Instant) {
        // An over-limit result is incomplete even if a future provider mislabels it.
        if read.rows.len() > ENTRY_LIMIT
            || read.rows.iter().map(StartupRow::text_bytes).sum::<usize>() > TEXT_LIMIT
        {
            read.state = ReadState::Failed;
            self.retention_limited = true;
        }
        self.last_attempt = Some(at);
        self.result = Some(read.state);
        for entry in &mut self.entries {
            entry.observed_in_attempt = false;
        }
        if read.state != ReadState::Failed {
            self.entries.clear();
            self.last_complete = Some(at);
            self.retention_limited = false;
        }
        if read.state == ReadState::Missing {
            return; // A confirmed absent key/folder is an authoritative empty result.
        }
        let mut entries = std::mem::take(&mut self.entries)
            .into_iter()
            .map(|entry| (entry.row.key.clone(), entry))
            .collect::<BTreeMap<_, _>>();
        let mut text_bytes = entries
            .values()
            .map(|entry| entry.row.text_bytes())
            .sum::<usize>();
        for mut row in read.rows.into_iter().take(ENTRY_LIMIT) {
            row.source = self.source;
            let old_bytes = entries
                .get(&row.key)
                .map_or(0, |entry| entry.row.text_bytes());
            let new_bytes = text_bytes - old_bytes + row.text_bytes();
            if (entries.len() == ENTRY_LIMIT && !entries.contains_key(&row.key))
                || new_bytes > TEXT_LIMIT
            {
                self.retention_limited = true;
                continue;
            }
            text_bytes = new_bytes;
            entries.insert(
                row.key.clone(),
                Entry {
                    row,
                    observed_at: at,
                    observed_in_attempt: true,
                },
            );
        }
        self.entries = entries.into_values().collect();
        self.entries
            .sort_by_cached_key(|entry| (entry.row.name.to_lowercase(), entry.row.key.clone()));
    }
}

#[derive(Clone, Debug)]
pub struct Snapshot {
    pub sources: Vec<SourceSnapshot>,
    order: Vec<(usize, usize)>,
}
impl Default for Snapshot {
    fn default() -> Self {
        Self {
            sources: Source::ALL.into_iter().map(SourceSnapshot::new).collect(),
            order: Vec::new(),
        }
    }
}
impl Snapshot {
    pub fn apply(&mut self, reads: Vec<Read>, at: Instant) {
        let mut reads = reads
            .into_iter()
            .map(|read| (read.source, read))
            .collect::<BTreeMap<_, _>>();
        for source in &mut self.sources {
            let read = reads.remove(&source.source).unwrap_or(Read {
                source: source.source,
                rows: Vec::new(),
                state: ReadState::Failed,
            });
            source.apply(read, at);
        }
        // Rebuild only when inventory/cache state changes, not every UI frame.
        self.order = self
            .sources
            .iter()
            .enumerate()
            .flat_map(|(s, source)| (0..source.entries.len()).map(move |e| (s, e)))
            .collect();
        self.order.sort_by_cached_key(|&(s, e)| {
            let row = &self.sources[s].entries[e].row;
            (
                row.name.to_lowercase(),
                self.sources[s].source,
                row.key.clone(),
            )
        });
    }

    pub fn coverage(&self) -> (usize, usize) {
        (
            self.sources
                .iter()
                .filter(|s| matches!(s.result, Some(ReadState::Readable | ReadState::Missing)))
                .count(),
            Source::ALL.len(),
        )
    }

    pub fn rows(&self) -> impl Iterator<Item = (&SourceSnapshot, &Entry)> {
        self.order
            .iter()
            .map(|&(s, e)| (&self.sources[s], &self.sources[s].entries[e]))
    }
}

#[cfg(test)]
mod tests;
