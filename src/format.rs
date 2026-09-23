/// Divides by 1024 until below that threshold or the largest unit (index 4)
/// is reached. Shared scaling for [`bytes`] and [`rate`], which differ only
/// in their unit labels.
fn scale_1024(value: f64) -> (f64, usize) {
    let mut scaled = value.max(0.0);
    let mut unit = 0;
    while scaled >= 1024.0 && unit < 4 {
        scaled /= 1024.0;
        unit += 1;
    }
    (scaled, unit)
}

fn format_scaled(raw: u64, scaled: f64, unit: usize, units: &[&str; 5]) -> String {
    if unit == 0 {
        format!("{raw} {}", units[unit])
    } else if scaled >= 100.0 {
        format!("{scaled:.0} {}", units[unit])
    } else if scaled >= 10.0 {
        format!("{scaled:.1} {}", units[unit])
    } else {
        format!("{scaled:.2} {}", units[unit])
    }
}

/// The one formatter for an absolute byte quantity (memory, VRAM, disk
/// capacity): binary (1024-based) math, IEC-labeled (KiB, MiB, GiB, TiB) so
/// the label always matches the math. Every memory and VRAM tile routes
/// through this, so the same byte count reads identically on every page.
pub fn bytes(value: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
    let (scaled, unit) = scale_1024(value as f64);
    format_scaled(value, scaled, unit, &UNITS)
}

/// A bytes/second reading: the same binary scaling as [`bytes`], but labeled
/// with the decimal-looking unit ("MB/s") Windows itself uses for rates, so a
/// live reading never reads "MiB/s" next to a plain "MB/s" axis.
pub fn rate(value: f64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    if value < 0.5 {
        return "0 B/s".into();
    }
    let raw = value.round() as u64;
    let (scaled, unit) = scale_1024(raw as f64);
    format!("{}/s", format_scaled(raw, scaled, unit, &UNITS))
}

/// Rounds a bytes/second peak up to a readable rate-axis top, in the same
/// bucket [`rate`] would display it in (KB/s, MB/s, GB/s), then converts
/// back to bytes/second so a history stored in raw bytes/second can compare
/// its points against it directly. A silent axis floors at "1 KB/s" rather
/// than a sub-byte-per-second "1 B/s".
pub fn nice_rate_top(bytes_per_second: f64) -> f64 {
    let (mut scaled, mut unit) = scale_1024(bytes_per_second);
    if unit == 0 {
        unit = 1;
        scaled /= 1024.0;
    }
    f64::from(nice_top(scaled as f32)) * 1024f64.powi(unit as i32)
}

/// Labels a bytes/second value already rounded by [`nice_rate_top`]: a clean
/// whole number in its bucket ("50 MB/s"), never [`rate`]'s decimal text.
pub fn rate_axis(bytes_per_second: f64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let (scaled, unit) = scale_1024(bytes_per_second);
    format!("{scaled:.0} {}/s", UNITS[unit])
}

/// Round an axis top up to a whole, readable step (1, 5, 50, 500, ...), so a
/// KPI sparkline's implied scale reads as "450", never "447.35".
pub fn nice_top(value: f32) -> f32 {
    if !value.is_finite() || value <= 1.0 {
        return 1.0;
    }
    let step = (10_f32.powf(value.log10().floor()) / 2.0).max(1.0);
    (value / step).ceil() * step
}

/// Temperature axis top: a fixed, familiar 0-100 degree band, unless a
/// reading actually runs hotter, in which case it grows by a readable step.
/// GPU and drive temperature share this one rule.
pub fn celsius_axis_top(peak: f32) -> f32 {
    if peak > 95.0 { nice_top(peak) } else { 100.0 }
}

/// Binary bytes for a graph value already expressed in GiB: converts back to
/// bytes and routes through [`bytes`], so the same byte count reads
/// identically on Graphs and on Performance.
pub fn gib(value: f32) -> String {
    bytes((f64::from(value) * 1_073_741_824.0).round().max(0.0) as u64)
}

/// Milliseconds: one decimal at 10 or more ("22.4 ms"), two below ("1.47 ms").
pub fn ms(value: f32) -> String {
    if value.abs() >= 9.995 {
        format!("{value:.1} ms")
    } else {
        format!("{value:.2} ms")
    }
}

/// A count is a whole number, never "369.0". `unit` is appended when not
/// empty ("2 req").
pub fn count(value: f32, unit: &str) -> String {
    let whole = value.round();
    // Avoid "-0" for tiny negative noise.
    let whole = if whole == 0.0 { 0.0 } else { whole };
    if unit.is_empty() {
        format!("{whole:.0}")
    } else {
        format!("{whole:.0} {unit}")
    }
}

/// A percentage always shows one decimal ("3.1%"), including at 0 and below
/// 10, so a busy process tree never fills with two-decimal noise. Callers
/// that want a calm muted "0%" for an exact-zero measurement (the CPU and
/// GPU columns) special-case that themselves; this function never rounds a
/// small positive reading down to "0%" on its own.
pub fn percent(value: f32) -> String {
    format!("{value:.1}%")
}

pub fn duration(total_seconds: u64) -> String {
    let days = total_seconds / 86_400;
    let hours = (total_seconds % 86_400) / 3_600;
    let minutes = (total_seconds % 3_600) / 60;
    if days > 0 {
        format!("{days}d {hours:02}h {minutes:02}m")
    } else {
        format!("{hours:02}h {minutes:02}m")
    }
}

pub fn age_from_unix(started_at_unix: u64) -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs());
    duration(now.saturating_sub(started_at_unix))
}

pub fn millis(value: u64) -> String {
    let total_seconds = value / 1_000;
    let hours = total_seconds / 3_600;
    let minutes = (total_seconds % 3_600) / 60;
    let seconds = total_seconds % 60;
    format!("{hours:02}:{minutes:02}:{seconds:02}")
}

/// Logical processor indices within one Windows processor-group mask.
pub fn cpu_set(mask: usize) -> String {
    let mut ranges = Vec::new();
    let mut bit = 0;
    while bit < usize::BITS {
        if mask & (1_usize << bit) == 0 {
            bit += 1;
            continue;
        }
        let first = bit;
        while bit + 1 < usize::BITS && mask & (1_usize << (bit + 1)) != 0 {
            bit += 1;
        }
        ranges.push(if first == bit {
            first.to_string()
        } else {
            format!("{first}-{bit}")
        });
        bit += 1;
    }
    if ranges.is_empty() {
        "None".into()
    } else {
        ranges.join(", ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_binary_units() {
        // Bytes are always IEC-labeled: the math is 1024-based, so the label
        // says so, on every page that shows an absolute byte count.
        assert_eq!(bytes(0), "0 B");
        assert_eq!(bytes(1024), "1.00 KiB");
        assert_eq!(bytes(1024 * 1024), "1.00 MiB");
        assert_eq!(bytes(1024 * 1024 * 1024), "1.00 GiB");
    }

    #[test]
    fn rate_keeps_decimal_looking_labels_despite_binary_math() {
        // Rates keep the "MB/s" label Windows itself uses; only absolute
        // byte quantities (`bytes`) switched to IEC labels.
        assert_eq!(rate(1_048_576.0), "1.00 MB/s");
        assert_eq!(rate(0.0), "0 B/s");
    }

    #[test]
    fn nice_rate_top_rounds_in_its_own_bucket_then_converts_back_to_bytes() {
        // 47.6 MB/s and 1.51 MB/s from the P15 polish spec: nice_top runs on
        // the scaled (MB) value, not the raw byte count, so the result
        // relabels back to a clean whole number in that same bucket.
        let mib = 1024.0 * 1024.0;
        assert_eq!(rate_axis(nice_rate_top(47.6 * mib)), "50 MB/s");
        assert_eq!(rate_axis(nice_rate_top(1.51 * mib)), "2 MB/s");
        // All-zero history floors at "1 KB/s", never a sub-byte "1 B/s".
        assert_eq!(rate_axis(nice_rate_top(0.0)), "1 KB/s");
    }

    #[test]
    fn nice_top_rounds_axis_maximums_up_to_a_readable_step() {
        for (value, top) in [
            (0.0, 1.0),
            (1.15, 2.0),
            (1.51, 2.0),
            (3.45, 4.0),
            (4.16, 5.0),
            (11.5, 15.0),
            (15.7, 20.0),
            (47.6, 50.0),
            (58.1, 60.0),
            (447.35, 450.0),
            (16101.0, 20000.0),
        ] {
            assert_eq!(nice_top(value), top, "{value}");
        }
    }

    #[test]
    fn celsius_axis_stays_at_a_familiar_100_unless_a_reading_runs_hot() {
        assert_eq!(celsius_axis_top(0.0), 100.0);
        assert_eq!(celsius_axis_top(47.0), 100.0);
        assert_eq!(celsius_axis_top(95.0), 100.0, "95 does not exceed 95");
        assert_eq!(celsius_axis_top(96.0), 100.0, "nice_top(96) is still 100");
        assert_eq!(celsius_axis_top(101.0), 150.0);
    }

    #[test]
    fn gib_and_bytes_agree_for_the_same_byte_count() {
        // Graphs stores memory/VRAM values pre-converted to GiB (Unit::Gib);
        // Performance shows the same quantity straight from bytes. Both must
        // read identically for the same underlying measurement.
        for raw in [0_u64, 536_870_912, 1_073_741_824, 4 * 1_073_741_824] {
            let as_gib = raw as f32 / 1_073_741_824.0;
            assert_eq!(gib(as_gib), bytes(raw), "{raw} bytes");
        }
    }

    #[test]
    fn graph_number_formats_pick_decimals_by_magnitude() {
        assert_eq!(gib(11.96), "12.0 GiB");
        // Below 1 GiB, gib auto-scales down through bytes rather than
        // forcing a "0.53 GiB" reading; the same value on Performance would
        // read the same way.
        assert_eq!(gib(0.53), "543 MiB");
        assert_eq!(gib(0.0), "0 B");
        assert_eq!(gib(9.994), "9.99 GiB");
        assert_eq!(gib(9.996), "10.00 GiB");
        assert_eq!(gib(10.0), "10.0 GiB");
        assert_eq!(ms(22.43), "22.4 ms");
        assert_eq!(ms(1.466), "1.47 ms");
        assert_eq!(ms(0.0), "0.00 ms");
        assert_eq!(ms(9.999), "10.0 ms");
        assert_eq!(count(369.0, ""), "369");
        assert_eq!(count(447.35, ""), "447");
        assert_eq!(count(2.0, "req"), "2 req");
        assert_eq!(count(-0.2, "req"), "0 req");
        assert_eq!(count(0.5, ""), "1");
    }

    #[test]
    fn percent_always_shows_one_decimal() {
        assert_eq!(percent(0.0), "0.0%");
        assert_eq!(percent(3.07), "3.1%");
        assert_eq!(percent(12.34), "12.3%");
        assert_eq!(percent(45.6), "45.6%");
    }

    #[test]
    fn formats_uptime() {
        assert_eq!(duration(90), "00h 01m");
        assert_eq!(duration(90_000), "1d 01h 00m");
    }

    #[test]
    fn formats_affinity_ranges_without_losing_sparse_or_high_bits() {
        assert_eq!(cpu_set(0), "None");
        assert_eq!(cpu_set(1), "0");
        assert_eq!(cpu_set(0b101110), "1-3, 5");
        assert_eq!(cpu_set(usize::MAX), format!("0-{}", usize::BITS - 1));
        assert_eq!(
            cpu_set(1 | (1_usize << (usize::BITS - 1))),
            format!("0, {}", usize::BITS - 1)
        );
    }
}
