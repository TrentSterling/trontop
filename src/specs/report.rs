//! Plain-text rendering for Copy and for read-only probes. Private values are
//! masked unless the caller explicitly reveals them.
use super::live::{BridgeReadings, LiveKey, resolve};
use super::model::{Group, Item, Section, SectionId, SummaryLine, Value};
use super::worker::Snapshot;
use crate::model::SystemSnapshot;
use std::fmt::Write;
use std::time::Instant;

pub const HIDDEN: &str = "[hidden]";

/// Where live values come from. None prints the key itself (probes).
pub type LiveSource<'a> = Option<(&'a SystemSnapshot, &'a BridgeReadings, Instant)>;

pub fn value_text(value: &Value) -> String {
    match value {
        Value::Known(text) => text.clone(),
        Value::Unavailable(reason) => format!("Unavailable ({reason})"),
    }
}

fn live_text(key: &LiveKey, live: LiveSource<'_>) -> String {
    match live {
        Some((snapshot, bridge, now)) => value_text(&resolve(key, snapshot, bridge, now).value),
        // Drive keys carry a private device interface path: never print it.
        None => format!("[live {}]", key_label(key)),
    }
}

fn summary_line(line: &SummaryLine, live: LiveSource<'_>, reveal: bool) -> String {
    let mut text = if line.private && !reveal {
        HIDDEN.to_string()
    } else {
        value_text(&line.text)
    };
    if let Some(key) = &line.live {
        let _ = write!(text, "    {}", live_text(key, live));
    }
    text
}

fn group(out: &mut String, group: &Group, depth: usize, live: LiveSource<'_>, reveal: bool) {
    let indent = "    ".repeat(depth);
    let _ = write!(out, "{indent}{}", group.title);
    if let Some(key) = &group.live {
        let _ = write!(out, "    {}", live_text(key, live));
    }
    out.push('\n');
    for item in &group.items {
        match item {
            Item::Group(child) => self::group(out, child, depth + 1, live, reveal),
            Item::Row(row) => {
                let value = if row.private && !reveal {
                    HIDDEN.to_string()
                } else if let Some(key) = &row.live {
                    live_text(key, live)
                } else {
                    row.display_text().unwrap_or_else(|| value_text(&row.value))
                };
                let _ = writeln!(out, "{indent}    {}: {value}", row.label);
            }
        }
    }
}

/// One collapsible group and everything under it, for Copy on the page.
pub fn group_text(tree: &Group, live: LiveSource<'_>, reveal: bool) -> String {
    let mut out = String::new();
    group(&mut out, tree, 0, live, reveal);
    out
}

/// "State: Complete; read 12 s before this report in 35.0 ms".
fn status_line(entry: &super::worker::Entry, live: LiveSource<'_>) -> String {
    let mut text = format!("State: {}", entry.health.state.label());
    if let (Some(at), Some((_, _, now))) = (entry.health.collected_at, live) {
        let _ = write!(
            text,
            "; read {:.0} s before this report",
            now.saturating_duration_since(at).as_secs_f64()
        );
    }
    if let Some(duration) = entry.health.duration {
        let _ = write!(text, " in {:.1} ms", duration.as_secs_f64() * 1000.0);
    }
    text
}

/// One section: summary lines, then its tree, then its issues.
pub fn section_text(out: &mut String, section: &Section, live: LiveSource<'_>, reveal: bool) {
    section_text_with(out, section, None, live, reveal);
}

fn section_text_with(
    out: &mut String,
    section: &Section,
    status: Option<String>,
    live: LiveSource<'_>,
    reveal: bool,
) {
    let _ = writeln!(out, "== {} ==", section.id.title());
    if let Some(status) = status {
        let _ = writeln!(out, "  {status}");
    }
    for line in &section.summary {
        let _ = writeln!(out, "  {}", summary_line(line, live, reveal));
    }
    for tree in &section.groups {
        group(out, tree, 0, live, reveal);
    }
    for issue in &section.issues {
        let _ = writeln!(out, "  Issue: {issue}");
    }
    out.push('\n');
}

/// The whole System page as text: Summary first, then every section.
pub fn text(snapshot: &Snapshot, live: LiveSource<'_>, reveal: bool) -> String {
    let mut out = format!(
        "Trontop system specs (v{})\nPrivate values: {}\n\n== {} ==\n",
        env!("CARGO_PKG_VERSION"),
        if reveal { "shown" } else { "hidden" },
        SectionId::Summary.title()
    );
    for entry in &snapshot.entries {
        if entry.id == SectionId::SensorBridge {
            continue;
        }
        let _ = writeln!(out, "{}", entry.id.title());
        match &entry.section {
            Some(section) => {
                for line in &section.summary {
                    let _ = writeln!(out, "    {}", summary_line(line, live, reveal));
                }
            }
            None => {
                let _ = writeln!(out, "    Unavailable ({})", entry.health.state.label());
            }
        }
    }
    out.push('\n');
    for entry in &snapshot.entries {
        match &entry.section {
            Some(section) => section_text_with(
                &mut out,
                section,
                Some(status_line(entry, live)),
                live,
                reveal,
            ),
            None => {
                let _ = writeln!(
                    out,
                    "== {} ==\n  Unavailable ({})\n",
                    entry.id.title(),
                    entry.health.state.label()
                );
            }
        }
    }
    out
}

/// A live key's kind for reports, never its private detail (drive interface
/// paths stay out of every export).
pub fn key_label(key: &LiveKey) -> String {
    match key {
        LiveKey::DriveTemperature { .. } => "DriveTemperature".into(),
        LiveKey::Gpu { adapter, metric } => {
            format!("Gpu({} #{}, {metric:?})", adapter.name, adapter.ordinal)
        }
        LiveKey::CpuCoreClock { group, number } => format!("CpuCoreClock({group}:{number})"),
        LiveKey::CpuCoreTemperature { index } => format!("CpuCoreTemperature({index})"),
        LiveKey::NetworkThroughput { interface } => format!("NetworkThroughput({interface})"),
        LiveKey::Sensor { id } => format!("Sensor({id})"),
        other => format!("{other:?}"),
    }
}

fn json_value(value: &Value, private: bool, reveal: bool) -> serde_json::Value {
    if private && !reveal {
        return serde_json::json!({"value": null, "private": true, "excluded": true});
    }
    match value {
        Value::Known(text) => serde_json::json!({"value": text, "private": private}),
        Value::Unavailable(reason) => {
            serde_json::json!({"value": null, "unavailable": reason, "private": private})
        }
    }
}

fn json_live(key: &LiveKey, live: LiveSource<'_>) -> serde_json::Value {
    let mut out = serde_json::json!({"key": key_label(key)});
    if let Some((snapshot, bridge, now)) = live {
        let value = resolve(key, snapshot, bridge, now);
        out["value"] = value
            .value
            .text()
            .map_or(serde_json::Value::Null, Into::into);
        if let Some(reason) = value.value.reason() {
            out["unavailable"] = reason.into();
        }
        if let Some(celsius) = value.celsius {
            out["celsius"] = f64::from(celsius).into();
        }
        if let Some(source) = value.source {
            out["source"] = source.into();
        }
    }
    out
}

fn json_group(group: &Group, live: LiveSource<'_>, reveal: bool) -> serde_json::Value {
    let items = group
        .items
        .iter()
        .map(|item| match item {
            Item::Group(child) => json_group(child, live, reveal),
            Item::Row(row) => {
                let mut out = serde_json::json!({"label": row.label});
                if let Some(key) = &row.live {
                    out["live"] = if row.private && !reveal {
                        serde_json::json!({"private": true, "excluded": true})
                    } else {
                        json_live(key, live)
                    };
                } else {
                    let value = json_value(&row.value, row.private, reveal);
                    if let serde_json::Value::Object(fields) = value {
                        for (name, field) in fields {
                            out[name] = field;
                        }
                    }
                    if let Some(unit) = &row.unit {
                        out["unit"] = unit.as_str().into();
                    }
                }
                if let Some(note) = &row.note {
                    out["note"] = note.as_str().into();
                }
                out
            }
        })
        .collect::<Vec<_>>();
    let mut out = serde_json::json!({"group": group.title, "items": items});
    if let Some(key) = &group.live {
        out["live"] = json_live(key, live);
    }
    out
}

/// The whole page as JSON (schema 1). Private values are excluded unless
/// `reveal`; drive interface paths never appear.
pub fn json(snapshot: &Snapshot, live: LiveSource<'_>, reveal: bool) -> serde_json::Value {
    let now = live.map(|(_, _, now)| now);
    let sections = snapshot
        .entries
        .iter()
        .map(|entry| {
            let mut out = serde_json::json!({
                "id": entry.id.key(),
                "title": entry.id.title(),
                "state": entry.health.state.label(),
                "read_ms": entry.health.duration.map(|d| d.as_secs_f64() * 1000.0),
                "age_seconds": match (entry.health.collected_at, now) {
                    (Some(at), Some(now)) => Some(now.saturating_duration_since(at).as_secs_f64()),
                    _ => None,
                },
                "worker_issues": entry.health.issues,
            });
            if let Some(section) = &entry.section {
                out["summary"] = section
                    .summary
                    .iter()
                    .map(|line| {
                        let mut value = json_value(&line.text, line.private, reveal);
                        if let Some(key) = &line.live {
                            value["live"] = json_live(key, live);
                        }
                        value
                    })
                    .collect::<Vec<_>>()
                    .into();
                out["groups"] = section
                    .groups
                    .iter()
                    .map(|g| json_group(g, live, reveal))
                    .collect::<Vec<_>>()
                    .into();
                out["issues"] = section.issues.clone().into();
            }
            out
        })
        .collect::<Vec<_>>();
    serde_json::json!({
        "schema_version": 1,
        "kind": "trontop_system_specs",
        "version": env!("CARGO_PKG_VERSION"),
        "private_values": if reveal { "included" } else { "excluded" },
        "bridge_status": value_text(&snapshot.bridge.status),
        "sections": sections,
    })
}

/// Probe output: private values masked, live keys printed as keys.
#[cfg(test)]
pub fn probe_text(sections: &[Section]) -> String {
    let mut out = String::new();
    for section in sections {
        section_text(&mut out, section, None, false);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::super::model::Row;
    use super::*;

    #[test]
    fn private_values_are_masked_unless_revealed_and_live_keys_resolve() {
        let section = Section::new(SectionId::Motherboard)
            .summary_line(SummaryLine::known("Fixture board"))
            .group(
                Group::new("Fixture board")
                    .row(Row::known("Serial", "FIXTURE-SERIAL-123").private())
                    .row(Row::unavailable("Chipset", "requires administrator"))
                    .row(Row::live("Uptime", LiveKey::Uptime)),
            )
            .issue("fixture issue");
        let masked = probe_text(std::slice::from_ref(&section));
        assert!(!masked.contains("FIXTURE-SERIAL-123"));
        assert!(masked.contains("Serial: [hidden]"));
        assert!(masked.contains("Chipset: Unavailable (requires administrator)"));
        assert!(masked.contains("Uptime: [live Uptime]"));
        assert!(masked.contains("Issue: fixture issue"));
        let mut revealed = String::new();
        let sampler = SystemSnapshot {
            sequence: 1,
            uptime_seconds: 3_661,
            ..Default::default()
        };
        section_text(
            &mut revealed,
            &section,
            Some((&sampler, &BridgeReadings::default(), Instant::now())),
            true,
        );
        assert!(revealed.contains("Serial: FIXTURE-SERIAL-123"));
        assert!(!revealed.contains("[live"));
    }
}
