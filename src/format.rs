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

pub fn percent(value: f32) -> String {
    if value >= 10.0 {
        format!("{value:.1}%")
    } else {
        format!("{value:.2}%")
    }
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
