//! Windows memory-manager counters. One read on the existing sampler thread;
//! no approximation from physical RAM usage or page-file estimates.
use crate::diagnostics::{Health, Issue, State};
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Values {
    pub commit_bytes: u64,
    pub commit_limit_bytes: u64,
    pub commit_peak_bytes: u64,
    pub physical_total_bytes: u64,
    pub system_cache_bytes: u64,
    pub kernel_paged_bytes: u64,
    pub kernel_nonpaged_bytes: u64,
}

impl Values {
    pub fn pressure(self) -> Option<f32> {
        (self.commit_limit_bytes > 0)
            .then(|| (self.commit_bytes as f64 / self.commit_limit_bytes as f64 * 100.0) as f32)
    }
}

#[derive(Default)]
pub struct Sampler {
    pub values: Option<Values>,
    pub health: Health,
}

impl Sampler {
    pub fn refresh(&mut self) {
        let at = Instant::now();
        let result = query();
        self.accept(result, at, at.elapsed());
    }

    fn accept(&mut self, result: Result<Values, u32>, at: Instant, elapsed: Duration) {
        let success = result.is_ok();
        if let Ok(values) = result {
            self.values = Some(values);
        }
        self.health.record(
            at,
            elapsed,
            if success {
                State::Live
            } else {
                State::Unavailable
            },
            None,
            (!success).then_some(Issue::MemoryCounters),
        );
    }
}

#[cfg(windows)]
fn decode(p: &windows::Win32::System::ProcessStatus::PERFORMANCE_INFORMATION) -> Option<Values> {
    let page_size = u64::try_from(p.PageSize).ok().filter(|v| *v > 0)?;
    let bytes = |pages: usize| u64::try_from(pages).ok()?.checked_mul(page_size);
    Some(Values {
        commit_bytes: bytes(p.CommitTotal)?,
        commit_limit_bytes: bytes(p.CommitLimit)?,
        commit_peak_bytes: bytes(p.CommitPeak)?,
        physical_total_bytes: bytes(p.PhysicalTotal)?,
        system_cache_bytes: bytes(p.SystemCache)?,
        kernel_paged_bytes: bytes(p.KernelPaged)?,
        kernel_nonpaged_bytes: bytes(p.KernelNonpaged)?,
    })
}

#[cfg(windows)]
fn query() -> Result<Values, u32> {
    use windows::Win32::System::ProcessStatus::{K32GetPerformanceInfo, PERFORMANCE_INFORMATION};
    let size = std::mem::size_of::<PERFORMANCE_INFORMATION>() as u32;
    let mut info = PERFORMANCE_INFORMATION {
        cb: size,
        ..Default::default()
    };
    // The structure is correctly sized and remains live for this synchronous call.
    if !unsafe { K32GetPerformanceInfo(&mut info, size) }.as_bool() {
        return Err(unsafe { windows::Win32::Foundation::GetLastError() }.0);
    }
    decode(&info).ok_or(13) // ERROR_INVALID_DATA, never wrap overflowing page counts.
}

#[cfg(not(windows))]
fn query() -> Result<Values, u32> {
    Err(50)
} // ERROR_NOT_SUPPORTED

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagnostics::Provider;

    #[test]
    fn memory_counters_keep_failure_gaps_and_recover_without_invented_zero() {
        let mut s = Sampler::default();
        let at = Instant::now();
        s.accept(Err(5), at, Duration::ZERO);
        assert!(s.values.is_none());
        assert_eq!(
            s.health.state(Provider::MemoryCounters, at),
            State::Unavailable
        );
        let values = Values {
            commit_bytes: 31 << 30,
            commit_limit_bytes: 80 << 30,
            ..Default::default()
        };
        s.accept(Ok(values), at, Duration::ZERO);
        let later = at + Duration::from_secs(1);
        s.accept(Err(5), later, Duration::ZERO);
        assert_eq!(s.values, Some(values));
        assert_eq!(s.health.last_success, Some(at));
        assert_eq!(s.health.last_attempt, Some(later));
        assert_eq!(
            s.health.state(Provider::MemoryCounters, later),
            State::Stale
        );
        s.accept(Ok(Values::default()), later, Duration::ZERO);
        assert_eq!(s.values.unwrap().commit_bytes, 0);
        assert_eq!(s.values.unwrap().pressure(), None);
        assert_eq!(s.health.state(Provider::MemoryCounters, later), State::Live);
        assert_eq!(
            s.health
                .state(Provider::MemoryCounters, later + Duration::from_secs(4)),
            State::Stale
        );
    }

    #[cfg(windows)]
    #[test]
    fn memory_counters_decode_pages_without_ram_plus_swap_estimate() {
        use windows::Win32::System::ProcessStatus::PERFORMANCE_INFORMATION;
        let mut raw = PERFORMANCE_INFORMATION {
            PageSize: 4096,
            CommitTotal: 3_000_000,
            CommitLimit: 20_000_000,
            CommitPeak: 4_000_000,
            PhysicalTotal: 16_000_000,
            SystemCache: 42,
            KernelPaged: 12,
            KernelNonpaged: 0,
            ..Default::default()
        };
        let v = decode(&raw).unwrap();
        assert_eq!(v.commit_bytes, 12_288_000_000);
        assert_eq!(v.commit_limit_bytes, 81_920_000_000);
        assert_eq!(v.commit_peak_bytes, 16_384_000_000);
        assert_eq!(v.system_cache_bytes, 42 * 4096);
        assert_eq!(v.kernel_paged_bytes, 12 * 4096);
        assert_eq!(v.kernel_nonpaged_bytes, 0);
        assert_eq!(v.pressure(), Some(15.0));
        raw.PageSize = 0;
        assert!(decode(&raw).is_none());
        raw.PageSize = usize::MAX;
        assert!(decode(&raw).is_none());
    }

    #[cfg(windows)]
    #[test]
    #[ignore = "read-only Windows memory-counter probe; no windows, processes or preferences changed"]
    fn native_memory_counters_read_only_probe() {
        let mut durations = Vec::new();
        let mut last = Values::default();
        for _ in 0..20 {
            let at = Instant::now();
            last = query().expect("Windows memory counters unavailable");
            durations.push(at.elapsed().as_secs_f64() * 1_000_000.0);
            assert!(last.commit_limit_bytes > 0 && last.physical_total_bytes > 0);
            assert!(last.commit_peak_bytes >= last.commit_bytes);
        }
        durations.sort_by(f64::total_cmp);
        println!(
            "Windows memory: {last:?}; 20 reads, median {:.1} us, max {:.1} us; not a UI/soak benchmark",
            durations[10], durations[19]
        );
    }
}
