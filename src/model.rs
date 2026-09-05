use std::cmp::Ordering;
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Clone, Debug, Default)]
pub struct ProcessRow {
    pub pid: u32,
    pub parent_pid: Option<u32>,
    pub name: String,
    pub status: String,
    pub user: String,
    pub cpu_percent: f32,
    pub gpu_percent: f32,
    pub memory_bytes: u64,
    pub virtual_memory_bytes: u64,
    pub read_bytes_per_sec: f64,
    pub write_bytes_per_sec: f64,
    pub total_read_bytes: u64,
    pub total_write_bytes: u64,
    pub accumulated_cpu_millis: u64,
    pub started_at_unix: u64,
    pub executable: Option<PathBuf>,
    pub command: String,
    pub cwd: Option<PathBuf>,
}

#[derive(Clone, Debug, Default)]
pub struct CpuInfo {
    pub brand: String,
    pub frequency_mhz: u64,
    pub physical_cores: usize,
    pub logical_cores: usize,
}

#[derive(Clone, Debug, Default)]
pub struct DiskRow {
    pub name: String,
    pub mount: String,
    pub file_system: String,
    pub kind: String,
    pub total_bytes: u64,
    pub available_bytes: u64,
    pub read_bytes_per_sec: f64,
    pub write_bytes_per_sec: f64,
    pub removable: bool,
}

#[derive(Clone, Debug, Default)]
pub struct NetworkRow {
    pub name: String,
    pub received_bytes_per_sec: f64,
    pub transmitted_bytes_per_sec: f64,
    pub total_received_bytes: u64,
    pub total_transmitted_bytes: u64,
}

#[derive(Clone, Debug, Default)]
pub struct GpuSnapshot {
    pub available: bool,
    pub utilization_percent: f32,
    pub engine_utilization: Vec<(String, f32)>,
    pub error: Option<String>,
}

#[derive(Clone, Debug, Default)]
pub struct StartupRow {
    pub name: String,
    pub command: String,
    pub source: String,
}

#[derive(Clone, Debug, Default)]
pub struct ServiceRow {
    pub name: String,
    pub display_name: String,
    pub status: String,
    pub pid: u32,
}

#[derive(Clone, Debug, Default)]
pub struct UserSummary {
    pub name: String,
    pub process_count: usize,
    pub cpu_percent: f32,
    pub gpu_percent: f32,
    pub memory_bytes: u64,
    pub disk_bytes_per_sec: f64,
}

#[derive(Clone, Debug, Default)]
pub struct SystemSnapshot {
    pub sequence: u64,
    pub cpu_percent: f32,
    pub memory_used_bytes: u64,
    pub memory_total_bytes: u64,
    pub memory_available_bytes: u64,
    pub swap_used_bytes: u64,
    pub swap_total_bytes: u64,
    pub process_count: usize,
    pub uptime_seconds: u64,
    pub sample_seconds: f64,
    pub host_name: String,
    pub os_name: String,
    pub cpu: CpuInfo,
    pub processes: Vec<ProcessRow>,
    pub disks: Vec<DiskRow>,
    pub networks: Vec<NetworkRow>,
    pub gpu: GpuSnapshot,
    pub users: Vec<UserSummary>,
    pub startup: Arc<Vec<StartupRow>>,
    pub services: Arc<Vec<ServiceRow>>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SortColumn {
    Name,
    Pid,
    Status,
    User,
    Cpu,
    Gpu,
    Memory,
    ReadRate,
    WriteRate,
    CpuTime,
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
            SortColumn::User => natural_name_cmp(&a.user, &b.user),
            SortColumn::Cpu => float_cmp(a.cpu_percent, b.cpu_percent),
            SortColumn::Gpu => float_cmp(a.gpu_percent, b.gpu_percent),
            SortColumn::Memory => a.memory_bytes.cmp(&b.memory_bytes),
            SortColumn::ReadRate => float_cmp(a.read_bytes_per_sec, b.read_bytes_per_sec),
            SortColumn::WriteRate => float_cmp(a.write_bytes_per_sec, b.write_bytes_per_sec),
            SortColumn::CpuTime => a.accumulated_cpu_millis.cmp(&b.accumulated_cpu_millis),
        };

        let ordering = ordering.then_with(|| a.pid.cmp(&b.pid));
        match direction {
            SortDirection::Ascending => ordering,
            SortDirection::Descending => ordering.reverse(),
        }
    });
}

fn float_cmp<T: PartialOrd>(a: T, b: T) -> Ordering {
    a.partial_cmp(&b).unwrap_or(Ordering::Equal)
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
