//! Read-only DXGI inventory and whole-adapter PDH memory. Runs on the sampler worker.
use super::*;
use windows::Win32::Graphics::Dxgi::{
    CreateDXGIFactory1, DXGI_ADAPTER_FLAG_SOFTWARE, DXGI_ERROR_NOT_FOUND, IDXGIFactory1,
};
use windows::Win32::System::Performance::*;
use windows::core::PCWSTR;

const MAX_ADAPTERS: usize = 64;
const MAX_BUFFER: usize = 1_048_576;
const RETRY: Duration = Duration::from_secs(30);
const RETAIN: Duration = Duration::from_secs(120);

struct Query(PDH_HQUERY, [PDH_HCOUNTER; 3]);
impl Drop for Query {
    fn drop(&mut self) {
        unsafe {
            let _ = PdhCloseQuery(self.0);
        }
    }
}
impl Query {
    fn open() -> Result<Self, String> {
        let mut query = PDH_HQUERY::default();
        let status = unsafe { PdhOpenQueryW(PCWSTR::null(), 0, &mut query) };
        if status != 0 {
            return Err(format!("GPU memory query: 0x{status:08X}"));
        }
        let mut result = Self(query, [PDH_HCOUNTER::default(); 3]);
        for (i, name) in ["Dedicated Usage", "Shared Usage", "Total Committed"]
            .iter()
            .enumerate()
        {
            let path: Vec<_> = format!(r"\GPU Adapter Memory(*)\{name}")
                .encode_utf16()
                .chain(Some(0))
                .collect();
            let status =
                unsafe { PdhAddEnglishCounterW(query, PCWSTR(path.as_ptr()), 0, &mut result.1[i]) };
            if status != 0 {
                return Err(format!("GPU memory counter {name}: 0x{status:08X}"));
            }
        }
        Ok(result)
    }
}

#[derive(Default)]
pub struct Sampler {
    query: Option<Query>,
    last_retry: Option<Instant>,
    descriptions: BTreeMap<(u32, u32), Description>,
    inventory_live: bool,
    rows: BTreeMap<Key, Adapter>,
    error: Option<String>,
}

impl Sampler {
    pub fn sample(&mut self, engines: Vec<Adapter>, now: Instant) -> Vec<Adapter> {
        if self
            .last_retry
            .is_none_or(|at| now.saturating_duration_since(at) >= RETRY)
        {
            self.last_retry = Some(now);
            match inventory() {
                Ok(descriptions) => {
                    self.descriptions = descriptions;
                    self.inventory_live = true;
                }
                Err(error) => {
                    self.inventory_live = false;
                    self.error = Some(error);
                }
            }
            if self.query.is_none() {
                match Query::open() {
                    Ok(query) => self.query = Some(query),
                    Err(error) => self.error = Some(error),
                }
            }
        }
        // Retain identities/values through temporary failures, but never relabel them live.
        self.rows.retain(|_, row| {
            row.last_seen
                .is_some_and(|at| now.saturating_duration_since(at) <= RETAIN)
        });
        for row in self.rows.values_mut() {
            row.activity = Usage::Unavailable;
            row.sampled_at = Some(now);
            for engine in &mut row.engines {
                engine.usage = Usage::Unavailable;
            }
            for value in &mut row.memory {
                value.record(None, now);
            }
        }
        for (&(high, low), description) in &self.descriptions {
            let key = Key {
                high,
                low,
                physical: 0,
            };
            if self.rows.len() < MAX_ADAPTERS || self.rows.contains_key(&key) {
                let row = self.rows.entry(key).or_insert_with(|| Adapter {
                    key,
                    ..Default::default()
                });
                row.description = Some(description.clone());
                row.description_current = self.inventory_live;
                if self.inventory_live {
                    row.last_seen = Some(now);
                }
            }
        }
        for fresh in engines {
            if self.rows.len() >= MAX_ADAPTERS && !self.rows.contains_key(&fresh.key) {
                continue;
            }
            let row = self.rows.entry(fresh.key).or_insert_with(|| Adapter {
                key: fresh.key,
                ..Default::default()
            });
            row.activity = fresh.activity;
            row.sampled_at = Some(now);
            row.last_seen = Some(now);
            for engine in fresh.engines {
                if let Some(old) = row.engines.iter_mut().find(|e| e.number == engine.number) {
                    *old = engine;
                } else if row.engines.len() < 256 {
                    row.engines.push(engine);
                }
            }
            row.engines.sort_by_key(|e| e.number);
        }
        if let Some(query) = &self.query {
            let status = unsafe { PdhCollectQueryData(query.0) };
            self.error = None;
            if status != 0 {
                self.error = Some(format!("GPU memory collection: 0x{status:08X}"));
            } else {
                for (metric, handle) in query.1.iter().enumerate() {
                    match array(*handle) {
                        Ok(values) => {
                            for (key, value) in values {
                                if self.rows.len() >= MAX_ADAPTERS && !self.rows.contains_key(&key)
                                {
                                    continue;
                                }
                                let row = self.rows.entry(key).or_insert_with(|| Adapter {
                                    key,
                                    ..Default::default()
                                });
                                row.last_seen = Some(now);
                                row.memory[metric].record(value, now);
                            }
                        }
                        Err(error) => self.error = Some(error),
                    }
                }
            }
        }
        for row in self.rows.values_mut() {
            // A linked-node capacity is logical-adapter-wide, not per physical node.
            if row.key.physical == 0 {
                row.description_current = self.inventory_live
                    && self.descriptions.contains_key(&(row.key.high, row.key.low));
            }
            row.memory_error = self.error.clone();
            row.sampled_at = Some(now);
        }
        self.rows.values().cloned().collect()
    }
}

pub(crate) fn inventory() -> Result<BTreeMap<(u32, u32), Description>, String> {
    let factory: IDXGIFactory1 =
        unsafe { CreateDXGIFactory1() }.map_err(|e| format!("DXGI inventory: {e}"))?;
    let mut result = BTreeMap::new();
    for index in 0..MAX_ADAPTERS as u32 {
        let adapter = match unsafe { factory.EnumAdapters1(index) } {
            Ok(adapter) => adapter,
            Err(error) if error.code() == DXGI_ERROR_NOT_FOUND => return Ok(result),
            Err(error) => return Err(format!("DXGI adapter: {error}")),
        };
        let desc = unsafe { adapter.GetDesc1() }.map_err(|e| format!("DXGI description: {e}"))?;
        let end = desc
            .Description
            .iter()
            .position(|v| *v == 0)
            .unwrap_or(desc.Description.len());
        result.insert(
            (desc.AdapterLuid.HighPart as u32, desc.AdapterLuid.LowPart),
            Description {
                name: String::from_utf16_lossy(&desc.Description[..end]),
                vendor_id: desc.VendorId,
                device_id: desc.DeviceId,
                dedicated_video: desc.DedicatedVideoMemory as u64,
                dedicated_system: desc.DedicatedSystemMemory as u64,
                shared_limit: desc.SharedSystemMemory as u64,
                software: desc.Flags & DXGI_ADAPTER_FLAG_SOFTWARE.0 as u32 != 0,
            },
        );
    }
    Err("DXGI inventory exceeds 64 adapters".into())
}

fn array(handle: PDH_HCOUNTER) -> Result<BTreeMap<Key, Option<u64>>, String> {
    // Fresh sizing query on each retry. PDH says a failed nonzero-size call need
    // not return a reliable required size. u64 storage provides native alignment.
    for _ in 0..4 {
        let mut bytes = 0;
        let mut count = 0;
        let flags = PDH_FMT_LARGE;
        let status =
            unsafe { PdhGetFormattedCounterArrayW(handle, flags, &mut bytes, &mut count, None) };
        if status == 0 && bytes == 0 {
            return Ok(BTreeMap::new());
        }
        if status != PDH_MORE_DATA {
            return Err(format!("GPU memory array sizing: 0x{status:08X}"));
        }
        if bytes == 0 || bytes as usize > MAX_BUFFER {
            return Err("GPU memory array size out of bounds".into());
        }
        let mut buffer = vec![0_u64; (bytes as usize).div_ceil(8)];
        let capacity = bytes as usize;
        let status = unsafe {
            PdhGetFormattedCounterArrayW(
                handle,
                flags,
                &mut bytes,
                &mut count,
                Some(buffer.as_mut_ptr().cast()),
            )
        };
        if status == PDH_MORE_DATA {
            continue;
        }
        if status != 0 {
            return Err(format!("GPU memory array: 0x{status:08X}"));
        }
        if bytes as usize > capacity
            || count as usize > MAX_ADAPTERS
            || (count as usize)
                .checked_mul(size_of::<PDH_FMT_COUNTERVALUE_ITEM_W>())
                .is_none_or(|n| n > bytes as usize)
        {
            return Err("GPU memory array count out of bounds".into());
        }
        let items = unsafe {
            std::slice::from_raw_parts(
                buffer.as_ptr().cast::<PDH_FMT_COUNTERVALUE_ITEM_W>(),
                count as usize,
            )
        };
        let base = buffer.as_ptr() as usize;
        let mut result = BTreeMap::new();
        for item in items {
            let offset = (item.szName.0 as usize)
                .checked_sub(base)
                .ok_or("GPU memory name before buffer")?;
            if !offset.is_multiple_of(2) || offset >= bytes as usize {
                return Err("GPU memory name outside buffer".into());
            }
            let chars =
                unsafe { std::slice::from_raw_parts(item.szName.0, (bytes as usize - offset) / 2) };
            let length = chars
                .iter()
                .take(256)
                .position(|c| *c == 0)
                .ok_or("GPU memory name is not terminated")?;
            let name = String::from_utf16_lossy(&chars[..length]);
            let key = Key::parse(&name).ok_or("Unknown GPU memory adapter identity")?;
            let number = unsafe { item.FmtValue.Anonymous.largeValue };
            let value = (matches!(
                item.FmtValue.CStatus,
                PDH_CSTATUS_VALID_DATA | PDH_CSTATUS_NEW_DATA
            ) && number >= 0)
                .then_some(number as u64);
            if result.insert(key, value).is_some() {
                return Err("Duplicate GPU memory adapter identity".into());
            }
        }
        return Ok(result);
    }
    Err("GPU memory inventory kept changing".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn adapter_retention_does_not_repeat_stale_engines_or_memory_as_live() {
        let now = Instant::now();
        // Suppress native initialization; this test exercises retained state only.
        let mut sampler = Sampler {
            last_retry: Some(now),
            ..Default::default()
        };
        let key = Key::default();
        let initial = Adapter {
            key,
            activity: Usage::Measured(12.),
            engines: vec![Engine {
                number: 1,
                kind: "3D".into(),
                usage: Usage::Measured(12.),
            }],
            ..Default::default()
        };
        let rows = sampler.sample(vec![initial], now);
        assert_eq!(rows[0].activity, Usage::Measured(12.));
        sampler.rows.get_mut(&key).unwrap().memory[0].record(Some(9_000_000_000), now);
        let rows = sampler.sample(Vec::new(), now + Duration::from_secs(1));
        assert_eq!(rows[0].engines[0].usage, Usage::Unavailable);
        assert_eq!(rows[0].memory[0].value, Some(9_000_000_000));
        assert_eq!(rows[0].memory[0].state(now), "Cached");
        let later = now + Duration::from_secs(121);
        sampler.last_retry = Some(later);
        assert!(sampler.sample(Vec::new(), later).is_empty());
    }
    #[test]
    #[ignore = "Read-only native DXGI/PDH adapter probe; no windows or input"]
    fn native_gpu_adapter_memory_read_only_probe() {
        let mut sampler = Sampler::default();
        for index in 0..3 {
            let now = Instant::now();
            let rows = sampler.sample(Vec::new(), now);
            eprintln!(
                "sample {index}, {:.3} ms, {} adapters",
                now.elapsed().as_secs_f64() * 1000.,
                rows.len()
            );
            for row in &rows {
                eprintln!(
                    "{} | {} | {:?} | {:?}",
                    row.key.label(),
                    row.name(),
                    row.memory.each_ref().map(|v| v.value),
                    row.memory_error
                );
            }
            assert!(!rows.is_empty());
            assert!(rows.iter().any(|r| r.description.is_some()));
            assert!(
                rows.iter().any(|r| r.memory.iter().all(|v| v.live)),
                "no complete memory sample"
            );
            std::thread::sleep(Duration::from_millis(1000));
        }
    }
}
