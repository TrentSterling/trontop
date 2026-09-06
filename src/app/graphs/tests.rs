use super::*;
use crate::diagnostics::{Provider, State};
use history::Id;

#[test]
fn gpu_adapter_histories_do_not_mix_devices_repeat_cached_values_or_lose_partial_gaps() {
    use crate::gpu_activity::Usage;
    use crate::gpu_adapters::{Adapter, Engine, Key};
    let now = Instant::now();
    let mut s = sample(now);
    s.gpu.adapters = (1..=2)
        .map(|low| {
            let mut a = Adapter {
                key: Key {
                    low,
                    ..Default::default()
                },
                sampled_at: Some(now),
                activity: Usage::Measured(low as f32),
                engines: vec![Engine {
                    number: u32::MAX,
                    kind: "3D".into(),
                    usage: Usage::Measured(low as f32),
                }],
                ..Default::default()
            };
            a.memory[0].record(Some(u64::from(low) * 1_073_741_824), now);
            a
        })
        .collect();
    let mut h = History::default();
    h.sample(&s, now);
    let key = s.gpu.adapters[0].key;
    let memory = Id::Adapter(key, 0);
    let engine = Id::Adapter(key, u64::from(u32::MAX) + 4);
    assert_eq!(h.chart(&memory).unwrap().current, Some(1.));
    assert_eq!(
        h.chart(&Id::Adapter(s.gpu.adapters[1].key, 0))
            .unwrap()
            .current,
        Some(2.)
    );
    assert_eq!(h.chart(&engine).unwrap().current, Some(1.));
    s.gpu.adapters.reverse();
    let later = now + Duration::from_secs(1);
    for a in &mut s.gpu.adapters {
        a.memory[0].record(None, later);
        a.sampled_at = Some(later);
        a.engines[0].usage = Usage::Partial(5.);
    }
    h.sample(&s, later);
    assert_eq!(h.chart(&memory).unwrap().current, Some(1.));
    assert_eq!(h.chart(&memory).unwrap().state(later), "Cached");
    assert_eq!(h.chart(&memory).unwrap().points.back().unwrap().value, None);
    assert_eq!(h.chart(&engine).unwrap().points.back().unwrap().value, None);
    assert!(h.chart(&engine).unwrap().value_label().starts_with(">="));
    h.sample(&s, later);
    assert_eq!(h.chart(&memory).unwrap().points.len(), 2);
}

#[test]
fn cpu_clock_graphs_keep_provider_timestamps_and_failure_gaps() {
    let now = Instant::now();
    let mut s = sample(now);
    s.cpu.frequency_mhz = 3700;
    let mut history = History::default();
    history.sample(&s, now);
    assert!(history.chart(&Id::CpuClock(0)).unwrap().current.is_none());
    s.cpu.clocks = Some(crate::cpu_clock::Values {
        average_mhz: 5125.0,
        fastest_mhz: 5400.0,
        slowest_mhz: 4300.0,
        interval_seconds: 1.0,
        processors: vec![],
    });
    s.diagnostics
        .get_mut(Provider::CpuClock)
        .record(now, Duration::ZERO, State::Live, None, None);
    history.sample(&s, now);
    assert_eq!(
        history.chart(&Id::CpuClock(0)).unwrap().current,
        Some(5125.0)
    );
    assert_eq!(
        history.chart(&Id::CpuClock(1)).unwrap().current,
        Some(5400.0)
    );
    let later = now + Duration::from_secs(1);
    s.diagnostics.get_mut(Provider::CpuClock).record(
        later,
        Duration::ZERO,
        State::Unavailable,
        None,
        None,
    );
    history.sample(&s, later);
    for id in [Id::CpuClock(0), Id::CpuClock(1)] {
        let c = history.chart(&id).unwrap();
        assert_eq!(c.state(later), "Cached");
        assert_eq!(c.points.len(), 2);
        assert_eq!(c.points.back().unwrap().value, None);
        assert_eq!(c.measured_at, Some(now));
    }
    // Provider backoff cannot produce duplicated points on UI repaints.
    history.sample(&s, later + Duration::from_secs(1));
    assert_eq!(history.chart(&Id::CpuClock(0)).unwrap().points.len(), 2);
    s.diagnostics.get_mut(Provider::CpuClock).record(
        later + Duration::from_secs(2),
        Duration::ZERO,
        State::Live,
        None,
        None,
    );
    s.cpu.clocks.as_mut().unwrap().average_mhz = 4900.0;
    history.sample(&s, later + Duration::from_secs(2));
    assert_eq!(
        history
            .chart(&Id::CpuClock(0))
            .unwrap()
            .points
            .back()
            .unwrap()
            .value,
        Some(4900.0)
    );
}

#[test]
fn logical_cpu_histories_are_distinct_and_missing_samples_leave_gaps() {
    let now = Instant::now();
    let mut s = sample(now);
    s.cpu.logical_cores = 4;
    s.cpu.logical_usage = vec![Some(3.0), Some(81.0), Some(0.0), None];
    let mut history = History::default();
    history.sample(&s, now);
    for (id, value) in [(0, Some(3.0)), (1, Some(81.0)), (2, Some(0.0)), (3, None)] {
        let c = history.charts.iter().find(|c| c.id == Id::Cpu(id)).unwrap();
        assert_eq!(c.current, value);
        assert_eq!(c.points[0].value, value);
        assert_eq!(c.range(now), (0.0, 100.0));
    }
    let later = now + Duration::from_secs(1);
    s.diagnostics
        .get_mut(Provider::System)
        .record(later, Duration::ZERO, State::Live, None, None);
    s.cpu.logical_usage = vec![Some(f32::NAN), Some(101.0), None, Some(57.0)];
    history.sample(&s, later);
    for (id, retained) in [(0, 3.0), (1, 81.0), (2, 0.0)] {
        let c = history.charts.iter().find(|c| c.id == Id::Cpu(id)).unwrap();
        assert_eq!(c.current, Some(retained));
        assert_eq!(c.points.back().unwrap().value, None);
        assert_eq!(c.state(later), "Cached");
    }
    let c = history.charts.iter().find(|c| c.id == Id::Cpu(3)).unwrap();
    assert_eq!(c.points.back().unwrap().value, Some(57.0));
    assert_eq!(c.state(later), "Live");
}

#[test]
fn cpu_grid_history_reserves_budget_for_other_hardware() {
    let now = Instant::now();
    let mut s = sample(now);
    s.cpu.logical_cores = 1024;
    s.cpu.logical_usage = vec![Some(12.0); 1024];
    let mut history = History::default();
    history.sample(&s, now);
    assert_eq!(
        history
            .charts
            .iter()
            .filter(|c| matches!(c.id, Id::Cpu(_)))
            .count(),
        256
    );
    assert!(
        history
            .charts
            .iter()
            .any(|c| c.id == Id::Gpu("test-gpu".into(), 0))
    );
    assert!(history.charts.len() <= 512);
}

fn sample(at: Instant) -> SystemSnapshot {
    let mut s = SystemSnapshot {
        memory_total_bytes: 64 << 30,
        memory_used_bytes: 32 << 30,
        cpu_percent: 24.0,
        ..Default::default()
    };
    s.diagnostics
        .get_mut(Provider::System)
        .record(at, Duration::ZERO, State::Live, None, None);
    s.diagnostics.get_mut(Provider::GpuActivity).record(
        at,
        Duration::ZERO,
        State::Live,
        None,
        None,
    );
    s.gpu = crate::model::GpuSnapshot {
        available: true,
        total_counters: 2,
        valid_counters: 2,
        utilization_percent: 20.0,
        ..Default::default()
    };
    s.gpu_sensors.last_success = Some(at);
    s.gpu_sensors.sampled_at = Some(at);
    s.gpu_sensors
        .adapters
        .push(crate::gpu_sensors::AdapterSensors {
            uuid: Some("test-gpu".into()),
            name: "test adapter".into(),
            temperature_c: Some(40),
            power_w: Some(70.0),
            fan_percent: Some(0),
            ..Default::default()
        });
    s
}

#[test]
fn timestamps_deduplicate_cached_values_and_leave_real_gaps() {
    let now = Instant::now();
    let mut history = History::default();
    let mut s = sample(now);
    history.sample(&s, now);
    history.sample(&s, now + Duration::from_secs(1));
    s.gpu_sensors.using_cached = true;
    s.gpu_sensors.sampled_at = Some(now + Duration::from_secs(2));
    history.sample(&s, now + Duration::from_secs(2));
    let chart = history
        .charts
        .iter()
        .find(|c| c.id == Id::Gpu("test-gpu".into(), 0))
        .unwrap();
    assert_eq!(chart.points.len(), 2);
    assert_eq!(chart.points[0].value, Some(40.0));
    assert_eq!(chart.points[1].value, None);
    assert_eq!(chart.current, Some(40.0));
    assert_eq!(chart.measured_at, Some(now));
    assert_eq!(chart.state(now + Duration::from_secs(2)), "Cached");
}

#[test]
fn missing_gpu_value_retains_readout_but_never_plots_cached_value() {
    let now = Instant::now();
    let mut history = History::default();
    history.sample(&sample(now), now);
    let later = now + Duration::from_secs(1);
    let mut s = sample(later);
    s.gpu_sensors.adapters[0].temperature_c = None;
    history.sample(&s, later);
    let chart = history
        .charts
        .iter()
        .find(|c| c.id == Id::Gpu("test-gpu".into(), 0))
        .unwrap();
    assert_eq!(chart.current, Some(40.0));
    assert_eq!(chart.points.back().unwrap().value, None);
    assert_eq!(chart.state(later), "Cached");
}

#[test]
fn partial_activity_is_labelled_lower_bound_and_not_graphed_as_exact() {
    let now = Instant::now();
    let mut s = sample(now);
    s.gpu.valid_counters = 1;
    let mut history = History::default();
    history.sample(&s, now);
    let chart = history
        .charts
        .iter()
        .find(|c| c.id == Id::Activity("GPU activity".into()))
        .unwrap();
    assert_eq!(chart.value_label(), ">=20.0%");
    assert_eq!(chart.points[0].value, None);
}

#[test]
fn gpu_uuid_prevents_reordered_devices_from_merging_and_missing_uuid_has_no_history() {
    let now = Instant::now();
    let mut s = sample(now);
    let mut second = s.gpu_sensors.adapters[0].clone();
    second.uuid = Some("other-gpu".into());
    second.temperature_c = Some(75);
    s.gpu_sensors.adapters.push(second);
    let mut history = History::default();
    history.sample(&s, now);
    s.gpu_sensors.adapters.reverse();
    s.gpu_sensors.sampled_at = Some(now + Duration::from_secs(1));
    history.sample(&s, now + Duration::from_secs(1));
    for chart in &history.charts {
        if let Id::Gpu(id, 0) = &chart.id {
            assert!(
                chart
                    .points
                    .iter()
                    .all(|p| p.value == Some(if id == "test-gpu" { 40.0 } else { 75.0 }))
            );
        }
    }
    s.gpu_sensors.adapters[0].uuid = None;
    history.sample(&s, now + Duration::from_secs(2));
    assert!(
        history
            .charts
            .iter()
            .filter(|c| matches!(&c.id, Id::Gpu(id, _) if id.starts_with("unidentified:")))
            .all(|c| c.points.is_empty())
    );
}

#[test]
fn drive_zero_and_negative_temperatures_are_valid_and_sampler_cadence_is_preserved() {
    let now = Instant::now();
    let mut s = sample(now);
    s.storage_sensors = std::sync::Arc::new(crate::storage_sensors::Snapshot {
        drives: vec![crate::storage_sensors::DriveReading {
            device: crate::storage_sensors::Device {
                id: "test-drive".into(),
                name: "test".into(),
            },
            temperatures: crate::storage_sensors::Temperatures {
                sensors: vec![crate::storage_sensors::Temperature {
                    index: 0,
                    celsius: Some(-5),
                    over_threshold: None,
                    under_threshold: None,
                    event: false,
                }],
                ..Default::default()
            },
            last_attempt: Some(now),
            last_success: Some(now),
            present: true,
            error: None,
            query_millis: None,
        }],
        ..Default::default()
    });
    let mut history = History::default();
    for seconds in 0..5 {
        history.sample(&s, now + Duration::from_secs(seconds));
    }
    let chart = history
        .charts
        .iter()
        .find(|c| matches!(c.id, Id::Temperature(_, 0)))
        .unwrap();
    assert_eq!(chart.points.len(), 1);
    assert_eq!(chart.range(now).0, -5.0);
    assert_eq!(chart.cadence, Duration::from_secs(5));
    let fan = history
        .charts
        .iter()
        .find(|c| c.id == Id::Gpu("test-gpu".into(), 4))
        .unwrap();
    assert_eq!(fan.points[0].value, Some(0.0));
}

#[test]
fn history_is_bounded_and_removed_devices_expire() {
    let now = Instant::now();
    let mut history = History::default();
    for index in 0..300 {
        let at = now + Duration::from_secs(index);
        history.sample(&sample(at), at);
    }
    assert!(history.charts.iter().all(|c| c.points.len() <= 121));
    history.sample(&SystemSnapshot::default(), now + Duration::from_secs(421));
    assert!(!history.charts.iter().any(|c| matches!(c.id, Id::Gpu(_, _))));
    let mut s = sample(now + Duration::from_secs(422));
    s.networks = (0..600)
        .map(|i| crate::model::NetworkRow {
            name: format!("fixture {i}"),
            ..Default::default()
        })
        .collect();
    history.sample(&s, now + Duration::from_secs(422));
    assert_eq!(history.charts.len(), 512);
    assert!(history.omitted > 0);
}

#[test]
fn empty_snapshot_never_becomes_zero_cpu_and_responsive_columns_are_bounded() {
    let now = Instant::now();
    let mut history = History::default();
    history.sample(&SystemSnapshot::default(), now);
    assert!(
        history
            .charts
            .iter()
            .all(|c| c.current.is_none() && c.points.is_empty())
    );
    for (width, expected) in [(180.0, 1), (600.0, 2), (900.0, 3), (1600.0, 4)] {
        assert_eq!(columns(width), expected);
    }
}

#[test]
fn memory_graphs_use_commit_counters_and_preserve_missing_intervals() {
    let now = Instant::now();
    let mut s = sample(now);
    s.memory_details = Some(crate::memory_metrics::Values {
        commit_bytes: 24 << 30,
        commit_limit_bytes: 80 << 30,
        kernel_nonpaged_bytes: 0,
        ..Default::default()
    });
    s.diagnostics.get_mut(Provider::MemoryCounters).record(
        now,
        Duration::ZERO,
        State::Live,
        None,
        None,
    );
    let mut history = History::default();
    history.sample(&s, now);
    let get = |history: &History, id| {
        history
            .charts
            .iter()
            .find(|c| c.id == Id::Memory(id))
            .unwrap()
            .current
    };
    // Physical usage is 32 GiB; commit is independently 24 GiB, never their sum.
    assert_eq!(get(&history, 0), Some(24.0));
    assert_eq!(get(&history, 1), Some(30.0));
    assert_eq!(get(&history, 4), Some(0.0));
    history.sample(&s, now + Duration::from_millis(500));
    let later = now + Duration::from_secs(1);
    s.diagnostics.get_mut(Provider::MemoryCounters).record(
        later,
        Duration::ZERO,
        State::Unavailable,
        None,
        None,
    );
    history.sample(&s, later);
    for c in history
        .charts
        .iter()
        .filter(|c| matches!(c.id, Id::Memory(_)))
    {
        assert_eq!(c.points.len(), 2);
        assert_eq!(c.points.back().unwrap().value, None);
        assert_eq!(c.state(later), "Cached");
        assert_eq!(c.measured_at, Some(now));
    }
    assert_eq!(get(&history, 0), Some(24.0));
    let recovery = now + Duration::from_secs(2);
    s.memory_details.as_mut().unwrap().commit_bytes = 16 << 30;
    s.diagnostics.get_mut(Provider::MemoryCounters).record(
        recovery,
        Duration::ZERO,
        State::Live,
        None,
        None,
    );
    history.sample(&s, recovery);
    let c = history
        .charts
        .iter()
        .find(|c| c.id == Id::Memory(0))
        .unwrap();
    assert_eq!(c.points.back().unwrap().value, Some(16.0));
    assert_eq!(c.state(recovery), "Live");
}
