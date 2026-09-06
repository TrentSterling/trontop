use super::{Capture, Format, seconds};
use crate::{diagnostics::Provider, gpu_activity::Usage};
use serde_json::{Value, json};
use std::io::{self, Write};
use std::sync::atomic::{AtomicBool, Ordering};

fn check(stop: &AtomicBool) -> io::Result<()> {
    if stop.load(Ordering::Acquire) {
        Err(io::Error::other("Export cancelled."))
    } else {
        Ok(())
    }
}
fn value(w: &mut impl Write, v: &Value) -> io::Result<()> {
    serde_json::to_writer(w, v).map_err(io::Error::other)
}
fn array(
    w: &mut impl Write,
    key: &str,
    values: impl Iterator<Item = Value>,
    stop: &AtomicBool,
) -> io::Result<()> {
    write!(w, ",\n\"{key}\":[")?;
    for (index, v) in values.enumerate() {
        check(stop)?;
        if index > 0 {
            w.write_all(b",")?;
        }
        w.write_all(b"\n")?;
        value(w, &v)?;
    }
    w.write_all(b"\n]")
}
fn gpu(usage: Usage) -> Value {
    if usage.value().is_some_and(|v| !v.is_finite()) {
        return json!({"state": "invalid", "percent": null, "lower_bound_percent": null});
    }
    let state = match usage {
        Usage::Measured(_) => "measured",
        Usage::Partial(_) => "partial",
        Usage::Warming => "warming",
        Usage::Unreported => "unreported",
        Usage::Unavailable => "unavailable",
    };
    json!({"state": state, "percent": usage.exact(),
        "lower_bound_percent": match usage { Usage::Partial(v) => Some(v), _ => None }})
}

pub(super) fn write(w: &mut impl Write, c: &Capture, stop: &AtomicBool) -> io::Result<()> {
    check(stop)?;
    match c.options.format {
        Format::Json => json_snapshot(w, c, stop),
        Format::Csv => csv_processes(w, c, stop),
    }
}

fn json_snapshot(w: &mut impl Write, c: &Capture, stop: &AtomicBool) -> io::Result<()> {
    let s = &c.snapshot;
    let private = c.options.private_details;
    w.write_all(b"{\n\"metadata\":")?;
    value(
        w,
        &json!({
            "schema_version": 1, "application": "Trontop", "version": env!("CARGO_PKG_VERSION"),
            "build": env!("TRONTOP_BUILD_ID"), "captured_unix_ms": c.unix_ms,
            "snapshot_sequence": s.sequence, "has_sample": s.sequence > 0, "private_details": private,
            "scope": "all sampler rows; no search filter, subtree aggregation, chart history or command annotations",
            "privacy": "Names and installed software remain identifiable. Not an anonymous support report.",
            "missing": "null is unavailable or excluded; partial GPU values are lower bounds",
            "units": "bytes, seconds, milliseconds, Celsius, watts, MHz and percent as named"
        }),
    )?;
    w.write_all(b",\n\"system\":")?;
    value(
        w,
        &if s.sequence == 0 {
            Value::Null
        } else {
            json!({
                "host_name": private.then_some(&s.host_name), "os_name": s.os_name,
                "cpu": {"brand": s.cpu.brand, "frequency_mhz": s.cpu.frequency_mhz,
                    "physical_cores": s.cpu.physical_cores, "logical_cores": s.cpu.logical_cores,
                    "percent": s.cpu_percent, "temperature_c": null, "temperature_provider": "not_connected"},
                "memory_used_bytes": s.memory_used_bytes, "memory_total_bytes": s.memory_total_bytes,
                "memory_available_bytes": s.memory_available_bytes,
                // Preserve legacy schema keys, but explicitly identify their derived semantics.
                "swap_used_bytes": s.memory_details.map(|m| m.commit_bytes.saturating_sub(m.physical_total_bytes)),
                "swap_total_bytes": s.memory_details.map(|m| m.commit_limit_bytes.saturating_sub(m.physical_total_bytes)),
                "swap_semantics": "legacy commit-minus-physical estimates; not page-file occupancy; see memory_counters for freshness",
                "memory_counters": memory_counters(s, c.at), "uptime_seconds": s.uptime_seconds,
                "sample_seconds": s.sample_seconds, "process_count": s.process_count, "gpu": gpu(s.gpu.reading())
            })
        },
    )?;
    array(w, "providers", Provider::ALL.into_iter().map(|provider| {
        let h = s.diagnostics.get(provider);
        json!({"provider": provider.name(), "state": h.state(provider, c.at).label(),
            "attempt_age_seconds": seconds(h.last_attempt, c.at), "last_usable_age_seconds": seconds(h.last_success, c.at),
            "query_millis": h.query_millis, "coverage": h.coverage, "issue": h.issue.map(|i| i.description())})
    }), stop)?;
    array(w, "processes", s.processes.iter().map(|p| json!({
        "pid": p.pid, "parent_pid": p.parent_pid, "name": p.name, "status": p.status,
        "cpu_percent": p.cpu_percent, "gpu": gpu(p.gpu_percent), "memory_bytes": p.memory_bytes,
        "virtual_memory_bytes": p.virtual_memory_bytes, "read_bytes_per_sec": p.read_bytes_per_sec,
        "write_bytes_per_sec": p.write_bytes_per_sec, "total_read_bytes": p.total_read_bytes,
        "total_write_bytes": p.total_write_bytes, "accumulated_cpu_millis": p.accumulated_cpu_millis,
        "started_at_unix": p.started_at_unix, "priority": p.control.priority.label(),
        "native_identity_available": p.identity().is_some(), "control_accessible": p.control.accessible,
        "affinity_mask": p.control.accessible.then(|| format!("0x{:X}", p.control.affinity_mask)),
        "details": private.then(|| json!({"user": p.user, "command": p.command,
            "executable": p.executable.as_ref().map(|p| p.to_string_lossy()),
            "cwd": p.cwd.as_ref().map(|p| p.to_string_lossy())}))
    })), stop)?;
    array(w, "disks", s.disks.iter().map(|d| json!({
        "name": d.name, "mount": private.then_some(&d.mount), "file_system": d.file_system,
        "kind": d.kind, "total_bytes": d.total_bytes, "available_bytes": d.available_bytes,
        "read_bytes_per_sec": d.read_bytes_per_sec, "write_bytes_per_sec": d.write_bytes_per_sec, "removable": d.removable
    })), stop)?;
    array(w, "physical_disks", s.physical_disks.devices.iter().map(|d| {
        let mut fields = serde_json::Map::new();
        for metric in crate::disk_activity::Metric::ALL {
            let reading = d.readings[metric as usize];
            fields.insert(metric.key().into(), json!({
                "value": reading.value, "state": reading.state(&s.physical_disks, c.at),
                "last_usable_age_seconds": seconds(reading.at, c.at)
            }));
        }
        json!({"disk_number": d.number, "instance": private.then_some(&d.instance),
            "identity_scope": "Windows PDH instance, not a persistent hardware serial or volume mapping",
            "provider_state": s.physical_disks.state(c.at).label(), "metrics": fields})
    }), stop)?;
    array(w, "networks", s.networks.iter().enumerate().map(|(index, n)| json!({
        "index": index, "name": private.then_some(&n.name), "received_bytes_per_sec": n.received_bytes_per_sec,
        "transmitted_bytes_per_sec": n.transmitted_bytes_per_sec, "total_received_bytes": n.total_received_bytes,
        "total_transmitted_bytes": n.total_transmitted_bytes
    })), stop)?;
    array(
        w,
        "gpu_engines",
        s.gpu
            .engine_utilization
            .iter()
            .map(|(name, usage)| json!({"name": name, "usage": gpu(*usage)})),
        stop,
    )?;
    array(w, "gpu_sensors", s.gpu_sensors.adapters.iter().map(|g| json!({
        "name": g.name, "uuid": private.then_some(&g.uuid), "cached": s.gpu_sensors.using_cached,
        "last_usable_age_seconds": seconds(s.gpu_sensors.last_success, c.at),
        "temperature_c": g.temperature_c, "power_w": g.power_w,
        "graphics_clock_mhz": g.graphics_clock_mhz, "memory_clock_mhz": g.memory_clock_mhz,
        "fan_target_percent": g.fan_percent, "vram_used_bytes": g.memory.map(|m| m.0),
        "vram_total_bytes": g.memory.map(|m| m.1), "vram_includes_reservations": g.memory_includes_reserved,
        "has_field_errors": g.error.is_some()
    })), stop)?;
    array(w, "drive_sensors", s.storage_sensors.drives.iter().map(|d| json!({
        "name": d.device.name, "id": private.then_some(&d.device.id), "state": d.status(c.at),
        "last_usable_age_seconds": seconds(d.last_success, c.at), "query_millis": d.query_millis,
        "warning_c": d.temperatures.warning, "critical_c": d.temperatures.critical,
        "sensors": d.temperatures.sensors.iter().map(|t| json!({"index": t.index, "celsius": t.celsius,
            "over_threshold_c": t.over_threshold, "under_threshold_c": t.under_threshold, "event": t.event})).collect::<Vec<_>>()
    })), stop)?;
    array(w, "users", s.users.iter().enumerate().map(|(index, u)| json!({
        "index": index, "name": private.then_some(&u.name), "process_count": u.process_count,
        "cpu_percent": u.cpu_percent, "gpu": gpu(u.gpu_percent), "memory_bytes": u.memory_bytes, "disk_bytes_per_sec": u.disk_bytes_per_sec
    })), stop)?;
    array(w, "startup_sources", s.startup.sources.iter().map(|source| json!({
        "source": source.source.name(), "state": source.state(c.at).label(), "retention_limited": source.retention_limited,
        "last_complete_age_seconds": seconds(source.last_complete, c.at), "entry_count": source.entries.len()
    })), stop)?;
    array(w, "startup", s.startup.rows().map(|(source, entry)| json!({
        "name": entry.row.name, "source": source.source.name(), "state": source.row_state(entry, c.at),
        "observed_age_seconds": seconds(Some(entry.observed_at), c.at),
        "key": private.then_some(&entry.row.key), "command": private.then_some(&entry.row.command)
    })), stop)?;
    array(w, "services", s.services.iter().map(|service| json!({
        "name": service.name, "display_name": service.display_name, "reported_state": service.status.state.label(),
        "host_pid": service.status.pid, "stop_accepted": service.status.accepts_stop,
        "source": "last complete service inventory; command observations not included"
    })), stop)?;
    w.write_all(b"\n}\n")
}

fn memory_counters(s: &crate::model::SystemSnapshot, at: std::time::Instant) -> Value {
    let h = s.diagnostics.get(Provider::MemoryCounters);
    let m = s.memory_details;
    json!({
        "source": "Windows K32GetPerformanceInfo", "state": h.state(Provider::MemoryCounters, at).label(),
        "last_usable_age_seconds": seconds(h.last_success, at),
        "commit_bytes": m.map(|v| v.commit_bytes), "commit_limit_bytes": m.map(|v| v.commit_limit_bytes),
        "commit_peak_bytes": m.map(|v| v.commit_peak_bytes), "system_cache_bytes": m.map(|v| v.system_cache_bytes),
        "kernel_paged_bytes": m.map(|v| v.kernel_paged_bytes), "kernel_nonpaged_bytes": m.map(|v| v.kernel_nonpaged_bytes)
    })
}

// Every CSV text field is quoted; dangerous leading text is additionally prefixed.
// This is defense in depth for spreadsheet import, not an application-independent guarantee.
fn cell(w: &mut impl Write, text: &str) -> io::Result<()> {
    w.write_all(b"\"")?;
    if text
        .trim_start_matches(|c: char| c.is_whitespace() || c.is_control() || c == '\u{feff}')
        .starts_with([
            '=', '+', '-', '@', '\u{ff1d}', '\u{ff0b}', '\u{ff0d}', '\u{ff20}',
        ])
        || text.starts_with(['\t', '\r', '\n'])
    {
        w.write_all(b"'")?;
    }
    for part in text.split_inclusive('"') {
        w.write_all(part.as_bytes())?;
        if part.ends_with('"') {
            w.write_all(b"\"")?;
        }
    }
    w.write_all(b"\"")
}
fn row(w: &mut impl Write, cells: impl IntoIterator<Item = String>) -> io::Result<()> {
    for (index, text) in cells.into_iter().enumerate() {
        if index > 0 {
            w.write_all(b",")?;
        }
        cell(w, &text)?;
    }
    w.write_all(b"\r\n")
}
fn number(n: f64) -> String {
    if n.is_finite() {
        n.to_string()
    } else {
        String::new()
    }
}

fn csv_processes(w: &mut impl Write, c: &Capture, stop: &AtomicBool) -> io::Result<()> {
    // BOM helps Windows spreadsheet readers identify UTF-8, without changing records.
    w.write_all(b"\xEF\xBB\xBF")?;
    let mut headers = vec![
        "schema_version",
        "captured_unix_ms",
        "snapshot_sequence",
        "system_state",
        "system_age_seconds",
        "gpu_provider_state",
        "gpu_age_seconds",
        "pid",
        "parent_pid",
        "name",
        "status",
        "cpu_percent",
        "gpu_state",
        "gpu_percent",
        "gpu_lower_bound_percent",
        "memory_bytes",
        "virtual_memory_bytes",
        "read_bytes_per_sec",
        "write_bytes_per_sec",
        "total_read_bytes",
        "total_write_bytes",
        "accumulated_cpu_millis",
        "started_at_unix",
        "priority",
    ];
    if c.options.private_details {
        headers.extend(["user", "executable", "command", "cwd"]);
    }
    row(w, headers.into_iter().map(str::to_owned))?;
    let health = c.snapshot.diagnostics.get(Provider::System);
    let gpu_health = c.snapshot.diagnostics.get(Provider::GpuActivity);
    for p in &c.snapshot.processes {
        check(stop)?;
        let usage = gpu(p.gpu_percent);
        let mut cells = vec![
            "1".into(),
            c.unix_ms.map_or(String::new(), |v| v.to_string()),
            c.snapshot.sequence.to_string(),
            health.state(Provider::System, c.at).label().into(),
            seconds(health.last_success, c.at).map_or(String::new(), number),
            gpu_health.state(Provider::GpuActivity, c.at).label().into(),
            seconds(gpu_health.last_success, c.at).map_or(String::new(), number),
            p.pid.to_string(),
            p.parent_pid.map_or(String::new(), |v| v.to_string()),
            p.name.clone(),
            p.status.clone(),
            number(p.cpu_percent as f64),
            usage["state"].as_str().unwrap().into(),
            p.gpu_percent
                .exact()
                .map_or(String::new(), |v| number(v as f64)),
            if let Usage::Partial(v) = p.gpu_percent {
                number(v as f64)
            } else {
                String::new()
            },
            p.memory_bytes.to_string(),
            p.virtual_memory_bytes.to_string(),
            number(p.read_bytes_per_sec),
            number(p.write_bytes_per_sec),
            p.total_read_bytes.to_string(),
            p.total_write_bytes.to_string(),
            p.accumulated_cpu_millis.to_string(),
            p.started_at_unix.to_string(),
            p.control.priority.label().into(),
        ];
        if c.options.private_details {
            cells.extend([
                p.user.clone(),
                p.executable
                    .as_ref()
                    .map_or(String::new(), |p| p.to_string_lossy().into_owned()),
                p.command.clone(),
                p.cwd
                    .as_ref()
                    .map_or(String::new(), |p| p.to_string_lossy().into_owned()),
            ]);
        }
        row(w, cells)?;
    }
    Ok(())
}
