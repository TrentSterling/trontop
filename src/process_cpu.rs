//! Per-process CPU time from one native process snapshot per sample.
//!
//! sysinfo 0.38.4 computes process CPU by calling `GetSystemTimes` once for
//! every process. On a busy 24-thread Windows 11 box that call (which is
//! `NtQuerySystemInformation(SystemProcessorPerformanceInformation)`) was
//! measured at 100 to 670 ms per call, so a 480-process refresh took 40 to 65 s.
//! This module reads every process's kernel/user time with one
//! `NtQuerySystemInformation(SystemProcessInformation)` call (about 15 ms) and
//! reads the system times once per sample. The percentage formula is the same
//! one sysinfo uses, already divided by the logical processor count the way the
//! sampler normalized it: `100 * process_cpu_delta / system_cpu_delta`.

use std::collections::HashMap;

/// Cumulative CPU time for one process instance, in 100 ns units.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ProcessTimes {
    /// Exact FILETIME creation time. Keys a PID to one process instance.
    pub created_at_100ns: u64,
    pub cpu_100ns: u64,
}

impl ProcessTimes {
    /// Same unit and rounding as sysinfo's `accumulated_cpu_time`.
    pub fn accumulated_millis(self) -> u64 {
        self.cpu_100ns / 10_000
    }

    /// Unix seconds, matching sysinfo's `start_time` for the same instance.
    pub fn started_at_unix(self) -> Option<u64> {
        const WINDOWS_UNIX_EPOCH_100NS: u64 = 116_444_736_000_000_000;
        self.created_at_100ns
            .checked_sub(WINDOWS_UNIX_EPOCH_100NS)
            .map(|time| time / 10_000_000)
    }
}

/// One native reading: every process's times plus the system's total
/// (kernel including idle, plus user) CPU time.
#[derive(Clone, Debug, Default)]
pub struct Reading {
    pub processes: HashMap<u32, ProcessTimes>,
    pub system_cpu_100ns: u64,
}

/// Keeps the previous reading and turns deltas into percentages.
#[derive(Default)]
pub struct Tracker {
    previous: Option<Reading>,
    current: Reading,
}

impl Tracker {
    /// Accepts a fresh reading. A failed read clears the baseline so the next
    /// success never divides across a gap it did not observe.
    pub fn update(&mut self, reading: Option<Reading>) {
        match reading {
            Some(reading) => {
                self.previous = Some(std::mem::replace(&mut self.current, reading));
            }
            None => {
                self.previous = None;
                self.current = Reading::default();
            }
        }
    }

    pub fn times(&self, pid: u32) -> Option<ProcessTimes> {
        self.current.processes.get(&pid).copied()
    }

    /// CPU share of the whole machine (0..=100) since the previous reading.
    /// `None` when this instance has no current reading. A process first seen
    /// in this reading, or a reused PID, reports 0 like sysinfo's first update.
    pub fn percent(&self, pid: u32) -> Option<f32> {
        let now = self.current.processes.get(&pid)?;
        let Some(previous) = &self.previous else {
            return Some(0.0);
        };
        let Some(before) = previous
            .processes
            .get(&pid)
            .filter(|before| before.created_at_100ns == now.created_at_100ns)
        else {
            return Some(0.0);
        };
        Some(percent(
            now.cpu_100ns.saturating_sub(before.cpu_100ns),
            self.current
                .system_cpu_100ns
                .saturating_sub(previous.system_cpu_100ns),
        ))
    }
}

/// sysinfo's formula `100 * dproc / dsystem * cpus`, then divided by `cpus`.
pub fn percent(process_delta_100ns: u64, system_delta_100ns: u64) -> f32 {
    let denominator = system_delta_100ns as f32;
    if denominator < 0.00001 {
        return 0.0;
    }
    (100.0 * (process_delta_100ns as f32 / denominator)).clamp(0.0, 100.0)
}

#[cfg(windows)]
pub fn read() -> Result<Reading, String> {
    use windows::Win32::Foundation::FILETIME;
    use windows::Win32::System::Threading::GetSystemTimes;

    // Read the processes before the system total, the same order sysinfo used
    // (GetProcessTimes, then GetSystemTimes).
    let processes = read_processes()?;
    let mut idle = FILETIME::default();
    let mut kernel = FILETIME::default();
    let mut user = FILETIME::default();
    unsafe { GetSystemTimes(Some(&mut idle), Some(&mut kernel), Some(&mut user)) }
        .map_err(|error| format!("GetSystemTimes failed: {error}"))?;
    let filetime =
        |time: FILETIME| (u64::from(time.dwHighDateTime) << 32) | u64::from(time.dwLowDateTime);
    Ok(Reading {
        processes,
        // Kernel time includes idle time, exactly as sysinfo summed it.
        system_cpu_100ns: filetime(kernel).saturating_add(filetime(user)),
    })
}

#[cfg(not(windows))]
pub fn read() -> Result<Reading, String> {
    Err("Native process times are only read on Windows.".into())
}

/// Leading fields of `SYSTEM_PROCESS_INFORMATION` (winternl.h / ntexapi.h).
/// `repr(C)` reproduces the native padding on both 32 and 64 bit targets.
#[cfg(windows)]
#[repr(C)]
#[derive(Clone, Copy)]
#[allow(dead_code)] // Layout fields: only the times and PID are read.
struct SystemProcessInformation {
    next_entry_offset: u32,
    number_of_threads: u32,
    working_set_private_size: i64,
    hard_fault_count: u32,
    number_of_threads_high_watermark: u32,
    cycle_time: u64,
    create_time: i64,
    user_time: i64,
    kernel_time: i64,
    image_name_length: u16,
    image_name_maximum_length: u16,
    image_name_buffer: *const u16,
    base_priority: i32,
    unique_process_id: usize,
}

#[cfg(windows)]
fn read_processes() -> Result<HashMap<u32, ProcessTimes>, String> {
    use std::ffi::c_void;
    #[link(name = "ntdll")]
    unsafe extern "system" {
        fn NtQuerySystemInformation(
            class: u32,
            output: *mut c_void,
            output_length: u32,
            returned: *mut u32,
        ) -> i32;
    }
    const SYSTEM_PROCESS_INFORMATION_CLASS: u32 = 5;
    const STATUS_INFO_LENGTH_MISMATCH: u32 = 0xc000_0004;
    const LIMIT_BYTES: usize = 256 << 20;

    // u64 elements keep the buffer 8-byte aligned for the native records.
    let mut buffer = vec![0_u64; (2 << 20) / 8];
    for _ in 0..6 {
        let capacity = buffer.len() * 8;
        let mut returned = 0_u32;
        let status = unsafe {
            NtQuerySystemInformation(
                SYSTEM_PROCESS_INFORMATION_CLASS,
                buffer.as_mut_ptr().cast(),
                u32::try_from(capacity).map_err(|_| "Process snapshot is too large.")?,
                &mut returned,
            )
        };
        if status as u32 == STATUS_INFO_LENGTH_MISMATCH {
            // Processes appear between calls; leave headroom over the hint.
            let wanted = (returned as usize).max(capacity).saturating_mul(3) / 2;
            if wanted > LIMIT_BYTES {
                return Err("Process snapshot exceeded 256 MB.".into());
            }
            buffer.resize(wanted.div_ceil(8), 0);
            continue;
        }
        if status < 0 {
            return Err(format!(
                "NtQuerySystemInformation(SystemProcessInformation) failed: 0x{:08x}",
                status as u32
            ));
        }
        let bytes = unsafe {
            std::slice::from_raw_parts(
                buffer.as_ptr().cast::<u8>(),
                (returned as usize).min(capacity),
            )
        };
        return parse(bytes);
    }
    Err("Process snapshot kept growing while it was read.".into())
}

#[cfg(windows)]
fn parse(bytes: &[u8]) -> Result<HashMap<u32, ProcessTimes>, String> {
    let record = std::mem::size_of::<SystemProcessInformation>();
    let mut processes = HashMap::new();
    let mut offset = 0_usize;
    loop {
        let end = offset
            .checked_add(record)
            .filter(|end| *end <= bytes.len())
            .ok_or("Process snapshot record is truncated.")?;
        // SAFETY: the range is in bounds; read_unaligned has no alignment need.
        let entry = unsafe {
            std::ptr::read_unaligned(
                bytes[offset..end]
                    .as_ptr()
                    .cast::<SystemProcessInformation>(),
            )
        };
        let pid = u32::try_from(entry.unique_process_id)
            .map_err(|_| "Process snapshot has an invalid PID.")?;
        // PID 0 is the Idle pseudo-process. Its "CPU" is idle time, and sysinfo
        // (no handle for PID 0) always reported 0 for it; keep that parity.
        if pid != 0 {
            processes.insert(
                pid,
                ProcessTimes {
                    created_at_100ns: entry.create_time.max(0) as u64,
                    cpu_100ns: (entry.user_time.max(0) as u64)
                        .saturating_add(entry.kernel_time.max(0) as u64),
                },
            );
        }
        if entry.next_entry_offset == 0 {
            return Ok(processes);
        }
        offset = offset
            .checked_add(entry.next_entry_offset as usize)
            .ok_or("Process snapshot offset overflowed.")?;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reading(system: u64, processes: &[(u32, u64, u64)]) -> Reading {
        Reading {
            system_cpu_100ns: system,
            processes: processes
                .iter()
                .map(|&(pid, created_at_100ns, cpu_100ns)| {
                    (
                        pid,
                        ProcessTimes {
                            created_at_100ns,
                            cpu_100ns,
                        },
                    )
                })
                .collect(),
        }
    }

    #[test]
    fn percent_matches_sysinfo_formula_divided_by_logical_cpus() {
        // sysinfo: 100 * (dproc / dsys) * cpus, then the sampler divided by cpus.
        let cpus = 24.0_f32;
        let sysinfo = 100.0 * (3_000_000_f32 / 240_000_000_f32) * cpus;
        assert!((percent(3_000_000, 240_000_000) - sysinfo / cpus).abs() < 1e-5);
        assert_eq!(percent(5, 0), 0.0);
        assert_eq!(percent(500, 100), 100.0);
    }

    #[test]
    fn tracker_reports_deltas_only_for_the_same_process_instance() {
        let mut tracker = Tracker::default();
        tracker.update(Some(reading(1_000, &[(10, 7, 100), (20, 8, 50)])));
        assert_eq!(tracker.percent(10), Some(0.0));
        tracker.update(Some(reading(
            2_000,
            &[(10, 7, 350), (20, 9, 60), (30, 1, 5)],
        )));
        assert_eq!(tracker.percent(10), Some(25.0));
        // PID 20 was reused by a new instance: no delta across instances.
        assert_eq!(tracker.percent(20), Some(0.0));
        assert_eq!(tracker.percent(30), Some(0.0));
        assert_eq!(tracker.percent(40), None);
        assert_eq!(tracker.times(10).unwrap().accumulated_millis(), 0);
        // A failed read never produces a delta across the gap.
        tracker.update(None);
        assert_eq!(tracker.percent(10), None);
        tracker.update(Some(reading(9_000, &[(10, 7, 9_000)])));
        assert_eq!(tracker.percent(10), Some(0.0));
    }

    #[test]
    fn start_time_matches_sysinfo_unix_seconds() {
        let times = ProcessTimes {
            created_at_100ns: 116_444_736_000_000_000 + 17_000_000_000_000_000,
            cpu_100ns: 25_000,
        };
        assert_eq!(times.started_at_unix(), Some(1_700_000_000));
        assert_eq!(times.accumulated_millis(), 2);
        assert_eq!(ProcessTimes::default().started_at_unix(), None);
    }

    #[cfg(windows)]
    #[test]
    fn native_snapshot_includes_this_process() {
        let reading = read().expect("process snapshot");
        let own = reading
            .processes
            .get(&std::process::id())
            .expect("own PID in snapshot");
        assert!(own.created_at_100ns > 0);
        assert!(reading.system_cpu_100ns > 0);
        assert!(reading.processes.len() > 3);
    }
}
