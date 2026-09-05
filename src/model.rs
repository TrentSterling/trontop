use std::cmp::Ordering;
use std::path::PathBuf;

#[derive(Clone, Debug, Default)]
pub struct ProcessRow {
    pub pid: u32,
    pub parent_pid: Option<u32>,
    pub name: String,
    pub status: String,
    pub cpu_percent: f32,
    pub memory_bytes: u64,
    pub virtual_memory_bytes: u64,
    pub read_bytes_per_sec: f64,
    pub write_bytes_per_sec: f64,
    pub started_at_unix: u64,
    pub executable: Option<PathBuf>,
}

#[derive(Clone, Debug, Default)]
pub struct SystemSnapshot {
    pub sequence: u64,
    pub cpu_percent: f32,
    pub memory_used_bytes: u64,
    pub memory_total_bytes: u64,
    pub process_count: usize,
    pub uptime_seconds: u64,
    pub sample_seconds: f64,
    pub processes: Vec<ProcessRow>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SortColumn {
    Name,
    Pid,
    Status,
    Cpu,
    Memory,
    ReadRate,
    WriteRate,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SortDirection {
    Ascending,
    Descending,
}

impl SortDirection {
    pub fn toggled(self) -> Self {
        match self {
            Self::Ascending => Self::Descending,
            Self::Descending => Self::Ascending,
        }
    }
}

pub fn sort_processes(rows: &mut [ProcessRow], column: SortColumn, direction: SortDirection) {
    rows.sort_by(|a, b| {
        let ordering = match column {
            SortColumn::Name => natural_name_cmp(&a.name, &b.name),
            SortColumn::Pid => a.pid.cmp(&b.pid),
            SortColumn::Status => a.status.cmp(&b.status),
            SortColumn::Cpu => a
                .cpu_percent
                .partial_cmp(&b.cpu_percent)
                .unwrap_or(Ordering::Equal),
            SortColumn::Memory => a.memory_bytes.cmp(&b.memory_bytes),
            SortColumn::ReadRate => a
                .read_bytes_per_sec
                .partial_cmp(&b.read_bytes_per_sec)
                .unwrap_or(Ordering::Equal),
            SortColumn::WriteRate => a
                .write_bytes_per_sec
                .partial_cmp(&b.write_bytes_per_sec)
                .unwrap_or(Ordering::Equal),
        };

        let ordering = ordering.then_with(|| a.pid.cmp(&b.pid));
        match direction {
            SortDirection::Ascending => ordering,
            SortDirection::Descending => ordering.reverse(),
        }
    });
}

fn natural_name_cmp(a: &str, b: &str) -> Ordering {
    a.to_ascii_lowercase().cmp(&b.to_ascii_lowercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(pid: u32, name: &str, cpu: f32) -> ProcessRow {
        ProcessRow {
            pid,
            name: name.into(),
            cpu_percent: cpu,
            ..Default::default()
        }
    }

    #[test]
    fn sorts_cpu_descending() {
        let mut rows = vec![row(2, "low", 1.0), row(1, "high", 42.0)];
        sort_processes(&mut rows, SortColumn::Cpu, SortDirection::Descending);
        assert_eq!(rows.iter().map(|row| row.pid).collect::<Vec<_>>(), [1, 2]);
    }

    #[test]
    fn sorts_names_case_insensitively() {
        let mut rows = vec![row(2, "zeta", 0.0), row(1, "Alpha", 0.0)];
        sort_processes(&mut rows, SortColumn::Name, SortDirection::Ascending);
        assert_eq!(rows.iter().map(|row| row.pid).collect::<Vec<_>>(), [1, 2]);
    }
}
