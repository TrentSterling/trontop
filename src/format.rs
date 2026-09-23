pub fn bytes(value: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut scaled = value as f64;
    let mut unit = 0;
    while scaled >= 1024.0 && unit < UNITS.len() - 1 {
        scaled /= 1024.0;
        unit += 1;
    }

    if unit == 0 {
        format!("{value} {}", UNITS[unit])
    } else if scaled >= 100.0 {
        format!("{scaled:.0} {}", UNITS[unit])
    } else if scaled >= 10.0 {
        format!("{scaled:.1} {}", UNITS[unit])
    } else {
        format!("{scaled:.2} {}", UNITS[unit])
    }
}

pub fn rate(value: f64) -> String {
    if value < 0.5 {
        "0 B/s".into()
    } else {
        format!("{}/s", bytes(value.round() as u64))
    }
}

/// Format a MiB/s rate with the same unit scaling and rounding as [`rate`].
/// Performance histories for disks and network are stored in MiB/s. A later
/// polish-gauntlet package wires this into disk/network graph card labels.
#[allow(dead_code)]
pub fn rate_mib(mib_per_s: f32) -> String {
    rate(f64::from(mib_per_s) * 1_048_576.0)
}

/// Round an axis top up to a whole, readable step (1, 5, 50, 500, ...), so a KPI
/// sparkline's implied scale reads as "450", never "447.35". Used by
/// [`crate::widgets::kpi_tile`], which later packages wire into pages.
#[allow(dead_code)]
pub fn nice_top(value: f32) -> f32 {
    if !value.is_finite() || value <= 1.0 {
        return 1.0;
    }
    let step = (10_f32.powf(value.log10().floor()) / 2.0).max(1.0);
    (value / step).ceil() * step
}

/// Binary gigabytes for graph values: one decimal at 10 or more, two below,
/// so "11.96" reads "12.0 GiB" and "0.53 GiB" keeps its precision.
pub fn gib(value: f32) -> String {
    if value.abs() >= 9.995 {
        format!("{value:.1} GiB")
    } else {
        format!("{value:.2} GiB")
    }
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
        assert_eq!(bytes(0), "0 B");
        assert_eq!(bytes(1024), "1.00 KB");
        assert_eq!(bytes(1024 * 1024), "1.00 MB");
    }

    #[test]
    fn rate_mib_matches_rate_rounding() {
        assert!(rate_mib(1.1).contains("MB/s"));
        assert_eq!(rate_mib(0.0), "0 B/s");
    }

    #[test]
    fn nice_top_rounds_axis_maximums_up_to_a_readable_step() {
        assert_eq!(nice_top(447.35), 450.0);
        assert_eq!(nice_top(58.1), 60.0);
        assert_eq!(nice_top(0.0), 1.0);
    }

    #[test]
    fn graph_number_formats_pick_decimals_by_magnitude() {
        assert_eq!(gib(11.96), "12.0 GiB");
        assert_eq!(gib(0.53), "0.53 GiB");
        assert_eq!(gib(0.0), "0.00 GiB");
        assert_eq!(gib(9.994), "9.99 GiB");
        // 9.996 would print "10.00" with two decimals; it crosses to one.
        assert_eq!(gib(9.996), "10.0 GiB");
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
