//! Read-only Windows performance-state distribution investigation.
//! ABI reference: System Informer/phnt d291af7dbb44c93a3a47e435956a4f8fa64f3fa6.
//! See docs/SYSTEM_INFORMER_NOTICE.txt. No driver, privileged write or PDH scaling.
use crate::diagnostics::{Health, Issue, State};
use std::collections::BTreeMap;
use std::time::{Duration, Instant};

pub const SOURCE: &str = "Windows performance-state deltas / per-processor nominal clocks";
pub const SEMANTICS: &str = "Hit-weighted frequency over the sampling interval, using each logical processor's nominal clock. Not an instantaneous clock or Task Manager's undocumented aggregate.";

#[derive(Clone, Debug, PartialEq)]
pub struct Processor {
    pub group: u16,
    pub number: u32,
    pub nominal_mhz: u32,
    pub mhz: Option<f64>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Values {
    pub average_mhz: f64,
    pub fastest_mhz: f64,
    pub slowest_mhz: f64,
    pub interval_seconds: f64,
    pub processors: Vec<Processor>,
}

impl Values {
    pub fn contributing(&self) -> usize {
        self.processors.iter().filter(|p| p.mhz.is_some()).count()
    }
}

#[derive(Default)]
pub struct Sampler {
    pub values: Option<Values>,
    pub health: Health,
    previous: Option<(Instant, Counters)>,
    retry_at: Option<Instant>,
}

impl Sampler {
    pub fn refresh(&mut self) {
        let at = Instant::now();
        if self.retry_at.is_some_and(|retry| at < retry) {
            return;
        }
        let result = query();
        if result.is_err() {
            // An unsupported/private API must not be hammered every second.
            self.retry_at = Some(at + Duration::from_secs(30));
        } else {
            self.retry_at = None;
        }
        self.accept(result, at, at.elapsed());
    }

    fn accept(&mut self, result: Result<Counters, Error>, at: Instant, elapsed: Duration) {
        let result = match result {
            Ok(current) => {
                let calculated =
                    self.previous
                        .as_ref()
                        .ok_or(Error::Baseline)
                        .and_then(|(then, old)| {
                            let interval = at.saturating_duration_since(*then);
                            if interval < Duration::from_millis(100)
                                || interval > Duration::from_secs(5)
                            {
                                return Err(Error::Baseline);
                            }
                            calculate(old, &current, interval)
                        });
                self.previous = Some((at, current));
                calculated
            }
            Err(error) => {
                self.previous = None; // Recovery needs two fresh samples, never bridge failure.
                Err(error)
            }
        };
        let (state, coverage, issue) = match result {
            Ok(values) => {
                let coverage = Some((values.contributing(), values.processors.len()));
                self.values = Some(values);
                (State::Live, coverage, None)
            }
            Err(Error::Baseline | Error::NoHits) => (
                if self.values.is_some() {
                    State::Stale
                } else {
                    State::Starting
                },
                None,
                None,
            ),
            Err(_) => (State::Unavailable, None, Some(Issue::CpuClock)),
        };
        self.health.record(at, elapsed, state, coverage, issue);
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Bucket {
    hits: u64,
    percent: u8,
}

#[derive(Clone, Debug)]
struct Counter {
    nominal_mhz: u32,
    buckets: Vec<Bucket>,
}

type Counters = BTreeMap<(u16, u32), Counter>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Error {
    Native(u32),
    Layout,
    Limit,
    Baseline,
    NoHits,
}

fn calculate(previous: &Counters, current: &Counters, interval: Duration) -> Result<Values, Error> {
    if previous.is_empty() || previous.len() != current.len() || current.len() > 4096 {
        return Err(Error::Baseline);
    }
    let mut total_hits = 0_u128;
    let mut total_weight = 0_u128;
    let mut processors = Vec::with_capacity(current.len());
    let mut fastest = 0.0_f64;
    let mut slowest = f64::INFINITY;
    for (&(group, number), value) in current {
        let old = previous.get(&(group, number)).ok_or(Error::Baseline)?;
        if value.nominal_mhz == 0
            || value.nominal_mhz > 100_000
            || old.nominal_mhz != value.nominal_mhz
            || old.buckets.len() != value.buckets.len()
            || value.buckets.is_empty()
            || value.buckets.len() > 256
        {
            return Err(Error::Baseline);
        }
        let mut hits = 0_u128;
        let mut weighted = 0_u128;
        for (a, b) in old.buckets.iter().zip(&value.buckets) {
            if a.percent != b.percent {
                return Err(Error::Baseline);
            }
            let delta = u128::from(b.hits.checked_sub(a.hits).ok_or(Error::Baseline)?);
            hits += delta;
            weighted += delta * u128::from(b.percent) * u128::from(value.nominal_mhz);
        }
        // u128 covers the bounded maximum (4096 processors * 256 buckets *
        // u64::MAX hits * 255 percent * 100000 MHz) without overflowing.
        let mhz = (hits > 0).then(|| weighted as f64 / hits as f64 / 100.0);
        if let Some(mhz) = mhz {
            fastest = fastest.max(mhz);
            slowest = slowest.min(mhz);
        }
        processors.push(Processor {
            group,
            number,
            nominal_mhz: value.nominal_mhz,
            mhz,
        });
        total_hits += hits;
        total_weight += weighted;
    }
    if total_hits == 0 {
        return Err(Error::NoHits);
    }
    Ok(Values {
        average_mhz: total_weight as f64 / total_hits as f64 / 100.0,
        fastest_mhz: fastest,
        slowest_mhz: slowest,
        interval_seconds: interval.as_secs_f64(),
        processors,
    })
}

// Parse bytes with checked slices, not untrusted offset-to-struct casts.
fn parse(data: &[u8]) -> Result<BTreeMap<u32, Vec<Bucket>>, Error> {
    let read32 = |offset: usize| -> Result<u32, Error> {
        Ok(u32::from_le_bytes(
            data.get(offset..offset + 4)
                .ok_or(Error::Layout)?
                .try_into()
                .unwrap(),
        ))
    };
    let count = read32(0)? as usize;
    if count == 0 || count > 64 {
        return Err(Error::Limit);
    }
    let header = 4 + count * 4;
    if data.len() < header {
        return Err(Error::Layout);
    }
    let mut result = BTreeMap::new();
    let mut regions = Vec::new();
    for i in 0..count {
        let offset = read32(4 + i * 4)? as usize;
        if offset < header || !offset.is_multiple_of(8) || offset > data.len().saturating_sub(8) {
            return Err(Error::Layout);
        }
        let states = read32(offset + 4)? as usize;
        if states == 0 || states > 256 {
            return Err(Error::Limit);
        }
        let end = offset + 8 + states * 16;
        if regions.iter().any(|&(a, b)| offset < b && a < end) {
            return Err(Error::Layout);
        }
        regions.push((offset, end));
        let bytes = data
            .get(offset + 8..offset + 8 + states * 16)
            .ok_or(Error::Layout)?;
        let buckets = bytes
            .chunks_exact(16)
            .map(|b| Bucket {
                hits: u64::from_le_bytes(b[..8].try_into().unwrap()),
                percent: b[8],
            })
            .collect();
        // Offset-table order is the group-local processor index (also used by
        // System Informer). On the reference Windows 11 box the embedded
        // ProcessorNumber repeats an unrelated index in many entries. Never
        // use that unreliable field to merge or assign processor histories.
        result.insert(i as u32, buckets);
    }
    Ok(result)
}

#[cfg(windows)]
fn query() -> Result<Counters, Error> {
    use std::ffi::c_void;
    use windows::Win32::System::Threading::{
        GetActiveProcessorCount, GetActiveProcessorGroupCount,
    };
    #[link(name = "ntdll")]
    unsafe extern "system" {
        fn NtQuerySystemInformationEx(
            class: u32,
            input: *const c_void,
            input_length: u32,
            output: *mut c_void,
            output_length: u32,
            returned: *mut u32,
        ) -> i32;
        fn NtPowerInformation(
            level: u32,
            input: *const c_void,
            input_length: u32,
            output: *mut c_void,
            output_length: u32,
        ) -> i32;
    }
    let groups = unsafe { GetActiveProcessorGroupCount() };
    if groups == 0 || groups > 64 {
        return Err(Error::Limit);
    }
    let mut result = Counters::new();
    for group in 0..groups {
        let expected = unsafe { GetActiveProcessorCount(group) };
        let mut buffer = vec![0_u64; 1024]; // 8-byte aligned, unlike Vec<u8>.
        let mut parsed = None;
        for _ in 0..4 {
            let capacity = (buffer.len() * 8) as u32;
            let mut returned = 0;
            // Information class 100 is query-only. Input selects a processor
            // group without changing affinity or touching the foreground app.
            let status = unsafe {
                NtQuerySystemInformationEx(
                    100,
                    (&group as *const u16).cast(),
                    2,
                    buffer.as_mut_ptr().cast(),
                    capacity,
                    &mut returned,
                )
            };
            if status as u32 == 0xc0000004 {
                // STATUS_INFO_LENGTH_MISMATCH
                if returned <= capacity || returned > 1_048_576 {
                    return Err(Error::Limit);
                }
                buffer.resize((returned as usize).div_ceil(8), 0);
                continue;
            }
            if status < 0 {
                return Err(Error::Native(status as u32));
            }
            if returned < 4 || returned > capacity {
                return Err(Error::Layout);
            }
            let bytes = unsafe {
                std::slice::from_raw_parts(buffer.as_ptr().cast::<u8>(), returned as usize)
            };
            parsed = Some(parse(bytes)?);
            break;
        }
        let parsed = parsed.ok_or(Error::Limit)?;
        if parsed.len() != expected as usize {
            return Err(Error::Layout);
        }
        for (number, buckets) in parsed {
            // POWER_INTERNAL_PROCESSOR_BRANDED_FREQUENCY_INPUT, version 1:
            // type=43; version=1; PROCESSOR_NUMBER={u16 group,u8 number,u8 reserved}.
            let input = [43_u32, 1, u32::from(group) | (number << 16)];
            let mut output = [1_u32, 0];
            let status = unsafe {
                NtPowerInformation(87, input.as_ptr().cast(), 12, output.as_mut_ptr().cast(), 8)
            };
            if status < 0 {
                return Err(Error::Native(status as u32));
            }
            if output[0] != 1 || !(1..=100_000).contains(&output[1]) {
                return Err(Error::Layout);
            }
            result.insert(
                (group, number),
                Counter {
                    nominal_mhz: output[1],
                    buckets,
                },
            );
        }
    }
    Ok(result)
}

#[cfg(not(windows))]
fn query() -> Result<Counters, Error> {
    Err(Error::Native(50))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagnostics::Provider;

    fn counters(step: u64) -> Counters {
        [
            (
                (0, 0),
                Counter {
                    nominal_mhz: 4000,
                    buckets: vec![Bucket {
                        hits: step * 100,
                        percent: 150,
                    }],
                },
            ),
            (
                (1, 0),
                Counter {
                    nominal_mhz: 3000,
                    buckets: vec![Bucket {
                        hits: step * 300,
                        percent: 100,
                    }],
                },
            ),
            (
                (1, 1),
                Counter {
                    nominal_mhz: 3000,
                    buckets: vec![Bucket {
                        hits: 0,
                        percent: 100,
                    }],
                },
            ),
        ]
        .into_iter()
        .collect()
    }

    #[test]
    fn cpu_clock_weights_interval_hits_and_each_nominal_not_cumulative_or_cpu_zero() {
        let v = calculate(&counters(1000), &counters(1001), Duration::from_secs(1)).unwrap();
        assert_eq!(v.average_mhz, 3750.0); // (100*6000 + 300*3000) / 400, not 4500 or 6000.
        assert_eq!(v.fastest_mhz, 6000.0);
        assert_eq!(v.slowest_mhz, 3000.0);
        assert_eq!(v.contributing(), 2);
        assert_eq!(v.processors[2].mhz, None); // no new hits does not mean zero MHz.
        assert_eq!(v.processors[1].group, 1); // same number in another group is distinct.
        let mut old = counters(1000);
        let mut new = counters(1001);
        for c in [&mut old, &mut new] {
            c.get_mut(&(0, 0)).unwrap().buckets.push(Bucket {
                hits: 999_999_999,
                percent: 20,
            });
        }
        assert_eq!(calculate(&old, &new, Duration::from_secs(1)).unwrap(), v);
    }

    #[test]
    fn cpu_clock_rejects_counter_reset_topology_nominal_and_bucket_changes() {
        let before = counters(1);
        let mut cases = Vec::new();
        cases.push(counters(0));
        let mut missing = counters(2);
        missing.remove(&(1, 1));
        cases.push(missing);
        let mut moved = counters(2);
        let v = moved.remove(&(1, 1)).unwrap();
        moved.insert((2, 1), v);
        cases.push(moved);
        let mut nominal = counters(2);
        nominal.get_mut(&(1, 0)).unwrap().nominal_mhz = 4000;
        cases.push(nominal);
        let mut percent = counters(2);
        percent.get_mut(&(0, 0)).unwrap().buckets[0].percent = 151;
        cases.push(percent);
        let mut buckets = counters(2);
        buckets.get_mut(&(0, 0)).unwrap().buckets.clear();
        cases.push(buckets);
        for case in cases {
            assert_eq!(
                calculate(&before, &case, Duration::from_secs(1)),
                Err(Error::Baseline)
            );
        }
        assert_eq!(
            calculate(&before, &before, Duration::from_secs(1)),
            Err(Error::NoHits)
        );
    }

    #[test]
    fn cpu_clock_large_counters_and_zero_percent_are_not_clamped_or_wrapped() {
        let old = [(
            (0, 0),
            Counter {
                nominal_mhz: 100_000,
                buckets: vec![Bucket {
                    hits: 0,
                    percent: 255,
                }],
            },
        )]
        .into_iter()
        .collect();
        let new = [(
            (0, 0),
            Counter {
                nominal_mhz: 100_000,
                buckets: vec![Bucket {
                    hits: u64::MAX,
                    percent: 255,
                }],
            },
        )]
        .into_iter()
        .collect();
        assert_eq!(
            calculate(&old, &new, Duration::from_secs(1))
                .unwrap()
                .average_mhz,
            255_000.0
        );
        let old = [(
            (0, 0),
            Counter {
                nominal_mhz: 4000,
                buckets: vec![Bucket {
                    hits: 0,
                    percent: 0,
                }],
            },
        )]
        .into_iter()
        .collect();
        let new = [(
            (0, 0),
            Counter {
                nominal_mhz: 4000,
                buckets: vec![Bucket {
                    hits: 1,
                    percent: 0,
                }],
            },
        )]
        .into_iter()
        .collect();
        assert_eq!(
            calculate(&old, &new, Duration::from_secs(1))
                .unwrap()
                .average_mhz,
            0.0
        );
    }

    #[test]
    fn cpu_clock_retains_failure_values_but_recovery_and_long_gaps_need_new_baseline() {
        let at = Instant::now();
        let mut s = Sampler::default();
        s.accept(Ok(counters(0)), at, Duration::ZERO);
        assert!(s.values.is_none());
        assert_eq!(s.health.state(Provider::CpuClock, at), State::Starting);
        s.accept(Ok(counters(1)), at + Duration::from_secs(1), Duration::ZERO);
        let value = s.values.clone();
        assert!(value.is_some());
        s.accept(
            Err(Error::Native(5)),
            at + Duration::from_secs(2),
            Duration::ZERO,
        );
        assert_eq!(s.values, value);
        assert_eq!(s.health.last_success, Some(at + Duration::from_secs(1)));
        assert_eq!(
            s.health
                .state(Provider::CpuClock, at + Duration::from_secs(2)),
            State::Stale
        );
        s.accept(Ok(counters(3)), at + Duration::from_secs(3), Duration::ZERO);
        assert_eq!(s.health.last_success, Some(at + Duration::from_secs(1)));
        s.accept(Ok(counters(4)), at + Duration::from_secs(4), Duration::ZERO);
        assert_eq!(s.health.last_success, Some(at + Duration::from_secs(4)));
        s.accept(
            Ok(counters(50)),
            at + Duration::from_secs(50),
            Duration::ZERO,
        );
        assert_eq!(
            s.health
                .state(Provider::CpuClock, at + Duration::from_secs(50)),
            State::Stale
        );
        s.accept(
            Ok(counters(51)),
            at + Duration::from_secs(51),
            Duration::ZERO,
        );
        assert_eq!(s.health.last_success, Some(at + Duration::from_secs(51)));
    }

    #[test]
    fn cpu_clock_native_parser_accepts_duplicate_embedded_numbers_but_not_overlapping_regions() {
        let mut data = vec![0_u8; 64];
        data[..4].copy_from_slice(&2_u32.to_le_bytes());
        data[4..8].copy_from_slice(&16_u32.to_le_bytes());
        data[8..12].copy_from_slice(&40_u32.to_le_bytes());
        for offset in [16, 40] {
            data[offset..offset + 4].copy_from_slice(&13_u32.to_le_bytes());
            data[offset + 4..offset + 8].copy_from_slice(&1_u32.to_le_bytes());
            data[offset + 8..offset + 16].copy_from_slice(&(offset as u64).to_le_bytes());
            data[offset + 16] = 255;
        }
        let parsed = parse(&data).unwrap();
        assert_eq!(parsed[&0][0].hits, 16);
        assert_eq!(parsed[&1][0].hits, 40);
        data[8..12].copy_from_slice(&16_u32.to_le_bytes());
        assert_eq!(parse(&data), Err(Error::Layout));
    }
    #[test]
    fn cpu_distribution_parser_checks_native_bounds_and_identity() {
        let mut data = vec![0_u8; 32];
        data[..4].copy_from_slice(&1_u32.to_le_bytes());
        data[4..8].copy_from_slice(&8_u32.to_le_bytes());
        data[8..12].copy_from_slice(&3_u32.to_le_bytes());
        data[12..16].copy_from_slice(&1_u32.to_le_bytes());
        data[16..24].copy_from_slice(&42_u64.to_le_bytes());
        data[24] = 140;
        assert_eq!(
            parse(&data).unwrap()[&0],
            vec![Bucket {
                hits: 42,
                percent: 140
            }]
        );
        for length in 0..data.len() {
            assert!(parse(&data[..length]).is_err(), "{length}");
        }
        for offset in [0, 4, 9, 32, u32::MAX] {
            let mut bad = data.clone();
            bad[4..8].copy_from_slice(&offset.to_le_bytes());
            assert!(parse(&bad).is_err());
        }
        for count in [0_u32, 65, u32::MAX] {
            let mut bad = data.clone();
            bad[..4].copy_from_slice(&count.to_le_bytes());
            assert!(parse(&bad).is_err());
        }
    }

    #[cfg(windows)]
    #[test]
    #[ignore = "Read-only Windows CPU distribution/nominal-clock probe; no UI, affinity changes or stress workload"]
    fn native_cpu_distribution_read_only_probe() {
        let mut sampler = Sampler::default();
        for sample in 0..4 {
            sampler.refresh();
            let values = sampler.values.as_ref();
            println!(
                "CPU_CLOCK sample={sample} query_ms={:?} avg_mhz={:?} fastest_mhz={:?} slowest_mhz={:?} contributors={:?}",
                sampler.health.query_millis,
                values.map(|v| v.average_mhz),
                values.map(|v| v.fastest_mhz),
                values.map(|v| v.slowest_mhz),
                values.map(|v| (v.contributing(), v.processors.len()))
            );
            if let Some(values) = values {
                for number in [0, 8] {
                    println!(
                        "CPU_CLOCK detail={:?}",
                        values
                            .processors
                            .iter()
                            .find(|p| p.group == 0 && p.number == number)
                    );
                }
            }
            if sample > 0 {
                assert!(values.is_some());
            }
            std::thread::sleep(std::time::Duration::from_secs(1));
        }
    }
}
