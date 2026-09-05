use super::{Batch, Column, MAX_DEVICES};
use std::time::{Duration, Instant};
use windows::Win32::System::Performance::*;
use windows::core::PCWSTR;

const COUNTERS: [&str; 5] = [
    "% Idle Time",
    "Avg. Disk sec/Transfer",
    "Current Disk Queue Length",
    "Disk Read Bytes/sec",
    "Disk Write Bytes/sec",
];
// PDH_FMT_NOCAP100 is documented by PDH but absent from these generated bindings.
const FORMAT: PDH_FMT = PDH_FMT(PDH_FMT_DOUBLE.0 | 0x8000);
const MAX_BUFFER: usize = 1024 * 1024;

#[derive(Default)]
pub(super) struct Sampler {
    query: Option<PDH_HQUERY>,
    counters: [Option<PDH_HCOUNTER>; 5],
    samples: u8,
    retry_at: Option<Instant>,
}

impl Sampler {
    fn initialize(&mut self) -> Result<(), &'static str> {
        if self.retry_at.is_some_and(|at| Instant::now() < at) {
            return Err("Physical disk counters are retrying.");
        }
        self.retry_at = Some(Instant::now() + Duration::from_secs(30));
        if self.query.is_none() {
            let mut query = PDH_HQUERY::default();
            if unsafe { PdhOpenQueryW(PCWSTR::null(), 0, &mut query) } != 0 {
                return Err("Physical disk query could not open.");
            }
            self.query = Some(query);
        }
        for (index, name) in COUNTERS.into_iter().enumerate() {
            if self.counters[index].is_some() {
                continue;
            }
            let path: Vec<u16> = format!("\\PhysicalDisk(*)\\{name}")
                .encode_utf16()
                .chain([0])
                .collect();
            let mut handle = PDH_HCOUNTER::default();
            if unsafe {
                PdhAddEnglishCounterW(self.query.unwrap(), PCWSTR(path.as_ptr()), 0, &mut handle)
            } == 0
            {
                self.counters[index] = Some(handle);
                self.samples = 0;
            }
        }
        Ok(())
    }
    pub(super) fn sample(&mut self) -> Batch {
        if self.query.is_none() || self.counters.iter().any(Option::is_none) {
            let _ = self.initialize();
        }
        let Some(query) = self.query else {
            return std::array::from_fn(|_| Err("Physical disk query unavailable."));
        };
        if unsafe { PdhCollectQueryData(query) } != 0 {
            self.samples = 0;
            return std::array::from_fn(|_| Err("Physical disk collection failed."));
        }
        self.samples = self.samples.saturating_add(1);
        if self.samples < 2 {
            // An instantaneous queue read supplies real identities while rate
            // counters collect their baseline, without displaying a false error.
            let baseline = self.counters[2]
                .map(array)
                .unwrap_or_else(|| Err("Physical disk baseline unavailable."));
            return std::array::from_fn(|index| {
                if self.counters[index].is_none() {
                    return Err("Some physical disk counters are unavailable.");
                }
                baseline.clone().map(|mut rows| {
                    if index != 2 {
                        for (_, value) in &mut rows {
                            *value = None;
                        }
                    }
                    rows
                })
            });
        }
        std::array::from_fn(|index| {
            let Some(handle) = self.counters[index] else {
                return Err("Some physical disk counters are unavailable.");
            };
            array(handle)
        })
    }
}

impl Drop for Sampler {
    fn drop(&mut self) {
        if let Some(query) = self.query.take() {
            unsafe {
                PdhCloseQuery(query);
            }
        }
    }
}

fn array(handle: PDH_HCOUNTER) -> Column {
    for _ in 0..3 {
        let mut bytes = 0;
        let mut count = 0;
        let status =
            unsafe { PdhGetFormattedCounterArrayW(handle, FORMAT, &mut bytes, &mut count, None) };
        if status == 0 && bytes == 0 {
            return Ok(Vec::new());
        }
        if status != PDH_MORE_DATA {
            return Err("Physical disk array not ready or unavailable.");
        }
        if bytes == 0 || bytes as usize > MAX_BUFFER {
            return Err("Physical disk array exceeded buffer limit.");
        }
        // u64 allocation aligns both the item structs and the trailing UTF-16.
        let capacity = bytes as usize;
        let mut buffer = vec![0_u64; capacity.div_ceil(8)];
        let status = unsafe {
            PdhGetFormattedCounterArrayW(
                handle,
                FORMAT,
                &mut bytes,
                &mut count,
                Some(buffer.as_mut_ptr().cast()),
            )
        };
        if status == PDH_MORE_DATA {
            continue;
        } // re-probe from zero, not this returned size
        if status != 0 {
            return Err("Physical disk array could not be read.");
        }
        return decode(&buffer, capacity, count as usize);
    }
    Err("Physical disk inventory changed during the read.")
}

fn decode(buffer: &[u64], capacity: usize, count: usize) -> Column {
    let item_size = std::mem::size_of::<PDH_FMT_COUNTERVALUE_ITEM_W>();
    if capacity > buffer.len() * 8 || count > MAX_DEVICES + 1 || count > capacity / item_size {
        return Err("Physical disk array exceeded item limit.");
    }
    let base = buffer.as_ptr() as usize;
    let mut rows = Vec::with_capacity(count);
    for index in 0..count {
        // Bounded array region, aligned allocation, native C struct containing no references.
        let item = unsafe {
            &*buffer
                .as_ptr()
                .cast::<PDH_FMT_COUNTERVALUE_ITEM_W>()
                .add(index)
        };
        let start = item.szName.0 as usize;
        let Some(offset) = start.checked_sub(base) else {
            return Err("Physical disk instance pointer out of range.");
        };
        if offset < count * item_size || offset >= capacity || offset % 2 != 0 {
            return Err("Physical disk instance pointer out of range.");
        }
        let words = ((capacity - offset) / 2).min(513);
        let name_words = unsafe {
            std::slice::from_raw_parts(
                buffer.as_ptr().cast::<u8>().add(offset).cast::<u16>(),
                words,
            )
        };
        let Some(length) = name_words.iter().position(|&c| c == 0) else {
            return Err("Physical disk instance name exceeded limit.");
        };
        let name = String::from_utf16(&name_words[..length])
            .map_err(|_| "Physical disk instance name is invalid.")?;
        let valid = matches!(
            item.FmtValue.CStatus,
            PDH_CSTATUS_VALID_DATA | PDH_CSTATUS_NEW_DATA
        );
        // doubleValue is the initialized union member because FORMAT requests DOUBLE.
        let raw = unsafe { item.FmtValue.Anonymous.doubleValue };
        rows.push((
            name,
            (valid && raw.is_finite() && raw >= 0.0).then_some(raw),
        ));
    }
    Ok(rows)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_unbounded_arrays_and_outside_string_pointers() {
        assert!(decode(&[0; 8], 64, 1000).is_err());
        assert!(decode(&[0; 8], 65, 1).is_err());
        assert!(decode(&[0; 8], 64, 1).is_err());
        assert!(decode(&[], 0, 0).unwrap().is_empty());
    }

    #[test]
    fn array_decoder_checks_status_zero_finite_values_and_bounded_utf16() {
        let mut buffer = vec![0_u64; 80];
        let item_size = std::mem::size_of::<PDH_FMT_COUNTERVALUE_ITEM_W>();
        let name: Vec<_> = "7 C:".encode_utf16().chain([0]).collect();
        let bytes = item_size + name.len() * 2;
        // Build a native-layout test array inside an owned aligned allocation.
        unsafe {
            let item = buffer.as_mut_ptr().cast::<PDH_FMT_COUNTERVALUE_ITEM_W>();
            let text = buffer
                .as_mut_ptr()
                .cast::<u8>()
                .add(item_size)
                .cast::<u16>();
            std::ptr::copy_nonoverlapping(name.as_ptr(), text, name.len());
            (*item).szName = windows::core::PWSTR(text);
            (*item).FmtValue.CStatus = PDH_CSTATUS_VALID_DATA;
            (*item).FmtValue.Anonymous.doubleValue = 0.0;
        }
        assert_eq!(
            decode(&buffer, bytes, 1).unwrap(),
            vec![("7 C:".into(), Some(0.0))]
        );
        unsafe {
            (*buffer.as_mut_ptr().cast::<PDH_FMT_COUNTERVALUE_ITEM_W>())
                .FmtValue
                .CStatus = PDH_CSTATUS_INVALID_DATA;
        }
        assert_eq!(decode(&buffer, bytes, 1).unwrap()[0].1, None);
        unsafe {
            let item = buffer.as_mut_ptr().cast::<PDH_FMT_COUNTERVALUE_ITEM_W>();
            (*item).FmtValue.CStatus = PDH_CSTATUS_NEW_DATA;
            (*item).FmtValue.Anonymous.doubleValue = f64::NAN;
        }
        assert_eq!(decode(&buffer, bytes, 1).unwrap()[0].1, None);
        assert!(
            decode(&buffer, bytes - 2, 1).is_err(),
            "No terminator in claimed buffer"
        );
        unsafe {
            (*buffer.as_mut_ptr().cast::<PDH_FMT_COUNTERVALUE_ITEM_W>())
                .szName
                .0 = buffer.as_mut_ptr().cast::<u16>();
        }
        assert!(
            decode(&buffer, bytes, 1).is_err(),
            "Name points into item headers"
        );
    }
    #[test]
    #[ignore = "read-only native physical disk PDH probe; no windows or input"]
    fn native_physical_disk_pdh_probe() {
        let mut sampler = Sampler::default();
        let mut snapshot = crate::disk_activity::Snapshot::default();
        for index in 0..4 {
            let started = Instant::now();
            let batch = sampler.sample();
            snapshot.apply(batch, Instant::now(), started.elapsed());
            println!(
                "sample {index}: state={:?}, query_ms={:.3}, error={:?}",
                snapshot.state(Instant::now()),
                snapshot.query_millis,
                snapshot.error
            );
            for d in &snapshot.devices {
                println!("disk {}: {:?}", d.number, d.readings.map(|r| r.value));
            }
            if index < 3 {
                std::thread::sleep(Duration::from_secs(1));
            }
        }
        assert!(!snapshot.devices.is_empty(), "No physical disks returned");
        assert!(
            snapshot.devices.iter().any(|d| d
                .readings
                .iter()
                .all(|r| r.live(&snapshot, Instant::now()).is_some())),
            "No fully measured physical disk"
        );
    }
}
