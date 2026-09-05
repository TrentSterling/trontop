//! Optional, read-only GPU sensors. No vendor DLL is linked or shipped with Trontop.
use std::collections::VecDeque;
use std::time::{Duration, Instant};

#[cfg(windows)]
mod nvml;

#[derive(Clone, Debug, Default)]
pub struct AdapterSensors {
    pub name: String,
    // Without a stable identity we show readings, but do not merge their history.
    pub uuid: Option<String>,
    pub temperature_c: Option<u32>,
    pub power_w: Option<f32>,
    pub graphics_clock_mhz: Option<u32>,
    pub memory_clock_mhz: Option<u32>,
    /// NVML's intended fan speed, not a physical tachometer measurement.
    pub fan_percent: Option<u32>,
    /// Used and total VRAM bytes. v2 excludes reservations from used memory.
    pub memory: Option<(u64, u64)>,
    pub memory_includes_reserved: bool,
    pub error: Option<String>,
}

#[derive(Clone, Debug, Default)]
pub struct SensorSnapshot {
    pub last_success: Option<Instant>,
    pub using_cached: bool,
    pub sampled_at: Option<Instant>,
    pub attempted_at: Option<Instant>,
    pub adapters: Vec<AdapterSensors>,
    pub error: Option<String>,
    pub query_millis: f64,
}

#[derive(Default)]
pub struct SensorSampler {
    last_good: Vec<AdapterSensors>,
    last_success: Option<Instant>,
    #[cfg(windows)]
    provider: Option<nvml::Nvml>,
    retry_at: Option<Instant>,
    last_error: Option<String>,
    last_attempt: Option<Instant>,
}

impl SensorSampler {
    pub fn sample(&mut self) -> SensorSnapshot {
        let started = Instant::now();
        if self.retry_at.is_none_or(|deadline| started >= deadline) {
            self.last_attempt = Some(started);
        }
        let result = self.collect(started);
        self.finish_sample(started, result)
    }

    fn finish_sample(
        &mut self,
        started: Instant,
        result: Result<Vec<AdapterSensors>, String>,
    ) -> SensorSnapshot {
        let (adapters, error) = match result {
            Ok(adapters) => {
                self.last_good = adapters.clone();
                self.last_success = Some(started);
                (adapters, None)
            }
            Err(error) => (self.last_good.clone(), Some(error)),
        };
        SensorSnapshot {
            last_success: self.last_success,
            using_cached: error.is_some() && !adapters.is_empty(),
            sampled_at: Some(started),
            attempted_at: self.last_attempt,
            adapters,
            error,
            query_millis: started.elapsed().as_secs_f64() * 1000.0,
        }
    }

    fn collect(&mut self, now: Instant) -> Result<Vec<AdapterSensors>, String> {
        if self.retry_at.is_some_and(|deadline| now < deadline) {
            return Err(self.last_error.clone().unwrap_or_default());
        }
        #[cfg(windows)]
        let result = (|| {
            if self.provider.is_none() {
                self.provider = Some(nvml::Nvml::load()?);
            }
            self.provider.as_ref().unwrap().sample()
        })();
        #[cfg(not(windows))]
        let result = Err("GPU sensors currently require Windows and an NVIDIA driver.".into());
        if let Err(error) = &result {
            #[cfg(windows)]
            {
                self.provider = None;
            }
            self.last_error = Some(error.clone());
            self.retry_at = Some(now + Duration::from_secs(30));
        } else {
            self.retry_at = None;
            self.last_error = None;
        }
        result
    }
}

#[derive(Clone, Copy)]
pub struct SensorPoint {
    pub at: Instant,
    pub temperature_c: Option<f32>,
    pub power_w: Option<f32>,
}

#[derive(Default)]
pub struct SensorHistory {
    pub points: VecDeque<SensorPoint>,
}

impl SensorHistory {
    pub fn push(&mut self, at: Instant, adapter: Option<&AdapterSensors>) {
        if self.points.back().is_some_and(|last| last.at >= at) {
            return;
        }
        self.points.push_back(SensorPoint {
            at,
            temperature_c: adapter.and_then(|a| a.temperature_c).map(|v| v as f32),
            power_w: adapter.and_then(|a| a.power_w),
        });
        while self.points.len() > 120
            || self
                .points
                .front()
                .is_some_and(|p| at.duration_since(p.at).as_secs_f64() > 120.0)
        {
            self.points.pop_front();
        }
    }

    pub fn peak(&self, power: bool) -> Option<f32> {
        self.points
            .iter()
            .filter_map(|p| if power { p.power_w } else { p.temperature_c })
            .reduce(f32::max)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failed_refresh_keeps_last_readings_explicitly_cached_and_recovers() {
        let now = Instant::now();
        let mut sampler = SensorSampler::default();
        let row = AdapterSensors {
            name: "Fixture GPU".into(),
            temperature_c: Some(54),
            power_w: Some(0.0),
            ..Default::default()
        };
        let first = sampler.finish_sample(now, Ok(vec![row.clone()]));
        assert!(!first.using_cached);
        let failed = sampler.finish_sample(now, Err("Fixture driver loss".into()));
        assert!(failed.using_cached);
        assert_eq!(failed.adapters[0].temperature_c, Some(54));
        assert_eq!(failed.last_success, first.last_success);
        let recovered = sampler.finish_sample(now, Ok(vec![row]));
        assert!(!recovered.using_cached);
        assert!(recovered.error.is_none());
    }

    #[test]
    fn history_preserves_unavailable_and_zero_and_bounds_time() {
        let start = Instant::now();
        let mut history = SensorHistory::default();
        let adapter = AdapterSensors {
            temperature_c: Some(50),
            power_w: Some(0.0),
            ..Default::default()
        };
        history.push(start, Some(&adapter));
        history.push(start, None); // duplicate snapshot must not erase a valid reading
        history.push(start + Duration::from_secs(1), None);
        assert_eq!(history.points.len(), 2);
        assert_eq!(history.points[0].power_w, Some(0.0));
        assert_eq!(history.points[1].power_w, None);
        assert_eq!(history.peak(false), Some(50.0));
        for second in 2..300 {
            history.push(start + Duration::from_secs(second), None);
        }
        assert_eq!(history.points.len(), 120);
        assert_eq!(history.peak(false), None);
        history.push(start + Duration::from_secs(500), Some(&adapter));
        assert_eq!(history.points.len(), 1);
    }

    #[test]
    fn unavailable_provider_retries_are_throttled_without_retaining_readings() {
        let mut sampler = SensorSampler {
            retry_at: Some(Instant::now() + Duration::from_secs(30)),
            last_error: Some("Fixture: driver missing".into()),
            ..Default::default()
        };
        let snapshot = sampler.sample();
        assert!(snapshot.adapters.is_empty());
        assert_eq!(snapshot.error.as_deref(), Some("Fixture: driver missing"));
        assert!(snapshot.sampled_at.is_some());
    }
}
