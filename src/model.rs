use crate::gpu_activity::Usage;
use std::cmp::Ordering;
use std::collections::HashMap;
#[cfg(test)]
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;

#[cfg(test)]
mod process_tree_tests;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum PriorityClass {
    Idle,
    BelowNormal,
    #[default]
    Normal,
    AboveNormal,
    High,
    Realtime,
    Unknown,
}

impl PriorityClass {
    pub const EDITABLE: [Self; 5] = [
        Self::Idle,
        Self::BelowNormal,
        Self::Normal,
        Self::AboveNormal,
        Self::High,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Idle => "Idle",
            Self::BelowNormal => "Below normal",
            Self::Normal => "Normal",
            Self::AboveNormal => "Above normal",
            Self::High => "High",
            Self::Realtime => "Realtime",
            Self::Unknown => "Unavailable",
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ProcessControlInfo {
    pub created_at_100ns: Option<u64>,
    pub priority: PriorityClass,
    pub affinity_mask: usize,
    pub system_affinity_mask: usize,
    pub accessible: bool,
}

impl Default for ProcessControlInfo {
    fn default() -> Self {
        Self {
            created_at_100ns: None,
            priority: PriorityClass::Unknown,
            affinity_mask: 0,
            system_affinity_mask: 0,
            accessible: false,
        }
    }
}

impl ProcessControlInfo {
    pub fn for_observed_start(self, unix_seconds: u64) -> Self {
        const WINDOWS_UNIX_EPOCH_100NS: u64 = 116_444_736_000_000_000;
        let native_start = self
            .created_at_100ns
            .and_then(|time| time.checked_sub(WINDOWS_UNIX_EPOCH_100NS))
            .map(|time| time / 10_000_000);
        if native_start == Some(unix_seconds) {
            self
        } else {
            Self::default()
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct ProcessRow {
    pub pid: u32,
    pub parent_pid: Option<u32>,
    pub name: String,
    pub status: String,
    pub user: String,
    pub cpu_percent: f32,
    pub gpu_percent: Usage,
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
    pub control: ProcessControlInfo,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProcessIdentity {
    pub pid: u32,
    /// Exact Windows FILETIME, not the rounded display timestamp.
    pub created_at_100ns: u64,
}

impl ProcessRow {
    pub fn identity(&self) -> Option<ProcessIdentity> {
        self.control
            .created_at_100ns
            .filter(|time| *time > 0)
            .map(|created_at_100ns| ProcessIdentity {
                pid: self.pid,
                created_at_100ns,
            })
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct ProcessTotals {
    pub process_count: usize,
    pub cpu_percent: f32,
    pub gpu_percent: Usage,
    pub memory_bytes: u64,
    pub read_bytes_per_sec: f64,
    pub write_bytes_per_sec: f64,
}

impl ProcessTotals {
    pub(crate) fn from_process(process: &ProcessRow) -> Self {
        Self {
            process_count: 1,
            cpu_percent: process.cpu_percent,
            gpu_percent: process.gpu_percent,
            memory_bytes: process.memory_bytes,
            read_bytes_per_sec: process.read_bytes_per_sec,
            write_bytes_per_sec: process.write_bytes_per_sec,
        }
    }

    fn add(&mut self, child: Self) {
        if child.process_count == 0 {
            return;
        }
        self.process_count += child.process_count;
        self.cpu_percent += child.cpu_percent;
        self.gpu_percent = self.gpu_percent.sum(child.gpu_percent);
        self.memory_bytes = self.memory_bytes.saturating_add(child.memory_bytes);
        self.read_bytes_per_sec += child.read_bytes_per_sec;
        self.write_bytes_per_sec += child.write_bytes_per_sec;
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ProcessTreeRow {
    /// Valid only against the snapshot used to build this view.
    pub process_index: usize,
    pub depth: usize,
    pub has_children: bool,
    pub descendant_count: usize,
    pub expanded: bool,
    pub totals: ProcessTotals,
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
    pub valid_counters: usize,
    pub total_counters: usize,
    pub utilization_percent: f32,
    pub engine_utilization: Vec<(String, Usage)>,
    pub error: Option<String>,
}

impl GpuSnapshot {
    pub fn reading(&self) -> Usage {
        if self.available {
            if self.valid_counters == self.total_counters && self.error.is_none() {
                Usage::Measured(self.utilization_percent)
            } else {
                Usage::Partial(self.utilization_percent)
            }
        } else if self.error.is_none() {
            Usage::Warming
        } else {
            Usage::Unavailable
        }
    }

    pub fn for_process(&self, readings: &HashMap<u32, Usage>, pid: u32) -> Usage {
        readings.get(&pid).copied().unwrap_or_else(|| {
            if self.available {
                Usage::Unreported
            } else {
                self.reading()
            }
        })
    }
}

#[derive(Clone, Debug, Default)]
pub struct StartupRow {
    /// Registry value name or full Startup-folder filename, not the display stem.
    pub key: String,
    pub name: String,
    pub command: String,
    pub source: crate::startup::Source,
}

impl StartupRow {
    pub fn text_bytes(&self) -> usize {
        self.key.len() + self.name.len() + self.command.len()
    }
}

#[derive(Clone, Debug, Default)]
pub struct ServiceRow {
    pub name: String,
    pub display_name: String,
    pub status: crate::service_control::Status,
}

#[derive(Clone, Debug, Default)]
pub struct UserSummary {
    pub name: String,
    pub process_count: usize,
    pub cpu_percent: f32,
    pub gpu_percent: Usage,
    pub memory_bytes: u64,
    pub disk_bytes_per_sec: f64,
}

#[derive(Clone, Debug, Default)]
pub struct SystemSnapshot {
    pub diagnostics: crate::diagnostics::Diagnostics,
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
    pub physical_disks: Arc<crate::disk_activity::Snapshot>,
    pub networks: Vec<NetworkRow>,
    pub gpu: GpuSnapshot,
    pub gpu_sensors: crate::gpu_sensors::SensorSnapshot,
    pub storage_sensors: Arc<crate::storage_sensors::Snapshot>,
    pub users: Vec<UserSummary>,
    pub startup: Arc<crate::startup::Snapshot>,
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

#[cfg(test)]
pub fn sort_processes(rows: &mut [ProcessRow], column: SortColumn, direction: SortDirection) {
    rows.sort_by(|a, b| compare_processes(a, b, column, direction));
}

pub fn sort_process_indices(
    indices: &mut [usize],
    processes: &[ProcessRow],
    column: SortColumn,
    direction: SortDirection,
) {
    indices.sort_by(|&a, &b| compare_processes(&processes[a], &processes[b], column, direction));
}

fn compare_processes(
    a: &ProcessRow,
    b: &ProcessRow,
    column: SortColumn,
    direction: SortDirection,
) -> Ordering {
    // Missing entries stay below numeric values in both sort directions.
    if column == SortColumn::Gpu {
        match (a.gpu_percent.value(), b.gpu_percent.value()) {
            (Some(_), None) => return Ordering::Less,
            (None, Some(_)) => return Ordering::Greater,
            _ => {}
        }
    }
    let ordering = match column {
        SortColumn::Name => natural_name_cmp(&a.name, &b.name),
        SortColumn::Pid => a.pid.cmp(&b.pid),
        SortColumn::Status => a.status.cmp(&b.status),
        SortColumn::User => natural_name_cmp(&a.user, &b.user),
        SortColumn::Cpu => float_cmp(a.cpu_percent, b.cpu_percent),
        SortColumn::Gpu => float_cmp(a.gpu_percent.value(), b.gpu_percent.value()),
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
}

mod process_tree;
pub use process_tree::build_process_tree;

fn float_cmp<T: PartialOrd>(a: T, b: T) -> Ordering {
    a.partial_cmp(&b).unwrap_or(Ordering::Equal)
}

fn natural_name_cmp(a: &str, b: &str) -> Ordering {
    // Preserve ASCII-folded UTF-8 ordering without allocating two strings per comparison.
    a.bytes()
        .map(|b| b.to_ascii_lowercase())
        .cmp(b.bytes().map(|b| b.to_ascii_lowercase()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ascii_name_comparator_matches_previous_order_without_folded_string_storage() {
        let names = [
            "Alpha", "alpha", "zeta", "ZETA", "éApp", "ÉApp", "测试", "", "10.exe", "2.exe", "A\0B",
        ];
        for a in names {
            for b in names {
                assert_eq!(
                    natural_name_cmp(a, b),
                    a.to_ascii_lowercase().cmp(&b.to_ascii_lowercase())
                );
            }
        }
    }

    #[test]
    fn index_sort_keeps_snapshot_records_in_place_for_all_columns_and_directions() {
        let processes = (0..50)
            .map(|i| ProcessRow {
                pid: i + 1,
                name: format!("Fixture.{}", i % 7),
                user: format!("User.{}", i % 3),
                cpu_percent: (i % 13) as f32,
                gpu_percent: if i % 5 == 0 {
                    Usage::Unavailable
                } else {
                    Usage::Measured((i % 7) as f32)
                },
                memory_bytes: i as u64 * 10,
                read_bytes_per_sec: (i % 17) as f64,
                write_bytes_per_sec: (i % 11) as f64,
                accumulated_cpu_millis: i as u64 * 100,
                status: if i % 2 == 0 {
                    "Running".into()
                } else {
                    "Unknown".into()
                },
                ..Default::default()
            })
            .collect::<Vec<_>>();
        let original_names = processes
            .iter()
            .map(|p| p.name.as_ptr())
            .collect::<Vec<_>>();
        for column in [
            SortColumn::Name,
            SortColumn::Pid,
            SortColumn::Status,
            SortColumn::User,
            SortColumn::Cpu,
            SortColumn::Gpu,
            SortColumn::Memory,
            SortColumn::ReadRate,
            SortColumn::WriteRate,
            SortColumn::CpuTime,
        ] {
            for direction in [SortDirection::Ascending, SortDirection::Descending] {
                let mut copied = processes.clone();
                sort_processes(&mut copied, column, direction);
                let mut indices = (0..processes.len()).collect::<Vec<_>>();
                sort_process_indices(&mut indices, &processes, column, direction);
                assert_eq!(
                    indices
                        .iter()
                        .map(|&i| processes[i].pid)
                        .collect::<Vec<_>>(),
                    copied.iter().map(|p| p.pid).collect::<Vec<_>>()
                );
            }
        }
        assert_eq!(
            processes
                .iter()
                .map(|p| p.name.as_ptr())
                .collect::<Vec<_>>(),
            original_names
        );
    }

    #[test]
    fn gpu_sort_keeps_missing_last_and_tree_preserves_incomplete_totals() {
        let mut root = row(1, "root", 1.0);
        root.gpu_percent = Usage::Measured(0.0);
        let mut child = row(2, "child", 2.0);
        child.parent_pid = Some(1);
        child.gpu_percent = Usage::Unreported;
        let mut high = row(3, "high", 3.0);
        high.gpu_percent = Usage::Measured(80.0);
        let mut rows = vec![root, child, high];
        sort_processes(&mut rows, SortColumn::Gpu, SortDirection::Ascending);
        assert_eq!(rows.iter().map(|r| r.pid).collect::<Vec<_>>(), [1, 3, 2]);
        sort_processes(&mut rows, SortColumn::Gpu, SortDirection::Descending);
        assert_eq!(rows.iter().map(|r| r.pid).collect::<Vec<_>>(), [3, 1, 2]);
        let tree = build_process_tree(
            &rows,
            &HashSet::from([1, 2, 3]),
            &HashSet::new(),
            SortColumn::Pid,
            SortDirection::Ascending,
        );
        assert_eq!(tree[0].totals.gpu_percent, Usage::Partial(0.0));
        let mut snapshot = GpuSnapshot::default();
        let empty = HashMap::new();
        assert_eq!(snapshot.for_process(&empty, 1), Usage::Warming);
        snapshot.error = Some("Fixture unavailable".into());
        assert_eq!(snapshot.for_process(&empty, 1), Usage::Unavailable);
        snapshot.available = true;
        snapshot.error = None;
        assert_eq!(snapshot.for_process(&empty, 1), Usage::Unreported);
        assert_eq!(
            snapshot.for_process(&HashMap::from([(1, Usage::Measured(0.0))]), 1),
            Usage::Measured(0.0)
        );
    }

    #[test]
    fn contradictory_native_identity_is_not_attached_to_a_snapshot_row() {
        let control = ProcessControlInfo {
            created_at_100ns: Some(133_444_736_000_000_001),
            ..Default::default()
        };
        assert!(
            control
                .for_observed_start(1_700_000_000)
                .created_at_100ns
                .is_some()
        );
        assert!(
            control
                .for_observed_start(1_700_000_001)
                .created_at_100ns
                .is_none()
        );
        assert!(
            ProcessControlInfo::default()
                .for_observed_start(0)
                .created_at_100ns
                .is_none()
        );
    }

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

    #[test]
    fn tree_collapses_and_aggregates_descendants() {
        let mut root = row(10, "root", 4.0);
        root.memory_bytes = 100;
        let mut child = row(11, "child", 6.0);
        child.parent_pid = Some(10);
        child.memory_bytes = 50;
        let mut grandchild = row(12, "grandchild", 10.0);
        grandchild.parent_pid = Some(11);
        grandchild.memory_bytes = 25;
        let processes = vec![root, child, grandchild];
        let matches = HashSet::from([10, 11, 12]);

        let collapsed = build_process_tree(
            &processes,
            &matches,
            &HashSet::new(),
            SortColumn::Name,
            SortDirection::Ascending,
        );
        assert_eq!(collapsed.len(), 1);
        assert_eq!(collapsed[0].descendant_count, 2);
        assert_eq!(collapsed[0].totals.memory_bytes, 175);
        assert_eq!(collapsed[0].totals.cpu_percent, 20.0);

        let expanded = build_process_tree(
            &processes,
            &matches,
            &HashSet::from([10, 11]),
            SortColumn::Name,
            SortDirection::Ascending,
        );
        assert_eq!(
            expanded
                .iter()
                .map(|row| processes[row.process_index].pid)
                .collect::<Vec<_>>(),
            [10, 11, 12]
        );
        assert_eq!(
            expanded.iter().map(|row| row.depth).collect::<Vec<_>>(),
            [0, 1, 2]
        );
    }

    #[test]
    fn tree_search_keeps_ancestors_for_context() {
        let root = row(10, "root", 0.0);
        let mut child = row(11, "matching", 0.0);
        child.parent_pid = Some(10);
        let mut grandchild = row(12, "target", 0.0);
        grandchild.parent_pid = Some(11);
        let processes = [root, child, grandchild];
        let rows = build_process_tree(
            &processes,
            &HashSet::from([12]),
            &HashSet::new(),
            SortColumn::Name,
            SortDirection::Ascending,
        );
        assert_eq!(
            rows.iter()
                .map(|row| processes[row.process_index].pid)
                .collect::<Vec<_>>(),
            [10, 11, 12]
        );
    }
}
