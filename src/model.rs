use crate::gpu_activity::Usage;
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;

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
    fn from_process(process: &ProcessRow) -> Self {
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

#[derive(Clone, Debug)]
pub struct ProcessTreeRow {
    pub process: ProcessRow,
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
    pub networks: Vec<NetworkRow>,
    pub gpu: GpuSnapshot,
    pub gpu_sensors: crate::gpu_sensors::SensorSnapshot,
    pub storage_sensors: Arc<crate::storage_sensors::Snapshot>,
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
    rows.sort_by(|a, b| compare_processes(a, b, column, direction));
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

pub fn build_process_tree(
    processes: &[ProcessRow],
    matching_pids: &HashSet<u32>,
    expanded_pids: &HashSet<u32>,
    column: SortColumn,
    direction: SortDirection,
) -> Vec<ProcessTreeRow> {
    let by_pid = processes
        .iter()
        .map(|process| (process.pid, process))
        .collect::<HashMap<_, _>>();
    let mut included = matching_pids.clone();
    let mut effective_expanded = expanded_pids.clone();
    let reveal_matches = matching_pids.len() != processes.len();
    for &pid in matching_pids {
        let mut cursor = pid;
        let mut visited = HashSet::new();
        while visited.insert(cursor) {
            let Some(parent_pid) = by_pid.get(&cursor).and_then(|row| row.parent_pid) else {
                break;
            };
            if !by_pid.contains_key(&parent_pid) {
                break;
            }
            included.insert(parent_pid);
            if reveal_matches {
                effective_expanded.insert(parent_pid);
            }
            cursor = parent_pid;
        }
    }

    let mut all_children = HashMap::<u32, Vec<u32>>::new();
    let mut visible_children = HashMap::<u32, Vec<u32>>::new();
    for process in processes {
        if let Some(parent_pid) = process.parent_pid.filter(|pid| *pid != process.pid)
            && by_pid.contains_key(&parent_pid)
        {
            all_children
                .entry(parent_pid)
                .or_default()
                .push(process.pid);
            if included.contains(&process.pid) && included.contains(&parent_pid) {
                visible_children
                    .entry(parent_pid)
                    .or_default()
                    .push(process.pid);
            }
        }
    }
    for children in visible_children.values_mut() {
        sort_pid_rows(children, &by_pid, column, direction);
    }

    let mut roots = included
        .iter()
        .copied()
        .filter(|pid| {
            by_pid.get(pid).is_some_and(|process| {
                process
                    .parent_pid
                    .is_none_or(|parent_pid| parent_pid == *pid || !included.contains(&parent_pid))
            })
        })
        .collect::<Vec<_>>();
    sort_pid_rows(&mut roots, &by_pid, column, direction);

    let mut totals_cache = HashMap::new();
    let mut rows = Vec::with_capacity(included.len());
    let mut emitted = HashSet::new();
    let mut reachable = HashSet::new();
    for &pid in &roots {
        mark_reachable(pid, &visible_children, &mut reachable);
        append_tree_rows(
            pid,
            0,
            &by_pid,
            &all_children,
            &visible_children,
            &effective_expanded,
            &mut totals_cache,
            &mut emitted,
            &mut rows,
        );
    }

    // A malformed or transient parent cycle has no natural root. Keep those processes
    // visible as roots instead of silently dropping them from the table.
    let mut leftovers = included.difference(&reachable).copied().collect::<Vec<_>>();
    sort_pid_rows(&mut leftovers, &by_pid, column, direction);
    for pid in leftovers {
        append_tree_rows(
            pid,
            0,
            &by_pid,
            &all_children,
            &visible_children,
            &effective_expanded,
            &mut totals_cache,
            &mut emitted,
            &mut rows,
        );
    }
    rows
}

fn mark_reachable(pid: u32, children: &HashMap<u32, Vec<u32>>, reached: &mut HashSet<u32>) {
    if !reached.insert(pid) {
        return;
    }
    for &child_pid in children.get(&pid).into_iter().flatten() {
        mark_reachable(child_pid, children, reached);
    }
}

fn sort_pid_rows(
    pids: &mut [u32],
    by_pid: &HashMap<u32, &ProcessRow>,
    column: SortColumn,
    direction: SortDirection,
) {
    pids.sort_by(|a, b| match (by_pid.get(a), by_pid.get(b)) {
        (Some(a), Some(b)) => compare_processes(a, b, column, direction),
        _ => a.cmp(b),
    });
}

#[allow(clippy::too_many_arguments)]
fn append_tree_rows(
    pid: u32,
    depth: usize,
    by_pid: &HashMap<u32, &ProcessRow>,
    all_children: &HashMap<u32, Vec<u32>>,
    visible_children: &HashMap<u32, Vec<u32>>,
    expanded_pids: &HashSet<u32>,
    totals_cache: &mut HashMap<u32, ProcessTotals>,
    emitted: &mut HashSet<u32>,
    output: &mut Vec<ProcessTreeRow>,
) {
    if !emitted.insert(pid) {
        return;
    }
    let Some(process) = by_pid.get(&pid) else {
        return;
    };
    let totals = tree_totals(pid, by_pid, all_children, totals_cache, &mut HashSet::new());
    let has_children = all_children.get(&pid).is_some_and(|rows| !rows.is_empty());
    let expanded = has_children && expanded_pids.contains(&pid);
    output.push(ProcessTreeRow {
        process: (*process).clone(),
        depth,
        has_children,
        descendant_count: totals.process_count.saturating_sub(1),
        expanded,
        totals,
    });
    if expanded {
        for &child_pid in visible_children.get(&pid).into_iter().flatten() {
            append_tree_rows(
                child_pid,
                depth + 1,
                by_pid,
                all_children,
                visible_children,
                expanded_pids,
                totals_cache,
                emitted,
                output,
            );
        }
    }
}

fn tree_totals(
    pid: u32,
    by_pid: &HashMap<u32, &ProcessRow>,
    all_children: &HashMap<u32, Vec<u32>>,
    cache: &mut HashMap<u32, ProcessTotals>,
    visiting: &mut HashSet<u32>,
) -> ProcessTotals {
    if let Some(total) = cache.get(&pid) {
        return *total;
    }
    let Some(process) = by_pid.get(&pid) else {
        return ProcessTotals::default();
    };
    if !visiting.insert(pid) {
        return ProcessTotals::default();
    }
    let mut total = ProcessTotals::from_process(process);
    for &child_pid in all_children.get(&pid).into_iter().flatten() {
        total.add(tree_totals(
            child_pid,
            by_pid,
            all_children,
            cache,
            visiting,
        ));
    }
    visiting.remove(&pid);
    cache.insert(pid, total);
    total
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
                .map(|row| row.process.pid)
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
        let rows = build_process_tree(
            &[root, child, grandchild],
            &HashSet::from([12]),
            &HashSet::new(),
            SortColumn::Name,
            SortDirection::Ascending,
        );
        assert_eq!(
            rows.iter().map(|row| row.process.pid).collect::<Vec<_>>(),
            [10, 11, 12]
        );
    }
}
